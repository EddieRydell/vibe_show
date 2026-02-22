#![allow(
    clippy::needless_pass_by_value,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::unreachable
)]

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::State;

use crate::agent;
use crate::analysis;
use crate::chat::ChatManager;
use crate::dsl;
use crate::effects::resolve_effect;
use crate::engine::{self, Frame};
use crate::error::AppError;
use crate::import::vixen::{VixenImportConfig, VixenImportResult};
use crate::model::{AnalysisFeatures, AudioAnalysis, PythonEnvStatus, Show};
use crate::profile::{self, Profile};
use crate::progress::emit_progress;
use crate::python;
use crate::state::{self, AppState};

// ── Helpers ──────────────────────────────────────────────────────

fn get_data_dir(state: &State<Arc<AppState>>) -> Result<std::path::PathBuf, AppError> {
    state::get_data_dir(state).map_err(|_| AppError::NoSettings)
}


// ── Async commands ───────────────────────────────────────────────

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

/// Load a sequence into the engine state for editing.
#[tauri::command]
pub async fn open_sequence(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    slug: String,
) -> Result<Show, AppError> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = state.require_profile()?;
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
    let profile_slug = state.require_profile()?;
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

// ── Binary / hot-path commands ───────────────────────────────────

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

/// Generate a spacetime heatmap preview for a compiled script.
/// Returns RGBA pixel data: rows = pixel indices, cols = time samples.
#[allow(clippy::cast_precision_loss)]
#[tauri::command]
pub fn preview_script(
    state: State<Arc<AppState>>,
    name: String,
    params: crate::model::timeline::EffectParams,
    pixel_count: usize,
    time_samples: usize,
) -> Result<ScriptPreviewData, AppError> {
    let cache = state.script_cache.lock();
    let compiled = cache.get(&name).ok_or_else(|| AppError::ApiError {
        message: format!("Script '{name}' not found in cache"),
    })?;

    let width = time_samples;
    let height = pixel_count;
    let mut pixels = vec![0u8; width * height * 4];

    for col in 0..time_samples {
        let t = if time_samples > 1 {
            col as f64 / (time_samples - 1) as f64
        } else {
            0.0
        };
        let mut frame = vec![crate::model::color::Color::default(); pixel_count];
        crate::effects::script::evaluate_pixels_batch(
            compiled,
            t,
            &mut frame,
            0,
            pixel_count,
            &params,
            crate::model::timeline::BlendMode::Override,
            1.0,
            None,
        );
        for (row, color) in frame.iter().enumerate() {
            let idx = (row * width + col) * 4;
            if let Some(chunk) = pixels.get_mut(idx..idx + 4) {
                chunk.copy_from_slice(&[color.r, color.g, color.b, color.a]);
            }
        }
    }

    Ok(ScriptPreviewData {
        width,
        height,
        pixels,
    })
}

/// Evaluate a single frame of a compiled script for the pixel strip preview.
#[tauri::command]
pub fn preview_script_frame(
    state: State<Arc<AppState>>,
    name: String,
    params: crate::model::timeline::EffectParams,
    pixel_count: usize,
    t: f64,
) -> Result<Vec<[u8; 4]>, AppError> {
    let cache = state.script_cache.lock();
    let compiled = cache.get(&name).ok_or_else(|| AppError::ApiError {
        message: format!("Script '{name}' not found in cache"),
    })?;

    let mut frame = vec![crate::model::color::Color::default(); pixel_count];
    crate::effects::script::evaluate_pixels_batch(
        compiled,
        t,
        &mut frame,
        0,
        pixel_count,
        &params,
        crate::model::timeline::BlendMode::Override,
        1.0,
        None,
    );

    Ok(frame.iter().map(|c| [c.r, c.g, c.b, c.a]).collect())
}

// ── Agent sidecar commands ───────────────────────────────────────

/// Send a message to the agent sidecar (Claude Agent SDK).
#[tauri::command]
pub async fn send_agent_message(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    message: String,
) -> Result<(), AppError> {
    let state_arc = (*state).clone();
    let emitter = crate::chat::TauriChatEmitter {
        app_handle: app_handle.clone(),
    };
    agent::send_message(&state_arc, &app_handle, &emitter, message).await
}

/// Cancel the in-flight agent query.
#[tauri::command]
pub async fn cancel_agent_message(
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let state_arc = (*state).clone();
    agent::cancel_message(&state_arc).await
}

/// Clear the agent session (reset conversation context).
#[tauri::command]
pub async fn clear_agent_session(
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let state_arc = (*state).clone();
    agent::clear_session(&state_arc).await
}

// ── Unified command registry ─────────────────────────────────────

/// Execute any Command through the unified registry.
#[tauri::command]
pub fn exec(
    state: State<Arc<AppState>>,
    cmd: crate::registry::Command,
) -> Result<crate::registry::CommandOutput, AppError> {
    crate::registry::execute::execute(&state, cmd)
}

/// Get the full command registry with schemas.
#[tauri::command]
pub fn get_command_registry() -> serde_json::Value {
    crate::registry::catalog::to_json_schema()
}

// ── Types ────────────────────────────────────────────────────────

pub use crate::state::EffectDetail;

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptCompileResult {
    pub success: bool,
    pub errors: Vec<ScriptError>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<ScriptParamInfo>>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptParamInfo {
    pub name: String,
    pub param_type: crate::model::timeline::ParamType,
    pub default: Option<crate::model::timeline::ParamValue>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptPreviewData {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptError {
    pub message: String,
    pub offset: usize,
}

// ── Helper functions (used by registry handlers and kept commands) ─

/// Recompile all scripts in the active sequence **and** the profile library
/// (e.g., after loading a show from disk).
/// Returns a list of scripts that failed to compile.
pub fn recompile_all_scripts(state: &AppState) -> Vec<String> {
    let sources: Vec<(String, String)> = state.with_show(|show| {
        show.sequences
            .first()
            .map(|seq| seq.scripts.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    });

    // Also load profile library scripts so their previews work.
    let profile_sources: Vec<(String, String)> = (|| {
        let slug = state.current_profile.lock().clone()?;
        let data_dir = state::get_data_dir(state).ok()?;
        let libs = profile::load_libraries(&data_dir, &slug).ok()?;
        Some(libs.scripts.into_iter().collect::<Vec<_>>())
    })()
    .unwrap_or_default();

    let mut failures = Vec::new();
    let mut cache = state.script_cache.lock();
    cache.clear();

    // Profile library scripts first (sequence scripts override on name collision)
    for (name, source) in profile_sources {
        match dsl::compile_source(&source) {
            Ok(compiled) => {
                cache.insert(name, std::sync::Arc::new(compiled));
            }
            Err(_) => {
                failures.push(name);
            }
        }
    }

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

/// Convert DSL `CompiledParam` entries to model-layer `ScriptParamInfo`.
pub fn extract_script_params(compiled: &dsl::compiler::CompiledScript) -> Vec<ScriptParamInfo> {
    compiled
        .params
        .iter()
        .map(|cp| {
            let (param_type, default) = dsl_param_to_model(cp, compiled);
            ScriptParamInfo {
                name: cp.name.clone(),
                param_type,
                default,
            }
        })
        .collect()
}

/// Extract the default `ParamValue` from the DSL default expression.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn eval_param_default(
    cp: &dsl::compiler::CompiledParam,
    compiled: &dsl::compiler::CompiledScript,
) -> Option<crate::model::timeline::ParamValue> {
    use crate::dsl::ast::{ExprKind, ParamType as Dsl};
    use crate::model::timeline::ParamValue;

    match (&cp.ty, &cp.default.kind) {
        // Float: literal number
        (Dsl::Float(_), ExprKind::FloatLit(v)) => Some(ParamValue::Float(*v)),
        (Dsl::Float(_), ExprKind::IntLit(v)) => Some(ParamValue::Float(f64::from(*v))),
        // Int: literal number
        (Dsl::Int(_), ExprKind::IntLit(v)) => Some(ParamValue::Int(*v)),
        (Dsl::Int(_), ExprKind::FloatLit(v)) => Some(ParamValue::Int(*v as i32)),
        // Bool: literal
        (Dsl::Bool, ExprKind::BoolLit(v)) => Some(ParamValue::Bool(*v)),
        // Color: hex literal
        (Dsl::Color, ExprKind::ColorLit { r, g, b }) => {
            Some(ParamValue::Color(crate::model::color::Color {
                r: *r,
                g: *g,
                b: *b,
                a: 255,
            }))
        }
        // Gradient: gradient literal with color stops
        (Dsl::Gradient, ExprKind::GradientLit(stops)) => {
            let count = stops.len();
            let model_stops: Vec<crate::model::color_gradient::ColorStop> = stops
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let position = s.position.unwrap_or_else(|| {
                        if count <= 1 {
                            0.0
                        } else {
                            i as f64 / (count - 1) as f64
                        }
                    });
                    crate::model::color_gradient::ColorStop {
                        position,
                        color: crate::model::color::Color {
                            r: s.color.0,
                            g: s.color.1,
                            b: s.color.2,
                            a: 255,
                        },
                    }
                })
                .collect();
            crate::model::color_gradient::ColorGradient::new(model_stops)
                .map(ParamValue::ColorGradient)
        }
        // Curve: curve literal with control points
        (Dsl::Curve, ExprKind::CurveLit(points)) => {
            let model_points: Vec<crate::model::curve::CurvePoint> = points
                .iter()
                .map(|(x, y)| crate::model::curve::CurvePoint { x: *x, y: *y })
                .collect();
            crate::model::curve::Curve::new(model_points).map(ParamValue::Curve)
        }
        // Enum: identifier (variant name)
        (Dsl::Named(type_name), ExprKind::Ident(variant)) => {
            // Verify it's an enum
            if compiled.enums.iter().any(|e| e.name == *type_name) {
                Some(ParamValue::EnumVariant(variant.clone()))
            } else {
                None
            }
        }
        // Flags: flag combination
        (Dsl::Named(_), ExprKind::FlagCombine(flags)) => {
            Some(ParamValue::FlagSet(flags.clone()))
        }
        _ => None,
    }
}

/// Map a DSL `CompiledParam` to model-layer `ParamType` + optional default `ParamValue`.
#[allow(clippy::cast_possible_truncation)]
pub fn dsl_param_to_model(
    cp: &dsl::compiler::CompiledParam,
    compiled: &dsl::compiler::CompiledScript,
) -> (
    crate::model::timeline::ParamType,
    Option<crate::model::timeline::ParamValue>,
) {
    use crate::dsl::ast::ParamType as Dsl;
    use crate::model::timeline::{ParamType as Model, ParamValue};

    let default = eval_param_default(cp, compiled);

    match &cp.ty {
        Dsl::Float(range) => {
            let (min, max) = range.unwrap_or((0.0, 1.0));
            (
                Model::Float {
                    min,
                    max,
                    step: 0.01,
                },
                default.or(Some(ParamValue::Float(min))),
            )
        }
        Dsl::Int(range) => {
            let (min, max) = range.unwrap_or((0, 100));
            (Model::Int { min, max }, default.or(Some(ParamValue::Int(min))))
        }
        Dsl::Bool => (Model::Bool, default.or(Some(ParamValue::Bool(false)))),
        Dsl::Color => (
            Model::Color,
            default.or(Some(ParamValue::Color(crate::model::color::Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }))),
        ),
        Dsl::Gradient => (
            Model::ColorGradient {
                min_stops: 2,
                max_stops: 8,
            },
            default,
        ),
        Dsl::Curve => (Model::Curve, default),
        Dsl::Named(type_name) => {
            // Check enums first, then flags
            for e in &compiled.enums {
                if e.name == *type_name {
                    return (
                        Model::Enum {
                            options: e.variants.clone(),
                        },
                        default.or_else(|| {
                            e.variants.first().map(|v| ParamValue::EnumVariant(v.clone()))
                        }),
                    );
                }
            }
            for f in &compiled.flags {
                if f.name == *type_name {
                    return (
                        Model::Flags {
                            options: f.flags.clone(),
                        },
                        default.or(Some(ParamValue::FlagSet(vec![]))),
                    );
                }
            }
            // Fallback
            (Model::Bool, None)
        }
    }
}
