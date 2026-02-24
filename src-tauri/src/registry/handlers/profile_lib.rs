#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::profile::{self, LibrariesFile};
use crate::registry::params::{
    NameParams, RenameParams, SetProfileCurveParams, SetProfileGradientParams, WriteScriptParams,
};
use crate::registry::CommandOutput;
use crate::state::{get_data_dir, AppState};

// ── Helpers ──────────────────────────────────────────────────────

/// Load the profile libraries (read-only).
pub(crate) fn read_libraries(state: &Arc<AppState>) -> Result<LibrariesFile, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })
}

/// Load profile libraries, apply a mutation, then save back.
pub(crate) fn with_libraries<F>(state: &Arc<AppState>, mutate: F) -> Result<(), AppError>
where
    F: FnOnce(&mut LibrariesFile),
{
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    mutate(&mut libs);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })
}

// ── Gradients ────────────────────────────────────────────────────

pub fn list_profile_gradients(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = read_libraries(state)?;
    let gradients: Vec<_> = libs.gradients.into_iter().collect();
    Ok(CommandOutput::json(
        format!("{} profile gradients.", gradients.len()),
        &gradients,
    ))
}

pub fn set_profile_gradient(
    state: &Arc<AppState>,
    p: SetProfileGradientParams,
) -> Result<CommandOutput, AppError> {
    with_libraries(state, |libs| {
        libs.gradients.insert(p.name.clone(), p.gradient);
    })?;
    Ok(CommandOutput::unit(format!(
        "Profile gradient \"{}\" saved.",
        p.name
    )))
}

pub fn delete_profile_gradient(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    with_libraries(state, |libs| {
        libs.gradients.remove(&p.name);
    })?;
    Ok(CommandOutput::unit(format!(
        "Profile gradient \"{}\" deleted.",
        p.name
    )))
}

pub fn rename_profile_gradient(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    with_libraries(state, |libs| {
        if let Some(g) = libs.gradients.remove(&p.old_name) {
            libs.gradients.insert(p.new_name.clone(), g);
        }
    })?;
    Ok(CommandOutput::unit(format!(
        "Profile gradient renamed to \"{}\".",
        p.new_name
    )))
}

// ── Curves ───────────────────────────────────────────────────────

pub fn list_profile_curves(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = read_libraries(state)?;
    let curves: Vec<_> = libs.curves.into_iter().collect();
    Ok(CommandOutput::json(
        format!("{} profile curves.", curves.len()),
        &curves,
    ))
}

pub fn set_profile_curve(
    state: &Arc<AppState>,
    p: SetProfileCurveParams,
) -> Result<CommandOutput, AppError> {
    with_libraries(state, |libs| {
        libs.curves.insert(p.name.clone(), p.curve);
    })?;
    Ok(CommandOutput::unit(format!(
        "Profile curve \"{}\" saved.",
        p.name
    )))
}

pub fn delete_profile_curve(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    with_libraries(state, |libs| {
        libs.curves.remove(&p.name);
    })?;
    Ok(CommandOutput::unit(format!(
        "Profile curve \"{}\" deleted.",
        p.name
    )))
}

pub fn rename_profile_curve(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    with_libraries(state, |libs| {
        if let Some(c) = libs.curves.remove(&p.old_name) {
            libs.curves.insert(p.new_name.clone(), c);
        }
    })?;
    Ok(CommandOutput::unit(format!(
        "Profile curve renamed to \"{}\".",
        p.new_name
    )))
}

// ── Scripts ──────────────────────────────────────────────────────

pub fn set_profile_script(
    state: &Arc<AppState>,
    p: WriteScriptParams,
) -> Result<CommandOutput, AppError> {
    with_libraries(state, |libs| {
        libs.scripts.insert(p.name.clone(), p.source);
    })?;
    Ok(CommandOutput::unit(format!(
        "Profile script \"{}\" saved.",
        p.name
    )))
}

pub fn compile_profile_script(
    state: &Arc<AppState>,
    p: WriteScriptParams,
) -> Result<CommandOutput, AppError> {
    let result = super::common::compile_and_cache(state, p.name.clone(), &p.source);
    if result.success {
        with_libraries(state, |libs| {
            libs.scripts.insert(p.name, p.source);
        })?;
    }
    Ok(super::common::compile_result_output(&result))
}
