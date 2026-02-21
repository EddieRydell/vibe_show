#![allow(
    clippy::needless_pass_by_value,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::unreachable
)]

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::State;

use crate::analysis;
use crate::chat::{ChatHistoryEntry, ChatManager};
use crate::dispatcher::{CommandDispatcher, CommandResult, EditCommand, UndoState};
use crate::dsl;
use crate::effects::resolve_effect;
use crate::engine::{self, Frame};
use crate::error::AppError;
use crate::import::vixen::{VixenDiscovery, VixenImportConfig, VixenImportResult};
use crate::model::{
    AnalysisFeatures, AudioAnalysis, EffectKind, EffectTarget, ParamKey, ParamValue,
    PythonEnvStatus, Show,
};
use crate::profile::{self, MediaFile, Profile, ProfileSummary, SequenceSummary, MEDIA_EXTENSIONS};
use crate::progress::emit_progress;
use crate::python;
use crate::settings::{self, AppSettings};
use crate::state::{self, AppState, EffectInfo, PlaybackInfo};

// ── Settings commands ──────────────────────────────────────────────

/// Check if settings exist (first launch detection).
#[tauri::command]
pub fn get_settings(state: State<Arc<AppState>>) -> Option<AppSettings> {
    state.settings.lock().clone()
}

/// Get the API server port (0 if not yet started).
#[tauri::command]
pub fn get_api_port(state: State<Arc<AppState>>) -> u16 {
    state.api_port.load(Ordering::Relaxed)
}

/// First launch: set data directory, create folder structure, save settings.
#[tauri::command]
pub fn initialize_data_dir(state: State<Arc<AppState>>, data_dir: String) -> Result<AppSettings, AppError> {
    let data_path = PathBuf::from(&data_dir);

    // Create the profiles directory
    std::fs::create_dir_all(data_path.join("profiles"))?;

    let new_settings = AppSettings::new(data_path);
    settings::save_settings(&state.app_config_dir, &new_settings)
        .map_err(|e| AppError::SettingsSaveError { message: e.to_string() })?;

    *state.settings.lock() = Some(new_settings.clone());
    Ok(new_settings)
}

// ── Profile commands ───────────────────────────────────────────────

fn get_data_dir(state: &State<Arc<AppState>>) -> Result<std::path::PathBuf, AppError> {
    state::get_data_dir(state).map_err(|_| AppError::NoSettings)
}

fn require_profile(state: &State<Arc<AppState>>) -> Result<String, AppError> {
    state.current_profile.lock().clone().ok_or(AppError::NoProfile)
}

fn require_sequence(state: &State<Arc<AppState>>) -> Result<String, AppError> {
    state.current_sequence.lock().clone().ok_or(AppError::NoSequence)
}

/// List all profiles.
#[tauri::command]
pub fn list_profiles(state: State<Arc<AppState>>) -> Result<Vec<ProfileSummary>, AppError> {
    let data_dir = get_data_dir(&state)?;
    profile::list_profiles(&data_dir).map_err(AppError::from)
}

/// Create a new empty profile.
#[tauri::command]
pub fn create_profile(state: State<Arc<AppState>>, name: String) -> Result<ProfileSummary, AppError> {
    let data_dir = get_data_dir(&state)?;
    profile::create_profile(&data_dir, &name).map_err(AppError::from)
}

/// Load a profile and set it as current.
#[tauri::command]
pub fn open_profile(state: State<Arc<AppState>>, slug: String) -> Result<Profile, AppError> {
    let data_dir = get_data_dir(&state)?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    *state.current_profile.lock() = Some(slug.clone());
    *state.current_sequence.lock() = None;

    // Update last_profile in settings
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.last_profile = Some(slug);
        if let Err(e) = settings::save_settings(&state.app_config_dir, s) {
            eprintln!("[VibeLights] Failed to save settings: {e}");
        }
    }

    Ok(loaded)
}

/// Delete a profile and all its data.
#[tauri::command]
pub fn delete_profile(state: State<Arc<AppState>>, slug: String) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    profile::delete_profile(&data_dir, &slug).map_err(AppError::from)?;

    // Clear current if it was the deleted one
    let mut current = state.current_profile.lock();
    if current.as_deref() == Some(&slug) {
        *current = None;
        *state.current_sequence.lock() = None;
    }

    Ok(())
}

/// Save the current profile's house data.
#[tauri::command]
pub fn save_profile(state: State<Arc<AppState>>) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)
}

/// Update fixtures and groups on the current profile.
#[tauri::command]
pub fn update_profile_fixtures(
    state: State<Arc<AppState>>,
    fixtures: Vec<crate::model::FixtureDef>,
    groups: Vec<crate::model::FixtureGroup>,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    loaded.fixtures = fixtures;
    loaded.groups = groups;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)
}

/// Update controllers and patches on the current profile.
#[tauri::command]
pub fn update_profile_setup(
    state: State<Arc<AppState>>,
    controllers: Vec<crate::model::Controller>,
    patches: Vec<crate::model::fixture::Patch>,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    loaded.controllers = controllers;
    loaded.patches = patches;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)
}

/// Update the layout on the current profile.
#[tauri::command]
pub fn update_profile_layout(
    state: State<Arc<AppState>>,
    layout: crate::model::Layout,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    loaded.layout = layout;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)
}

// ── Sequence commands ──────────────────────────────────────────────

/// List sequences in the current profile.
#[tauri::command]
pub fn list_sequences(state: State<Arc<AppState>>) -> Result<Vec<SequenceSummary>, AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    profile::list_sequences(&data_dir, &profile_slug).map_err(AppError::from)
}

/// Create a new empty sequence in the current profile.
#[tauri::command]
pub fn create_sequence(state: State<Arc<AppState>>, name: String) -> Result<SequenceSummary, AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    profile::create_sequence(&data_dir, &profile_slug, &name).map_err(AppError::from)
}

/// Delete a sequence from the current profile.
#[tauri::command]
pub fn delete_sequence(state: State<Arc<AppState>>, slug: String) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    profile::delete_sequence(&data_dir, &profile_slug, &slug).map_err(AppError::from)?;

    // Clear current sequence if it was the deleted one
    let mut current = state.current_sequence.lock();
    if current.as_deref() == Some(&slug) {
        *current = None;
    }

    Ok(())
}

/// Load a sequence into the engine state for editing.
#[tauri::command]
pub async fn open_sequence(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    slug: String,
) -> Result<Show, AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    let state_arc = (*state).clone();
    let app = app_handle.clone();

    let assembled = tokio::task::spawn_blocking(move || {
        emit_progress(&app, "open_sequence", "Loading profile...", 0.1, None);
        let profile_data =
            profile::load_profile(&data_dir, &profile_slug).map_err(AppError::from)?;

        emit_progress(&app, "open_sequence", "Loading sequence...", 0.4, None);
        let sequence =
            profile::load_sequence(&data_dir, &profile_slug, &slug).map_err(AppError::from)?;

        emit_progress(&app, "open_sequence", "Assembling show...", 0.7, None);
        let assembled = profile::assemble_show(&profile_data, &sequence);

        *state_arc.show.lock() = assembled.clone();
        state_arc.dispatcher.lock().clear();

        state_arc.with_playback_mut(|playback| {
            playback.playing = false;
            playback.current_time = 0.0;
            playback.sequence_index = 0;
            playback.region = None;
            playback.looping = false;
        });

        *state_arc.current_sequence.lock() = Some(slug);

        // Recompile all DSL scripts from the loaded show
        recompile_all_scripts(&state_arc);

        emit_progress(&app, "open_sequence", "Ready", 1.0, None);

        Ok::<Show, AppError>(assembled)
    })
    .await
    .map_err(|e| AppError::ApiError { message: e.to_string() })??;

    Ok(assembled)
}

/// Save the currently active sequence back to disk.
#[tauri::command]
pub fn save_current_sequence(state: State<Arc<AppState>>) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    let seq_slug = require_sequence(&state)?;

    let show = state.show.lock();
    let sequence = show
        .sequences
        .first()
        .ok_or(AppError::NotFound { what: "sequence in show".into() })?;
    profile::save_sequence(&data_dir, &profile_slug, &seq_slug, sequence)
        .map_err(AppError::from)
}

// ── Media commands ─────────────────────────────────────────────────

/// List audio files in the current profile.
#[tauri::command]
pub fn list_media(state: State<Arc<AppState>>) -> Result<Vec<MediaFile>, AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    profile::list_media(&data_dir, &profile_slug).map_err(AppError::from)
}

/// Import (copy) an audio file into the current profile's media directory.
#[tauri::command]
pub fn import_media(state: State<Arc<AppState>>, source_path: String) -> Result<MediaFile, AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    profile::import_media(
        &data_dir,
        &profile_slug,
        std::path::Path::new(&source_path),
    )
    .map_err(AppError::from)
}

/// Delete an audio file from the current profile.
#[tauri::command]
pub fn delete_media(state: State<Arc<AppState>>, filename: String) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    profile::delete_media(&data_dir, &profile_slug, &filename).map_err(AppError::from)
}

// ── Media path resolution ──────────────────────────────────────────

/// Resolve a media filename to its absolute filesystem path.
#[tauri::command]
pub fn resolve_media_path(state: State<Arc<AppState>>, filename: String) -> Result<String, AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    let path = profile::media_dir(&data_dir, &profile_slug).join(&filename);
    if !path.exists() {
        return Err(AppError::NotFound { what: format!("Media file: {filename}") });
    }
    Ok(path.to_string_lossy().to_string())
}

// ── Sequence settings ─────────────────────────────────────────────

/// Partial update of sequence settings. Only provided fields are changed.
#[tauri::command]
pub fn update_sequence_settings(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    name: Option<String>,
    audio_file: Option<Option<String>>,
    duration: Option<f64>,
    frame_rate: Option<f64>,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::UpdateSequenceSettings {
        sequence_index,
        name,
        audio_file,
        duration,
        frame_rate,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

// ── Effects listing ────────────────────────────────────────────────

/// List all available built-in effect types with their parameter schemas.
#[tauri::command]
pub fn list_effects() -> Vec<EffectInfo> {
    state::all_effect_info()
}

// ── Undo / Redo commands ──────────────────────────────────────────

/// Undo the last editing command.
#[tauri::command]
pub fn undo(state: State<Arc<AppState>>) -> Result<String, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.undo(&mut show)
}

/// Redo the last undone editing command.
#[tauri::command]
pub fn redo(state: State<Arc<AppState>>) -> Result<String, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.redo(&mut show)
}

/// Get the current undo/redo state.
#[tauri::command]
pub fn get_undo_state(state: State<Arc<AppState>>) -> UndoState {
    state.with_dispatcher(CommandDispatcher::undo_state)
}

// ── Chat commands ─────────────────────────────────────────────────

/// Send a message to the Claude chat. Starts the agentic tool-use loop.
#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    message: String,
) -> Result<(), AppError> {
    let state_arc = (*state).clone();
    let emitter = crate::chat::TauriChatEmitter { app_handle };
    ChatManager::send_message(state_arc, &emitter, message)
        .await
        .map_err(AppError::from)
}

/// Get the chat history for display.
#[tauri::command]
pub fn get_chat_history(state: State<Arc<AppState>>) -> Vec<ChatHistoryEntry> {
    state.chat.lock().history_for_display()
}

/// Clear the chat conversation.
#[tauri::command]
pub fn clear_chat(state: State<Arc<AppState>>) {
    state.chat.lock().clear();
}

/// Cancel any in-flight chat request.
#[tauri::command]
pub fn stop_chat(state: State<Arc<AppState>>) {
    state.chat.lock().cancel();
}

/// Update the Claude API key in settings.
#[tauri::command]
pub fn set_claude_api_key(state: State<Arc<AppState>>, api_key: String) -> Result<(), AppError> {
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.claude_api_key = if api_key.is_empty() {
            None
        } else {
            Some(api_key)
        };
        settings::save_settings(&state.app_config_dir, s)
            .map_err(|e| AppError::SettingsSaveError { message: e.to_string() })?;
    }
    Ok(())
}

/// Check if a Claude API key is configured.
#[tauri::command]
pub fn has_claude_api_key(state: State<Arc<AppState>>) -> bool {
    state
        .settings
        .lock()
        .as_ref()
        .and_then(|s| s.claude_api_key.as_ref())
        .is_some_and(|k| !k.is_empty())
}

// ── Engine / playback commands ─────────────────────────────────────

/// Get the full show model as JSON.
#[tauri::command]
pub fn get_show(state: State<Arc<AppState>>) -> Show {
    state.with_show(Clone::clone)
}

/// Evaluate and return a single frame at the given time.
#[tauri::command]
pub fn get_frame(state: State<Arc<AppState>>, time: f64) -> Frame {
    let show = state.show.lock();
    let playback = state.playback.lock();
    let scripts = state.script_cache.lock();
    engine::evaluate(&show, playback.sequence_index, time, None, Some(&scripts))
}

/// Evaluate and return a single frame at the given time, rendering only the
/// specified (`track_index`, `effect_index`) pairs. Used by the preview loop to
/// show only selected effects.
#[tauri::command]
pub fn get_frame_filtered(
    state: State<Arc<AppState>>,
    time: f64,
    effects: Vec<(usize, usize)>,
) -> Frame {
    let show = state.show.lock();
    let playback = state.playback.lock();
    let scripts = state.script_cache.lock();
    engine::evaluate(&show, playback.sequence_index, time, Some(&effects), Some(&scripts))
}

/// Start playback.
#[tauri::command]
pub fn play(state: State<Arc<AppState>>) {
    state.with_playback_mut(|playback| {
        playback.playing = true;
        playback.last_tick = Some(std::time::Instant::now());
    });
}

/// Pause playback.
#[tauri::command]
pub fn pause(state: State<Arc<AppState>>) {
    state.with_playback_mut(|playback| {
        playback.playing = false;
        playback.last_tick = None;
    });
}

/// Seek to a specific time.
#[tauri::command]
pub fn seek(state: State<Arc<AppState>>, time: f64) {
    state.with_playback_mut(|playback| {
        playback.current_time = time.max(0.0);
        // Reset clock anchor so next tick doesn't jump
        if playback.playing {
            playback.last_tick = Some(std::time::Instant::now());
        }
    });
}

/// Get current playback state.
#[tauri::command]
pub fn get_playback(state: State<Arc<AppState>>) -> PlaybackInfo {
    let playback = state.playback.lock();
    let show = state.show.lock();
    let duration = show
        .sequences
        .get(playback.sequence_index)
        .map_or(0.0, |s| s.duration);
    PlaybackInfo {
        playing: playback.playing,
        current_time: playback.current_time,
        duration,
        sequence_index: playback.sequence_index,
        region: playback.region,
        looping: playback.looping,
    }
}

/// Set a playback region (start, end) in seconds, or clear it.
#[tauri::command]
pub fn set_region(state: State<Arc<AppState>>, region: Option<(f64, f64)>) {
    state.with_playback_mut(|playback| {
        playback.region = region;
    });
}

/// Set whether playback should loop within the region.
#[tauri::command]
pub fn set_looping(state: State<Arc<AppState>>, looping: bool) {
    state.with_playback_mut(|playback| {
        playback.looping = looping;
    });
}

/// Advance playback using real clock time. Returns the new frame if playing.
///
/// Uses `Instant`-based dt so multiple windows can call `tick` without
/// double-advancing time (each call measures elapsed since last tick).
/// The `dt` parameter is accepted for API compatibility but ignored.
#[tauri::command]
pub fn tick(state: State<Arc<AppState>>, _dt: f64) -> Option<TickResult> {
    let mut playback = state.playback.lock();
    if !playback.playing {
        return None;
    }

    let now = std::time::Instant::now();
    let real_dt = match playback.last_tick {
        Some(prev) => now.duration_since(prev).as_secs_f64(),
        None => 0.0,
    };
    playback.last_tick = Some(now);

    let show = state.show.lock();
    let duration = show
        .sequences
        .get(playback.sequence_index)
        .map_or(0.0, |s| s.duration);

    playback.current_time += real_dt;

    // Determine the effective end boundary (region end or sequence end)
    let effective_end = playback
        .region
        .map_or(duration, |(_, end)| end.min(duration));

    if playback.current_time >= effective_end {
        if playback.looping {
            if let Some((region_start, _)) = playback.region {
                // Loop back to region start
                playback.current_time = region_start;
                playback.last_tick = Some(now);
            } else {
                playback.current_time = effective_end;
                playback.playing = false;
                playback.last_tick = None;
            }
        } else {
            playback.current_time = effective_end;
            playback.playing = false;
            playback.last_tick = None;
        }
    }

    let scripts = state.script_cache.lock();
    let frame = engine::evaluate(&show, playback.sequence_index, playback.current_time, None, Some(&scripts));
    Some(TickResult {
        frame,
        current_time: playback.current_time,
        playing: playback.playing,
    })
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TickResult {
    pub frame: Frame,
    pub current_time: f64,
    pub playing: bool,
}

/// Pre-render an effect as a thumbnail image for the timeline.
#[allow(clippy::cast_precision_loss)]
#[tauri::command]
pub fn render_effect_thumbnail(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    track_index: usize,
    effect_index: usize,
    time_samples: usize,
    pixel_rows: usize,
) -> Option<EffectThumbnail> {
    let show = state.show.lock();
    let sequence = show.sequences.get(sequence_index)?;
    let track = sequence.tracks.get(track_index)?;
    let effect_instance = track.effects.get(effect_index)?;

    let effect = resolve_effect(&effect_instance.kind)?;
    let time_range = &effect_instance.time_range;

    let mut pixels = Vec::with_capacity(pixel_rows * time_samples * 4);

    for row in 0..pixel_rows {
        for col in 0..time_samples {
            let t = if time_samples > 1 {
                col as f64 / (time_samples - 1) as f64
            } else {
                0.5
            };

            let color = effect.evaluate(t, row, pixel_rows, &effect_instance.params);
            pixels.push(color.r);
            pixels.push(color.g);
            pixels.push(color.b);
            pixels.push(255);
        }
    }

    Some(EffectThumbnail {
        width: time_samples,
        height: pixel_rows,
        pixels,
        start_time: time_range.start(),
        end_time: time_range.end(),
    })
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct EffectThumbnail {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub start_time: f64,
    pub end_time: f64,
}

pub use crate::state::EffectDetail;

/// Get the schema, current params, and metadata for a placed effect.
#[tauri::command]
pub fn get_effect_detail(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    track_index: usize,
    effect_index: usize,
) -> Option<EffectDetail> {
    let show = state.show.lock();
    let sequence = show.sequences.get(sequence_index)?;
    let track = sequence.tracks.get(track_index)?;
    let effect_instance = track.effects.get(effect_index)?;

    let schema = resolve_effect(&effect_instance.kind)
        .map_or_else(Vec::new, |e| e.param_schema());

    Some(EffectDetail {
        kind: effect_instance.kind.clone(),
        schema,
        params: effect_instance.params.clone(),
        time_range: effect_instance.time_range,
        track_name: track.name.clone(),
        blend_mode: effect_instance.blend_mode,
        opacity: effect_instance.opacity,
    })
}

/// Update a single parameter on a placed effect.
#[tauri::command]
pub fn update_effect_param(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    track_index: usize,
    effect_index: usize,
    key: ParamKey,
    value: ParamValue,
) -> bool {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::UpdateEffectParam {
        sequence_index,
        track_index,
        effect_index,
        key,
        value,
    };
    match dispatcher.execute(&mut show, &cmd) {
        Ok(CommandResult::Bool(v)) => v,
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Add a new effect to a track with default params.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn add_effect(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    track_index: usize,
    kind: EffectKind,
    start: f64,
    end: f64,
    blend_mode: Option<crate::model::BlendMode>,
    opacity: Option<f64>,
) -> Result<usize, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::AddEffect {
        sequence_index,
        track_index,
        kind,
        start,
        end,
        blend_mode: blend_mode.unwrap_or(crate::model::BlendMode::Override),
        opacity: opacity.unwrap_or(1.0),
    };
    match dispatcher.execute(&mut show, &cmd)? {
        CommandResult::Index(idx) => Ok(idx),
        _ => Ok(0),
    }
}

/// Add a new track to a sequence.
#[tauri::command]
pub fn add_track(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    name: String,
    target: EffectTarget,
) -> Result<usize, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::AddTrack {
        sequence_index,
        name,
        target,
    };
    match dispatcher.execute(&mut show, &cmd)? {
        CommandResult::Index(idx) => Ok(idx),
        _ => Ok(0),
    }
}

/// Delete a track from a sequence (and all its effects).
#[tauri::command]
pub fn delete_track(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    track_index: usize,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::DeleteTrack {
        sequence_index,
        track_index,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

/// Delete multiple effects across tracks. Targets are (`track_index`, `effect_index`) pairs.
#[tauri::command]
pub fn delete_effects(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    targets: Vec<(usize, usize)>,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::DeleteEffects {
        sequence_index,
        targets,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

/// Update the time range of an effect.
#[tauri::command]
pub fn update_effect_time_range(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    track_index: usize,
    effect_index: usize,
    start: f64,
    end: f64,
) -> Result<bool, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::UpdateEffectTimeRange {
        sequence_index,
        track_index,
        effect_index,
        start,
        end,
    };
    match dispatcher.execute(&mut show, &cmd)? {
        CommandResult::Bool(v) => Ok(v),
        _ => Ok(true),
    }
}

/// Move an effect from one track to another. Returns the new effect index in the destination track.
#[tauri::command]
pub fn move_effect_to_track(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    from_track: usize,
    effect_index: usize,
    to_track: usize,
) -> Result<usize, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::MoveEffectToTrack {
        sequence_index,
        from_track,
        effect_index,
        to_track,
    };
    match dispatcher.execute(&mut show, &cmd)? {
        CommandResult::Index(idx) => Ok(idx),
        _ => Ok(0),
    }
}

/// Import a Vixen 3 show into the current profile, creating a new profile + show from the import.
#[tauri::command]
pub fn import_vixen(
    state: State<Arc<AppState>>,
    system_config_path: String,
    sequence_paths: Vec<String>,
) -> Result<ProfileSummary, AppError> {
    let data_dir = get_data_dir(&state)?;

    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(std::path::Path::new(&system_config_path))
        .map_err(|e| AppError::ImportError { message: e.to_string() })?;
    for seq_path in &sequence_paths {
        importer
            .parse_sequence(std::path::Path::new(seq_path))
            .map_err(|e| AppError::ImportError { message: e.to_string() })?;
    }

    let guid_map = importer.guid_map().clone();
    let show = importer.into_show();

    // Create a profile from the imported data
    let profile_name = if show.name.is_empty() {
        "Vixen Import".to_string()
    } else {
        show.name.clone()
    };
    let summary = profile::create_profile(&data_dir, &profile_name).map_err(AppError::from)?;

    // Save the house data into the profile
    let prof = Profile {
        name: profile_name,
        slug: summary.slug.clone(),
        fixtures: show.fixtures.clone(),
        groups: show.groups.clone(),
        controllers: show.controllers.clone(),
        patches: show.patches.clone(),
        layout: show.layout.clone(),
    };
    profile::save_profile(&data_dir, &summary.slug, &prof).map_err(AppError::from)?;

    // Persist the GUID map for future sequence imports
    profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
        .map_err(AppError::from)?;

    // Save sequences directly into the profile
    for seq in &show.sequences {
        profile::create_sequence(&data_dir, &summary.slug, &seq.name)
            .map_err(AppError::from)?;
        let seq_slug = crate::project::slugify(&seq.name);
        profile::save_sequence(&data_dir, &summary.slug, &seq_slug, seq)
            .map_err(AppError::from)?;
    }

    // Re-read summary to get accurate counts
    let profiles = profile::list_profiles(&data_dir).map_err(AppError::from)?;
    let updated_summary = profiles
        .into_iter()
        .find(|p| p.slug == summary.slug)
        .unwrap_or(summary);

    Ok(updated_summary)
}

/// Import only the Vixen profile (fixtures, groups, controllers) from SystemConfig.xml.
/// No sequences are imported. Returns the new profile summary.
#[tauri::command]
pub fn import_vixen_profile(
    state: State<Arc<AppState>>,
    system_config_path: String,
) -> Result<ProfileSummary, AppError> {
    let data_dir = get_data_dir(&state)?;

    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(std::path::Path::new(&system_config_path))
        .map_err(|e| AppError::ImportError { message: e.to_string() })?;

    let guid_map = importer.guid_map().clone();
    let show = importer.into_show();

    let profile_name = "Vixen Import".to_string();
    let summary = profile::create_profile(&data_dir, &profile_name).map_err(AppError::from)?;

    let prof = Profile {
        name: profile_name,
        slug: summary.slug.clone(),
        fixtures: show.fixtures.clone(),
        groups: show.groups.clone(),
        controllers: show.controllers.clone(),
        patches: show.patches.clone(),
        layout: show.layout.clone(),
    };
    profile::save_profile(&data_dir, &summary.slug, &prof).map_err(AppError::from)?;

    // Persist the GUID map for future sequence imports
    profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
        .map_err(AppError::from)?;

    // Re-read summary to get accurate counts
    let profiles = profile::list_profiles(&data_dir).map_err(AppError::from)?;
    let updated_summary = profiles
        .into_iter()
        .find(|p| p.slug == summary.slug)
        .unwrap_or(summary);

    Ok(updated_summary)
}

/// Import a single Vixen sequence (.tim) into an existing profile.
/// Requires the profile to have a saved GUID map from a prior profile import.
#[tauri::command]
pub fn import_vixen_sequence(
    state: State<Arc<AppState>>,
    profile_slug: String,
    tim_path: String,
) -> Result<SequenceSummary, AppError> {
    let data_dir = get_data_dir(&state)?;

    let prof = profile::load_profile(&data_dir, &profile_slug).map_err(AppError::from)?;
    let guid_map =
        profile::load_vixen_guid_map(&data_dir, &profile_slug).map_err(AppError::from)?;

    if guid_map.is_empty() {
        return Err(AppError::ImportError {
            message: "No Vixen GUID map found for this profile. Import the profile from Vixen first.".into(),
        });
    }

    let mut importer = crate::import::vixen::VixenImporter::from_profile(
        prof.fixtures,
        prof.groups,
        prof.controllers,
        prof.patches,
        guid_map,
    );

    importer
        .parse_sequence(std::path::Path::new(&tim_path))
        .map_err(|e| AppError::ImportError { message: e.to_string() })?;

    let sequences = importer.into_sequences();
    let seq = sequences
        .into_iter()
        .next()
        .ok_or(AppError::ImportError { message: "No sequence parsed from file".into() })?;

    let seq_slug = crate::project::slugify(&seq.name);

    // Create and save the sequence
    if let Err(e) = profile::create_sequence(&data_dir, &profile_slug, &seq.name) {
        eprintln!("[VibeLights] Failed to create sequence entry: {e}");
    }
    profile::save_sequence(&data_dir, &profile_slug, &seq_slug, &seq)
        .map_err(AppError::from)?;

    Ok(SequenceSummary {
        name: seq.name,
        slug: seq_slug,
    })
}

/// Scan a Vixen 3 data directory and return a summary of what's available for import.
#[tauri::command]
pub fn scan_vixen_directory(vixen_dir: String) -> Result<VixenDiscovery, AppError> {
    use crate::import::vixen_preview;

    let vixen_path = std::path::Path::new(&vixen_dir);

    // Validate directory structure
    let config_path = vixen_path.join("SystemData").join("SystemConfig.xml");
    if !config_path.exists() {
        return Err(AppError::ImportError {
            message: format!(
                "Not a valid Vixen 3 directory: SystemData/SystemConfig.xml not found in {vixen_dir}"
            ),
        });
    }

    // Parse SystemConfig.xml to count fixtures, groups, controllers
    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(&config_path)
        .map_err(|e| AppError::ImportError { message: e.to_string() })?;

    let fixtures_found = importer.fixture_count();
    let groups_found = importer.group_count();
    let controllers_found = importer.controller_count();

    // Check for preview data
    let preview_file = vixen_preview::find_preview_file(vixen_path);
    let (preview_available, preview_item_count) = if let Some(ref pf) = preview_file {
        match vixen_preview::parse_preview_file(pf) {
            Ok(data) => (!data.display_items.is_empty(), data.display_items.len()),
            Err(_) => (false, 0),
        }
    } else {
        (false, 0)
    };
    let preview_file_path = preview_file.map(|p| p.to_string_lossy().to_string());

    // Scan for sequence files (.tim)
    let mut sequences = Vec::new();
    let seq_dir = vixen_path.join("Sequence");
    if seq_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&seq_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ext == "tim" {
                        let filename = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        sequences.push(crate::import::vixen::VixenSequenceInfo {
                            filename,
                            path: path.to_string_lossy().to_string(),
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    sequences.sort_by(|a, b| a.filename.cmp(&b.filename));

    // Scan for media files
    let mut media_files = Vec::new();
    let media_dir = vixen_path.join("Media");
    if media_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&media_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if MEDIA_EXTENSIONS.contains(&ext.as_str()) {
                        let filename = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        media_files.push(crate::import::vixen::VixenMediaInfo {
                            filename,
                            path: path.to_string_lossy().to_string(),
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    media_files.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(VixenDiscovery {
        vixen_dir,
        fixtures_found,
        groups_found,
        controllers_found,
        preview_available,
        preview_item_count,
        preview_file_path,
        sequences,
        media_files,
    })
}

/// Validate a user-selected file as containing Vixen preview/layout data.
/// Returns the number of display items found, or an error if the file doesn't
/// contain valid preview data.
#[tauri::command]
pub fn check_vixen_preview_file(file_path: String) -> Result<usize, AppError> {
    use crate::import::vixen_preview;

    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(AppError::NotFound { what: "Preview file".into() });
    }

    let data = vixen_preview::parse_preview_file(path)
        .map_err(|e| AppError::ImportError { message: e.to_string() })?;
    if data.display_items.is_empty() {
        return Err(AppError::ImportError {
            message: "File was parsed but no display items were found. \
                      The file may not contain Vixen preview/layout data.".into(),
        });
    }

    Ok(data.display_items.len())
}

/// Execute a full Vixen import based on user-selected configuration from the wizard.
#[allow(clippy::too_many_lines)]
#[tauri::command]
pub async fn execute_vixen_import(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    config: VixenImportConfig,
) -> Result<VixenImportResult, AppError> {
    let data_dir = get_data_dir(&state)?;
    let app = app_handle.clone();

    tokio::task::spawn_blocking(move || {
        let vixen_path = std::path::Path::new(&config.vixen_dir);
        let config_path = vixen_path.join("SystemData").join("SystemConfig.xml");

        // Phase 1: Parse SystemConfig
        emit_progress(&app, "import", "Parsing system config...", 0.05, None);
        let mut importer = crate::import::vixen::VixenImporter::new();
        importer
            .parse_system_config(&config_path)
            .map_err(|e| AppError::ImportError { message: e.to_string() })?;

        // Phase 2: Parse preview layout if requested
        let layout_items = if config.import_layout {
            emit_progress(&app, "import", "Parsing layout...", 0.1, None);
            let override_path = config
                .preview_file_override
                .as_deref()
                .map(std::path::Path::new);
            match importer.parse_preview(vixen_path, override_path) {
                Ok(layouts) => layouts,
                Err(e) => {
                    eprintln!("[VibeLights] Preview import warning: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // Phase 3: Parse selected sequences
        let total_seqs = config.sequence_paths.len();
        let mut sequences_imported = 0usize;
        for (i, seq_path) in config.sequence_paths.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let progress = 0.15 + 0.45 * (i as f64 / total_seqs.max(1) as f64);
            emit_progress(
                &app,
                "import",
                "Parsing sequences...",
                progress,
                Some(&format!("Sequence {} of {}", i + 1, total_seqs)),
            );
            match importer.parse_sequence(std::path::Path::new(seq_path)) {
                Ok(()) => sequences_imported += 1,
                Err(e) => {
                    eprintln!("[VibeLights] Sequence import warning: {e}");
                }
            }
        }

        let guid_map = importer.guid_map().clone();
        let warnings: Vec<String> = importer.warnings().to_vec();
        let show = importer.into_show();

        let fixtures_imported = show.fixtures.len();
        let groups_imported = show.groups.len();
        let controllers_imported = if config.import_controllers {
            show.controllers.len()
        } else {
            0
        };
        let layout_items_imported = layout_items.len();

        // Phase 4: Save profile
        emit_progress(&app, "import", "Saving profile...", 0.65, None);
        let profile_name = if config.profile_name.trim().is_empty() {
            "Vixen Import".to_string()
        } else {
            config.profile_name.trim().to_string()
        };
        let summary =
            profile::create_profile(&data_dir, &profile_name).map_err(AppError::from)?;

        let layout = if layout_items.is_empty() {
            show.layout.clone()
        } else {
            crate::model::show::Layout {
                fixtures: layout_items,
            }
        };

        let prof = Profile {
            name: profile_name,
            slug: summary.slug.clone(),
            fixtures: show.fixtures.clone(),
            groups: show.groups.clone(),
            controllers: if config.import_controllers {
                show.controllers.clone()
            } else {
                Vec::new()
            },
            patches: if config.import_controllers {
                show.patches.clone()
            } else {
                Vec::new()
            },
            layout,
        };
        profile::save_profile(&data_dir, &summary.slug, &prof).map_err(AppError::from)?;

        profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
            .map_err(AppError::from)?;

        // Phase 5: Copy media (before saving sequences so audio_file can be remapped)
        let mut media_imported = 0usize;
        if !config.media_filenames.is_empty() {
            emit_progress(&app, "import", "Copying media files...", 0.70, None);
            for media_filename in &config.media_filenames {
                let source = vixen_path.join("Media").join(media_filename);
                if source.exists() {
                    match profile::import_media(&data_dir, &summary.slug, &source) {
                        Ok(_) => media_imported += 1,
                        Err(e) => {
                            eprintln!("[VibeLights] Media import warning: {e}");
                        }
                    }
                }
            }
        }

        // Phase 6: Save sequences (remap audio_file to local media filename)
        emit_progress(&app, "import", "Saving sequences...", 0.75, None);
        for (i, seq) in show.sequences.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let progress = 0.75 + 0.15 * (i as f64 / show.sequences.len().max(1) as f64);
            emit_progress(
                &app,
                "import",
                "Saving sequences...",
                progress,
                Some(&format!("Sequence {} of {}", i + 1, show.sequences.len())),
            );
            let mut seq = seq.clone();
            // Remap audio_file: extract the filename from the Vixen path and check
            // if it matches one of the imported media files.
            if let Some(ref audio_path) = seq.audio_file {
                let audio_basename = std::path::Path::new(audio_path)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string());
                if let Some(ref basename) = audio_basename {
                    if config.media_filenames.iter().any(|m| m == basename) {
                        seq.audio_file = Some(basename.clone());
                    } else {
                        // Media wasn't selected for import — clear the path since it
                        // points to the original Vixen location which may not exist.
                        seq.audio_file = None;
                    }
                }
            }
            if let Err(e) = profile::create_sequence(&data_dir, &summary.slug, &seq.name) {
                eprintln!("[VibeLights] Failed to create sequence entry: {e}");
            }
            let seq_slug = crate::project::slugify(&seq.name);
            profile::save_sequence(&data_dir, &summary.slug, &seq_slug, &seq)
                .map_err(AppError::from)?;
        }

        // Done
        emit_progress(&app, "import", "Import complete", 1.0, None);

        Ok(VixenImportResult {
            profile_slug: summary.slug,
            fixtures_imported,
            groups_imported,
            controllers_imported,
            layout_items_imported,
            sequences_imported,
            media_imported,
            warnings,
        })
    })
    .await
    .map_err(|e| AppError::ApiError { message: e.to_string() })?
}

// ── DSL Script commands ──────────────────────────────────────────

/// Compile a DSL script source and return errors (if any).
/// On success, caches the compiled script and saves the source into the show.
#[tauri::command]
pub fn compile_script(
    state: State<Arc<AppState>>,
    name: String,
    source: String,
) -> Result<ScriptCompileResult, AppError> {
    match dsl::compile_source(&source) {
        Ok(compiled) => {
            // Cache the compiled script
            state
                .script_cache
                .lock()
                .insert(name.clone(), std::sync::Arc::new(compiled));
            // Save source into the active sequence via the dispatcher (undoable)
            let mut dispatcher = state.dispatcher.lock();
            let mut show = state.show.lock();
            let cmd = EditCommand::SetScript {
                sequence_index: 0,
                name: name.clone(),
                source,
            };
            dispatcher.execute(&mut show, &cmd)?;
            Ok(ScriptCompileResult {
                success: true,
                errors: vec![],
                name,
            })
        }
        Err(errors) => Ok(ScriptCompileResult {
            success: false,
            errors: errors
                .iter()
                .map(|e| ScriptError {
                    message: e.message.clone(),
                    offset: e.span.start,
                })
                .collect(),
            name,
        }),
    }
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptCompileResult {
    pub success: bool,
    pub errors: Vec<ScriptError>,
    pub name: String,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptError {
    pub message: String,
    pub offset: usize,
}

/// List all scripts in the active sequence.
#[tauri::command]
pub fn list_scripts(state: State<Arc<AppState>>) -> Vec<String> {
    state.with_show(|show| {
        show.sequences
            .first()
            .map(|seq| seq.scripts.keys().cloned().collect())
            .unwrap_or_default()
    })
}

/// Get the source of a script by name.
#[tauri::command]
pub fn get_script_source(
    state: State<Arc<AppState>>,
    name: String,
) -> Option<String> {
    state.with_show(|show| {
        show.sequences
            .first()
            .and_then(|seq| seq.scripts.get(&name).cloned())
    })
}

/// Delete a script from the active sequence and the cache.
#[tauri::command]
pub fn delete_script(state: State<Arc<AppState>>, name: String) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::DeleteScript {
        sequence_index: 0,
        name: name.clone(),
    };
    dispatcher.execute(&mut show, &cmd)?;
    drop(show);
    drop(dispatcher);
    state.script_cache.lock().remove(&name);
    Ok(())
}

/// Recompile all scripts in the active sequence (e.g., after loading a show from disk).
/// Returns a list of scripts that failed to compile.
pub fn recompile_all_scripts(state: &AppState) -> Vec<String> {
    let sources: Vec<(String, String)> = state.with_show(|show| {
        show.sequences
            .first()
            .map(|seq| seq.scripts.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    });

    let mut failures = Vec::new();
    let mut cache = state.script_cache.lock();
    cache.clear();

    for (name, source) in sources {
        match dsl::compile_source(&source) {
            Ok(compiled) => {
                cache.insert(name, std::sync::Arc::new(compiled));
            }
            Err(_) => {
                failures.push(name);
            }
        }
    }

    failures
}

// ── Profile library commands ──────────────────────────────────────

/// List all gradients in the current profile's library.
#[tauri::command]
pub fn list_profile_gradients(
    state: State<Arc<AppState>>,
) -> Result<Vec<(String, crate::model::ColorGradient)>, AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    Ok(libs.gradients.into_iter().collect())
}

/// Add or update a gradient in the current profile's library.
#[tauri::command]
pub fn set_profile_gradient(
    state: State<Arc<AppState>>,
    name: String,
    gradient: crate::model::ColorGradient,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    libs.gradients.insert(name, gradient);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)
}

/// Delete a gradient from the current profile's library.
#[tauri::command]
pub fn delete_profile_gradient(
    state: State<Arc<AppState>>,
    name: String,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    libs.gradients.remove(&name);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)
}

/// Rename a gradient in the current profile's library.
#[tauri::command]
pub fn rename_profile_gradient(
    state: State<Arc<AppState>>,
    old_name: String,
    new_name: String,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    if let Some(g) = libs.gradients.remove(&old_name) {
        libs.gradients.insert(new_name, g);
    }
    profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)
}

/// List all curves in the current profile's library.
#[tauri::command]
pub fn list_profile_curves(
    state: State<Arc<AppState>>,
) -> Result<Vec<(String, crate::model::Curve)>, AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    Ok(libs.curves.into_iter().collect())
}

/// Add or update a curve in the current profile's library.
#[tauri::command]
pub fn set_profile_curve(
    state: State<Arc<AppState>>,
    name: String,
    curve: crate::model::Curve,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    libs.curves.insert(name, curve);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)
}

/// Delete a curve from the current profile's library.
#[tauri::command]
pub fn delete_profile_curve(
    state: State<Arc<AppState>>,
    name: String,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    libs.curves.remove(&name);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)
}

/// Rename a curve in the current profile's library.
#[tauri::command]
pub fn rename_profile_curve(
    state: State<Arc<AppState>>,
    old_name: String,
    new_name: String,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    if let Some(c) = libs.curves.remove(&old_name) {
        libs.curves.insert(new_name, c);
    }
    profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)
}

/// List all scripts in the current profile's library.
#[tauri::command]
pub fn list_profile_scripts(
    state: State<Arc<AppState>>,
) -> Result<Vec<(String, String)>, AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    Ok(libs.scripts.into_iter().collect())
}

/// Save a script into the current profile's library.
#[tauri::command]
pub fn set_profile_script(
    state: State<Arc<AppState>>,
    name: String,
    source: String,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    libs.scripts.insert(name, source);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)
}

/// Delete a script from the current profile's library.
#[tauri::command]
pub fn delete_profile_script(
    state: State<Arc<AppState>>,
    name: String,
) -> Result<(), AppError> {
    let data_dir = get_data_dir(&state)?;
    let slug = require_profile(&state)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
    libs.scripts.remove(&name);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)
}

/// Compile a profile-level script. On success, saves into the profile's library.
#[tauri::command]
pub fn compile_profile_script(
    state: State<Arc<AppState>>,
    name: String,
    source: String,
) -> Result<ScriptCompileResult, AppError> {
    match dsl::compile_source(&source) {
        Ok(_compiled) => {
            // Save source into the profile's library
            let data_dir = get_data_dir(&state)?;
            let slug = require_profile(&state)?;
            let mut libs = profile::load_libraries(&data_dir, &slug).map_err(AppError::from)?;
            libs.scripts.insert(name.clone(), source);
            profile::save_libraries(&data_dir, &slug, &libs).map_err(AppError::from)?;
            Ok(ScriptCompileResult {
                success: true,
                errors: vec![],
                name,
            })
        }
        Err(errors) => Ok(ScriptCompileResult {
            success: false,
            errors: errors
                .iter()
                .map(|e| ScriptError {
                    message: e.message.clone(),
                    offset: e.span.start,
                })
                .collect(),
            name,
        }),
    }
}

// ── Gradient library commands ─────────────────────────────────────

/// List all gradients in the active sequence's library.
#[tauri::command]
pub fn list_library_gradients(
    state: State<Arc<AppState>>,
) -> Vec<(String, crate::model::ColorGradient)> {
    state.with_show(|show| {
        show.sequences
            .first()
            .map(|seq| {
                seq.gradient_library
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Add or update a gradient in the active sequence's library.
#[tauri::command]
pub fn set_library_gradient(
    state: State<Arc<AppState>>,
    name: String,
    gradient: crate::model::ColorGradient,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::SetGradient {
        sequence_index: 0,
        name,
        gradient,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

/// Delete a gradient from the active sequence's library.
#[tauri::command]
pub fn delete_library_gradient(
    state: State<Arc<AppState>>,
    name: String,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::DeleteGradient {
        sequence_index: 0,
        name,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

/// Rename a gradient in the active sequence's library (updates all refs).
#[tauri::command]
pub fn rename_library_gradient(
    state: State<Arc<AppState>>,
    old_name: String,
    new_name: String,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::RenameGradient {
        sequence_index: 0,
        old_name,
        new_name,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

// ── Curve library commands ────────────────────────────────────────

/// List all curves in the active sequence's library.
#[tauri::command]
pub fn list_library_curves(
    state: State<Arc<AppState>>,
) -> Vec<(String, crate::model::Curve)> {
    state.with_show(|show| {
        show.sequences
            .first()
            .map(|seq| {
                seq.curve_library
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Add or update a curve in the active sequence's library.
#[tauri::command]
pub fn set_library_curve(
    state: State<Arc<AppState>>,
    name: String,
    curve: crate::model::Curve,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::SetCurve {
        sequence_index: 0,
        name,
        curve,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

/// Delete a curve from the active sequence's library.
#[tauri::command]
pub fn delete_library_curve(
    state: State<Arc<AppState>>,
    name: String,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::DeleteCurve {
        sequence_index: 0,
        name,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

/// Rename a curve in the active sequence's library (updates all refs).
#[tauri::command]
pub fn rename_library_curve(
    state: State<Arc<AppState>>,
    old_name: String,
    new_name: String,
) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::RenameCurve {
        sequence_index: 0,
        old_name,
        new_name,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(())
}

// ── Python environment commands ───────────────────────────────────

/// Check the current status of the Python analysis environment.
#[tauri::command]
pub async fn get_python_status(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<PythonEnvStatus, AppError> {
    let state_arc = (*state).clone();
    Ok(python::check_env_status(&state_arc.app_config_dir, &app_handle, &state_arc).await)
}

/// Bootstrap the Python environment (install Python, create venv, install deps).
#[tauri::command]
pub async fn setup_python_env(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<(), AppError> {
    let state_arc = (*state).clone();
    python::bootstrap_python(&app_handle, &state_arc.app_config_dir).await
}

/// Start the Python analysis sidecar. Returns the port it's listening on.
#[tauri::command]
pub async fn start_python_sidecar(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<u16, AppError> {
    let state_arc = (*state).clone();
    python::ensure_sidecar(&state_arc, &app_handle).await
}

/// Stop the Python analysis sidecar.
#[tauri::command]
pub async fn stop_python_sidecar(
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let state_arc = (*state).clone();
    let port = state_arc.python_port.load(Ordering::Relaxed);

    // Take the child out of the mutex so the guard is dropped before await
    let mut child_opt = state_arc.python_sidecar.lock().take();

    if let Some(ref mut child) = child_opt {
        python::stop_sidecar(child, port).await?;
    }
    state_arc.python_port.store(0, Ordering::Relaxed);
    Ok(())
}

// ── Audio analysis commands ───────────────────────────────────────

/// Get cached analysis for the current sequence's audio file.
#[tauri::command]
pub fn get_analysis(state: State<Arc<AppState>>) -> Option<AudioAnalysis> {
    let audio_file = state.with_show(|show| {
        show.sequences
            .first()
            .and_then(|s| s.audio_file.clone())
    })?;

    // Check memory cache
    if let Some(cached) = state.analysis_cache.lock().get(&audio_file) {
        return Some(cached.clone());
    }

    // Check disk cache
    let data_dir = state::get_data_dir(&state).ok()?;
    let profile_slug = state.current_profile.lock().clone()?;
    let media_dir = profile::media_dir(&data_dir, &profile_slug);
    let path = analysis::analysis_path(&media_dir, &audio_file);

    if path.exists() {
        if let Ok(loaded) = analysis::load_analysis(&path) {
            state
                .analysis_cache
                .lock()
                .insert(audio_file, loaded.clone());
            return Some(loaded);
        }
    }

    None
}

/// Run audio analysis on the current sequence's audio file.
#[tauri::command]
pub async fn analyze_audio(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    features: Option<AnalysisFeatures>,
) -> Result<AudioAnalysis, AppError> {
    let state_arc = (*state).clone();

    // Get audio file info
    let audio_file = state_arc
        .with_show(|show| {
            show.sequences
                .first()
                .and_then(|s| s.audio_file.clone())
        })
        .ok_or(AppError::AnalysisError {
            message: "No audio file in current sequence".into(),
        })?;

    let data_dir = state::get_data_dir(&state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(&state)?;
    let media_dir = profile::media_dir(&data_dir, &profile_slug);
    let audio_path = media_dir.join(&audio_file);

    if !audio_path.exists() {
        return Err(AppError::NotFound {
            what: format!("Audio file: {audio_file}"),
        });
    }

    // Determine features to analyze
    let features = features.unwrap_or_else(|| {
        state_arc
            .settings
            .lock()
            .as_ref()
            .and_then(|s| s.default_analysis_features.clone())
            .unwrap_or_default()
    });

    // Determine GPU setting
    let use_gpu = state_arc
        .settings
        .lock()
        .as_ref()
        .is_some_and(|s| s.use_gpu);

    // Ensure sidecar is running
    let port = python::ensure_sidecar(&state_arc, &app_handle).await?;

    // Run the analysis
    let output_dir = analysis::stems_dir(&media_dir, &audio_file);
    let models = python::models_dir(&state_arc.app_config_dir);

    let result = analysis::run_analysis(
        &app_handle,
        port,
        &audio_path,
        &output_dir,
        &features,
        &models,
        use_gpu,
    )
    .await?;

    // Save to disk cache
    let cache_path = analysis::analysis_path(&media_dir, &audio_file);
    if let Err(e) = analysis::save_analysis(&cache_path, &result) {
        eprintln!("[VibeLights] Failed to save analysis cache: {e}");
    }

    // Save to memory cache
    state_arc
        .analysis_cache
        .lock()
        .insert(audio_file, result.clone());

    Ok(result)
}
