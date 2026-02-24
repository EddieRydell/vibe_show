#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::setup;
use crate::registry::params::{CreateSequenceParams, SlugParams};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::{get_data_dir, AppState};
use crate::commands;

pub fn list_sequences(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    let sequences =
        setup::list_sequences(&data_dir, &setup_slug).map_err(AppError::from)?;
    let current = state.current_sequence.lock().clone();
    let mut lines = vec![format!(
        "{} sequences in setup \"{}\":",
        sequences.len(),
        setup_slug
    )];
    for s in &sequences {
        let marker = if current.as_deref() == Some(&s.slug) { " (current)" } else { "" };
        lines.push(format!("  - \"{}\" (slug: {}){marker}", s.name, s.slug));
    }
    Ok(CommandOutput::new(lines.join("\n"), CommandResult::ListSequences(sequences)))
}

pub fn create_sequence(
    state: &Arc<AppState>,
    p: CreateSequenceParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    let summary =
        setup::create_sequence(&data_dir, &setup_slug, &p.name).map_err(AppError::from)?;
    Ok(CommandOutput::new(format!("Sequence \"{}\" created.", summary.name), CommandResult::CreateSequence(summary)))
}

pub fn open_sequence(state: &Arc<AppState>, p: SlugParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;

    let setup_data =
        setup::load_setup(&data_dir, &setup_slug).map_err(AppError::from)?;
    let sequence =
        setup::load_sequence(&data_dir, &setup_slug, &p.slug).map_err(AppError::from)?;
    let assembled = setup::assemble_show(&setup_data, &sequence);

    *state.show.lock() = assembled.clone();
    state.dispatcher.lock().clear();
    state.script_cache.lock().clear();

    state.with_playback_mut(|playback| {
        playback.playing = false;
        playback.current_time = 0.0;
        playback.sequence_index = 0;
        playback.region = None;
        playback.looping = false;
    });

    *state.current_sequence.lock() = Some(p.slug.clone());

    commands::recompile_all_scripts(state);

    let track_count = assembled.sequences.first().map_or(0, |s| s.tracks.len());
    let effect_count: usize = assembled
        .sequences
        .first()
        .map_or(0, |s| s.tracks.iter().map(|t| t.effects.len()).sum());

    Ok(CommandOutput::new(format!(
        "Opened sequence \"{}\". {} tracks, {} effects.",
        p.slug, track_count, effect_count
    ), CommandResult::OpenSequence(Box::new(assembled))))
}

pub fn delete_sequence(state: &Arc<AppState>, p: SlugParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    setup::delete_sequence(&data_dir, &setup_slug, &p.slug).map_err(AppError::from)?;

    let mut current = state.current_sequence.lock();
    if current.as_deref() == Some(&p.slug) {
        *current = None;
        // Clear script cache since the deleted sequence may have had scripts
        state.script_cache.lock().clear();
    }

    Ok(CommandOutput::new(format!(
        "Sequence \"{}\" deleted.",
        p.slug
    ), CommandResult::DeleteSequence))
}

pub fn save_current_sequence(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    let seq_slug = state.require_sequence()?;

    let show = state.show.lock();
    let sequence = show.sequences.first().ok_or(AppError::NotFound {
        what: "sequence in show".into(),
    })?;
    setup::save_sequence(&data_dir, &setup_slug, &seq_slug, sequence)
        .map_err(AppError::from)?;
    Ok(CommandOutput::new("Sequence saved.", CommandResult::SaveCurrentSequence))
}
