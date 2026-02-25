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
    fn emit_tool_call(&self, tool: &str);
    fn emit_tool_result(&self, tool: &str, result: &str);
    fn emit_complete(&self);
    fn emit_thinking(&self, thinking: bool);
}

/// Tauri-specific emitter: forwards events to the frontend via `AppHandle`.
#[cfg(feature = "tauri-app")]
pub struct TauriChatEmitter {
    pub app_handle: tauri::AppHandle,
}

#[cfg(feature = "tauri-app")]
impl ChatEmitter for TauriChatEmitter {
    fn emit_token(&self, text: &str) {
        let _ = tauri::Emitter::emit(&self.app_handle, crate::events::CHAT_TOKEN, text);
    }
    fn emit_tool_call(&self, tool: &str) {
        let _ = tauri::Emitter::emit(&self.app_handle, crate::events::CHAT_TOOL_CALL, tool);
    }
    fn emit_tool_result(&self, tool: &str, result: &str) {
        let _ = tauri::Emitter::emit(
            &self.app_handle,
            crate::events::CHAT_TOOL_RESULT,
            serde_json::json!({ "tool": tool, "result": result }),
        );
    }
    fn emit_complete(&self) {
        let _ = tauri::Emitter::emit(&self.app_handle, crate::events::CHAT_COMPLETE, true);
    }
    fn emit_thinking(&self, thinking: bool) {
        let _ = tauri::Emitter::emit(&self.app_handle, crate::events::CHAT_THINKING, thinking);
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

/// Result from tool execution, including an optional path to a scratch file
/// containing the full data for large results.
pub struct ToolResult {
    pub message: String,
    pub data_file: Option<String>,
}

/// Write a JSON value to a scratch file for agent file-based exploration.
/// Returns the absolute path to the written file, or `None` on failure.
fn write_scratch_file(state: &Arc<AppState>, filename: &str, data: &Value) -> Option<String> {
    let data_dir = crate::state::get_data_dir(state).ok()?;
    let dir = crate::paths::scratch_dir(&data_dir);
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(filename);
    let json = serde_json::to_string_pretty(data).ok()?;
    crate::project::atomic_write(&path, json.as_bytes()).ok()?;
    Some(path.to_string_lossy().to_string())
}

/// Threshold in bytes — data larger than this gets written to a scratch file.
const SCRATCH_THRESHOLD: usize = 500;

/// Execute a registry command by name. Used by the agent sidecar.
/// Handles the `help` meta-tool directly, then dispatches to the registry.
/// Returns a `ToolResult` with the message and an optional scratch file path.
///
/// Wraps the actual execution with timing + audit logging.
pub fn execute_tool_api(state: &Arc<AppState>, name: &str, input: &Value) -> Result<ToolResult, String> {
    let start = Instant::now();
    let conversation_id = state.agent_chats.lock().active_id.clone();

    let result = execute_tool_api_inner(state, name, input);

    crate::audit::log_tool_call(
        &state.app_config_dir,
        conversation_id.as_deref(),
        name,
        input,
        result.as_ref().map_err(String::as_str),
        start.elapsed(),
    );

    result
}

/// Inner implementation of tool execution (no logging).
fn execute_tool_api_inner(state: &Arc<AppState>, name: &str, input: &Value) -> Result<ToolResult, String> {
    if name == "help" {
        let topic = input.get("topic").and_then(Value::as_str);
        return Ok(ToolResult {
            message: registry::catalog::help_text(topic),
            data_file: None,
        });
    }
    let cmd = registry::catalog::deserialize_from_tool_call(name, input)?;
    let output = registry::execute::execute(state, cmd).map_err(|e| e.to_string())?;

    // If the command produced structured data, write large payloads to a scratch file.
    let data_file = {
        let result_json = serde_json::to_value(&output.result).unwrap_or(Value::Null);
        let data_value = result_json.get("data").cloned().unwrap_or(Value::Null);
        if data_value.is_null() {
            None
        } else {
            let serialized = serde_json::to_string(&data_value).unwrap_or_default();
            if serialized.len() > SCRATCH_THRESHOLD {
                let filename = format!("{name}.json");
                write_scratch_file(state, &filename, &data_value)
            } else {
                None
            }
        }
    };

    Ok(ToolResult {
        message: output.message,
        data_file,
    })
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


fn iso_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{now}")
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
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
    if let Ok(json) = serde_json::to_string_pretty(&chats_clone) {
        let _ = std::fs::write(&path, json);
    }

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
        let _ = std::fs::remove_file(&path);
    } else if let Ok(json) = serde_json::to_string_pretty(&chats_clone) {
        let _ = std::fs::write(&path, json);
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
