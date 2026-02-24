#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use crate::error::AppError;
use crate::registry::params::ConversationIdParams;
use crate::registry::{CommandOutput, CommandResult};
use crate::state::AppState;

/// Typed return for NewAgentConversation.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct NewConversationResult {
    pub id: String,
}

pub fn get_agent_chat_history(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let messages = state.agent_display_messages.lock().clone();
    Ok(CommandOutput::new(
        format!("{} messages.", messages.len()),
        CommandResult::GetAgentChatHistory(messages),
    ))
}

pub fn list_agent_conversations(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let summaries = crate::chat::list_agent_conversations(state);
    Ok(CommandOutput::new(
        format!("{} conversations.", summaries.len()),
        CommandResult::ListAgentConversations(summaries),
    ))
}

pub fn new_agent_conversation(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let id = crate::chat::new_agent_conversation(state);
    Ok(CommandOutput::new(
        "New conversation created.",
        CommandResult::NewAgentConversation(NewConversationResult { id }),
    ))
}

pub fn switch_agent_conversation(
    state: &Arc<AppState>,
    p: ConversationIdParams,
) -> Result<CommandOutput, AppError> {
    crate::chat::switch_agent_conversation(state, &p.conversation_id)
        .map_err(|e| AppError::NotFound { what: e })?;
    Ok(CommandOutput::new("Switched conversation.", CommandResult::SwitchAgentConversation))
}

pub fn delete_agent_conversation(
    state: &Arc<AppState>,
    p: ConversationIdParams,
) -> Result<CommandOutput, AppError> {
    crate::chat::delete_agent_conversation(state, &p.conversation_id)
        .map_err(|e| AppError::NotFound { what: e })?;
    Ok(CommandOutput::new("Conversation deleted.", CommandResult::DeleteAgentConversation))
}
