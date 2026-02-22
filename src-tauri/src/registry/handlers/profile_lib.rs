#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::profile;
use crate::registry::params::{
    NameParams, RenameParams, SetProfileCurveParams, SetProfileGradientParams, WriteScriptParams,
};
use crate::registry::CommandOutput;
use crate::state::{get_data_dir, AppState};

// ── Gradients ────────────────────────────────────────────────────

pub fn list_profile_gradients(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
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
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    libs.gradients.insert(p.name.clone(), p.gradient);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
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
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    libs.gradients.remove(&p.name);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
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
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    if let Some(g) = libs.gradients.remove(&p.old_name) {
        libs.gradients.insert(p.new_name.clone(), g);
    }
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    Ok(CommandOutput::unit(format!(
        "Profile gradient renamed to \"{}\".",
        p.new_name
    )))
}

// ── Curves ───────────────────────────────────────────────────────

pub fn list_profile_curves(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
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
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    libs.curves.insert(p.name.clone(), p.curve);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
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
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    libs.curves.remove(&p.name);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
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
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    if let Some(c) = libs.curves.remove(&p.old_name) {
        libs.curves.insert(p.new_name.clone(), c);
    }
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
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
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
    let slug = state.require_profile()?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    libs.scripts.insert(p.name.clone(), p.source);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
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
    match crate::dsl::compile_source(&p.source) {
        Ok(compiled) => {
            let params = crate::commands::extract_script_params(&compiled);
            state
                .script_cache
                .lock()
                .insert(p.name.clone(), Arc::new(compiled));
            let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;
            let slug = state.require_profile()?;
            let mut libs =
                profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
                    message: e.to_string(),
                })?;
            libs.scripts.insert(p.name.clone(), p.source);
            profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
                message: e.to_string(),
            })?;
            let result = crate::commands::ScriptCompileResult {
                success: true,
                errors: vec![],
                name: p.name,
                params: Some(params),
            };
            Ok(CommandOutput::json("Compiled and saved.", &result))
        }
        Err(errors) => {
            let result = crate::commands::ScriptCompileResult {
                success: false,
                errors: errors
                    .iter()
                    .map(|e| crate::commands::ScriptError {
                        message: e.message.clone(),
                        offset: e.span.start,
                    })
                    .collect(),
                name: p.name,
                params: None,
            };
            Ok(CommandOutput::json("Compile failed.", &result))
        }
    }
}
