#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;
use std::time::Instant;

use crate::error::AppError;
use crate::registry::params::{SeekParams, SetLoopingParams, SetRegionParams};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::{AppState, PlaybackInfo};

pub fn play(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let mut playback = state.playback.lock();
    playback.playing = true;
    playback.last_tick = Some(Instant::now());
    Ok(CommandOutput::new("Playing.", CommandResult::Play))
}

pub fn pause(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let mut playback = state.playback.lock();
    playback.playing = false;
    playback.last_tick = None;
    Ok(CommandOutput::new("Paused.", CommandResult::Pause))
}

pub fn seek(state: &Arc<AppState>, p: SeekParams) -> Result<CommandOutput, AppError> {
    if !p.time.is_finite() {
        return Err(AppError::ValidationError {
            message: "Seek time must be a finite number.".to_string(),
        });
    }

    // Read sequence duration from show first (lock ordering: show before playback).
    let duration = {
        let show = state.show.lock();
        let playback = state.playback.lock();
        show.sequences
            .get(playback.sequence_index)
            .map_or(0.0, |s| s.duration)
    };

    let mut playback = state.playback.lock();
    playback.current_time = p.time.clamp(0.0, duration);
    if playback.playing {
        playback.last_tick = Some(Instant::now());
    } else {
        playback.last_tick = None;
    }
    Ok(CommandOutput::new(
        format!("Seeked to {:.1}s.", playback.current_time),
        CommandResult::Seek,
    ))
}

pub fn undo(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let desc = dispatcher.undo(&mut show)?;
    Ok(CommandOutput::new(format!("Undone: {desc}"), CommandResult::Undo))
}

pub fn redo(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let desc = dispatcher.redo(&mut show)?;
    Ok(CommandOutput::new(format!("Redone: {desc}"), CommandResult::Redo))
}

pub fn get_playback(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let playback = state.playback.lock();
    let show = state.show.lock();
    let duration = show
        .sequences
        .get(playback.sequence_index)
        .map_or(0.0, |s| s.duration);
    let info = PlaybackInfo {
        playing: playback.playing,
        current_time: playback.current_time,
        duration,
        sequence_index: playback.sequence_index,
        region: playback.region,
        looping: playback.looping,
    };
    Ok(CommandOutput::new("Playback state", CommandResult::GetPlayback(info)))
}

pub fn set_region(state: &Arc<AppState>, p: SetRegionParams) -> Result<CommandOutput, AppError> {
    state.with_playback_mut(|playback| {
        playback.region = p.region.map(|r| (r.start, r.end));
    });
    Ok(CommandOutput::new("Region updated.", CommandResult::SetRegion))
}

pub fn set_looping(
    state: &Arc<AppState>,
    p: SetLoopingParams,
) -> Result<CommandOutput, AppError> {
    state.with_playback_mut(|playback| {
        playback.looping = p.looping;
    });
    Ok(CommandOutput::new(
        if p.looping {
            "Looping enabled."
        } else {
            "Looping disabled."
        },
        CommandResult::SetLooping,
    ))
}

pub fn get_undo_state(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let undo_state = state.with_dispatcher(crate::dispatcher::CommandDispatcher::undo_state);
    Ok(CommandOutput::new("Undo state", CommandResult::GetUndoState(undo_state)))
}
