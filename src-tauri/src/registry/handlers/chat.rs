#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::error::AppError;
use crate::registry::CommandOutput;
use crate::state::AppState;

pub fn get_chat_history(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let history = state.chat.lock().history_for_display();
    Ok(CommandOutput::json(
        format!("{} messages.", history.len()),
        &history,
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
