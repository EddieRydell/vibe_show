#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::profile;
use crate::registry::params::{
    CreateProfileParams, SlugParams, UpdateProfileFixturesParams, UpdateProfileLayoutParams,
    UpdateProfileSetupParams,
};
use crate::registry::CommandOutput;
use crate::settings;
use crate::state::{get_data_dir, AppState};

pub fn list_profiles(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profiles = profile::list_profiles(&data_dir).map_err(AppError::from)?;
    let current = state.current_profile.lock().clone();
    let mut lines = vec![format!("{} profiles:", profiles.len())];
    for p in &profiles {
        let marker = if current.as_deref() == Some(&p.slug) { " (current)" } else { "" };
        lines.push(format!(
            "  - \"{}\" (slug: {}, {} fixtures, {} sequences){marker}",
            p.name, p.slug, p.fixture_count, p.sequence_count,
        ));
    }
    Ok(CommandOutput::data(lines.join("\n"), serde_json::to_value(&profiles).unwrap_or_default()))
}

pub fn create_profile(
    state: &Arc<AppState>,
    p: CreateProfileParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let summary = profile::create_profile(&data_dir, &p.name).map_err(AppError::from)?;
    Ok(CommandOutput::json(
        format!("Profile \"{}\" created.", summary.name),
        &summary,
    ))
}

pub fn open_profile(state: &Arc<AppState>, p: SlugParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let loaded = profile::load_profile(&data_dir, &p.slug).map_err(AppError::from)?;
    *state.current_profile.lock() = Some(p.slug.clone());
    *state.current_sequence.lock() = None;

    // Clear script cache when switching profiles
    state.script_cache.lock().clear();

    // Update last_profile in settings
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.last_profile = Some(p.slug);
        if let Err(e) = settings::save_settings(&state.app_config_dir, s) {
            eprintln!("[VibeLights] Failed to save settings: {e}");
        }
    }

    Ok(CommandOutput::json(
        format!("Profile \"{}\" opened.", loaded.name),
        &loaded,
    ))
}

pub fn delete_profile(state: &Arc<AppState>, p: SlugParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    profile::delete_profile(&data_dir, &p.slug).map_err(AppError::from)?;

    let mut current = state.current_profile.lock();
    if current.as_deref() == Some(&p.slug) {
        *current = None;
        *state.current_sequence.lock() = None;
    }

    Ok(CommandOutput::unit(format!(
        "Profile \"{}\" deleted.",
        p.slug
    )))
}

pub fn save_profile(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::unit("Profile saved."))
}

pub fn update_profile_fixtures(
    state: &Arc<AppState>,
    p: UpdateProfileFixturesParams,
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
    let slug = state.require_profile()?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    loaded.fixtures = p.fixtures;
    loaded.groups = p.groups;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::unit("Profile fixtures updated."))
}

pub fn update_profile_setup(
    state: &Arc<AppState>,
    p: UpdateProfileSetupParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    loaded.controllers = p.controllers;
    loaded.patches = p.patches;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::unit("Profile setup updated."))
}

pub fn update_profile_layout(
    state: &Arc<AppState>,
    p: UpdateProfileLayoutParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    loaded.layout = p.layout;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::unit("Profile layout updated."))
}
