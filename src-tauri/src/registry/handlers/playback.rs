#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;
use std::time::Instant;

use crate::error::AppError;
use crate::registry::params::{SeekParams, SetLoopingParams, SetRegionParams};
use crate::registry::CommandOutput;
use crate::state::{AppState, PlaybackInfo};

pub fn play(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let mut playback = state.playback.lock();
    playback.playing = true;
    playback.last_tick = Some(Instant::now());
    Ok(CommandOutput::unit("Playing."))
}

pub fn pause(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let mut playback = state.playback.lock();
    playback.playing = false;
    playback.last_tick = None;
    Ok(CommandOutput::unit("Paused."))
}

pub fn seek(state: &Arc<AppState>, p: SeekParams) -> Result<CommandOutput, AppError> {
    let mut playback = state.playback.lock();
    playback.current_time = p.time.max(0.0);
    if playback.playing {
        playback.last_tick = Some(Instant::now());
    } else {
        playback.last_tick = None;
    }
    Ok(CommandOutput::unit(format!("Seeked to {:.1}s.", p.time)))
}

pub fn undo(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let desc = dispatcher.undo(&mut show)?;
    Ok(CommandOutput::unit(format!("Undone: {desc}")))
}

pub fn redo(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let desc = dispatcher.redo(&mut show)?;
    Ok(CommandOutput::unit(format!("Redone: {desc}")))
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
    Ok(CommandOutput::json("Playback state", &info))
}

pub fn set_region(state: &Arc<AppState>, p: SetRegionParams) -> Result<CommandOutput, AppError> {
    state.with_playback_mut(|playback| {
        playback.region = p.region.map(|r| (r.start, r.end));
    });
    Ok(CommandOutput::unit("Region updated."))
}

pub fn set_looping(
    state: &Arc<AppState>,
    p: SetLoopingParams,
) -> Result<CommandOutput, AppError> {
    state.with_playback_mut(|playback| {
        playback.looping = p.looping;
    });
    Ok(CommandOutput::unit(if p.looping {
        "Looping enabled."
    } else {
        "Looping disabled."
    }))
}

pub fn get_undo_state(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let undo_state = state.with_dispatcher(crate::dispatcher::CommandDispatcher::undo_state);
    Ok(CommandOutput::json("Undo state", &undo_state))
}
