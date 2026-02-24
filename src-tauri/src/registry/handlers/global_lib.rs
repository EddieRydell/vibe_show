#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use crate::error::AppError;
use crate::setup;
use crate::registry::params::{
    NameParams, RenameParams, SetGlobalCurveParams, SetGlobalGradientParams, WriteScriptParams,
};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::{get_data_dir, AppState};

/// Typed return for ListGlobalLibrary.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct GlobalLibrarySummary {
    pub gradients: Vec<String>,
    pub curves: Vec<String>,
    pub scripts: Vec<String>,
}

// ── Helpers ──────────────────────────────────────────────────────

/// Persist global libraries to disk (best-effort).
pub(crate) fn persist_inner(state: &Arc<AppState>) {
    if let Ok(data_dir) = get_data_dir(state) {
        let libs = state.global_libraries.lock();
        let _ = setup::save_global_libraries(&data_dir, &libs);
    }
}

// ── Gradients ────────────────────────────────────────────────────

pub fn list_global_gradients(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let gradients: Vec<_> = libs.gradients.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Ok(CommandOutput::new(
        format!("{} gradients.", gradients.len()),
        CommandResult::ListGlobalGradients(gradients),
    ))
}

pub fn set_global_gradient(
    state: &Arc<AppState>,
    p: SetGlobalGradientParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().gradients.insert(p.name.clone(), p.gradient);
    persist_inner(state);
    Ok(CommandOutput::new(
        format!("Gradient \"{}\" saved.", p.name),
        CommandResult::SetGlobalGradient,
    ))
}

pub fn delete_global_gradient(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().gradients.remove(&p.name);
    persist_inner(state);
    Ok(CommandOutput::new(
        format!("Gradient \"{}\" deleted.", p.name),
        CommandResult::DeleteGlobalGradient,
    ))
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
    Ok(CommandOutput::new(
        format!("Gradient renamed to \"{}\".", p.new_name),
        CommandResult::RenameGlobalGradient,
    ))
}

// ── Curves ───────────────────────────────────────────────────────

pub fn list_global_curves(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let curves: Vec<_> = libs.curves.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Ok(CommandOutput::new(
        format!("{} curves.", curves.len()),
        CommandResult::ListGlobalCurves(curves),
    ))
}

pub fn set_global_curve(
    state: &Arc<AppState>,
    p: SetGlobalCurveParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().curves.insert(p.name.clone(), p.curve);
    persist_inner(state);
    Ok(CommandOutput::new(
        format!("Curve \"{}\" saved.", p.name),
        CommandResult::SetGlobalCurve,
    ))
}

pub fn delete_global_curve(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().curves.remove(&p.name);
    persist_inner(state);
    Ok(CommandOutput::new(
        format!("Curve \"{}\" deleted.", p.name),
        CommandResult::DeleteGlobalCurve,
    ))
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
    Ok(CommandOutput::new(
        format!("Curve renamed to \"{}\".", p.new_name),
        CommandResult::RenameGlobalCurve,
    ))
}

// ── Scripts ──────────────────────────────────────────────────────

pub fn set_global_script(
    state: &Arc<AppState>,
    p: WriteScriptParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().scripts.insert(p.name.clone(), p.source);
    persist_inner(state);
    Ok(CommandOutput::new(
        format!("Script \"{}\" saved.", p.name),
        CommandResult::SetGlobalScript,
    ))
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
    let msg = if result.success { "Compiled and saved." } else { "Compile failed." };
    Ok(CommandOutput::new(msg, CommandResult::CompileGlobalScript(result)))
}

/// List all gradients, curves, and scripts in the global library.
pub fn list_global_library(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let summary = GlobalLibrarySummary {
        gradients: libs.gradients.keys().cloned().collect(),
        curves: libs.curves.keys().cloned().collect(),
        scripts: libs.scripts.keys().cloned().collect(),
    };
    Ok(CommandOutput::new(
        serde_json::to_string(&summary).unwrap_or_default(),
        CommandResult::ListGlobalLibrary(summary),
    ))
}
