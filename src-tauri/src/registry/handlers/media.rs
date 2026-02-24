#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::setup;
use crate::registry::params::{ImportMediaParams, NameParams};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::{get_data_dir, AppState};

pub fn list_media(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    let media = setup::list_media(&data_dir, &setup_slug).map_err(AppError::from)?;
    Ok(CommandOutput::new(format!("{} media files.", media.len()), CommandResult::ListMedia(media)))
}

pub fn import_media(
    state: &Arc<AppState>,
    p: ImportMediaParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    let media = setup::import_media(
        &data_dir,
        &setup_slug,
        std::path::Path::new(&p.source_path),
    )
    .map_err(AppError::from)?;
    let msg = format!("Imported \"{}\".", media.filename);
    Ok(CommandOutput::new(msg, CommandResult::ImportMedia(media)))
}

pub fn delete_media(state: &Arc<AppState>, p: NameParams) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    setup::delete_media(&data_dir, &setup_slug, &p.name).map_err(AppError::from)?;
    Ok(CommandOutput::new(
        format!("Media \"{}\" deleted.", p.name),
        CommandResult::DeleteMedia,
    ))
}

pub fn resolve_media_path(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    setup::validate_filename(&p.name).map_err(AppError::from)?;
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let setup_slug = state.require_setup()?;
    let path = crate::paths::media_dir(&data_dir, &setup_slug).join(&p.name);
    if !path.exists() {
        return Err(AppError::NotFound {
            what: format!("Media file: {}", p.name),
        });
    }
    let path_str = path.to_string_lossy().to_string();
    Ok(CommandOutput::new(path_str.clone(), CommandResult::ResolveMediaPath(path_str)))
}
