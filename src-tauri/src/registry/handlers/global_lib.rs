#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::profile;
use crate::registry::params::{
    NameParams, RenameParams, SetGlobalCurveParams, SetGlobalGradientParams, WriteScriptParams,
};
use crate::registry::CommandOutput;
use crate::state::{get_data_dir, AppState};

// ── Helpers ──────────────────────────────────────────────────────

/// Persist global libraries to disk (best-effort).
pub(crate) fn persist_inner(state: &Arc<AppState>) {
    if let Ok(data_dir) = get_data_dir(state) {
        let libs = state.global_libraries.lock();
        let _ = profile::save_global_libraries(&data_dir, &libs);
    }
}

// ── Gradients ────────────────────────────────────────────────────

pub fn list_global_gradients(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let gradients: Vec<_> = libs.gradients.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Ok(CommandOutput::json(
        format!("{} gradients.", gradients.len()),
        &gradients,
    ))
}

pub fn set_global_gradient(
    state: &Arc<AppState>,
    p: SetGlobalGradientParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().gradients.insert(p.name.clone(), p.gradient);
    persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Gradient \"{}\" saved.",
        p.name
    )))
}

pub fn delete_global_gradient(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().gradients.remove(&p.name);
    persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Gradient \"{}\" deleted.",
        p.name
    )))
}

pub fn rename_global_gradient(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    {
        let mut libs = state.global_libraries.lock();
        if let Some(g) = libs.gradients.remove(&p.old_name) {
            libs.gradients.insert(p.new_name.clone(), g);
        }
    }
    persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Gradient renamed to \"{}\".",
        p.new_name
    )))
}

// ── Curves ───────────────────────────────────────────────────────

pub fn list_global_curves(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let curves: Vec<_> = libs.curves.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Ok(CommandOutput::json(
        format!("{} curves.", curves.len()),
        &curves,
    ))
}

pub fn set_global_curve(
    state: &Arc<AppState>,
    p: SetGlobalCurveParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().curves.insert(p.name.clone(), p.curve);
    persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Curve \"{}\" saved.",
        p.name
    )))
}

pub fn delete_global_curve(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().curves.remove(&p.name);
    persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Curve \"{}\" deleted.",
        p.name
    )))
}

pub fn rename_global_curve(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    {
        let mut libs = state.global_libraries.lock();
        if let Some(c) = libs.curves.remove(&p.old_name) {
            libs.curves.insert(p.new_name.clone(), c);
        }
    }
    persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Curve renamed to \"{}\".",
        p.new_name
    )))
}

// ── Scripts ──────────────────────────────────────────────────────

pub fn set_global_script(
    state: &Arc<AppState>,
    p: WriteScriptParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().scripts.insert(p.name.clone(), p.source);
    persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Script \"{}\" saved.",
        p.name
    )))
}

pub fn compile_global_script(
    state: &Arc<AppState>,
    p: WriteScriptParams,
) -> Result<CommandOutput, AppError> {
    let result = super::common::compile_and_cache(state, p.name.clone(), &p.source);
    if result.success {
        state.global_libraries.lock().scripts.insert(p.name, p.source);
        persist_inner(state);
    }
    Ok(super::common::compile_result_output(&result))
}

/// List all gradients, curves, and scripts in the global library.
pub fn list_global_library(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let gradients: Vec<&String> = libs.gradients.keys().collect();
    let curves: Vec<&String> = libs.curves.keys().collect();
    let scripts: Vec<&String> = libs.scripts.keys().collect();
    let data = serde_json::json!({
        "gradients": gradients,
        "curves": curves,
        "scripts": scripts,
    });
    Ok(CommandOutput::data(
        serde_json::to_string(&data).unwrap_or_default(),
        data,
    ))
}
