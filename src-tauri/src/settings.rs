use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::AnalysisFeatures;
use crate::project::{read_json, write_json, ProjectError};

// ── LLM provider types ──────────────────────────────────────────

/// Which LLM provider to use for the chat assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
pub enum LlmProvider {
    Anthropic,
    OpenAiCompatible,
}

/// Full configuration for the chosen LLM provider.
///
/// The `api_key` field is never written to `settings.json`. It is stored in a
/// separate credentials file and loaded/saved via [`load_api_key`]/[`save_api_key`].
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LlmProviderConfig {
    pub provider: LlmProvider,
    /// Received over IPC but never persisted in settings.json (stored in separate credentials file).
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    /// Base URL for OpenAI-compatible providers (ignored for Anthropic).
    #[serde(default)]
    pub base_url: Option<String>,
    /// Model override. None = use provider default.
    #[serde(default)]
    pub model: Option<String>,
}

impl Default for LlmProviderConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Anthropic,
            api_key: None,
            base_url: None,
            model: None,
        }
    }
}

/// Redacted view of the LLM config returned to the frontend (no raw API key).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct LlmConfigInfo {
    pub provider: LlmProvider,
    pub has_api_key: bool,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

impl LlmConfigInfo {
    #[must_use]
    pub fn from_config(config: &LlmProviderConfig) -> Self {
        Self {
            provider: config.provider,
            has_api_key: config.api_key.as_ref().is_some_and(|k| !k.is_empty()),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
        }
    }
}

// ── App settings ─────────────────────────────────────────────────

/// Application-level settings stored in the OS config directory.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSettings {
    pub version: u32,
    pub data_dir: PathBuf,
    pub last_setup: Option<String>,
    /// Legacy field — read during deserialization for backward compat, never written.
    #[serde(default, skip_serializing)]
    #[ts(skip)]
    claude_api_key: Option<String>,
    #[serde(default)]
    pub llm: LlmProviderConfig,
    /// Whether to attempt GPU acceleration for audio analysis (requires NVIDIA CUDA).
    #[serde(default)]
    pub use_gpu: bool,
    /// Default features to run when analyzing audio. None = all enabled.
    #[serde(default)]
    pub default_analysis_features: Option<AnalysisFeatures>,
}

const SETTINGS_VERSION: u32 = 1;

impl AppSettings {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            version: SETTINGS_VERSION,
            data_dir,
            last_setup: None,
            claude_api_key: None,
            llm: LlmProviderConfig::default(),
            use_gpu: false,
            default_analysis_features: None,
        }
    }
}

/// Load the API key from the separate credentials file.
pub fn load_api_key(app_config_dir: &Path) -> Option<String> {
    let path = crate::paths::credentials_path(app_config_dir);
    std::fs::read_to_string(path)
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// Save the API key to the separate credentials file (atomic write).
pub fn save_api_key(app_config_dir: &Path, key: &str) -> Result<(), ProjectError> {
    std::fs::create_dir_all(app_config_dir)?;
    let path = crate::paths::credentials_path(app_config_dir);
    if key.is_empty() {
        let _ = std::fs::remove_file(&path);
    } else {
        crate::project::atomic_write(&path, key.as_bytes())?;
    }
    Ok(())
}

/// Load settings from the app config directory. Returns None if no settings file exists.
///
/// Handles backward compat: if the old `claude_api_key` field is present, migrates
/// it into `llm.api_key` with provider=Anthropic and saves it to the credentials file.
pub fn load_settings(app_config_dir: &Path) -> Option<AppSettings> {
    let path = crate::paths::settings_path(app_config_dir);
    if !path.exists() {
        return None;
    }
    let mut settings = read_json::<AppSettings>(&path).ok()?;

    // Migrate legacy claude_api_key → llm.api_key + credentials file
    if let Some(ref key) = settings.claude_api_key {
        if !key.is_empty() && settings.llm.api_key.is_none() {
            settings.llm.api_key = Some(key.clone());
            settings.llm.provider = LlmProvider::Anthropic;
            // Persist to credentials file and re-save settings (drops claude_api_key)
            let _ = save_api_key(app_config_dir, key);
            let _ = save_settings(app_config_dir, &settings);
        }
        settings.claude_api_key = None;
    }

    // Load API key from credentials file
    if settings.llm.api_key.is_none() {
        settings.llm.api_key = load_api_key(app_config_dir);
    }

    Some(settings)
}

/// Save settings to the app config directory.
pub fn save_settings(
    app_config_dir: &Path,
    settings: &AppSettings,
) -> Result<(), ProjectError> {
    std::fs::create_dir_all(app_config_dir)?;
    write_json(&crate::paths::settings_path(app_config_dir), settings)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_round_trip() {
        let dir = std::env::temp_dir().join("vibelights_test_settings");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let settings = AppSettings::new(PathBuf::from("/some/data/dir"));
        save_settings(&dir, &settings).unwrap();

        let loaded = load_settings(&dir).expect("should load");
        assert_eq!(loaded.data_dir, PathBuf::from("/some/data/dir"));
        assert_eq!(loaded.last_setup, None);
        assert!(matches!(loaded.llm.provider, LlmProvider::Anthropic));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_legacy_claude_api_key_migration() {
        let dir = std::env::temp_dir().join("vibelights_test_migration");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Write old-format settings with claude_api_key
        let old_json = serde_json::json!({
            "version": 1,
            "data_dir": "/some/dir",
            "claude_api_key": "sk-ant-test-key"
        });
        std::fs::write(crate::paths::settings_path(&dir), serde_json::to_string_pretty(&old_json).unwrap()).unwrap();

        let loaded = load_settings(&dir).expect("should load");
        // Key should be migrated into llm config
        assert_eq!(loaded.llm.api_key.as_deref(), Some("sk-ant-test-key"));
        assert!(matches!(loaded.llm.provider, LlmProvider::Anthropic));
        // And saved to credentials file
        assert_eq!(load_api_key(&dir).as_deref(), Some("sk-ant-test-key"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_missing_returns_none() {
        let dir = std::env::temp_dir().join("vibelights_test_no_settings");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load_settings(&dir).is_none());
    }
}
