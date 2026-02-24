use std::sync::Arc;

use crate::error::AppError;
use crate::state::AppState;

use super::{Command, CommandOutput};

/// Execute a Command against the application state.
/// This is the single dispatch point for all surfaces (GUI, CLI, AI, REST, MCP).
pub fn execute(state: &Arc<AppState>, cmd: Command) -> Result<CommandOutput, AppError> {
    cmd.dispatch(state)
}
