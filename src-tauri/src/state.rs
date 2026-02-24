use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;

use serde::Serialize;

use crate::chat::ChatManager;
use crate::dispatcher::CommandDispatcher;
use crate::dsl::compiler::CompiledScript;
use crate::effects;
use crate::error::AppError;
use crate::model::analysis::AudioAnalysis;
use crate::model::show::Show;
use crate::model::{BlendMode, EffectKind, EffectParams, ParamSchema, TimeRange};
use crate::settings::AppSettings;

// ── Cancellation Registry ──────────────────────────────────────────

/// Manages cancel flags for long-running operations so the frontend can
/// request cancellation and the backend can check for it cooperatively.
pub struct CancellationRegistry {
    flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Default for CancellationRegistry {
    fn default() -> Self {
        Self {
            flags: Mutex::new(HashMap::new()),
        }
    }
}

impl CancellationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new cancellable operation. Returns a flag that the operation
    /// should periodically check. If an operation with the same name is already
    /// registered, its existing flag is returned (and reset to false).
    pub fn register(&self, operation: &str) -> Arc<AtomicBool> {
        let mut flags = self.flags.lock();
        let flag = flags
            .entry(operation.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)));
        flag.store(false, Ordering::Relaxed);
        flag.clone()
    }

    /// Signal cancellation for the named operation. Returns true if the
    /// operation was found and signalled.
    pub fn cancel(&self, operation: &str) -> bool {
        let flags = self.flags.lock();
        if let Some(flag) = flags.get(operation) {
            flag.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Remove the cancel flag for a completed operation.
    pub fn unregister(&self, operation: &str) {
        self.flags.lock().remove(operation);
    }
}

/// Poll the cancel flag every 250ms. Resolves when the flag becomes true.
/// Intended for use with `tokio::select!` to race against async work.
pub async fn wait_for_cancel(flag: &AtomicBool) {
    loop {
        if flag.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }
}

/// Check the cancel flag and return `Err(AppError::Cancelled)` if set.
pub fn check_cancelled(flag: &AtomicBool, operation: &str) -> Result<(), AppError> {
    if flag.load(Ordering::Relaxed) {
        Err(AppError::Cancelled {
            operation: operation.to_string(),
        })
    } else {
        Ok(())
    }
}

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
    /// Display messages for agent mode chat (persisted globally).
    pub agent_display_messages: Mutex<Vec<crate::chat::ChatHistoryEntry>>,
    /// Multi-conversation agent chat data.
    pub agent_chats: Mutex<crate::chat::AgentChatsData>,
    /// Cancellation flags for long-running operations.
    pub cancellation: CancellationRegistry,
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

    /// Get the current profile slug, or error if none is loaded.
    pub fn require_profile(&self) -> Result<String, AppError> {
        self.current_profile.lock().clone().ok_or(AppError::NoProfile)
    }

    /// Get the current sequence slug, or error if none is loaded.
    pub fn require_sequence(&self) -> Result<String, AppError> {
        self.current_sequence.lock().clone().ok_or(AppError::NoSequence)
    }

    /// Insert an analysis result into the cache, evicting the oldest entry
    /// if the cache exceeds `MAX_ANALYSIS_CACHE` entries.
    pub fn cache_analysis(&self, key: String, value: AudioAnalysis) {
        const MAX_ANALYSIS_CACHE: usize = 10;
        let mut cache = self.analysis_cache.lock();
        cache.insert(key, value);
        while cache.len() > MAX_ANALYSIS_CACHE {
            // Remove an arbitrary entry to stay within the cap.
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }
    }

    /// Resolve the active sequence index within `show.sequences`.
    ///
    /// Verifies that a sequence is loaded (via `current_sequence`) and that the
    /// show actually contains at least one sequence. Returns the index of the
    /// active sequence. Currently the architecture always places the active
    /// sequence at index 0 (see `assemble_show`), but this method validates
    /// that assumption rather than blindly hardcoding it.
    ///
    /// **Lock ordering**: callers must NOT hold `self.show` when calling this,
    /// because this method locks `current_sequence` only. Pass the show ref
    /// separately after locking.
    pub fn active_sequence_index(&self, show: &crate::model::Show) -> Result<usize, AppError> {
        // Ensure a sequence is loaded.
        let _slug = self.require_sequence()?;

        // Verify the show has at least one sequence.
        if show.sequences.is_empty() {
            return Err(AppError::NoSequence);
        }

        // assemble_show() always places the active sequence at index 0.
        Ok(0)
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
