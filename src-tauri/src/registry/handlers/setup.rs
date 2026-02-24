#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::setup;
use crate::registry::params::{
    CreateSetupParams, SlugParams, UpdateSetupFixturesParams, UpdateSetupLayoutParams,
    UpdateSetupOutputsParams,
};
use crate::registry::{CommandOutput, CommandResult};
use crate::settings;
use crate::state::{get_data_dir, AppState};

pub fn list_setups(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setups = setup::list_setups(&data_dir).map_err(AppError::from)?;
    let current = state.current_setup.lock().clone();
    let mut lines = vec![format!("{} setups:", setups.len())];
    for p in &setups {
        let marker = if current.as_deref() == Some(&p.slug) { " (current)" } else { "" };
        lines.push(format!(
            "  - \"{}\" (slug: {}, {} fixtures, {} sequences){marker}",
            p.name, p.slug, p.fixture_count, p.sequence_count,
        ));
    }
    Ok(CommandOutput::new(lines.join("\n"), CommandResult::ListSetups(setups)))
}

pub fn create_setup(
    state: &Arc<AppState>,
    p: CreateSetupParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let summary = setup::create_setup(&data_dir, &p.name).map_err(AppError::from)?;
    Ok(CommandOutput::new(
        format!("Setup \"{}\" created.", summary.name),
        CommandResult::CreateSetup(summary),
    ))
}

pub fn open_setup(state: &Arc<AppState>, p: SlugParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let loaded = setup::load_setup(&data_dir, &p.slug).map_err(AppError::from)?;
    *state.current_setup.lock() = Some(p.slug.clone());
    *state.current_sequence.lock() = None;

    // Clear script cache when switching setups
    state.script_cache.lock().clear();

    // Update last_setup in settings
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.last_setup = Some(p.slug);
        if let Err(e) = settings::save_settings(&state.app_config_dir, s) {
            eprintln!("[VibeLights] Failed to save settings: {e}");
        }
    }

    let msg = format!("Setup \"{}\" opened.", loaded.name);
    Ok(CommandOutput::new(msg, CommandResult::OpenSetup(Box::new(loaded))))
}

pub fn delete_setup(state: &Arc<AppState>, p: SlugParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    setup::delete_setup(&data_dir, &p.slug).map_err(AppError::from)?;

    let mut current = state.current_setup.lock();
    if current.as_deref() == Some(&p.slug) {
        *current = None;
        *state.current_sequence.lock() = None;
    }

    Ok(CommandOutput::new(
        format!("Setup \"{}\" deleted.", p.slug),
        CommandResult::DeleteSetup,
    ))
}

pub fn save_setup(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_setup()?;
    let loaded = setup::load_setup(&data_dir, &slug).map_err(AppError::from)?;
    setup::save_setup(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::new("Setup saved.", CommandResult::SaveSetup))
}

pub fn update_setup_fixtures(
    state: &Arc<AppState>,
    p: UpdateSetupFixturesParams,
) -> Result<CommandOutput, AppError> {
    use std::collections::HashSet;
    use crate::model::{FixtureId, GroupId, GroupMember};

    // Validate group members reference valid fixtures and groups
    let fixture_ids: HashSet<FixtureId> = p.fixtures.iter().map(|f| f.id).collect();
    let group_ids: HashSet<GroupId> = p.groups.iter().map(|g| g.id).collect();
    for group in &p.groups {
        for member in &group.members {
            match member {
                GroupMember::Fixture(fid) => {
                    if !fixture_ids.contains(fid) {
                        return Err(AppError::ValidationError {
                            message: format!(
                                "Group \"{}\" references fixture {} which does not exist.",
                                group.name, fid.0
                            ),
                        });
                    }
                }
                GroupMember::Group(gid) => {
                    if !group_ids.contains(gid) {
                        return Err(AppError::ValidationError {
                            message: format!(
                                "Group \"{}\" references group {} which does not exist.",
                                group.name, gid.0
                            ),
                        });
                    }
                }
            }
        }
    }

    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_setup()?;
    let mut loaded = setup::load_setup(&data_dir, &slug).map_err(AppError::from)?;
    loaded.fixtures = p.fixtures;
    loaded.groups = p.groups;
    setup::save_setup(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::new("Setup fixtures updated.", CommandResult::UpdateSetupFixtures))
}

pub fn update_setup_outputs(
    state: &Arc<AppState>,
    p: UpdateSetupOutputsParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_setup()?;
    let mut loaded = setup::load_setup(&data_dir, &slug).map_err(AppError::from)?;
    loaded.controllers = p.controllers;
    loaded.patches = p.patches;
    setup::save_setup(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::new("Setup outputs updated.", CommandResult::UpdateSetupOutputs))
}

pub fn update_setup_layout(
    state: &Arc<AppState>,
    p: UpdateSetupLayoutParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_setup()?;
    let mut loaded = setup::load_setup(&data_dir, &slug).map_err(AppError::from)?;
    loaded.layout = p.layout;
    setup::save_setup(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::new("Setup layout updated.", CommandResult::UpdateSetupLayout))
}
