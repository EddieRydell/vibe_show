#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::commands::{self, ScriptCompileResult, ScriptError};
use crate::error::AppError;
use crate::registry::params::CancelOperationParams;
use crate::registry::{CommandOutput, CommandResult};
use crate::state::AppState;

pub fn cancel_operation(
    state: &Arc<AppState>,
    p: CancelOperationParams,
) -> Result<CommandOutput, AppError> {
    let cancelled = state.cancellation.cancel(&p.operation);
    Ok(CommandOutput::new(
        if cancelled {
            format!("Cancelled operation '{}'.", p.operation)
        } else {
            format!("No active operation '{}' to cancel.", p.operation)
        },
        CommandResult::CancelOperation(cancelled),
    ))
}

/// Compile a script source, cache the result, and return a `ScriptCompileResult`.
///
/// On success the compiled script is inserted into `state.script_cache`.
/// On failure the cache is not modified.
pub fn compile_and_cache(
    state: &Arc<AppState>,
    name: String,
    source: &str,
) -> ScriptCompileResult {
    match crate::dsl::compile_source(source) {
        Ok(compiled) => {
            let params = commands::extract_script_params(&compiled);
            state
                .script_cache
                .lock()
                .insert(name.clone(), Arc::new(compiled));
            ScriptCompileResult {
                success: true,
                errors: vec![],
                name,
                params: Some(params),
            }
        }
        Err(errors) => ScriptCompileResult {
            success: false,
            errors: errors
                .iter()
                .map(|e| ScriptError {
                    message: e.message.clone(),
                    offset: e.span.start,
                })
                .collect(),
            name,
            params: None,
        },
    }
}

/// Compile a script source for preview only (no caching, no persistence).
pub fn compile_preview(name: String, source: &str) -> ScriptCompileResult {
    match crate::dsl::compile_source(source) {
        Ok(compiled) => {
            let params = commands::extract_script_params(&compiled);
            ScriptCompileResult {
                success: true,
                errors: vec![],
                name: if name.is_empty() {
                    compiled.name.clone()
                } else {
                    name
                },
                params: Some(params),
            }
        }
        Err(errors) => ScriptCompileResult {
            success: false,
            errors: errors
                .iter()
                .map(|e| ScriptError {
                    message: e.message.clone(),
                    offset: e.span.start,
                })
                .collect(),
            name,
            params: None,
        },
    }
}

