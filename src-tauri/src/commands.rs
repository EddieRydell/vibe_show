use std::path::PathBuf;
use std::sync::Mutex;

use tauri::State;

use crate::effects::{self, resolve_effect};
use crate::engine::{self, Frame};
use crate::model::{
    BlendMode, EffectInstance, EffectKind, EffectParams, EffectTarget, ParamSchema, ParamValue,
    Show, TimeRange, Track,
};
use crate::profile::{self, MediaFile, Profile, ProfileSummary, ShowData, ShowSummary};
use crate::settings::{self, AppSettings};

// ── Application State ──────────────────────────────────────────────

/// Application state shared across Tauri commands.
pub struct AppState {
    pub show: Mutex<Show>,
    pub playback: Mutex<PlaybackState>,
    pub app_config_dir: PathBuf,
    pub settings: Mutex<Option<AppSettings>>,
    pub current_profile: Mutex<Option<String>>,
    pub current_show: Mutex<Option<String>>,
}

pub struct PlaybackState {
    pub playing: bool,
    pub current_time: f64,
    pub sequence_index: usize,
}

#[derive(serde::Serialize)]
pub struct PlaybackInfo {
    pub playing: bool,
    pub current_time: f64,
    pub duration: f64,
    pub sequence_index: usize,
}

// ── Settings commands ──────────────────────────────────────────────

/// Check if settings exist (first launch detection).
#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Option<AppSettings> {
    state.settings.lock().unwrap().clone()
}

/// First launch: set data directory, create folder structure, save settings.
#[tauri::command]
pub fn initialize_data_dir(state: State<AppState>, data_dir: String) -> Result<AppSettings, String> {
    let data_path = PathBuf::from(&data_dir);

    // Create the profiles directory
    std::fs::create_dir_all(data_path.join("profiles")).map_err(|e| e.to_string())?;

    let new_settings = AppSettings::new(data_path);
    settings::save_settings(&state.app_config_dir, &new_settings)
        .map_err(|e| e.to_string())?;

    *state.settings.lock().unwrap() = Some(new_settings.clone());
    Ok(new_settings)
}

// ── Profile commands ───────────────────────────────────────────────

fn get_data_dir(state: &State<AppState>) -> Result<PathBuf, String> {
    state
        .settings
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.data_dir.clone())
        .ok_or_else(|| "No data directory configured".to_string())
}

/// List all profiles.
#[tauri::command]
pub fn list_profiles(state: State<AppState>) -> Result<Vec<ProfileSummary>, String> {
    let data_dir = get_data_dir(&state)?;
    profile::list_profiles(&data_dir).map_err(|e| e.to_string())
}

/// Create a new empty profile.
#[tauri::command]
pub fn create_profile(state: State<AppState>, name: String) -> Result<ProfileSummary, String> {
    let data_dir = get_data_dir(&state)?;
    profile::create_profile(&data_dir, &name).map_err(|e| e.to_string())
}

/// Load a profile and set it as current.
#[tauri::command]
pub fn open_profile(state: State<AppState>, slug: String) -> Result<Profile, String> {
    let data_dir = get_data_dir(&state)?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(|e| e.to_string())?;
    *state.current_profile.lock().unwrap() = Some(slug.clone());
    *state.current_show.lock().unwrap() = None;

    // Update last_profile in settings
    let mut settings_guard = state.settings.lock().unwrap();
    if let Some(ref mut s) = *settings_guard {
        s.last_profile = Some(slug);
        let _ = settings::save_settings(&state.app_config_dir, s);
    }

    Ok(loaded)
}

/// Delete a profile and all its data.
#[tauri::command]
pub fn delete_profile(state: State<AppState>, slug: String) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    profile::delete_profile(&data_dir, &slug).map_err(|e| e.to_string())?;

    // Clear current if it was the deleted one
    let mut current = state.current_profile.lock().unwrap();
    if current.as_deref() == Some(&slug) {
        *current = None;
        *state.current_show.lock().unwrap() = None;
    }

    Ok(())
}

/// Save the current profile's house data.
#[tauri::command]
pub fn save_profile(state: State<AppState>) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(|e| e.to_string())?;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| e.to_string())
}

/// Update fixtures and groups on the current profile.
#[tauri::command]
pub fn update_profile_fixtures(
    state: State<AppState>,
    fixtures: Vec<crate::model::FixtureDef>,
    groups: Vec<crate::model::FixtureGroup>,
) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let slug = state
        .current_profile
        .lock()
        .unwrap()
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
    state: State<AppState>,
    controllers: Vec<crate::model::Controller>,
    patches: Vec<crate::model::fixture::Patch>,
) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let slug = state
        .current_profile
        .lock()
        .unwrap()
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
    state: State<AppState>,
    layout: crate::model::Layout,
) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(|e| e.to_string())?;
    loaded.layout = layout;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| e.to_string())
}

// ── Show commands ──────────────────────────────────────────────────

/// List shows in the current profile.
#[tauri::command]
pub fn list_shows(state: State<AppState>) -> Result<Vec<ShowSummary>, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;
    profile::list_shows(&data_dir, &profile_slug).map_err(|e| e.to_string())
}

/// Create a new empty show in the current profile.
#[tauri::command]
pub fn create_show(state: State<AppState>, name: String) -> Result<ShowSummary, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;
    profile::create_show(&data_dir, &profile_slug, &name).map_err(|e| e.to_string())
}

/// Load a show into the engine state for editing.
#[tauri::command]
pub fn open_show(state: State<AppState>, slug: String) -> Result<Show, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;

    let profile_data =
        profile::load_profile(&data_dir, &profile_slug).map_err(|e| e.to_string())?;
    let show_data =
        profile::load_show(&data_dir, &profile_slug, &slug).map_err(|e| e.to_string())?;

    let assembled = profile::assemble_show(&profile_data, &show_data);
    *state.show.lock().unwrap() = assembled.clone();

    let mut playback = state.playback.lock().unwrap();
    playback.playing = false;
    playback.current_time = 0.0;
    playback.sequence_index = 0;

    *state.current_show.lock().unwrap() = Some(slug);

    Ok(assembled)
}

/// Save the currently active show back to disk.
#[tauri::command]
pub fn save_current_show(state: State<AppState>) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;
    let show_slug = state
        .current_show
        .lock()
        .unwrap()
        .clone()
        .ok_or("No show open")?;

    let show = state.show.lock().unwrap();
    let show_data = ShowData {
        name: show.name.clone(),
        sequences: show.sequences.clone(),
    };
    profile::save_show(&data_dir, &profile_slug, &show_slug, &show_data)
        .map_err(|e| e.to_string())
}

/// Delete a show from the current profile.
#[tauri::command]
pub fn delete_show(state: State<AppState>, slug: String) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;
    profile::delete_show(&data_dir, &profile_slug, &slug).map_err(|e| e.to_string())?;

    // Clear current show if it was the deleted one
    let mut current = state.current_show.lock().unwrap();
    if current.as_deref() == Some(&slug) {
        *current = None;
    }

    Ok(())
}

// ── Media commands ─────────────────────────────────────────────────

/// List audio files in the current profile.
#[tauri::command]
pub fn list_media(state: State<AppState>) -> Result<Vec<MediaFile>, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;
    profile::list_media(&data_dir, &profile_slug).map_err(|e| e.to_string())
}

/// Import (copy) an audio file into the current profile's media directory.
#[tauri::command]
pub fn import_media(state: State<AppState>, source_path: String) -> Result<MediaFile, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
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
pub fn delete_media(state: State<AppState>, filename: String) -> Result<(), String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .ok_or("No profile open")?;
    profile::delete_media(&data_dir, &profile_slug, &filename).map_err(|e| e.to_string())
}

// ── Media path resolution ──────────────────────────────────────────

/// Resolve a media filename to its absolute filesystem path.
#[tauri::command]
pub fn resolve_media_path(state: State<AppState>, filename: String) -> Result<String, String> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state
        .current_profile
        .lock()
        .unwrap()
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
    state: State<AppState>,
    sequence_index: usize,
    name: Option<String>,
    audio_file: Option<Option<String>>,
    duration: Option<f64>,
    frame_rate: Option<f64>,
) -> Result<(), String> {
    let mut show = state.show.lock().unwrap();
    let sequence = show
        .sequences
        .get_mut(sequence_index)
        .ok_or("Invalid sequence index")?;

    if let Some(n) = name {
        sequence.name = n;
    }
    if let Some(af) = audio_file {
        sequence.audio_file = af;
    }
    if let Some(d) = duration {
        if d <= 0.0 {
            return Err("Duration must be positive".into());
        }
        sequence.duration = d;
    }
    if let Some(fr) = frame_rate {
        if fr <= 0.0 {
            return Err("Frame rate must be positive".into());
        }
        sequence.frame_rate = fr;
    }
    Ok(())
}

// ── Effects listing ────────────────────────────────────────────────

#[derive(serde::Serialize)]
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

// ── Engine / playback commands ─────────────────────────────────────

/// Get the full show model as JSON.
#[tauri::command]
pub fn get_show(state: State<AppState>) -> Show {
    state.show.lock().unwrap().clone()
}

/// Evaluate and return a single frame at the given time.
#[tauri::command]
pub fn get_frame(state: State<AppState>, time: f64) -> Frame {
    let show = state.show.lock().unwrap();
    let playback = state.playback.lock().unwrap();
    engine::evaluate(&show, playback.sequence_index, time)
}

/// Start playback.
#[tauri::command]
pub fn play(state: State<AppState>) {
    let mut playback = state.playback.lock().unwrap();
    playback.playing = true;
}

/// Pause playback.
#[tauri::command]
pub fn pause(state: State<AppState>) {
    let mut playback = state.playback.lock().unwrap();
    playback.playing = false;
}

/// Seek to a specific time.
#[tauri::command]
pub fn seek(state: State<AppState>, time: f64) {
    let mut playback = state.playback.lock().unwrap();
    playback.current_time = time.max(0.0);
}

/// Get current playback state.
#[tauri::command]
pub fn get_playback(state: State<AppState>) -> PlaybackInfo {
    let playback = state.playback.lock().unwrap();
    let show = state.show.lock().unwrap();
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

/// Advance playback by dt seconds. Returns the new frame if playing.
#[tauri::command]
pub fn tick(state: State<AppState>, dt: f64) -> Option<TickResult> {
    let mut playback = state.playback.lock().unwrap();
    if !playback.playing {
        return None;
    }

    let show = state.show.lock().unwrap();
    let duration = show
        .sequences
        .get(playback.sequence_index)
        .map(|s| s.duration)
        .unwrap_or(0.0);

    playback.current_time += dt;
    if playback.current_time >= duration {
        playback.current_time = duration;
        playback.playing = false;
    }

    let frame = engine::evaluate(&show, playback.sequence_index, playback.current_time);
    Some(TickResult {
        frame,
        current_time: playback.current_time,
        playing: playback.playing,
    })
}

#[derive(serde::Serialize)]
pub struct TickResult {
    pub frame: Frame,
    pub current_time: f64,
    pub playing: bool,
}

/// Pre-render an effect as a thumbnail image for the timeline.
#[tauri::command]
pub fn render_effect_thumbnail(
    state: State<AppState>,
    sequence_index: usize,
    track_index: usize,
    effect_index: usize,
    time_samples: usize,
    pixel_rows: usize,
) -> Option<EffectThumbnail> {
    let show = state.show.lock().unwrap();
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

#[derive(serde::Serialize)]
pub struct EffectThumbnail {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub start_time: f64,
    pub end_time: f64,
}

#[derive(serde::Serialize)]
pub struct EffectDetail {
    pub kind: EffectKind,
    pub schema: Vec<ParamSchema>,
    pub params: EffectParams,
    pub time_range: TimeRange,
    pub track_name: String,
    pub blend_mode: BlendMode,
}

/// Get the schema, current params, and metadata for a placed effect.
#[tauri::command]
pub fn get_effect_detail(
    state: State<AppState>,
    sequence_index: usize,
    track_index: usize,
    effect_index: usize,
) -> Option<EffectDetail> {
    let show = state.show.lock().unwrap();
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
        blend_mode: track.blend_mode,
    })
}

/// Update a single parameter on a placed effect.
#[tauri::command]
pub fn update_effect_param(
    state: State<AppState>,
    sequence_index: usize,
    track_index: usize,
    effect_index: usize,
    key: String,
    value: ParamValue,
) -> bool {
    let mut show = state.show.lock().unwrap();
    let Some(sequence) = show.sequences.get_mut(sequence_index) else {
        return false;
    };
    let Some(track) = sequence.tracks.get_mut(track_index) else {
        return false;
    };
    let Some(effect_instance) = track.effects.get_mut(effect_index) else {
        return false;
    };
    effect_instance.params.set_mut(key, value);
    true
}

/// Add a new effect to a track with default params.
#[tauri::command]
pub fn add_effect(
    state: State<AppState>,
    sequence_index: usize,
    track_index: usize,
    kind: EffectKind,
    start: f64,
    end: f64,
) -> Result<usize, String> {
    let time_range = TimeRange::new(start, end).ok_or("Invalid time range")?;
    let mut show = state.show.lock().unwrap();
    let sequence = show
        .sequences
        .get_mut(sequence_index)
        .ok_or("Invalid sequence index")?;
    let track = sequence
        .tracks
        .get_mut(track_index)
        .ok_or("Invalid track index")?;
    let effect = EffectInstance {
        kind,
        params: EffectParams::new(),
        time_range,
    };
    track.effects.push(effect);
    Ok(track.effects.len() - 1)
}

/// Add a new track to a sequence.
#[tauri::command]
pub fn add_track(
    state: State<AppState>,
    sequence_index: usize,
    name: String,
    target: EffectTarget,
    blend_mode: BlendMode,
) -> Result<usize, String> {
    let mut show = state.show.lock().unwrap();
    let sequence = show
        .sequences
        .get_mut(sequence_index)
        .ok_or("Invalid sequence index")?;
    let track = Track {
        name,
        target,
        effects: Vec::new(),
        blend_mode,
    };
    sequence.tracks.push(track);
    Ok(sequence.tracks.len() - 1)
}

/// Delete multiple effects across tracks. Targets are (track_index, effect_index) pairs.
#[tauri::command]
pub fn delete_effects(
    state: State<AppState>,
    sequence_index: usize,
    targets: Vec<(usize, usize)>,
) -> Result<(), String> {
    let mut show = state.show.lock().unwrap();
    let sequence = show
        .sequences
        .get_mut(sequence_index)
        .ok_or("Invalid sequence index")?;

    // Group by track index, then sort effect indices descending to preserve indices during removal
    let mut by_track: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for (track_idx, effect_idx) in targets {
        by_track.entry(track_idx).or_default().push(effect_idx);
    }
    for (track_idx, mut effect_indices) in by_track {
        let track = sequence
            .tracks
            .get_mut(track_idx)
            .ok_or(format!("Invalid track index {track_idx}"))?;
        effect_indices.sort_unstable();
        effect_indices.dedup();
        // Remove from highest index first
        for &idx in effect_indices.iter().rev() {
            if idx < track.effects.len() {
                track.effects.remove(idx);
            }
        }
    }
    Ok(())
}

/// Update the time range of an effect.
#[tauri::command]
pub fn update_effect_time_range(
    state: State<AppState>,
    sequence_index: usize,
    track_index: usize,
    effect_index: usize,
    start: f64,
    end: f64,
) -> Result<bool, String> {
    let time_range = TimeRange::new(start, end).ok_or("Invalid time range")?;
    let mut show = state.show.lock().unwrap();
    let Some(sequence) = show.sequences.get_mut(sequence_index) else {
        return Ok(false);
    };
    let Some(track) = sequence.tracks.get_mut(track_index) else {
        return Ok(false);
    };
    let Some(effect) = track.effects.get_mut(effect_index) else {
        return Ok(false);
    };
    effect.time_range = time_range;
    Ok(true)
}

/// Move an effect from one track to another. Returns the new effect index in the destination track.
#[tauri::command]
pub fn move_effect_to_track(
    state: State<AppState>,
    sequence_index: usize,
    from_track: usize,
    effect_index: usize,
    to_track: usize,
) -> Result<usize, String> {
    let mut show = state.show.lock().unwrap();
    let sequence = show
        .sequences
        .get_mut(sequence_index)
        .ok_or("Invalid sequence index")?;

    if from_track >= sequence.tracks.len() {
        return Err(format!("Invalid source track index {from_track}"));
    }
    if to_track >= sequence.tracks.len() {
        return Err(format!("Invalid destination track index {to_track}"));
    }
    if effect_index >= sequence.tracks[from_track].effects.len() {
        return Err(format!("Invalid effect index {effect_index}"));
    }

    let effect = sequence.tracks[from_track].effects.remove(effect_index);
    sequence.tracks[to_track].effects.push(effect);
    Ok(sequence.tracks[to_track].effects.len() - 1)
}

/// Switch to a different sequence by index.
#[tauri::command]
pub fn select_sequence(state: State<AppState>, index: usize) -> Option<PlaybackInfo> {
    let show = state.show.lock().unwrap();
    let sequence = show.sequences.get(index)?;
    let mut playback = state.playback.lock().unwrap();
    playback.sequence_index = index;
    playback.current_time = 0.0;
    playback.playing = false;
    Some(PlaybackInfo {
        playing: false,
        current_time: 0.0,
        duration: sequence.duration,
        sequence_index: index,
    })
}

/// Import a Vixen 3 show into the current profile, creating a new profile + show from the import.
#[tauri::command]
pub fn import_vixen(
    state: State<AppState>,
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

    // Create a show with the sequences
    if !show.sequences.is_empty() {
        let show_summary =
            profile::create_show(&data_dir, &summary.slug, "Imported Show").map_err(|e| e.to_string())?;
        let show_data = ShowData {
            name: "Imported Show".into(),
            sequences: show.sequences,
        };
        profile::save_show(&data_dir, &summary.slug, &show_summary.slug, &show_data)
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
