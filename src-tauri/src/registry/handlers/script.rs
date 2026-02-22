#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::dispatcher::EditCommand;
use crate::error::AppError;
use crate::profile;
use crate::registry::params::{
    CompileScriptParams, CompileScriptPreviewParams, NameParams, RenameParams, WriteScriptParams,
};
use crate::registry::reference;
use crate::registry::CommandOutput;
use crate::state::{get_data_dir, AppState};

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
            let cmd = EditCommand::SetScript {
                sequence_index: 0,
                name: p.name.clone(),
                source: p.source,
            };
            let mut dispatcher = state.dispatcher.lock();
            let mut show = state.show.lock();
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
    let cmd = EditCommand::DeleteScript {
        sequence_index: 0,
        name: p.name.clone(),
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
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
            let data_dir = get_data_dir(state)?;
            let slug = state
                .current_profile
                .lock()
                .clone()
                .ok_or(AppError::NoProfile)?;
            let mut libs = profile::load_libraries(&data_dir, &slug)
                .map_err(|e| AppError::IoError {
                    message: e.to_string(),
                })?;
            libs.scripts.insert(p.name.clone(), p.source);
            profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
                message: e.to_string(),
            })?;
            if params_desc.is_empty() {
                Ok(CommandOutput::unit(format!(
                    "Compiled and saved \"{}\" to profile library (no params).",
                    p.name
                )))
            } else {
                Ok(CommandOutput::unit(format!(
                    "Compiled and saved \"{}\" to profile library with params: {}.",
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

pub fn list_profile_scripts(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state)?;
    let slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or(AppError::NoProfile)?;
    let libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
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
    let data_dir = get_data_dir(state)?;
    let slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or(AppError::NoProfile)?;
    let libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
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
    let data_dir = get_data_dir(state)?;
    let slug = state
        .current_profile
        .lock()
        .clone()
        .ok_or(AppError::NoProfile)?;
    let mut libs = profile::load_libraries(&data_dir, &slug).map_err(|e| AppError::IoError {
        message: e.to_string(),
    })?;
    libs.scripts.remove(&p.name);
    profile::save_libraries(&data_dir, &slug, &libs).map_err(|e| AppError::IoError {
        message: e.to_string(),
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
    match crate::dsl::compile_source(&p.source) {
        Ok(compiled) => {
            let params = crate::commands::extract_script_params(&compiled);
            state
                .script_cache
                .lock()
                .insert(p.name.clone(), Arc::new(compiled));
            let cmd = EditCommand::SetScript {
                sequence_index: 0,
                name: p.name.clone(),
                source: p.source,
            };
            let mut dispatcher = state.dispatcher.lock();
            let mut show = state.show.lock();
            dispatcher.execute(&mut show, &cmd)?;
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

pub fn compile_script_preview(
    p: CompileScriptPreviewParams,
) -> Result<CommandOutput, AppError> {
    match crate::dsl::compile_source(&p.source) {
        Ok(compiled) => {
            let params = crate::commands::extract_script_params(&compiled);
            let result = crate::commands::ScriptCompileResult {
                success: true,
                errors: vec![],
                name: compiled.name.clone(),
                params: Some(params),
            };
            Ok(CommandOutput::json("Compile successful.", &result))
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
                name: String::new(),
                params: None,
            };
            Ok(CommandOutput::json("Compile failed.", &result))
        }
    }
}

pub fn rename_script(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    let cmd = EditCommand::RenameScript {
        sequence_index: 0,
        old_name: p.old_name.clone(),
        new_name: p.new_name.clone(),
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
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
