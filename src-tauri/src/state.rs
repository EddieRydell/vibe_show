use std::path::PathBuf;
use std::sync::atomic::AtomicU16;
use std::time::Instant;

use parking_lot::Mutex;

use serde::Serialize;

use crate::chat::ChatManager;
use crate::dispatcher::CommandDispatcher;
use crate::model::show::Show;
use crate::model::{BlendMode, EffectKind, EffectParams, ParamSchema, TimeRange};
use crate::settings::AppSettings;

// ── Application State ──────────────────────────────────────────────

/// Application state shared across Tauri commands and the HTTP API.
pub struct AppState {
    pub show: Mutex<Show>,
    pub playback: Mutex<PlaybackState>,
    pub dispatcher: Mutex<CommandDispatcher>,
    pub chat: Mutex<ChatManager>,
    pub api_port: AtomicU16,
    pub app_config_dir: PathBuf,
    pub settings: Mutex<Option<AppSettings>>,
    pub current_profile: Mutex<Option<String>>,
    pub current_sequence: Mutex<Option<String>>,
}

pub struct PlaybackState {
    pub playing: bool,
    pub current_time: f64,
    pub sequence_index: usize,
    /// Real-time clock anchor for computing dt in `tick()`.
    /// Set when playback starts; cleared on pause/seek.
    pub last_tick: Option<Instant>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct PlaybackInfo {
    pub playing: bool,
    pub current_time: f64,
    pub duration: f64,
    pub sequence_index: usize,
}

#[derive(Serialize)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct EffectDetail {
    pub kind: EffectKind,
    pub schema: Vec<ParamSchema>,
    pub params: EffectParams,
    pub time_range: TimeRange,
    pub track_name: String,
    pub blend_mode: BlendMode,
    pub opacity: f64,
}

/// Helper to get data_dir from settings.
pub fn get_data_dir(state: &AppState) -> Result<PathBuf, String> {
    state
        .settings
        .lock()
        .as_ref()
        .map(|s| s.data_dir.clone())
        .ok_or_else(|| "No data directory configured".to_string())
}
