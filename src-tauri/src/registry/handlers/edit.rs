#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::dispatcher::EditCommand;
use crate::error::AppError;
use crate::model::{EffectTarget, FixtureId};
use crate::registry::params::{
    AddEffectParams, AddTrackParams, BatchAction, BatchEditParams, DeleteEffectsParams,
    DeleteTrackParams, MoveEffectToTrackParams, UpdateEffectParamParams,
    UpdateEffectTimeRangeParams, UpdateSequenceSettingsParams,
};
use crate::registry::validation::{validate_opacity, validate_positive_finite, validate_time_range};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::AppState;

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
    let index = match result {
        crate::dispatcher::CommandResult::Index(i) => i,
        _ => 0,
    };
    let track_name = show
        .sequences
        .get(seq_idx)
        .and_then(|s| s.tracks.get(p.track_index))
        .map_or("unknown", |t| t.name.as_str());
    Ok(CommandOutput::new(
        format!(
            "Added {:?} effect to track {} (\"{}\") at {:.1}s-{:.1}s (index {index}).",
            p.kind, p.track_index, track_name, p.start, p.end
        ),
        CommandResult::AddEffect(index),
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
    Ok(CommandOutput::new(format!("Deleted {n} effect(s)."), CommandResult::DeleteEffects))
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
    Ok(CommandOutput::new(format!("Updated param \"{key_str}\"."), CommandResult::UpdateEffectParam))
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
    Ok(CommandOutput::new(
        format!("Updated time range to {:.1}s-{:.1}s.", p.start, p.end),
        CommandResult::UpdateEffectTimeRange,
    ))
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
    let index = match result {
        crate::dispatcher::CommandResult::Index(i) => i,
        _ => 0,
    };
    Ok(CommandOutput::new(
        format!(
            "Created track \"{}\" targeting {} (index {index}).",
            p.name, fixture_name
        ),
        CommandResult::AddTrack(index),
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
    Ok(CommandOutput::new(
        format!("Deleted track {}.", p.track_index),
        CommandResult::DeleteTrack,
    ))
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
    let index = match result {
        crate::dispatcher::CommandResult::Index(i) => i,
        _ => 0,
    };
    Ok(CommandOutput::new(
        format!(
            "Moved effect from track {} to track {} (index {index}).",
            p.from_track, p.to_track
        ),
        CommandResult::MoveEffectToTrack(index),
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
    Ok(CommandOutput::new("Updated sequence settings.", CommandResult::UpdateSequenceSettings))
}

pub fn batch_edit(state: &Arc<AppState>, p: BatchEditParams) -> Result<CommandOutput, AppError> {
    // Resolve active sequence index up front so batch commands target the right sequence.
    let seq_idx = {
        let show = state.show.lock();
        state.active_sequence_index(&show)?
    };

    let mut edit_commands = Vec::new();
    for (i, action) in p.commands.into_iter().enumerate() {
        // Pre-process WriteScript: compile + cache before converting to edit commands
        if let BatchAction::WriteScript(ref ws) = action {
            match crate::dsl::compile_source(&ws.source) {
                Ok(compiled) => {
                    state
                        .script_cache
                        .lock()
                        .insert(ws.name.clone(), Arc::new(compiled));
                }
                Err(errors) => {
                    let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
                    return Err(AppError::ValidationError {
                        message: format!(
                            "Command {i} (write_script \"{}\"): compile errors: {}",
                            ws.name,
                            msgs.join("; ")
                        ),
                    });
                }
            }
        }

        if let Some(cmd) = action.into_edit_command(seq_idx).map_err(|e| {
            AppError::ValidationError {
                message: format!("Command {i}: {e}"),
            }
        })? {
            edit_commands.push(cmd);
        }
    }

    let n = edit_commands.len();
    let description = if p.description.is_empty() {
        "Batch edit".to_string()
    } else {
        p.description
    };
    let batch = EditCommand::Batch {
        description: description.clone(),
        commands: edit_commands,
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &batch)?;
    Ok(CommandOutput::new(
        format!("Executed {n} operations as single undoable batch: \"{description}\"."),
        CommandResult::BatchEdit,
    ))
}
