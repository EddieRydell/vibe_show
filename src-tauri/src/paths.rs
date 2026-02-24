//! Centralized path definitions for all data files and directories.
//!
//! This module is the single source of truth for leaf filenames, directory names,
//! and path-building functions. No other module should hard-code these strings.
//!
//! Functions accept `&Path` (not `AppHandle`) so they work in both Tauri and CLI contexts.

use std::path::{Path, PathBuf};

// ── Application identity ─────────────────────────────────────────

pub const APP_ID: &str = "com.vibelights.app";

// ── Leaf filenames ───────────────────────────────────────────────

pub const SETTINGS_FILE: &str = "settings.json";
pub const CREDENTIALS_FILE: &str = ".credentials";
pub const AGENT_CHATS_FILE: &str = "agent-chats.json";
pub const SETUP_META_FILE: &str = "meta.json";
pub const FIXTURES_FILE: &str = "fixtures.json";
pub const SETUP_FILE: &str = "setup.json";
pub const LAYOUT_FILE: &str = "layout.json";
pub const GLOBAL_LIBRARIES_FILE: &str = "libraries.json";
pub const DEPS_INSTALLED_MARKER: &str = ".deps_installed";

// ── Directory names ──────────────────────────────────────────────

pub const SETUPS_DIR: &str = "setups";
pub const SEQUENCES_DIR: &str = "sequences";
pub const MEDIA_DIR: &str = "media";
pub const PYTHON_ENV_DIR: &str = "python_env";
pub const MODELS_DIR: &str = "models";
pub const AGENT_LOGS_DIR: &str = "agent-logs";
pub const SCRATCH_DIR: &str = ".scratch";

// ── Config-dir functions (take app_config_dir) ───────────────────

pub fn settings_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(SETTINGS_FILE)
}

pub fn credentials_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(CREDENTIALS_FILE)
}

pub fn agent_chats_file_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(AGENT_CHATS_FILE)
}

pub fn agent_logs_dir(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(AGENT_LOGS_DIR)
}

pub fn python_env_dir(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(PYTHON_ENV_DIR)
}

pub fn models_dir(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(MODELS_DIR)
}

pub fn python_exe(app_config_dir: &Path) -> PathBuf {
    let venv = python_env_dir(app_config_dir);
    if cfg!(target_os = "windows") {
        venv.join("Scripts").join("python.exe")
    } else {
        venv.join("bin").join("python")
    }
}

pub fn deps_installed_marker(app_config_dir: &Path) -> PathBuf {
    python_env_dir(app_config_dir).join(DEPS_INSTALLED_MARKER)
}

pub fn scratch_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(SCRATCH_DIR)
}

// ── Resource-dir functions (take resource_dir) ───────────────────

pub fn uv_binary_path(resource_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        resource_dir.join("resources").join("uv.exe")
    } else {
        resource_dir.join("resources").join("uv")
    }
}

pub fn sidecar_script_path(resource_dir: &Path) -> PathBuf {
    resource_dir
        .join("resources")
        .join("python")
        .join("sidecar.py")
}

pub fn requirements_path(resource_dir: &Path) -> PathBuf {
    resource_dir
        .join("resources")
        .join("python")
        .join("requirements.txt")
}

// ── Global data-dir functions ─────────────────────────────────────

pub fn global_libraries_path(data_dir: &Path) -> PathBuf {
    data_dir.join(GLOBAL_LIBRARIES_FILE)
}

// ── Data-dir functions (take data_dir + slugs) ───────────────────

pub fn setups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(SETUPS_DIR)
}

pub fn setup_dir(data_dir: &Path, slug: &str) -> PathBuf {
    setups_dir(data_dir).join(slug)
}

pub fn sequences_dir(data_dir: &Path, setup_slug: &str) -> PathBuf {
    setup_dir(data_dir, setup_slug).join(SEQUENCES_DIR)
}

pub fn media_dir(data_dir: &Path, setup_slug: &str) -> PathBuf {
    setup_dir(data_dir, setup_slug).join(MEDIA_DIR)
}

pub fn sequence_file(data_dir: &Path, setup_slug: &str, seq_slug: &str) -> PathBuf {
    sequences_dir(data_dir, setup_slug).join(format!("{seq_slug}.json"))
}

// ── Analysis functions (take media_dir) ──────────────────────────

pub fn analysis_path(media_dir: &Path, filename: &str) -> PathBuf {
    media_dir.join(format!("{filename}.analysis.json"))
}

pub fn stems_dir(media_dir: &Path, filename: &str) -> PathBuf {
    let slug = filename.replace('.', "-");
    media_dir.join("stems").join(slug)
}
