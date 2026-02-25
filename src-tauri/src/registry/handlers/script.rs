#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::registry::params::{
    CompileScriptPreviewParams, NameParams, RenameParams, WriteScriptParams,
};
use crate::registry::reference;
use crate::registry::{CommandOutput, CommandResult};
use crate::state::AppState;

use super::global_lib;

pub fn get_dsl_reference(_state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let reference = reference::dsl_reference();
    Ok(CommandOutput::new(reference.clone(), CommandResult::GetDslReference(reference)))
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
                Ok(CommandOutput::new(
                    format!("Compiled \"{}\" (no params).", p.name),
                    CommandResult::WriteGlobalScript,
                ))
            } else {
                Ok(CommandOutput::new(
                    format!(
                        "Compiled \"{}\" with params: {}.",
                        p.name,
                        params_desc.join(", ")
                    ),
                    CommandResult::WriteGlobalScript,
                ))
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
    Ok(CommandOutput::new(source.clone(), CommandResult::GetGlobalScriptSource(source)))
}

pub fn delete_global_script(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    state.global_libraries.lock().scripts.remove(&p.name);
    state.script_cache.lock().remove(&p.name);
    global_lib::persist_inner(state);
    Ok(CommandOutput::new(
        format!("Script \"{}\" deleted.", p.name),
        CommandResult::DeleteGlobalScript,
    ))
}

pub fn list_global_scripts(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = state.global_libraries.lock();
    let pairs: Vec<(String, String)> = libs.scripts.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Ok(CommandOutput::new(
        serde_json::to_string(&pairs).unwrap_or_default(),
        CommandResult::ListGlobalScripts(pairs),
    ))
}

pub fn compile_script_preview(
    _state: &Arc<AppState>,
    p: CompileScriptPreviewParams,
) -> Result<CommandOutput, AppError> {
    let result = super::common::compile_preview(String::new(), &p.source);
    let msg = if result.success { "Compiled and saved." } else { "Compile failed." };
    Ok(CommandOutput::new(msg, CommandResult::CompileScriptPreview(result)))
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
    Ok(CommandOutput::new(
        format!("Script renamed to \"{}\".", p.new_name),
        CommandResult::RenameGlobalScript,
    ))
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
    Ok(CommandOutput::new(
        format!("{} params.", params.len()),
        CommandResult::GetScriptParams(params),
    ))
}
