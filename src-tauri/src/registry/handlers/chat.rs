#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::registry::params::ConversationIdParams;
use crate::registry::CommandOutput;
use crate::state::AppState;

pub fn get_chat_history(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let history = state.chat.lock().history_for_display();
    Ok(CommandOutput::json(
        format!("{} messages.", history.len()),
        &history,
    ))
}

pub fn get_agent_chat_history(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let messages = state.agent_display_messages.lock().clone();
    Ok(CommandOutput::json(
        format!("{} messages.", messages.len()),
        &messages,
    ))
}

pub fn clear_chat(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    state.chat.lock().clear();
    crate::chat::save_chat_history(state);
    Ok(CommandOutput::unit("Chat cleared."))
}

pub fn stop_chat(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    state.chat.lock().cancel();
    Ok(CommandOutput::unit("Chat cancelled."))
}

pub fn list_agent_conversations(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let summaries = crate::chat::list_agent_conversations(state);
    Ok(CommandOutput::json(
        format!("{} conversations.", summaries.len()),
        &summaries,
    ))
}

pub fn new_agent_conversation(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let id = crate::chat::new_agent_conversation(state);
    Ok(CommandOutput::json(
        "New conversation created.",
        &serde_json::json!({ "id": id }),
    ))
}

pub fn switch_agent_conversation(
    state: &Arc<AppState>,
    p: ConversationIdParams,
) -> Result<CommandOutput, AppError> {
    crate::chat::switch_agent_conversation(state, &p.conversation_id)
        .map_err(|e| AppError::NotFound { what: e })?;
    Ok(CommandOutput::unit("Switched conversation."))
}

pub fn delete_agent_conversation(
    state: &Arc<AppState>,
    p: ConversationIdParams,
) -> Result<CommandOutput, AppError> {
    crate::chat::delete_agent_conversation(state, &p.conversation_id)
        .map_err(|e| AppError::NotFound { what: e })?;
    Ok(CommandOutput::unit("Conversation deleted."))
}
