#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::error::AppError;
use crate::model::analysis::AudioAnalysis;
use crate::registry::params::{GetAnalysisDetailParams, GetBeatsInRangeParams};
use crate::registry::{CommandOutput, CommandResult, JsonValue};
use crate::state::AppState;

use crate::setup;
use crate::state::get_data_dir;

/// Typed return for GetAnalysisSummary.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct AnalysisSummary {
    pub tempo: Option<f64>,
    pub time_signature: Option<u32>,
    pub beat_count: Option<usize>,
    pub key: Option<String>,
    pub key_confidence: Option<f64>,
    pub valence: Option<f64>,
    pub arousal: Option<f64>,
    pub danceability: Option<f64>,
    pub genres: Option<std::collections::HashMap<String, f64>>,
    pub sections: Option<Vec<SectionSummary>>,
}

/// Lightweight section info for the summary.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SectionSummary {
    pub label: String,
    pub start: f64,
    pub end: f64,
}

/// Typed return for GetBeatsInRange.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct BeatsInRange {
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
    pub count: usize,
    pub tempo: f64,
}

fn current_analysis(state: &Arc<AppState>) -> Option<AudioAnalysis> {
    let show = state.show.lock();
    let audio_file = show.sequences.first()?.audio_file.as_ref()?;
    let cache = state.analysis_cache.lock();
    cache.get(audio_file).cloned()
}

pub fn get_analysis_summary(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let analysis = current_analysis(state).ok_or(AppError::ValidationError {
        message: "No audio analysis available. Load a song and run analysis first.".into(),
    })?;

    let summary = AnalysisSummary {
        tempo: analysis.beats.as_ref().map(|b| b.tempo),
        time_signature: analysis.beats.as_ref().map(|b| b.time_signature),
        beat_count: analysis.beats.as_ref().map(|b| b.beats.len()),
        key: analysis.harmony.as_ref().map(|h| h.key.clone()),
        key_confidence: analysis.harmony.as_ref().map(|h| h.key_confidence),
        valence: analysis.mood.as_ref().map(|m| m.valence),
        arousal: analysis.mood.as_ref().map(|m| m.arousal),
        danceability: analysis.mood.as_ref().map(|m| m.danceability),
        genres: analysis.mood.as_ref().and_then(|m| {
            if m.genres.is_empty() { None } else { Some(m.genres.clone()) }
        }),
        sections: analysis.structure.as_ref().map(|s| {
            s.sections.iter().map(|sec| SectionSummary {
                label: sec.label.clone(),
                start: sec.start,
                end: sec.end,
            }).collect()
        }),
    };

    Ok(CommandOutput::new(
        serde_json::to_string_pretty(&summary).unwrap_or_default(),
        CommandResult::GetAnalysisSummary(summary),
    ))
}

pub fn get_beats_in_range(
    state: &Arc<AppState>,
    p: GetBeatsInRangeParams,
) -> Result<CommandOutput, AppError> {
    let analysis = current_analysis(state).ok_or(AppError::ValidationError {
        message: "No audio analysis available.".into(),
    })?;
    let beats = analysis
        .beats
        .as_ref()
        .ok_or(AppError::ValidationError {
            message: "No beat analysis available.".into(),
        })?;

    let filtered: Vec<f64> = beats
        .beats
        .iter()
        .copied()
        .filter(|&b| b >= p.start && b <= p.end)
        .collect();
    let downbeats: Vec<f64> = beats
        .downbeats
        .iter()
        .copied()
        .filter(|&b| b >= p.start && b <= p.end)
        .collect();

    let result = BeatsInRange {
        count: filtered.len(),
        beats: filtered,
        downbeats,
        tempo: beats.tempo,
    };
    Ok(CommandOutput::new(
        serde_json::to_string(&result).unwrap_or_default(),
        CommandResult::GetBeatsInRange(result),
    ))
}

pub fn get_sections(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let analysis = current_analysis(state).ok_or(AppError::ValidationError {
        message: "No audio analysis available.".into(),
    })?;
    let structure = analysis
        .structure
        .as_ref()
        .ok_or(AppError::ValidationError {
            message: "No structure analysis available.".into(),
        })?;
    Ok(CommandOutput::new(
        serde_json::to_string_pretty(&structure.sections).unwrap_or_default(),
        CommandResult::GetSections(structure.sections.clone()),
    ))
}

pub fn get_analysis_detail(
    state: &Arc<AppState>,
    p: GetAnalysisDetailParams,
) -> Result<CommandOutput, AppError> {
    let analysis = current_analysis(state).ok_or(AppError::ValidationError {
        message: "No audio analysis available.".into(),
    })?;

    let detail: Value = match p.feature.as_str() {
        "beats" => serde_json::to_value(&analysis.beats).unwrap_or(Value::Null),
        "structure" => serde_json::to_value(&analysis.structure).unwrap_or(Value::Null),
        "mood" => serde_json::to_value(&analysis.mood).unwrap_or(Value::Null),
        "harmony" => serde_json::to_value(&analysis.harmony).unwrap_or(Value::Null),
        "lyrics" => serde_json::to_value(&analysis.lyrics).unwrap_or(Value::Null),
        "pitch" => serde_json::to_value(&analysis.pitch).unwrap_or(Value::Null),
        "drums" => serde_json::to_value(&analysis.drums).unwrap_or(Value::Null),
        "vocal_presence" => serde_json::to_value(&analysis.vocal_presence).unwrap_or(Value::Null),
        "low_level" => serde_json::to_value(&analysis.low_level).unwrap_or(Value::Null),
        _ => {
            return Err(AppError::ValidationError {
                message: format!(
                    "Unknown feature: {}. Valid: beats, structure, mood, harmony, lyrics, pitch, drums, vocal_presence, low_level",
                    p.feature
                ),
            })
        }
    };

    if detail.is_null() {
        Ok(CommandOutput::new(
            format!("No {} analysis data available.", p.feature),
            CommandResult::GetAnalysisDetail(JsonValue(serde_json::Value::Null)),
        ))
    } else {
        Ok(CommandOutput::new(
            serde_json::to_string_pretty(&detail).unwrap_or_default(),
            CommandResult::GetAnalysisDetail(JsonValue(detail)),
        ))
    }
}

// ── Async handler ────────────────────────────────────────────────

#[cfg(feature = "tauri-app")]
pub async fn analyze_audio(
    state: Arc<AppState>,
    app: Option<tauri::AppHandle>,
    p: crate::registry::params::AnalyzeAudioParams,
) -> Result<CommandOutput, AppError> {
    let app_handle = app.ok_or_else(|| AppError::ApiError {
        message: "AppHandle required for analyze_audio".into(),
    })?;

    let audio_file = state
        .with_show(|show| {
            show.sequences
                .first()
                .and_then(|s| s.audio_file.clone())
        })
        .ok_or(AppError::AnalysisError {
            message: "No audio file in current sequence".into(),
        })?;

    crate::setup::validate_filename(&audio_file).map_err(|_| AppError::ValidationError {
        message: format!("Invalid audio filename: {audio_file}"),
    })?;

    let data_dir = get_data_dir(&state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    let media_dir = crate::paths::media_dir(&data_dir, &setup_slug);
    let audio_path = media_dir.join(&audio_file);

    if !audio_path.exists() {
        return Err(AppError::NotFound {
            what: format!("Audio file: {audio_file}"),
        });
    }

    let features = p.features.unwrap_or_else(|| {
        state
            .settings
            .lock()
            .as_ref()
            .and_then(|s| s.default_analysis_features.clone())
            .unwrap_or_default()
    });

    let use_gpu = state
        .settings
        .lock()
        .as_ref()
        .is_some_and(|s| s.use_gpu);

    let port = crate::python::ensure_sidecar(&state, &app_handle).await?;

    let cancel_flag = state.cancellation.register("analysis");
    let output_dir = crate::paths::stems_dir(&media_dir, &audio_file);
    let models = crate::paths::models_dir(&state.app_config_dir);

    let analysis_result = tokio::select! {
        result = crate::analysis::run_analysis(
            &app_handle,
            port,
            &audio_path,
            &output_dir,
            &features,
            &models,
            use_gpu,
        ) => {
            state.cancellation.unregister("analysis");
            result?
        }
        () = crate::state::wait_for_cancel(&cancel_flag) => {
            state.cancellation.unregister("analysis");
            crate::progress::emit_progress(&app_handle, "analysis", "Cancelled", 1.0, None);
            return Err(AppError::Cancelled { operation: "analysis".into() });
        }
    };

    let cache_path = crate::paths::analysis_path(&media_dir, &audio_file);
    if let Err(e) = crate::analysis::save_analysis(&cache_path, &analysis_result) {
        eprintln!("[VibeLights] Failed to save analysis cache: {e}");
    }

    state.cache_analysis(audio_file, analysis_result.clone());

    Ok(CommandOutput::new(
        "Audio analysis complete.",
        CommandResult::AnalyzeAudio(Box::new(analysis_result)),
    ))
}

pub fn get_analysis(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let audio_file = state.with_show(|show| {
        show.sequences
            .first()
            .and_then(|s| s.audio_file.clone())
    });

    let Some(audio_file) = audio_file else {
        return Ok(CommandOutput::new("No audio file.", CommandResult::GetAnalysis(None)));
    };

    // Validate audio filename to prevent path traversal
    if let Err(e) = setup::validate_filename(&audio_file) {
        return Err(AppError::ValidationError {
            message: format!("Invalid audio filename: {e}"),
        });
    }

    // Resolve the full path to the audio file and verify it exists
    let Ok(data_dir) = get_data_dir(state) else {
        return Ok(CommandOutput::new("No data dir.", CommandResult::GetAnalysis(None)));
    };
    let Some(setup_slug) = state.current_setup.lock().clone() else {
        return Ok(CommandOutput::new("No setup.", CommandResult::GetAnalysis(None)));
    };
    let media_dir = crate::paths::media_dir(&data_dir, &setup_slug);

    let audio_path = media_dir.join(&audio_file);
    if !audio_path.exists() {
        return Err(AppError::ValidationError {
            message: format!("Audio file '{audio_file}' not found in media directory"),
        });
    }

    // Check memory cache
    if let Some(cached) = state.analysis_cache.lock().get(&audio_file) {
        return Ok(CommandOutput::new("Analysis from cache.", CommandResult::GetAnalysis(Some(Box::new(cached.clone())))));
    }

    // Check disk cache
    let path = crate::paths::analysis_path(&media_dir, &audio_file);

    if path.exists() {
        if let Ok(loaded) = crate::analysis::load_analysis(&path) {
            state.cache_analysis(audio_file, loaded.clone());
            return Ok(CommandOutput::new("Analysis loaded from disk.", CommandResult::GetAnalysis(Some(Box::new(loaded)))));
        }
    }

    Ok(CommandOutput::new("No analysis available.", CommandResult::GetAnalysis(None)))
}
