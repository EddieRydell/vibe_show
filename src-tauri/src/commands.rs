use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::State;

use crate::chat::{ChatHistoryEntry, ChatManager};
use crate::dispatcher::{CommandResult, EditCommand, UndoState};
use crate::effects::{self, resolve_effect};
use crate::engine::{self, Frame};
use crate::import::vixen::{VixenDiscovery, VixenImportConfig, VixenImportResult};
use crate::model::{
    EffectKind, EffectTarget, ParamKey, ParamSchema, ParamValue, Show,
};
use crate::profile::{self, MediaFile, Profile, ProfileSummary, SequenceSummary};
use crate::progress::emit_progress;
use crate::settings::{self, AppSettings};
use crate::state::{self, AppState, PlaybackInfo};

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
pub fn initialize_data_dir(state: State<Arc<AppState>>, data_dir: String) -> Result<AppSettings, String> {
    let data_path = PathBuf::from(&data_dir);

    // Create the profiles directory
    std::fs::create_dir_all(data_path.join("profiles")).map_err(|e| e.to_string())?;

    let new_settings = AppSettings::new(data_path);
    settings::save_settings(&state.app_config_dir, &new_settings)
        .map_err(|e| e.to_string())?;

    *state.settings.lock() = Some(new_settings.clone());
    Ok(new_settings)
}

// ── Profile commands ───────────────────────────────────────────────

fn get_data_dir(state: &State<Arc<AppState>>) -> Result<std::path::PathBuf, String> {
    state::get_data_dir(state)
}

/// List all profiles.
#[tauri::command]
pub fn list_profiles(state: State<Arc<AppState>>) -> Result<Vec<ProfileSummary>, String> {
    let data_dir = get_data_dir(&state)?;
    profile::list_profiles(&data_dir).map_err(|e| e.to_string())
}

/// Create a new empty profile.
#[tauri::command]
pub fn create_profile(state: State<Arc<AppState>>, name: String) -> Result<ProfileSummary, String> {
    let data_dir = get_data_dir(&state)?;
    profile::create_profile(&data_dir, &name).map_err(|e| e.to_string())
}

/// Load a profile and set it as current.
#[tauri::command]
pub fn open_profile(state: State<Arc<AppState>>, slug: String) -> Result<Profile, String> {
    let data_dir = get_data_dir(&state)?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(|e| e.to_string())?;
    *state.current_profile.lock() = Some(slug.clone());
    *state.current_sequence.lock() = None;

    // Update last_profile in settings
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.last_profile = Some(slug);
        let _ = settings::save_settings(&state.app_config_dir, s);
    }

    Ok(loaded)
}

/// Delete a profile and all its data.
#[tauri::command]
pub fn delete_profile(state: State<Arc<AppState>>, slug: String) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    profile::delete_profile(&data_dir, &slug).map_err(|e| e.to_string())?;

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
pub fn save_profile(state: State<Arc<AppState>>) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(|e| e.to_string())?;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| e.to_string())
}

/// Update fixtures and groups on the current profile.
#[tauri::command]
pub fn update_profile_fixtures(
    state: State<Arc<AppState>>,
    fixtures: Vec<crate::model::FixtureDef>,
    groups: Vec<crate::model::FixtureGroup>,
) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(|e| e.to_string())?;
    loaded.fixtures = fixtures;
    loaded.groups = groups;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| e.to_string())
}

/// Update controllers and patches on the current profile.
#[tauri::command]
pub fn update_profile_setup(
    state: State<Arc<AppState>>,
    controllers: Vec<crate::model::Controller>,
    patches: Vec<crate::model::fixture::Patch>,
) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(|e| e.to_string())?;
    loaded.controllers = controllers;
    loaded.patches = patches;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| e.to_string())
}

/// Update the layout on the current profile.
#[tauri::command]
pub fn update_profile_layout(
    state: State<Arc<AppState>>,
    layout: crate::model::Layout,
) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(|e| e.to_string())?;
    loaded.layout = layout;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| e.to_string())
}

// ── Sequence commands ──────────────────────────────────────────────

/// List sequences in the current profile.
#[tauri::command]
pub fn list_sequences(state: State<Arc<AppState>>) -> Result<Vec<SequenceSummary>, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    profile::list_sequences(&data_dir, &profile_slug).map_err(|e| e.to_string())
}

/// Create a new empty sequence in the current profile.
#[tauri::command]
pub fn create_sequence(state: State<Arc<AppState>>, name: String) -> Result<SequenceSummary, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    profile::create_sequence(&data_dir, &profile_slug, &name).map_err(|e| e.to_string())
}

/// Delete a sequence from the current profile.
#[tauri::command]
pub fn delete_sequence(state: State<Arc<AppState>>, slug: String) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    profile::delete_sequence(&data_dir, &profile_slug, &slug).map_err(|e| e.to_string())?;

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
) -> Result<Show, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    let state_arc = (*state).clone();
    let app = app_handle.clone();

    let assembled = tokio::task::spawn_blocking(move || {
        emit_progress(&app, "open_sequence", "Loading profile...", 0.1, None);
        let profile_data =
            profile::load_profile(&data_dir, &profile_slug).map_err(|e| e.to_string())?;

        emit_progress(&app, "open_sequence", "Loading sequence...", 0.4, None);
        let sequence =
            profile::load_sequence(&data_dir, &profile_slug, &slug).map_err(|e| e.to_string())?;

        emit_progress(&app, "open_sequence", "Assembling show...", 0.7, None);
        let assembled = profile::assemble_show(&profile_data, &sequence);

        *state_arc.show.lock() = assembled.clone();
        state_arc.dispatcher.lock().clear();

        let mut playback = state_arc.playback.lock();
        playback.playing = false;
        playback.current_time = 0.0;
        playback.sequence_index = 0;

        *state_arc.current_sequence.lock() = Some(slug);

        emit_progress(&app, "open_sequence", "Ready", 1.0, None);

        Ok::<Show, String>(assembled)
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(assembled)
}

/// Save the currently active sequence back to disk.
#[tauri::command]
pub fn save_current_sequence(state: State<Arc<AppState>>) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    let seq_slug = state
        .current_sequence
        .lock()
        .clone()
        .ok_or("No sequence open")?;

    let show = state.show.lock();
    let sequence = show
        .sequences
        .first()
        .ok_or("No sequence in show")?;
    profile::save_sequence(&data_dir, &profile_slug, &seq_slug, sequence)
        .map_err(|e| e.to_string())
}

// ── Media commands ─────────────────────────────────────────────────

/// List audio files in the current profile.
#[tauri::command]
pub fn list_media(state: State<Arc<AppState>>) -> Result<Vec<MediaFile>, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    profile::list_media(&data_dir, &profile_slug).map_err(|e| e.to_string())
}

/// Import (copy) an audio file into the current profile's media directory.
#[tauri::command]
pub fn import_media(state: State<Arc<AppState>>, source_path: String) -> Result<MediaFile, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    profile::import_media(
        &data_dir,
        &profile_slug,
        std::path::Path::new(&source_path),
    )
    .map_err(|e| e.to_string())
}

/// Delete an audio file from the current profile.
#[tauri::command]
pub fn delete_media(state: State<Arc<AppState>>, filename: String) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    profile::delete_media(&data_dir, &profile_slug, &filename).map_err(|e| e.to_string())
}

// ── Media path resolution ──────────────────────────────────────────

/// Resolve a media filename to its absolute filesystem path.
#[tauri::command]
pub fn resolve_media_path(state: State<Arc<AppState>>, filename: String) -> Result<String, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or("No profile open")?;
    let path = profile::media_dir(&data_dir, &profile_slug).join(&filename);
    if !path.exists() {
        return Err(format!("Media file not found: {}", filename));
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
) -> Result<(), String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::UpdateSequenceSettings {
        sequence_index,
        name,
        audio_file,
        duration,
        frame_rate,
    };
    dispatcher.execute(&mut show, cmd)?;
    Ok(())
}

// ── Effects listing ────────────────────────────────────────────────

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct EffectInfo {
    pub kind: EffectKind,
    pub name: String,
    pub schema: Vec<ParamSchema>,
}

/// List all available built-in effect types with their parameter schemas.
#[tauri::command]
pub fn list_effects() -> Vec<EffectInfo> {
    let kinds = [
        EffectKind::Solid,
        EffectKind::Chase,
        EffectKind::Rainbow,
        EffectKind::Strobe,
        EffectKind::Gradient,
        EffectKind::Twinkle,
        EffectKind::Fade,
        EffectKind::Wipe,
    ];
    kinds
        .into_iter()
        .map(|kind| {
            let effect = effects::resolve_effect(&kind);
            EffectInfo {
                kind,
                name: effect.name().to_string(),
                schema: effect.param_schema(),
            }
        })
        .collect()
}

// ── Undo / Redo commands ──────────────────────────────────────────

/// Undo the last editing command.
#[tauri::command]
pub fn undo(state: State<Arc<AppState>>) -> Result<String, String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.undo(&mut show).map_err(|e| e.to_string())
}

/// Redo the last undone editing command.
#[tauri::command]
pub fn redo(state: State<Arc<AppState>>) -> Result<String, String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.redo(&mut show).map_err(|e| e.to_string())
}

/// Get the current undo/redo state.
#[tauri::command]
pub fn get_undo_state(state: State<Arc<AppState>>) -> UndoState {
    state.dispatcher.lock().undo_state()
}

// ── Chat commands ─────────────────────────────────────────────────

/// Send a message to the Claude chat. Starts the agentic tool-use loop.
#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    message: String,
) -> Result<(), String> {
    let state_arc = (*state).clone();
    let emitter = crate::chat::TauriChatEmitter { app_handle };
    ChatManager::send_message(state_arc, &emitter, message).await
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
pub fn set_claude_api_key(state: State<Arc<AppState>>, api_key: String) -> Result<(), String> {
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.claude_api_key = if api_key.is_empty() {
            None
        } else {
            Some(api_key)
        };
        settings::save_settings(&state.app_config_dir, s).map_err(|e| e.to_string())?;
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
        .map(|k| !k.is_empty())
        .unwrap_or(false)
}

// ── Engine / playback commands ─────────────────────────────────────

/// Get the full show model as JSON.
#[tauri::command]
pub fn get_show(state: State<Arc<AppState>>) -> Show {
    state.show.lock().clone()
}

/// Evaluate and return a single frame at the given time.
#[tauri::command]
pub fn get_frame(state: State<Arc<AppState>>, time: f64) -> Frame {
    let show = state.show.lock();
    let playback = state.playback.lock();
    engine::evaluate(&show, playback.sequence_index, time, None)
}

/// Evaluate and return a single frame at the given time, rendering only the
/// specified (track_index, effect_index) pairs. Used by the preview loop to
/// show only selected effects.
#[tauri::command]
pub fn get_frame_filtered(
    state: State<Arc<AppState>>,
    time: f64,
    effects: Vec<(usize, usize)>,
) -> Frame {
    let show = state.show.lock();
    let playback = state.playback.lock();
    engine::evaluate(&show, playback.sequence_index, time, Some(&effects))
}

/// Start playback.
#[tauri::command]
pub fn play(state: State<Arc<AppState>>) {
    let mut playback = state.playback.lock();
    playback.playing = true;
    playback.last_tick = Some(std::time::Instant::now());
}

/// Pause playback.
#[tauri::command]
pub fn pause(state: State<Arc<AppState>>) {
    let mut playback = state.playback.lock();
    playback.playing = false;
    playback.last_tick = None;
}

/// Seek to a specific time.
#[tauri::command]
pub fn seek(state: State<Arc<AppState>>, time: f64) {
    let mut playback = state.playback.lock();
    playback.current_time = time.max(0.0);
    // Reset clock anchor so next tick doesn't jump
    if playback.playing {
        playback.last_tick = Some(std::time::Instant::now());
    }
}

/// Get current playback state.
#[tauri::command]
pub fn get_playback(state: State<Arc<AppState>>) -> PlaybackInfo {
    let playback = state.playback.lock();
    let show = state.show.lock();
    let duration = show
        .sequences
        .get(playback.sequence_index)
        .map(|s| s.duration)
        .unwrap_or(0.0);
    PlaybackInfo {
        playing: playback.playing,
        current_time: playback.current_time,
        duration,
        sequence_index: playback.sequence_index,
    }
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
        .map(|s| s.duration)
        .unwrap_or(0.0);

    playback.current_time += real_dt;
    if playback.current_time >= duration {
        playback.current_time = duration;
        playback.playing = false;
        playback.last_tick = None;
    }

    let frame = engine::evaluate(&show, playback.sequence_index, playback.current_time, None);
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

    let effect = resolve_effect(&effect_instance.kind);
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

    let effect = resolve_effect(&effect_instance.kind);

    Some(EffectDetail {
        kind: effect_instance.kind.clone(),
        schema: effect.param_schema(),
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
    match dispatcher.execute(&mut show, cmd) {
        Ok(CommandResult::Bool(v)) => v,
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Add a new effect to a track with default params.
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
) -> Result<usize, String> {
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
    match dispatcher.execute(&mut show, cmd)? {
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
) -> Result<usize, String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::AddTrack {
        sequence_index,
        name,
        target,
    };
    match dispatcher.execute(&mut show, cmd)? {
        CommandResult::Index(idx) => Ok(idx),
        _ => Ok(0),
    }
}

/// Delete multiple effects across tracks. Targets are (track_index, effect_index) pairs.
#[tauri::command]
pub fn delete_effects(
    state: State<Arc<AppState>>,
    sequence_index: usize,
    targets: Vec<(usize, usize)>,
) -> Result<(), String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::DeleteEffects {
        sequence_index,
        targets,
    };
    dispatcher.execute(&mut show, cmd)?;
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
) -> Result<bool, String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::UpdateEffectTimeRange {
        sequence_index,
        track_index,
        effect_index,
        start,
        end,
    };
    match dispatcher.execute(&mut show, cmd)? {
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
) -> Result<usize, String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let cmd = EditCommand::MoveEffectToTrack {
        sequence_index,
        from_track,
        effect_index,
        to_track,
    };
    match dispatcher.execute(&mut show, cmd)? {
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
) -> Result<ProfileSummary, String> {
    let data_dir = get_data_dir(&state)?;

    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(std::path::Path::new(&system_config_path))
        .map_err(|e| e.to_string())?;
    for seq_path in &sequence_paths {
        importer
            .parse_sequence(std::path::Path::new(seq_path))
            .map_err(|e| e.to_string())?;
    }

    let guid_map = importer.guid_map().clone();
    let show = importer.into_show();

    // Create a profile from the imported data
    let profile_name = if show.name.is_empty() {
        "Vixen Import".to_string()
    } else {
        show.name.clone()
    };
    let summary = profile::create_profile(&data_dir, &profile_name).map_err(|e| e.to_string())?;

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
    profile::save_profile(&data_dir, &summary.slug, &prof).map_err(|e| e.to_string())?;

    // Persist the GUID map for future sequence imports
    profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
        .map_err(|e| e.to_string())?;

    // Save sequences directly into the profile
    for seq in &show.sequences {
        profile::create_sequence(&data_dir, &summary.slug, &seq.name)
            .map_err(|e| e.to_string())?;
        let seq_slug = crate::project::slugify(&seq.name);
        profile::save_sequence(&data_dir, &summary.slug, &seq_slug, seq)
            .map_err(|e| e.to_string())?;
    }

    // Re-read summary to get accurate counts
    let profiles = profile::list_profiles(&data_dir).map_err(|e| e.to_string())?;
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
) -> Result<ProfileSummary, String> {
    let data_dir = get_data_dir(&state)?;

    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(std::path::Path::new(&system_config_path))
        .map_err(|e| e.to_string())?;

    let guid_map = importer.guid_map().clone();
    let show = importer.into_show();

    let profile_name = "Vixen Import".to_string();
    let summary = profile::create_profile(&data_dir, &profile_name).map_err(|e| e.to_string())?;

    let prof = Profile {
        name: profile_name,
        slug: summary.slug.clone(),
        fixtures: show.fixtures.clone(),
        groups: show.groups.clone(),
        controllers: show.controllers.clone(),
        patches: show.patches.clone(),
        layout: show.layout.clone(),
    };
    profile::save_profile(&data_dir, &summary.slug, &prof).map_err(|e| e.to_string())?;

    // Persist the GUID map for future sequence imports
    profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
        .map_err(|e| e.to_string())?;

    // Re-read summary to get accurate counts
    let profiles = profile::list_profiles(&data_dir).map_err(|e| e.to_string())?;
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
) -> Result<SequenceSummary, String> {
    let data_dir = get_data_dir(&state)?;

    let prof = profile::load_profile(&data_dir, &profile_slug).map_err(|e| e.to_string())?;
    let guid_map =
        profile::load_vixen_guid_map(&data_dir, &profile_slug).map_err(|e| e.to_string())?;

    if guid_map.is_empty() {
        return Err(
            "No Vixen GUID map found for this profile. Import the profile from Vixen first."
                .to_string(),
        );
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
        .map_err(|e| e.to_string())?;

    let sequences = importer.into_sequences();
    let seq = sequences
        .into_iter()
        .next()
        .ok_or("No sequence parsed from file")?;

    let seq_slug = crate::project::slugify(&seq.name);

    // Create and save the sequence
    let _ = profile::create_sequence(&data_dir, &profile_slug, &seq.name);
    profile::save_sequence(&data_dir, &profile_slug, &seq_slug, &seq)
        .map_err(|e| e.to_string())?;

    Ok(SequenceSummary {
        name: seq.name,
        slug: seq_slug,
    })
}

/// Scan a Vixen 3 data directory and return a summary of what's available for import.
#[tauri::command]
pub fn scan_vixen_directory(vixen_dir: String) -> Result<VixenDiscovery, String> {
    use crate::import::vixen_preview;

    let vixen_path = std::path::Path::new(&vixen_dir);

    // Validate directory structure
    let config_path = vixen_path.join("SystemData").join("SystemConfig.xml");
    if !config_path.exists() {
        return Err(format!(
            "Not a valid Vixen 3 directory: SystemData/SystemConfig.xml not found in {}",
            vixen_dir
        ));
    }

    // Parse SystemConfig.xml to count fixtures, groups, controllers
    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(&config_path)
        .map_err(|e| e.to_string())?;

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
    let audio_extensions = ["mp3", "wav", "ogg", "flac", "m4a", "aac", "wma"];
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
                    if audio_extensions.contains(&ext.as_str()) {
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
pub fn check_vixen_preview_file(file_path: String) -> Result<usize, String> {
    use crate::import::vixen_preview;

    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err("File not found".into());
    }

    let data = vixen_preview::parse_preview_file(path).map_err(|e| e.to_string())?;
    if data.display_items.is_empty() {
        return Err(
            "File was parsed but no display items were found. \
             The file may not contain Vixen preview/layout data."
                .into(),
        );
    }

    Ok(data.display_items.len())
}

/// Execute a full Vixen import based on user-selected configuration from the wizard.
#[tauri::command]
pub async fn execute_vixen_import(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    config: VixenImportConfig,
) -> Result<VixenImportResult, String> {
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
            .map_err(|e| e.to_string())?;

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
            profile::create_profile(&data_dir, &profile_name).map_err(|e| e.to_string())?;

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
        profile::save_profile(&data_dir, &summary.slug, &prof).map_err(|e| e.to_string())?;

        profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
            .map_err(|e| e.to_string())?;

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
            let _ = profile::create_sequence(&data_dir, &summary.slug, &seq.name);
            let seq_slug = crate::project::slugify(&seq.name);
            profile::save_sequence(&data_dir, &summary.slug, &seq_slug, &seq)
                .map_err(|e| e.to_string())?;
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
    .map_err(|e| e.to_string())?
}
