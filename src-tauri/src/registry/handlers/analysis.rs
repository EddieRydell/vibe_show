#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::Value;

use crate::error::AppError;
use crate::model::analysis::AudioAnalysis;
use crate::registry::params::{GetAnalysisDetailParams, GetBeatsInRangeParams};
use crate::registry::CommandOutput;
use crate::state::AppState;

use crate::profile;
use crate::state::get_data_dir;

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

    let mut summary = serde_json::Map::new();

    if let Some(ref beats) = analysis.beats {
        summary.insert("tempo".to_string(), serde_json::json!(beats.tempo));
        summary.insert(
            "time_signature".to_string(),
            serde_json::json!(beats.time_signature),
        );
        summary.insert(
            "beat_count".to_string(),
            serde_json::json!(beats.beats.len()),
        );
    }
    if let Some(ref harmony) = analysis.harmony {
        summary.insert("key".to_string(), serde_json::json!(harmony.key));
        summary.insert(
            "key_confidence".to_string(),
            serde_json::json!(harmony.key_confidence),
        );
    }
    if let Some(ref mood) = analysis.mood {
        summary.insert("valence".to_string(), serde_json::json!(mood.valence));
        summary.insert("arousal".to_string(), serde_json::json!(mood.arousal));
        summary.insert(
            "danceability".to_string(),
            serde_json::json!(mood.danceability),
        );
        if !mood.genres.is_empty() {
            summary.insert("genres".to_string(), serde_json::json!(mood.genres));
        }
    }
    if let Some(ref structure) = analysis.structure {
        let sections: Vec<Value> = structure
            .sections
            .iter()
            .map(|s| {
                serde_json::json!({
                    "label": s.label,
                    "start": s.start,
                    "end": s.end,
                })
            })
            .collect();
        summary.insert("sections".to_string(), Value::Array(sections));
    }

    let data = Value::Object(summary);
    Ok(CommandOutput::data(
        serde_json::to_string_pretty(&data).unwrap_or_default(),
        data,
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

    let data = serde_json::json!({
        "beats": filtered,
        "downbeats": downbeats,
        "count": filtered.len(),
        "tempo": beats.tempo,
    });
    Ok(CommandOutput::data(
        serde_json::to_string(&data).unwrap_or_default(),
        data,
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
    Ok(CommandOutput::json(
        serde_json::to_string_pretty(&structure.sections).unwrap_or_default(),
        &structure.sections,
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
        Ok(CommandOutput::unit(format!(
            "No {} analysis data available.",
            p.feature
        )))
    } else {
        Ok(CommandOutput::data(
            serde_json::to_string_pretty(&detail).unwrap_or_default(),
            detail,
        ))
    }
}

pub fn get_analysis(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let audio_file = state.with_show(|show| {
        show.sequences
            .first()
            .and_then(|s| s.audio_file.clone())
    });

    let Some(audio_file) = audio_file else {
        return Ok(CommandOutput::json("No audio file.", &Option::<AudioAnalysis>::None));
    };

    // Check memory cache
    if let Some(cached) = state.analysis_cache.lock().get(&audio_file) {
        return Ok(CommandOutput::json("Analysis from cache.", &Some(cached.clone())));
    }

    // Check disk cache
    let Ok(data_dir) = get_data_dir(state) else {
        return Ok(CommandOutput::json("No data dir.", &Option::<AudioAnalysis>::None));
    };
    let Some(profile_slug) = state.current_profile.lock().clone() else {
        return Ok(CommandOutput::json("No profile.", &Option::<AudioAnalysis>::None));
    };
    let media_dir = profile::media_dir(&data_dir, &profile_slug);
    let path = crate::analysis::analysis_path(&media_dir, &audio_file);

    if path.exists() {
        if let Ok(loaded) = crate::analysis::load_analysis(&path) {
            state
                .analysis_cache
                .lock()
                .insert(audio_file, loaded.clone());
            return Ok(CommandOutput::json("Analysis loaded from disk.", &Some(loaded)));
        }
    }

    Ok(CommandOutput::json("No analysis available.", &Option::<AudioAnalysis>::None))
}
