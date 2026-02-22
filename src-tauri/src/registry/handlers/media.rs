#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::profile;
use crate::registry::params::{ImportMediaParams, NameParams};
use crate::registry::CommandOutput;
use crate::state::{get_data_dir, AppState};

fn require_profile(state: &Arc<AppState>) -> Result<String, AppError> {
    state
        .current_profile
        .lock()
        .clone()
        .ok_or(AppError::NoProfile)
}

pub fn list_media(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;
    let media = profile::list_media(&data_dir, &profile_slug).map_err(AppError::from)?;
    Ok(CommandOutput::json(
        format!("{} media files.", media.len()),
        &media,
    ))
}

pub fn import_media(
    state: &Arc<AppState>,
    p: ImportMediaParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;
    let media = profile::import_media(
        &data_dir,
        &profile_slug,
        std::path::Path::new(&p.source_path),
    )
    .map_err(AppError::from)?;
    Ok(CommandOutput::json(
        format!("Imported \"{}\".", media.filename),
        &media,
    ))
}

pub fn delete_media(state: &Arc<AppState>, p: NameParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;
    profile::delete_media(&data_dir, &profile_slug, &p.name).map_err(AppError::from)?;
    Ok(CommandOutput::unit(format!(
        "Media \"{}\" deleted.",
        p.name
    )))
}

pub fn resolve_media_path(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let profile_slug = require_profile(state)?;
    let path = profile::media_dir(&data_dir, &profile_slug).join(&p.name);
    if !path.exists() {
        return Err(AppError::NotFound {
            what: format!("Media file: {}", p.name),
        });
    }
    let path_str = path.to_string_lossy().to_string();
    Ok(CommandOutput::data(
        path_str.clone(),
        serde_json::json!(path_str),
    ))
}
