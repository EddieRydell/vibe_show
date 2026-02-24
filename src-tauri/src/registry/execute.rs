use std::sync::Arc;

use crate::error::AppError;
use crate::state::AppState;

use super::{Command, CommandOutput};

/// Execute a sync Command against the application state.
/// This is the single dispatch point for all surfaces (GUI, CLI, AI, REST, MCP).
/// Returns an error for async commands — use `execute_async` instead.
pub fn execute(state: &Arc<AppState>, cmd: Command) -> Result<CommandOutput, AppError> {
    cmd.dispatch(state)
}

/// Execute any Command (sync or async) against the application state.
/// Async commands are awaited; sync commands run inline.
#[cfg(feature = "tauri-app")]
pub async fn execute_async(
    state: Arc<AppState>,
    app: Option<tauri::AppHandle>,
    cmd: Command,
) -> Result<CommandOutput, AppError> {
    cmd.dispatch_async(state, app).await
}
