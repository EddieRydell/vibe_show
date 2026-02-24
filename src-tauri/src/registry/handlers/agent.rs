#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::agent;
use crate::error::AppError;
use crate::registry::params::SendAgentMessageParams;
use crate::registry::{CommandOutput, CommandResult};
use crate::state::AppState;

#[cfg(feature = "tauri-app")]
pub async fn send_agent_message(
    state: Arc<AppState>,
    app: Option<tauri::AppHandle>,
    p: SendAgentMessageParams,
) -> Result<CommandOutput, AppError> {
    let app_handle = app.ok_or_else(|| AppError::ApiError {
        message: "AppHandle required for send_agent_message".into(),
    })?;
    let emitter = crate::chat::TauriChatEmitter {
        app_handle: app_handle.clone(),
    };
    agent::send_message(&state, &app_handle, &emitter, p.message, p.context.as_deref()).await?;
    Ok(CommandOutput::new(
        "Agent message sent.",
        CommandResult::SendAgentMessage,
    ))
}

#[cfg(feature = "tauri-app")]
pub async fn cancel_agent_message(
    state: Arc<AppState>,
    _app: Option<tauri::AppHandle>,
) -> Result<CommandOutput, AppError> {
    agent::cancel_message(&state).await?;
    Ok(CommandOutput::new(
        "Agent message cancelled.",
        CommandResult::CancelAgentMessage,
    ))
}

#[cfg(feature = "tauri-app")]
pub async fn clear_agent_session(
    state: Arc<AppState>,
    _app: Option<tauri::AppHandle>,
) -> Result<CommandOutput, AppError> {
    agent::clear_session(&state).await?;
    Ok(CommandOutput::new(
        "Agent session cleared.",
        CommandResult::ClearAgentSession,
    ))
}
