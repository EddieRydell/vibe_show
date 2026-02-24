#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::dispatcher::EditCommand;
use crate::error::AppError;
use crate::model::{ColorGradient, Curve, EffectTarget, FixtureId};
use crate::registry::params::{
    AddEffectParams, AddTrackParams, BatchEditParams, DeleteEffectsParams, DeleteTrackParams,
    MoveEffectToTrackParams, UpdateEffectParamParams, UpdateEffectTimeRangeParams,
    UpdateSequenceSettingsParams,
};
use crate::registry::CommandOutput;
use crate::state::AppState;

// ── Validation helpers ──────────────────────────────────────────

fn validate_time_range(start: f64, end: f64) -> Result<(), AppError> {
    if !start.is_finite() || !end.is_finite() {
        return Err(AppError::ValidationError {
            message: "Time values must be finite".to_string(),
        });
    }
    if start < 0.0 {
        return Err(AppError::ValidationError {
            message: "Start time must be non-negative".to_string(),
        });
    }
    if start >= end {
        return Err(AppError::ValidationError {
            message: format!("Start ({start:.3}) must be less than end ({end:.3})"),
        });
    }
    Ok(())
}

fn validate_opacity(opacity: f64) -> Result<(), AppError> {
    if !opacity.is_finite() {
        return Err(AppError::ValidationError {
            message: "Opacity must be finite".to_string(),
        });
    }
    if !(0.0..=1.0).contains(&opacity) {
        return Err(AppError::ValidationError {
            message: format!("Opacity ({opacity:.3}) must be between 0.0 and 1.0"),
        });
    }
    Ok(())
}

fn validate_positive_finite(value: f64, name: &str) -> Result<(), AppError> {
    if !value.is_finite() {
        return Err(AppError::ValidationError {
            message: format!("{name} must be finite"),
        });
    }
    if value <= 0.0 {
        return Err(AppError::ValidationError {
            message: format!("{name} must be positive"),
        });
    }
    Ok(())
}

// ── String-returning variants for batch_edit context ────────────

fn validate_time_range_str(start: f64, end: f64) -> Result<(), String> {
    validate_time_range(start, end).map_err(|e| e.to_string())
}

fn validate_opacity_str(opacity: f64) -> Result<(), String> {
    validate_opacity(opacity).map_err(|e| e.to_string())
}

// ── Handlers ────────────────────────────────────────────────────

pub fn add_effect(state: &Arc<AppState>, p: AddEffectParams) -> Result<CommandOutput, AppError> {
    validate_time_range(p.start, p.end)?;
    validate_opacity(p.opacity)?;

    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::AddEffect {
        sequence_index: seq_idx,
        track_index: p.track_index,
        kind: p.kind.clone(),
        start: p.start,
        end: p.end,
        blend_mode: p.blend_mode,
        opacity: p.opacity,
    };
    let result = dispatcher.execute(&mut show, &cmd)?;
    let track_name = show
        .sequences
        .get(seq_idx)
        .and_then(|s| s.tracks.get(p.track_index))
        .map_or("unknown", |t| t.name.as_str());
    Ok(CommandOutput::data(
        format!(
            "Added {:?} effect to track {} (\"{}\") at {:.1}s-{:.1}s. Result: {:?}.",
            p.kind, p.track_index, track_name, p.start, p.end, result
        ),
        serde_json::json!({ "result": format!("{result:?}") }),
    ))
}

pub fn delete_effects(
    state: &Arc<AppState>,
    p: DeleteEffectsParams,
) -> Result<CommandOutput, AppError> {
    let n = p.targets.len();
    let targets = p
        .targets
        .into_iter()
        .map(|t| (t.track_index, t.effect_index))
        .collect();
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::DeleteEffects {
        sequence_index: seq_idx,
        targets,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!("Deleted {n} effect(s).")))
}

pub fn update_effect_param(
    state: &Arc<AppState>,
    p: UpdateEffectParamParams,
) -> Result<CommandOutput, AppError> {
    let key_str = format!("{:?}", p.key);
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::UpdateEffectParam {
        sequence_index: seq_idx,
        track_index: p.track_index,
        effect_index: p.effect_index,
        key: p.key,
        value: p.value,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!("Updated param \"{key_str}\".")))
}

pub fn update_effect_time_range(
    state: &Arc<AppState>,
    p: UpdateEffectTimeRangeParams,
) -> Result<CommandOutput, AppError> {
    validate_time_range(p.start, p.end)?;

    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::UpdateEffectTimeRange {
        sequence_index: seq_idx,
        track_index: p.track_index,
        effect_index: p.effect_index,
        start: p.start,
        end: p.end,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!(
        "Updated time range to {:.1}s-{:.1}s.",
        p.start, p.end
    )))
}

pub fn add_track(state: &Arc<AppState>, p: AddTrackParams) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let fixture = show
        .fixtures
        .iter()
        .find(|f| f.id.0 == p.fixture_id)
        .ok_or_else(|| AppError::ValidationError {
            message: format!(
                "Fixture with ID {} does not exist",
                p.fixture_id
            ),
        })?;
    let fixture_name = fixture.name.clone();
    let cmd = EditCommand::AddTrack {
        sequence_index: seq_idx,
        name: p.name.clone(),
        target: EffectTarget::Fixtures(vec![FixtureId(p.fixture_id)]),
    };
    let result = dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::data(
        format!(
            "Created track \"{}\" targeting {}. Result: {:?}.",
            p.name, fixture_name, result
        ),
        serde_json::json!({ "result": format!("{result:?}") }),
    ))
}

pub fn delete_track(
    state: &Arc<AppState>,
    p: DeleteTrackParams,
) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::DeleteTrack {
        sequence_index: seq_idx,
        track_index: p.track_index,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!(
        "Deleted track {}.",
        p.track_index
    )))
}

pub fn move_effect_to_track(
    state: &Arc<AppState>,
    p: MoveEffectToTrackParams,
) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::MoveEffectToTrack {
        sequence_index: seq_idx,
        from_track: p.from_track,
        effect_index: p.effect_index,
        to_track: p.to_track,
    };
    let result = dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::data(
        format!(
            "Moved effect from track {} to track {}. Result: {:?}.",
            p.from_track, p.to_track, result
        ),
        serde_json::json!({ "result": format!("{result:?}") }),
    ))
}

pub fn update_sequence_settings(
    state: &Arc<AppState>,
    p: UpdateSequenceSettingsParams,
) -> Result<CommandOutput, AppError> {
    if let Some(duration) = p.duration {
        validate_positive_finite(duration, "Duration")?;
    }
    if let Some(frame_rate) = p.frame_rate {
        validate_positive_finite(frame_rate, "Frame rate")?;
    }

    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::UpdateSequenceSettings {
        sequence_index: seq_idx,
        name: p.name,
        audio_file: p.audio_file,
        duration: p.duration,
        frame_rate: p.frame_rate,
    };
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit("Updated sequence settings."))
}

#[allow(clippy::cast_possible_truncation)]
pub fn batch_edit(state: &Arc<AppState>, p: BatchEditParams) -> Result<CommandOutput, AppError> {
    // Resolve active sequence index up front so batch commands target the right sequence.
    let seq_idx = {
        let show = state.show.lock();
        state.active_sequence_index(&show)?
    };

    let mut edit_commands = Vec::new();
    for (i, cmd_val) in p.commands.iter().enumerate() {
        let action = cmd_val
            .get("action")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AppError::ValidationError {
                message: format!("Command {i}: missing action"),
            })?;
        let params = cmd_val
            .get("params")
            .unwrap_or(&serde_json::Value::Null);

        // Special handling for write_script: compile first
        if action == "write_script" {
            let source = params
                .get("source")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| AppError::ValidationError {
                    message: format!("Command {i}: missing source"),
                })?;
            let script_name = params
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| AppError::ValidationError {
                    message: format!("Command {i}: missing name"),
                })?;
            match crate::dsl::compile_source(source) {
                Ok(compiled) => {
                    state
                        .script_cache
                        .lock()
                        .insert(script_name.to_string(), Arc::new(compiled));
                }
                Err(errors) => {
                    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
                    return Err(AppError::ValidationError {
                        message: format!(
                            "Command {i} (write_script \"{script_name}\"): compile errors: {}",
                            msgs.join("; ")
                        ),
                    });
                }
            }
        }

        let cmd =
            parse_batch_command(action, params, seq_idx).map_err(|e| AppError::ValidationError {
                message: format!("Command {i} ({action}): {e}"),
            })?;
        edit_commands.push(cmd);
    }

    let n = edit_commands.len();
    let description = if p.description.is_empty() {
        "Batch edit".to_string()
    } else {
        p.description.clone()
    };
    let batch = EditCommand::Batch {
        description: description.clone(),
        commands: edit_commands,
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &batch)?;
    Ok(CommandOutput::unit(format!(
        "Executed {n} operations as single undoable batch: \"{description}\"."
    )))
}

/// Parse a single command from a batch_edit entry into an EditCommand.
/// Reuses the same logic as chat.rs::parse_batch_command.
#[allow(clippy::cast_possible_truncation)]
fn parse_batch_command(
    action: &str,
    params: &serde_json::Value,
    sequence_index: usize,
) -> Result<EditCommand, String> {
    use serde_json::Value;
    match action {
        "add_effect" => {
            let blend_mode = params
                .get("blend_mode")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(crate::model::BlendMode::Override);
            let opacity = params
                .get("opacity")
                .and_then(Value::as_f64)
                .unwrap_or(1.0);
            let start = params
                .get("start")
                .and_then(Value::as_f64)
                .ok_or("Missing start")?;
            let end = params
                .get("end")
                .and_then(Value::as_f64)
                .ok_or("Missing end")?;
            validate_time_range_str(start, end)?;
            validate_opacity_str(opacity)?;
            Ok(EditCommand::AddEffect {
                sequence_index,
                track_index: params
                    .get("track_index")
                    .and_then(Value::as_u64)
                    .ok_or("Missing track_index")? as usize,
                kind: serde_json::from_value(
                    params.get("kind").cloned().unwrap_or(Value::Null),
                )
                .map_err(|e| e.to_string())?,
                start,
                end,
                blend_mode,
                opacity,
            })
        }
        "delete_effects" => {
            let targets: Vec<(usize, usize)> = params
                .get("targets")
                .and_then(Value::as_array)
                .ok_or("Missing targets")?
                .iter()
                .map(|pair| {
                    let arr = pair.as_array().ok_or("Invalid target pair")?;
                    Ok((
                        arr.first()
                            .and_then(Value::as_u64)
                            .ok_or("Invalid track index")? as usize,
                        arr.get(1)
                            .and_then(Value::as_u64)
                            .ok_or("Invalid effect index")? as usize,
                    ))
                })
                .collect::<Result<_, String>>()?;
            Ok(EditCommand::DeleteEffects {
                sequence_index,
                targets,
            })
        }
        "update_effect_param" => Ok(EditCommand::UpdateEffectParam {
            sequence_index,
            track_index: params
                .get("track_index")
                .and_then(Value::as_u64)
                .ok_or("Missing track_index")? as usize,
            effect_index: params
                .get("effect_index")
                .and_then(Value::as_u64)
                .ok_or("Missing effect_index")? as usize,
            key: serde_json::from_value(params.get("key").cloned().unwrap_or(Value::Null))
                .map_err(|e| format!("Invalid param key: {e}"))?,
            value: serde_json::from_value(params.get("value").cloned().unwrap_or(Value::Null))
                .map_err(|e| e.to_string())?,
        }),
        "update_effect_time_range" => {
            let start = params
                .get("start")
                .and_then(Value::as_f64)
                .ok_or("Missing start")?;
            let end = params
                .get("end")
                .and_then(Value::as_f64)
                .ok_or("Missing end")?;
            validate_time_range_str(start, end)?;
            Ok(EditCommand::UpdateEffectTimeRange {
                sequence_index,
                track_index: params
                    .get("track_index")
                    .and_then(Value::as_u64)
                    .ok_or("Missing track_index")? as usize,
                effect_index: params
                    .get("effect_index")
                    .and_then(Value::as_u64)
                    .ok_or("Missing effect_index")? as usize,
                start,
                end,
            })
        }
        "add_track" => {
            let fixture_id = params
                .get("fixture_id")
                .and_then(Value::as_u64)
                .ok_or("Missing fixture_id")? as u32;
            Ok(EditCommand::AddTrack {
                sequence_index,
                name: params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("Missing name")?
                    .to_string(),
                target: crate::model::EffectTarget::Fixtures(vec![crate::model::FixtureId(
                    fixture_id,
                )]),
            })
        }
        "delete_track" => Ok(EditCommand::DeleteTrack {
            sequence_index,
            track_index: params
                .get("track_index")
                .and_then(Value::as_u64)
                .ok_or("Missing track_index")? as usize,
        }),
        "set_library_gradient" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Missing name")?
                .to_string();
            let stops: Vec<crate::model::ColorStop> = serde_json::from_value(
                params.get("stops").cloned().unwrap_or(Value::Null),
            )
            .map_err(|e| format!("Invalid stops: {e}"))?;
            let gradient = ColorGradient::new(stops).ok_or("Gradient needs at least 2 stops")?;
            Ok(EditCommand::SetGradient {
                sequence_index,
                name,
                gradient,
            })
        }
        "set_library_curve" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Missing name")?
                .to_string();
            let points: Vec<crate::model::CurvePoint> = serde_json::from_value(
                params.get("points").cloned().unwrap_or(Value::Null),
            )
            .map_err(|e| format!("Invalid points: {e}"))?;
            let curve = Curve::new(points).ok_or("Curve needs at least 2 points")?;
            Ok(EditCommand::SetCurve {
                sequence_index,
                name,
                curve,
            })
        }
        "delete_library_gradient" => Ok(EditCommand::DeleteGradient {
            sequence_index,
            name: params
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Missing name")?
                .to_string(),
        }),
        "delete_library_curve" => Ok(EditCommand::DeleteCurve {
            sequence_index,
            name: params
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Missing name")?
                .to_string(),
        }),
        "write_script" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Missing name")?
                .to_string();
            let source = params
                .get("source")
                .and_then(Value::as_str)
                .ok_or("Missing source")?
                .to_string();
            Ok(EditCommand::SetScript {
                sequence_index,
                name,
                source,
            })
        }
        "delete_script" => Ok(EditCommand::DeleteScript {
            sequence_index,
            name: params
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Missing name")?
                .to_string(),
        }),
        _ => Err(format!("Unsupported batch action: {action}")),
    }
}
