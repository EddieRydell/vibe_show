use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::registry;
use crate::state::AppState;

// ── ChatEmitter trait ────────────────────────────────────────────

/// Abstraction over event emission so agent/chat works without Tauri.
pub trait ChatEmitter: Send + Sync {
    fn emit_token(&self, text: &str);
    fn emit_tool_call(&self, id: &str, tool: &str);
    fn emit_tool_result(&self, id: &str, tool: &str, result: &str);
    fn emit_complete(&self);
    fn emit_thinking(&self, thinking: bool);
    fn emit_error(&self, message: &str);
}

/// Tauri-specific emitter: forwards events to the frontend via `AppHandle`.
#[cfg(feature = "tauri-app")]
pub struct TauriChatEmitter {
    pub app_handle: tauri::AppHandle,
}

#[cfg(feature = "tauri-app")]
impl ChatEmitter for TauriChatEmitter {
    fn emit_token(&self, text: &str) {
        if let Err(e) = tauri::Emitter::emit(&self.app_handle, crate::events::CHAT_TOKEN, text) {
            eprintln!("[VibeLights] Failed to emit chat token: {e}");
        }
    }
    fn emit_tool_call(&self, id: &str, tool: &str) {
        if let Err(e) = tauri::Emitter::emit(
            &self.app_handle,
            crate::events::CHAT_TOOL_CALL,
            serde_json::json!({ "id": id, "tool": tool }),
        ) {
            eprintln!("[VibeLights] Failed to emit tool call: {e}");
        }
    }
    fn emit_tool_result(&self, id: &str, tool: &str, result: &str) {
        if let Err(e) = tauri::Emitter::emit(
            &self.app_handle,
            crate::events::CHAT_TOOL_RESULT,
            serde_json::json!({ "id": id, "tool": tool, "result": result }),
        ) {
            eprintln!("[VibeLights] Failed to emit tool result: {e}");
        }
    }
    fn emit_complete(&self) {
        if let Err(e) = tauri::Emitter::emit(&self.app_handle, crate::events::CHAT_COMPLETE, true) {
            eprintln!("[VibeLights] Failed to emit chat complete: {e}");
        }
    }
    fn emit_thinking(&self, thinking: bool) {
        if let Err(e) = tauri::Emitter::emit(&self.app_handle, crate::events::CHAT_THINKING, thinking) {
            eprintln!("[VibeLights] Failed to emit chat thinking: {e}");
        }
    }
    fn emit_error(&self, message: &str) {
        if let Err(e) = tauri::Emitter::emit(&self.app_handle, crate::events::CHAT_ERROR, message) {
            eprintln!("[VibeLights] Failed to emit chat error: {e}");
        }
    }
}

// ── Types ────────────────────────────────────────────────────────

/// Chat message role. Serializes to lowercase strings matching the Claude API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChatHistoryEntry {
    pub role: ChatRole,
    pub text: String,
}

// ── Tool execution ───────────────────────────────────────────────

/// Execute a registry command by name. Used by the agent sidecar.
/// Returns the full typed `CommandOutput` (message + `CommandResult`).
///
/// Wraps the actual execution with timing + audit logging.
pub fn execute_tool_api(
    state: &Arc<AppState>,
    name: &str,
    input: &Value,
) -> Result<registry::CommandOutput, String> {
    let start = Instant::now();
    let conversation_id = state.agent_chats.lock().active_id.clone();

    let result = execute_tool_api_inner(state, name, input);

    crate::audit::log_tool_call(
        &state.app_config_dir,
        conversation_id.as_deref(),
        name,
        input,
        result.as_ref().map(|o| o.message.as_str()).map_err(String::as_str),
        start.elapsed(),
    );

    result
}

/// Inner implementation of tool execution (no logging).
fn execute_tool_api_inner(
    state: &Arc<AppState>,
    name: &str,
    input: &Value,
) -> Result<registry::CommandOutput, String> {
    let cmd = registry::catalog::deserialize_from_tool_call(name, input)?;
    registry::execute::execute(state, cmd).map_err(|e| e.to_string())
}

// ── Agent chat persistence (multi-conversation) ────────────────

/// A single conversation in the agent chat history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConversation {
    pub id: String,
    pub created_at: String,
    pub title: String,
    pub session_id: Option<String>,
    pub messages: Vec<ChatHistoryEntry>,
}

/// Root structure stored in `{app_config_dir}/agent-chats.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentChatsData {
    pub conversations: Vec<AgentConversation>,
    pub active_id: Option<String>,
}

/// Summary returned to the frontend for listing conversations.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub message_count: usize,
    pub is_active: bool,
}

const MAX_CONVERSATIONS: usize = 20;


#[allow(clippy::expect_used)] // system clock before Unix epoch is unrecoverable
fn iso_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs();
    // Convert Unix timestamp to ISO 8601 UTC (YYYY-MM-DDTHH:MM:SSZ)
    let days = secs / 86400;
    let time = secs % 86400;
    let h = time / 3600;
    let m = (time % 3600) / 60;
    let s = time % 60;
    // Civil date from days since epoch (Euclidean affine algorithm)
    let z = days as i64 + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

#[allow(clippy::expect_used)] // system clock before Unix epoch is unrecoverable
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_nanos();
    format!("{ts:x}")
}

/// Sync current in-memory state into the active conversation within `AgentChatsData`.
fn sync_active_to_chats(state: &AppState, chats: &mut AgentChatsData) {
    let session_id = state.agent_session_id.lock().clone();
    let messages = state.agent_display_messages.lock().clone();

    if let Some(ref active_id) = chats.active_id {
        if let Some(conv) = chats.conversations.iter_mut().find(|c| &c.id == active_id) {
            conv.session_id = session_id;
            conv.messages = messages;
        }
    }
}

/// Load the active conversation from `AgentChatsData` into in-memory state.
fn load_active_from_chats(state: &AppState, chats: &AgentChatsData) {
    if let Some(ref active_id) = chats.active_id {
        if let Some(conv) = chats.conversations.iter().find(|c| &c.id == active_id) {
            (*state.agent_session_id.lock()).clone_from(&conv.session_id);
            (*state.agent_display_messages.lock()).clone_from(&conv.messages);
            return;
        }
    }
    // No active conversation
    *state.agent_session_id.lock() = None;
    state.agent_display_messages.lock().clear();
}

/// Load agent chats from disk.
pub fn load_agent_chats(state: &Arc<AppState>) {
    let path = crate::paths::agent_chats_file_path(&state.app_config_dir);

    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(chats_data) = serde_json::from_str::<AgentChatsData>(&data) {
                load_active_from_chats(state, &chats_data);
                *state.agent_chats.lock() = chats_data;
                return;
            }
        }
    }

    // Nothing found — start fresh
    *state.agent_session_id.lock() = None;
    state.agent_display_messages.lock().clear();
    *state.agent_chats.lock() = AgentChatsData::default();
}

/// Save agent chats to disk. Syncs current in-memory state into the active conversation first.
pub fn save_agent_chats(state: &Arc<AppState>) {
    let path = crate::paths::agent_chats_file_path(&state.app_config_dir);

    let mut chats = state.agent_chats.lock().clone();
    sync_active_to_chats(state, &mut chats);

    // Auto-prune to MAX_CONVERSATIONS (keep active)
    if chats.conversations.len() > MAX_CONVERSATIONS {
        let active_id = chats.active_id.clone();
        // Remove oldest non-active conversations
        while chats.conversations.len() > MAX_CONVERSATIONS {
            if let Some(pos) = chats
                .conversations
                .iter()
                .position(|c| Some(&c.id) != active_id.as_ref())
            {
                chats.conversations.remove(pos);
            } else {
                break;
            }
        }
    }

    *state.agent_chats.lock() = chats.clone();

    if chats.conversations.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }

    match serde_json::to_string_pretty(&chats) {
        Ok(json) => {
            if let Err(e) = crate::project::atomic_write(&path, json.as_bytes()) {
                eprintln!("[VibeLights] Failed to save agent chats: {e}");
            }
        }
        Err(e) => {
            eprintln!("[VibeLights] Failed to serialize agent chats: {e}");
        }
    }
}

/// Create a new conversation. Saves the current one first, returns the new conversation ID.
pub fn new_agent_conversation(state: &Arc<AppState>) -> String {
    // Save current state into the active conversation
    {
        let mut chats = state.agent_chats.lock();
        sync_active_to_chats(state, &mut chats);
    }

    // Remove empty conversations (no messages)
    {
        let mut chats = state.agent_chats.lock();
        chats
            .conversations
            .retain(|c| !c.messages.is_empty());
    }

    let new_id = generate_id();

    // Create new conversation and set as active
    {
        let mut chats = state.agent_chats.lock();
        let conv = AgentConversation {
            id: new_id.clone(),
            created_at: iso_now(),
            title: "New conversation".to_string(),
            session_id: None,
            messages: Vec::new(),
        };
        chats.conversations.push(conv);
        chats.active_id = Some(new_id.clone());
    }

    // Clear active state
    *state.agent_session_id.lock() = None;
    state.agent_display_messages.lock().clear();

    // Persist
    save_agent_chats(state);

    new_id
}

/// Switch to a different conversation by ID.
pub fn switch_agent_conversation(state: &Arc<AppState>, id: &str) -> Result<(), String> {
    // Save current first
    {
        let mut chats = state.agent_chats.lock();
        sync_active_to_chats(state, &mut chats);
    }

    let mut chats = state.agent_chats.lock();
    if !chats.conversations.iter().any(|c| c.id == id) {
        return Err(format!("Conversation '{id}' not found"));
    }

    chats.active_id = Some(id.to_string());
    load_active_from_chats(state, &chats);

    // Persist
    let chats_clone = chats.clone();
    drop(chats);
    let path = crate::paths::agent_chats_file_path(&state.app_config_dir);
    let json = serde_json::to_string_pretty(&chats_clone)
        .map_err(|e| format!("Failed to serialize agent chats: {e}"))?;
    crate::project::atomic_write(&path, json.as_bytes())
        .map_err(|e| format!("Failed to save agent chats: {e}"))?;

    Ok(())
}

/// Delete a conversation by ID.
pub fn delete_agent_conversation(state: &Arc<AppState>, id: &str) -> Result<(), String> {
    let mut chats = state.agent_chats.lock();

    let was_active = chats.active_id.as_deref() == Some(id);
    chats.conversations.retain(|c| c.id != id);

    if was_active {
        chats.active_id = None;
        *state.agent_session_id.lock() = None;
        state.agent_display_messages.lock().clear();
    }

    let chats_clone = chats.clone();
    drop(chats);

    let path = crate::paths::agent_chats_file_path(&state.app_config_dir);
    if chats_clone.conversations.is_empty() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to remove agent chats file: {e}"))?;
    } else {
        let json = serde_json::to_string_pretty(&chats_clone)
            .map_err(|e| format!("Failed to serialize agent chats: {e}"))?;
        crate::project::atomic_write(&path, json.as_bytes())
            .map_err(|e| format!("Failed to save agent chats: {e}"))?;
    }

    Ok(())
}

/// List conversation summaries.
pub fn list_agent_conversations(state: &Arc<AppState>) -> Vec<ConversationSummary> {
    // Sync current state so the active conversation's message count is accurate
    {
        let mut chats = state.agent_chats.lock();
        sync_active_to_chats(state, &mut chats);
    }

    let chats = state.agent_chats.lock();
    chats
        .conversations
        .iter()
        .map(|c| ConversationSummary {
            id: c.id.clone(),
            title: c.title.clone(),
            created_at: c.created_at.clone(),
            message_count: c.messages.len(),
            is_active: chats.active_id.as_deref() == Some(&c.id),
        })
        .collect()
}

/// Clear agent chat state (in-memory and on disk). Used for full reset.
pub fn clear_agent_chat(state: &Arc<AppState>) {
    *state.agent_session_id.lock() = None;
    state.agent_display_messages.lock().clear();
    *state.agent_chats.lock() = AgentChatsData::default();
    let path = crate::paths::agent_chats_file_path(&state.app_config_dir);
    let _ = std::fs::remove_file(&path);
}
