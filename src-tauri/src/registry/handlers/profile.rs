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

fn require_profile(state: &Arc<AppState>) -> Result<String, AppError> {
    state
        .current_profile
        .lock()
        .clone()
        .ok_or(AppError::NoProfile)
}

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

    // Load persisted chat history for this profile
    crate::chat::load_chat_history(state);
    crate::chat::load_agent_chats(state);

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
    let slug = require_profile(state)?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::unit("Profile saved."))
}

pub fn update_profile_fixtures(
    state: &Arc<AppState>,
    p: UpdateProfileFixturesParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = require_profile(state)?;
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
    let slug = require_profile(state)?;
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
    let slug = require_profile(state)?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(AppError::from)?;
    loaded.layout = p.layout;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(AppError::from)?;
    Ok(CommandOutput::unit("Profile layout updated."))
}
