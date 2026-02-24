#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::registry::params::{
    CompileScriptParams, CompileScriptPreviewParams, NameParams, RenameParams, WriteScriptParams,
};
use crate::registry::reference;
use crate::registry::CommandOutput;
use crate::state::AppState;

use super::global_lib;

pub fn get_dsl_reference() -> Result<CommandOutput, AppError> {
    let reference = reference::dsl_reference();
    Ok(CommandOutput::data(
        reference.clone(),
        serde_json::json!(reference),
    ))
}

pub fn write_global_script(
    state: &Arc<AppState>,
    p: WriteScriptParams,
) -> Result<CommandOutput, AppError> {
    match crate::dsl::compile_source(&p.source) {
        Ok(compiled) => {
            let params_desc: Vec<String> = compiled
                .params
                .iter()
                .map(|param| format!("{} ({:?})", param.name, param.ty))
                .collect();
            state
                .script_cache
                .lock()
                .insert(p.name.clone(), Arc::new(compiled));
            state.global_libraries.lock().scripts.insert(p.name.clone(), p.source);
            global_lib::persist_inner(state);
            if params_desc.is_empty() {
                Ok(CommandOutput::unit(format!(
                    "Compiled \"{}\" (no params).",
                    p.name
                )))
            } else {
                Ok(CommandOutput::unit(format!(
                    "Compiled \"{}\" with params: {}.",
                    p.name,
                    params_desc.join(", ")
                )))
            }
        }
        Err(errors) => {
            let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            Err(AppError::ValidationError {
                message: format!("Compile errors:\n{}", msgs.join("\n")),
            })
        }
    }
}

pub fn get_global_script_source(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let source = libs
        .scripts
        .get(&p.name)
        .cloned()
        .ok_or_else(|| AppError::NotFound {
            what: format!("Script \"{}\"", p.name),
        })?;
    Ok(CommandOutput::data(
        source.clone(),
        serde_json::json!(source),
    ))
}

pub fn delete_global_script(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().scripts.remove(&p.name);
    state.script_cache.lock().remove(&p.name);
    global_lib::persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Script \"{}\" deleted.",
        p.name
    )))
}

pub fn list_global_scripts(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let names: Vec<&String> = libs.scripts.keys().collect();
    Ok(CommandOutput::json(
        serde_json::to_string(&names).unwrap_or_default(),
        &names,
    ))
}

pub fn compile_global_script(
    state: &Arc<AppState>,
    p: CompileScriptParams,
) -> Result<CommandOutput, AppError> {
    let result = super::common::compile_and_cache(state, p.name.clone(), &p.source);
    if result.success {
        state.global_libraries.lock().scripts.insert(p.name, p.source);
        global_lib::persist_inner(state);
    }
    Ok(super::common::compile_result_output(&result))
}

pub fn compile_script_preview(
    p: CompileScriptPreviewParams,
) -> Result<CommandOutput, AppError> {
    let result = super::common::compile_preview(String::new(), &p.source);
    Ok(super::common::compile_result_output(&result))
}

pub fn rename_global_script(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    {
        let mut libs = state.global_libraries.lock();
        if let Some(source) = libs.scripts.remove(&p.old_name) {
            libs.scripts.insert(p.new_name.clone(), source);
        }
    }
    {
        let mut cache = state.script_cache.lock();
        if let Some(compiled) = cache.remove(&p.old_name) {
            cache.insert(p.new_name.clone(), compiled);
        }
    }
    global_lib::persist_inner(state);
    Ok(CommandOutput::unit(format!(
        "Script renamed to \"{}\".",
        p.new_name
    )))
}

pub fn get_script_params(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let cache = state.script_cache.lock();
    let compiled = cache.get(&p.name).ok_or_else(|| AppError::ApiError {
        message: format!("Script '{}' not found in cache", p.name),
    })?;
    let params = crate::commands::extract_script_params(compiled);
    Ok(CommandOutput::json(
        format!("{} params.", params.len()),
        &params,
    ))
}
