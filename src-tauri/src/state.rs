use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU16;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;

use serde::Serialize;

use crate::chat::ChatManager;
use crate::dispatcher::CommandDispatcher;
use crate::dsl::compiler::CompiledScript;
use crate::effects;
use crate::model::analysis::AudioAnalysis;
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
    /// Cache of compiled DSL scripts. Key is script name, value is compiled bytecode.
    pub script_cache: Mutex<HashMap<String, Arc<CompiledScript>>>,
    /// Handle to the Python analysis sidecar process.
    pub python_sidecar: Mutex<Option<tokio::process::Child>>,
    /// Port the Python sidecar is listening on (0 = not running).
    pub python_port: AtomicU16,
    /// Cache of audio analysis results. Key is media filename.
    pub analysis_cache: Mutex<HashMap<String, AudioAnalysis>>,
    /// Handle to the agent sidecar process (Node.js).
    pub agent_sidecar: Mutex<Option<tokio::process::Child>>,
    /// Port the agent sidecar is listening on (0 = not running).
    pub agent_port: AtomicU16,
    /// Session ID for agent conversation continuity.
    pub agent_session_id: Mutex<Option<String>>,
}

impl AppState {
    /// Read-only access to the show. Locks the mutex for the duration of `f`.
    pub fn with_show<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Show) -> R,
    {
        let guard = self.show.lock();
        f(&guard)
    }

    /// Mutating access to the show. Locks the mutex for the duration of `f`.
    pub fn with_show_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Show) -> R,
    {
        let mut guard = self.show.lock();
        f(&mut guard)
    }

    /// Read-only access to playback state.
    pub fn with_playback<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&PlaybackState) -> R,
    {
        let guard = self.playback.lock();
        f(&guard)
    }

    /// Mutating access to playback state.
    pub fn with_playback_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut PlaybackState) -> R,
    {
        let mut guard = self.playback.lock();
        f(&mut guard)
    }

    /// Read-only access to the dispatcher.
    pub fn with_dispatcher<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&CommandDispatcher) -> R,
    {
        let guard = self.dispatcher.lock();
        f(&guard)
    }

    /// Mutating access to the dispatcher.
    pub fn with_dispatcher_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut CommandDispatcher) -> R,
    {
        let mut guard = self.dispatcher.lock();
        f(&mut guard)
    }
}

pub struct PlaybackState {
    pub playing: bool,
    pub current_time: f64,
    pub sequence_index: usize,
    /// Real-time clock anchor for computing dt in `tick()`.
    /// Set when playback starts; cleared on pause/seek.
    pub last_tick: Option<Instant>,
    /// Optional playback region (start, end) in seconds.
    pub region: Option<(f64, f64)>,
    /// Whether playback should loop within the region.
    pub looping: bool,
}

#[derive(Serialize)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct PlaybackInfo {
    pub playing: bool,
    pub current_time: f64,
    pub duration: f64,
    pub sequence_index: usize,
    pub region: Option<(f64, f64)>,
    pub looping: bool,
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

#[derive(Serialize)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct EffectInfo {
    pub kind: EffectKind,
    pub name: String,
    pub schema: Vec<ParamSchema>,
}

/// Build the full list of available effects from `EffectKind::all_builtin()`.
pub fn all_effect_info() -> Vec<EffectInfo> {
    EffectKind::all_builtin()
        .iter()
        .filter_map(|kind| {
            let effect = effects::resolve_effect(kind)?;
            Some(EffectInfo {
                kind: kind.clone(),
                name: effect.name().to_string(),
                schema: effect.param_schema(),
            })
        })
        .collect()
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
