#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::dispatcher::EditCommand;
use crate::error::AppError;
use crate::registry::params::{
    CompileScriptParams, CompileScriptPreviewParams, NameParams, RenameParams, WriteScriptParams,
};
use crate::registry::reference;
use crate::registry::CommandOutput;
use crate::state::AppState;

use super::common;
use super::profile_lib;

pub fn get_dsl_reference() -> Result<CommandOutput, AppError> {
    let reference = reference::dsl_reference();
    Ok(CommandOutput::data(
        reference.clone(),
        serde_json::json!(reference),
    ))
}

pub fn write_script(
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
            let mut dispatcher = state.dispatcher.lock();
            let mut show = state.show.lock();
            let seq_idx = state.active_sequence_index(&show)?;
            let cmd = EditCommand::SetScript {
                sequence_index: seq_idx,
                name: p.name.clone(),
                source: p.source,
            };
            dispatcher.execute(&mut show, &cmd)?;
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

pub fn get_script_source(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let source = show
        .sequences
        .first()
        .and_then(|seq| seq.scripts.get(&p.name))
        .cloned()
        .ok_or_else(|| AppError::NotFound {
            what: format!("Script \"{}\"", p.name),
        })?;
    Ok(CommandOutput::data(
        source.clone(),
        serde_json::json!(source),
    ))
}

pub fn delete_script(state: &Arc<AppState>, p: NameParams) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::DeleteScript {
        sequence_index: seq_idx,
        name: p.name.clone(),
    };
    dispatcher.execute(&mut show, &cmd)?;
    // Also remove from cache
    state.script_cache.lock().remove(&p.name);
    Ok(CommandOutput::unit(format!(
        "Script \"{}\" deleted.",
        p.name
    )))
}

pub fn list_scripts(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let names: Vec<&String> = show
        .sequences
        .first()
        .map(|seq| seq.scripts.keys().collect())
        .unwrap_or_default();
    Ok(CommandOutput::json(
        serde_json::to_string(&names).unwrap_or_default(),
        &names,
    ))
}

pub fn write_profile_script(
    state: &Arc<AppState>,
    p: WriteScriptParams,
) -> Result<CommandOutput, AppError> {
    let result = common::compile_and_cache(state, p.name.clone(), &p.source);
    if result.success {
        profile_lib::with_libraries(state, |libs| {
            libs.scripts.insert(p.name, p.source);
        })?;
        Ok(common::compile_result_output(&result))
    } else {
        Ok(common::compile_result_output(&result))
    }
}

pub fn list_profile_scripts(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let libs = profile_lib::read_libraries(state)?;
    let scripts: Vec<(String, String)> = libs.scripts.into_iter().collect();
    Ok(CommandOutput::json(
        format!("{} scripts.", scripts.len()),
        &scripts,
    ))
}

pub fn get_profile_script_source(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let libs = profile_lib::read_libraries(state)?;
    let source = libs
        .scripts
        .get(&p.name)
        .cloned()
        .ok_or_else(|| AppError::NotFound {
            what: format!("Script \"{}\" in profile library", p.name),
        })?;
    Ok(CommandOutput::data(
        source.clone(),
        serde_json::json!(source),
    ))
}

pub fn delete_profile_script(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    profile_lib::with_libraries(state, |libs| {
        libs.scripts.remove(&p.name);
    })?;
    state.script_cache.lock().remove(&p.name);
    Ok(CommandOutput::unit(format!(
        "Script \"{}\" deleted from profile library.",
        p.name
    )))
}

pub fn compile_script(
    state: &Arc<AppState>,
    p: CompileScriptParams,
) -> Result<CommandOutput, AppError> {
    let result = common::compile_and_cache(state, p.name.clone(), &p.source);
    if result.success {
        let mut dispatcher = state.dispatcher.lock();
        let mut show = state.show.lock();
        let seq_idx = state.active_sequence_index(&show)?;
        let cmd = EditCommand::SetScript {
            sequence_index: seq_idx,
            name: p.name,
            source: p.source,
        };
        dispatcher.execute(&mut show, &cmd)?;
    }
    Ok(common::compile_result_output(&result))
}

pub fn compile_script_preview(
    p: CompileScriptPreviewParams,
) -> Result<CommandOutput, AppError> {
    let result = common::compile_preview(String::new(), &p.source);
    Ok(common::compile_result_output(&result))
}

pub fn rename_script(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    let cmd = EditCommand::RenameScript {
        sequence_index: seq_idx,
        old_name: p.old_name.clone(),
        new_name: p.new_name.clone(),
    };
    dispatcher.execute(&mut show, &cmd)?;
    drop(show);
    drop(dispatcher);
    let mut cache = state.script_cache.lock();
    if let Some(compiled) = cache.remove(&p.old_name) {
        cache.insert(p.new_name.clone(), compiled);
    }
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
