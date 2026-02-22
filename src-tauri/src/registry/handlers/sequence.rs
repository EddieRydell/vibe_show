#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::profile;
use crate::registry::params::{CreateSequenceParams, SlugParams};
use crate::registry::CommandOutput;
use crate::state::{get_data_dir, AppState};
use crate::commands;

fn require_profile(state: &Arc<AppState>) -> Result<String, AppError> {
    state
        .current_profile
        .lock()
        .clone()
        .ok_or(AppError::NoProfile)
}

fn require_sequence(state: &Arc<AppState>) -> Result<String, AppError> {
    state
        .current_sequence
        .lock()
        .clone()
        .ok_or(AppError::NoSequence)
}

pub fn list_sequences(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;
    let sequences =
        profile::list_sequences(&data_dir, &profile_slug).map_err(AppError::from)?;
    let current = state.current_sequence.lock().clone();
    let mut lines = vec![format!(
        "{} sequences in profile \"{}\":",
        sequences.len(),
        profile_slug
    )];
    for s in &sequences {
        let marker = if current.as_deref() == Some(&s.slug) { " (current)" } else { "" };
        lines.push(format!("  - \"{}\" (slug: {}){marker}", s.name, s.slug));
    }
    Ok(CommandOutput::data(
        lines.join("\n"),
        serde_json::to_value(&sequences).unwrap_or_default(),
    ))
}

pub fn create_sequence(
    state: &Arc<AppState>,
    p: CreateSequenceParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;
    let summary =
        profile::create_sequence(&data_dir, &profile_slug, &p.name).map_err(AppError::from)?;
    Ok(CommandOutput::json(
        format!("Sequence \"{}\" created.", summary.name),
        &summary,
    ))
}

pub fn open_sequence(state: &Arc<AppState>, p: SlugParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;

    let profile_data =
        profile::load_profile(&data_dir, &profile_slug).map_err(AppError::from)?;
    let sequence =
        profile::load_sequence(&data_dir, &profile_slug, &p.slug).map_err(AppError::from)?;
    let assembled = profile::assemble_show(&profile_data, &sequence);

    *state.show.lock() = assembled;
    state.dispatcher.lock().clear();

    state.with_playback_mut(|playback| {
        playback.playing = false;
        playback.current_time = 0.0;
        playback.sequence_index = 0;
        playback.region = None;
        playback.looping = false;
    });

    *state.current_sequence.lock() = Some(p.slug.clone());

    commands::recompile_all_scripts(state);

    let show = state.show.lock();
    let track_count = show.sequences.first().map_or(0, |s| s.tracks.len());
    let effect_count: usize = show
        .sequences
        .first()
        .map_or(0, |s| s.tracks.iter().map(|t| t.effects.len()).sum());

    Ok(CommandOutput::unit(format!(
        "Opened sequence \"{}\". {} tracks, {} effects.",
        p.slug, track_count, effect_count
    )))
}

pub fn delete_sequence(state: &Arc<AppState>, p: SlugParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;
    profile::delete_sequence(&data_dir, &profile_slug, &p.slug).map_err(AppError::from)?;

    let mut current = state.current_sequence.lock();
    if current.as_deref() == Some(&p.slug) {
        *current = None;
    }

    Ok(CommandOutput::unit(format!(
        "Sequence \"{}\" deleted.",
        p.slug
    )))
}

pub fn save_current_sequence(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;
    let seq_slug = require_sequence(state)?;

    let show = state.show.lock();
    let sequence = show.sequences.first().ok_or(AppError::NotFound {
        what: "sequence in show".into(),
    })?;
    profile::save_sequence(&data_dir, &profile_slug, &seq_slug, sequence)
        .map_err(AppError::from)?;
    Ok(CommandOutput::unit("Sequence saved."))
}
