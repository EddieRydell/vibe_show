use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::AnalysisFeatures;
use crate::project::{read_json, write_json, ProjectError};

/// Application-level settings stored in the OS config directory.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSettings {
    pub version: u32,
    pub data_dir: PathBuf,
    pub last_profile: Option<String>,
    #[serde(default)]
    pub claude_api_key: Option<String>,
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
            last_profile: None,
            claude_api_key: None,
            use_gpu: false,
            default_analysis_features: None,
        }
    }
}

/// Path to the settings file within the app config directory.
pub fn settings_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("settings.json")
}

/// Load settings from the app config directory. Returns None if no settings file exists.
pub fn load_settings(app_config_dir: &Path) -> Option<AppSettings> {
    let path = settings_path(app_config_dir);
    if !path.exists() {
        return None;
    }
    read_json::<AppSettings>(&path).ok()
}

/// Save settings to the app config directory.
pub fn save_settings(
    app_config_dir: &Path,
    settings: &AppSettings,
) -> Result<(), ProjectError> {
    std::fs::create_dir_all(app_config_dir)?;
    write_json(&settings_path(app_config_dir), settings)
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
        assert_eq!(loaded.last_profile, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_missing_returns_none() {
        let dir = std::env::temp_dir().join("vibelights_test_no_settings");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load_settings(&dir).is_none());
    }
}
