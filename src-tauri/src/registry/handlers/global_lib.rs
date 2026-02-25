#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::setup;
use crate::registry::params::{
    NameParams, RenameParams, SetGlobalCurveParams, SetGlobalGradientParams, WriteScriptParams,
};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::{get_data_dir, AppState};

// ── Helpers ──────────────────────────────────────────────────────

/// Persist global libraries to disk (best-effort).
pub(crate) fn persist_inner(state: &Arc<AppState>) {
    if let Ok(data_dir) = get_data_dir(state) {
        let libs = state.global_libraries.lock();
        let _ = setup::save_global_libraries(&data_dir, &libs);
    }
}

// ── Library CRUD macro ───────────────────────────────────────────

/// Generates set / delete / rename handlers for a global library collection.
/// Scripts keep manual handlers due to extra `script_cache` logic.
macro_rules! library_crud {
    (
        type_name: $type_name:literal,
        field: $field:ident,
        value_field: $vf:ident,
        set_fn: $set_fn:ident, set_params: $set_params:ty, set_result: $set_result:ident,
        delete_fn: $delete_fn:ident, delete_result: $delete_result:ident,
        rename_fn: $rename_fn:ident, rename_result: $rename_result:ident,
        list_fn: $list_fn:ident, list_result: $list_result:ident,
    ) => {
        pub fn $list_fn(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
            let libs = state.global_libraries.lock();
            let items: Vec<_> = libs.$field.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            Ok(CommandOutput::new(
                format!(concat!("{} ", $type_name, "s."), items.len()),
                CommandResult::$list_result(items),
            ))
        }

        pub fn $set_fn(
            state: &Arc<AppState>,
            p: $set_params,
        ) -> Result<CommandOutput, AppError> {
            state.global_libraries.lock().$field.insert(p.name.clone(), p.$vf);
            persist_inner(state);
            Ok(CommandOutput::new(
                format!(concat!($type_name, " \"{}\" saved."), p.name),
                CommandResult::$set_result,
            ))
        }

        pub fn $delete_fn(
            state: &Arc<AppState>,
            p: NameParams,
        ) -> Result<CommandOutput, AppError> {
            state.global_libraries.lock().$field.remove(&p.name);
            persist_inner(state);
            Ok(CommandOutput::new(
                format!(concat!($type_name, " \"{}\" deleted."), p.name),
                CommandResult::$delete_result,
            ))
        }

        pub fn $rename_fn(
            state: &Arc<AppState>,
            p: RenameParams,
        ) -> Result<CommandOutput, AppError> {
            {
                let mut libs = state.global_libraries.lock();
                if let Some(v) = libs.$field.remove(&p.old_name) {
                    libs.$field.insert(p.new_name.clone(), v);
                }
            }
            persist_inner(state);
            Ok(CommandOutput::new(
                format!(concat!($type_name, " renamed to \"{}\"."), p.new_name),
                CommandResult::$rename_result,
            ))
        }
    };
}

// ── Gradients ────────────────────────────────────────────────────

library_crud! {
    type_name: "Gradient",
    field: gradients,
    value_field: gradient,
    set_fn: set_global_gradient, set_params: SetGlobalGradientParams, set_result: SetGlobalGradient,
    delete_fn: delete_global_gradient, delete_result: DeleteGlobalGradient,
    rename_fn: rename_global_gradient, rename_result: RenameGlobalGradient,
    list_fn: list_global_gradients, list_result: ListGlobalGradients,
}

// ── Curves ───────────────────────────────────────────────────────

library_crud! {
    type_name: "Curve",
    field: curves,
    value_field: curve,
    set_fn: set_global_curve, set_params: SetGlobalCurveParams, set_result: SetGlobalCurve,
    delete_fn: delete_global_curve, delete_result: DeleteGlobalCurve,
    rename_fn: rename_global_curve, rename_result: RenameGlobalCurve,
    list_fn: list_global_curves, list_result: ListGlobalCurves,
}

// ── Scripts ──────────────────────────────────────────────────────

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
