use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::llm;
use crate::registry;
use crate::state::AppState;

// ── ChatEmitter trait ────────────────────────────────────────────

/// Abstraction over event emission so chat works without Tauri.
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
        let _ = tauri::Emitter::emit(&self.app_handle, "chat:token", text);
    }
    fn emit_tool_call(&self, tool: &str) {
        let _ = tauri::Emitter::emit(&self.app_handle, "chat:tool_call", tool);
    }
    fn emit_tool_result(&self, tool: &str, result: &str) {
        let _ = tauri::Emitter::emit(
            &self.app_handle,
            "chat:tool_result",
            serde_json::json!({ "tool": tool, "result": result }),
        );
    }
    fn emit_complete(&self) {
        let _ = tauri::Emitter::emit(&self.app_handle, "chat:complete", true);
    }
    fn emit_thinking(&self, thinking: bool) {
        let _ = tauri::Emitter::emit(&self.app_handle, "chat:thinking", thinking);
    }
}

/// No-op emitter for CLI/API use — results are returned in the HTTP response.
pub struct NoopChatEmitter;

impl ChatEmitter for NoopChatEmitter {
    fn emit_token(&self, _text: &str) {}
    fn emit_tool_call(&self, _tool: &str) {}
    fn emit_tool_result(&self, _tool: &str, _result: &str) {}
    fn emit_complete(&self) {}
    fn emit_thinking(&self, _thinking: bool) {}
}

// ── Types ────────────────────────────────────────────────────────

/// Chat message role. Serializes to lowercase strings matching the Claude API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: ChatContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistoryEntry {
    pub role: ChatRole,
    pub text: String,
}

// ── System prompt builder ────────────────────────────────────────

fn build_system_prompt(state: &Arc<AppState>) -> String {
    let show = state.show.lock();
    let playback = state.playback.lock();

    let mut lines = Vec::new();

    lines.push("You are VibeLights AI, a creative light show design assistant.".to_string());
    lines.push("You have 3 tools: help (discover commands), run (execute one), batch (execute many as one undo step).".to_string());
    lines.push("Key categories: query (inspect show), edit (modify effects/tracks), playback, profile, sequence, library, script, analysis.".to_string());
    lines.push("Use help() to see all categories, help({topic: \"query\"}) for commands in a category.".to_string());
    lines.push("The user only sees your text responses, not tool calls or results. Summarize results concisely in your reply.".to_string());
    lines.push(String::new());

    // Param value format (essential — the LLM needs this to construct correct params)
    lines.push("## Param value format".to_string());
    lines.push("Effect params use tagged format: {\"Float\": 1.0}, {\"Color\": {\"r\":255,\"g\":0,\"b\":0,\"a\":255}}, {\"Bool\": true}, {\"Int\": 5}".to_string());
    lines.push("Curve: {\"Curve\":{\"points\":[{\"x\":0,\"y\":0},{\"x\":1,\"y\":1}]}}".to_string());
    lines.push("Gradient: {\"ColorGradient\":{\"stops\":[{\"position\":0,\"color\":{\"r\":255,\"g\":0,\"b\":0,\"a\":255}},{\"position\":1,\"color\":{\"r\":0,\"g\":0,\"b\":255,\"a\":255}}]}}".to_string());
    lines.push("Library refs: {\"GradientRef\":\"name\"} or {\"CurveRef\":\"name\"}".to_string());
    lines.push(String::new());

    // Minimal orientation — just counts, not full dumps
    lines.push("## Current state".to_string());
    lines.push(format!("Fixtures: {}", show.fixtures.len()));

    if let Some(seq) = show.sequences.first() {
        lines.push(format!("Sequence: {} ({:.1}s, {} tracks, {} total effects)",
            seq.name, seq.duration, seq.tracks.len(),
            seq.tracks.iter().map(|t| t.effects.len()).sum::<usize>(),
        ));
        if let Some(ref audio) = seq.audio_file {
            lines.push(format!("Audio: {audio}"));
        }
    }

    lines.push(format!("Playback: {} at {:.1}s",
        if playback.playing { "playing" } else { "paused" },
        playback.current_time,
    ));

    // Analysis hint
    if let Some(seq) = show.sequences.first() {
        if let Some(ref audio) = seq.audio_file {
            let cache = state.analysis_cache.lock();
            if cache.contains_key(audio) {
                lines.push("Audio analysis available.".to_string());
            }
        }
    }

    lines.push(String::new());
    lines.push("## Tool usage".to_string());
    lines.push("run({command: \"get_show\"}) — commands without parameters".to_string());
    lines.push("run({command: \"open_sequence\", params: {slug: \"my-sequence\"}}) — commands with parameters".to_string());
    lines.push("batch({description: \"...\", commands: [{command: \"add_effect\", params: {...}}, ...]}) — multiple edits as one undo step".to_string());

    lines.join("\n")
}

// ── Tool execution ───────────────────────────────────────────────

/// Execute a registry command by name. Used by REST API and MCP.
/// Handles the `help` meta-tool directly, then dispatches to the registry.
pub fn execute_tool_api(state: &Arc<AppState>, name: &str, input: &Value) -> Result<String, String> {
    if name == "help" {
        let topic = input.get("topic").and_then(Value::as_str);
        return Ok(registry::catalog::help_text(topic));
    }
    let cmd = registry::catalog::deserialize_from_tool_call(name, input)?;
    let output = registry::execute::execute(state, cmd).map_err(|e| e.to_string())?;
    Ok(output.message)
}

fn execute_tool(state: &Arc<AppState>, name: &str, input: &Value) -> Result<String, String> {
    match name {
        "help" => {
            let topic = input.get("topic").and_then(Value::as_str);
            Ok(registry::catalog::help_text(topic))
        }
        "run" => {
            let command = input
                .get("command")
                .and_then(Value::as_str)
                .ok_or("Missing 'command' field")?;
            let params = input.get("params").unwrap_or(&Value::Null);
            let cmd = registry::catalog::deserialize_from_tool_call(command, params)
                .map_err(|e| {
                    if params.is_null() {
                        format!(
                            "Command \"{command}\" requires params but none were provided. \
                             Use help({{topic: \"{command}\"}}) to see the required parameters. \
                             Pass them as: run({{command: \"{command}\", params: {{...}}}})"
                        )
                    } else {
                        format!("Error deserializing params for \"{command}\": {e}")
                    }
                })?;
            let output = registry::execute::execute(state, cmd).map_err(|e| e.to_string())?;
            Ok(output.message)
        }
        "batch" => {
            let description = input
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("Batch edit")
                .to_string();
            let commands = input
                .get("commands")
                .and_then(Value::as_array)
                .ok_or("Missing 'commands' array")?;

            // Transform {command, params} → {action, params} for batch_edit handler
            let batch_commands: Vec<Value> = commands
                .iter()
                .map(|c| {
                    let action = c.get("command").and_then(Value::as_str).unwrap_or("");
                    let params = c.get("params").cloned().unwrap_or(Value::Null);
                    serde_json::json!({ "action": action, "params": params })
                })
                .collect();

            let batch_params = registry::params::BatchEditParams {
                description,
                commands: batch_commands,
            };
            let cmd = registry::Command::BatchEdit(batch_params);
            let output = registry::execute::execute(state, cmd).map_err(|e| e.to_string())?;
            Ok(output.message)
        }
        _ => Err(format!(
            "Unknown tool: {name}. Available tools: help, run, batch."
        )),
    }
}

// ── Chat manager ─────────────────────────────────────────────────

#[derive(Default)]
pub struct ChatManager {
    history: Vec<ChatMessage>,
    cancelled: bool,
}

impl ChatManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            cancelled: false,
        }
    }

    /// Return only user messages and assistant text responses for display.
    /// Tool calls and results are internal implementation details — the user
    /// only sees the conversation (their messages + the AI's final answers).
    #[must_use]
    pub fn history_for_display(&self) -> Vec<ChatHistoryEntry> {
        let mut entries = Vec::new();
        for msg in &self.history {
            match &msg.content {
                ChatContent::Text(text) => {
                    entries.push(ChatHistoryEntry {
                        role: msg.role,
                        text: text.clone(),
                    });
                }
                ChatContent::Blocks(blocks) => {
                    for block in blocks {
                        if let ContentBlock::Text { text } = block {
                            // Only include non-empty text from user/assistant
                            if !text.is_empty() {
                                entries.push(ChatHistoryEntry {
                                    role: msg.role,
                                    text: text.clone(),
                                });
                            }
                        }
                        // Skip ToolUse and ToolResult — they're behind-the-scenes
                    }
                }
            }
        }
        entries
    }

    /// Get the raw chat history for persistence.
    #[must_use]
    pub fn history(&self) -> &[ChatMessage] {
        &self.history
    }

    /// Load chat history (e.g. from a persisted file). Truncates to last 50 messages.
    pub fn load_history(&mut self, messages: Vec<ChatMessage>) {
        let len = messages.len();
        if len > 50 {
            self.history = messages.into_iter().skip(len - 50).collect();
        } else {
            self.history = messages;
        }
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// Save chat history to a JSON file.
    pub fn save_to_file(&self, path: &Path) {
        if self.history.is_empty() {
            // Remove stale file if chat is empty
            let _ = std::fs::remove_file(path);
            return;
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.history) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Load chat history from a JSON file. Truncates to 50 messages.
    pub fn load_from_file(&mut self, path: &Path) {
        self.history.clear();
        if !path.exists() {
            return;
        }
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(messages) = serde_json::from_str::<Vec<ChatMessage>>(&data) {
                self.load_history(messages);
            }
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Send a message and run the agentic loop. Uses `ChatEmitter` for event output.
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is missing, the API request fails, or
    /// the response cannot be parsed.
    #[allow(clippy::too_many_lines)]
    pub async fn send_message(
        state: Arc<AppState>,
        emitter: &dyn ChatEmitter,
        message: String,
    ) -> Result<(), String> {
        // Resolve LLM provider from settings
        let provider = {
            let settings = state.settings.lock();
            let config = settings
                .as_ref()
                .map(|s| &s.llm)
                .ok_or("No settings configured.")?;
            llm::ResolvedProvider::from_config(config)?
        };

        // Add user message
        {
            let mut chat = state.chat.lock();
            chat.cancelled = false;
            chat.history.push(ChatMessage {
                role: ChatRole::User,
                content: ChatContent::Text(message),
            });
        }

        let client = reqwest::Client::new();
        let max_iterations = 20;

        for _ in 0..max_iterations {
            // Check cancellation
            if state.chat.lock().cancelled {
                return Ok(());
            }

            let system_prompt = build_system_prompt(&state);
            let messages: Vec<Value> = {
                let chat = state.chat.lock();
                chat.history
                    .iter()
                    .filter_map(|m| serde_json::to_value(m).ok())
                    .collect()
            };

            // Minimal tool set: help + run + batch (discovery-based)
            let tool_defs = registry::catalog::to_llm_tools();

            emitter.emit_thinking(true);

            let response = llm::build_request(
                &client,
                &provider,
                &system_prompt,
                &messages,
                &tool_defs,
            )
            .send()
            .await
            .map_err(|e| format!("API request failed: {e}"))?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                return Err(format!("API error {status}: {text}"));
            }

            let response_json: Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {e}"))?;

            let (content_blocks, stop_reason) =
                llm::parse_response(&provider.provider, &response_json)?;

            // Process response blocks
            let mut has_tool_use = false;
            let mut assistant_blocks = Vec::new();
            let mut tool_results = Vec::new();

            for block in &content_blocks {
                let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                match block_type {
                    "text" => {
                        let text = block.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                        emitter.emit_token(&text);
                        assistant_blocks.push(ContentBlock::Text { text });
                    }
                    "tool_use" => {
                        has_tool_use = true;
                        let id = block.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                        let name = block.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                        let input = block.get("input").cloned().unwrap_or(Value::Null);

                        emitter.emit_tool_call(&name);

                        assistant_blocks.push(ContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        });

                        // Execute the tool via unified registry
                        let result = execute_tool(&state, &name, &input)
                            .unwrap_or_else(|e| format!("Error: {e}"));

                        emitter.emit_tool_result(&name, &result);

                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: result,
                        });
                    }
                    _ => {}
                }
            }

            // Add assistant message to history
            {
                let mut chat = state.chat.lock();
                chat.history.push(ChatMessage {
                    role: ChatRole::Assistant,
                    content: ChatContent::Blocks(assistant_blocks),
                });

                // If there were tool uses, add tool results
                if !tool_results.is_empty() {
                    chat.history.push(ChatMessage {
                        role: ChatRole::User,
                        content: ChatContent::Blocks(tool_results),
                    });
                }
            }

            // If no tool use, we're done
            if !has_tool_use || stop_reason == "end_turn" || stop_reason.is_empty() {
                break;
            }
        }

        emitter.emit_complete();

        // Persist chat history after each completed exchange
        save_chat_history(&state);

        Ok(())
    }

    /// Get the last assistant text response (useful for API/CLI returning results).
    #[must_use]
    pub fn last_assistant_text(&self) -> Option<String> {
        for msg in self.history.iter().rev() {
            if msg.role == ChatRole::Assistant {
                match &msg.content {
                    ChatContent::Text(text) => return Some(text.clone()),
                    ChatContent::Blocks(blocks) => {
                        for block in blocks {
                            if let ContentBlock::Text { text } = block {
                                return Some(text.clone());
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

// ── Chat persistence helpers ─────────────────────────────────────

/// Compute the chat file path: {app_config_dir}/chat.json (global, not per-profile)
fn chat_file_path(state: &AppState) -> std::path::PathBuf {
    state.app_config_dir.join("chat.json")
}

/// Save the current chat history to disk (best-effort, non-blocking).
pub fn save_chat_history(state: &Arc<AppState>) {
    let path = chat_file_path(state);
    let chat = state.chat.lock();
    chat.save_to_file(&path);
}

/// Load chat history from disk (global).
pub fn load_chat_history(state: &Arc<AppState>) {
    let path = chat_file_path(state);
    let mut chat = state.chat.lock();
    chat.load_from_file(&path);
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub message_count: usize,
    pub is_active: bool,
}

const MAX_CONVERSATIONS: usize = 20;

/// Compute the agent chats file path: {app_config_dir}/agent-chats.json (global, not per-profile)
fn agent_chats_file_path(state: &AppState) -> std::path::PathBuf {
    state.app_config_dir.join("agent-chats.json")
}

fn iso_now() -> String {
    // Use a simple ISO 8601 timestamp
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
    let path = agent_chats_file_path(state);

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
    let path = agent_chats_file_path(state);

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

    if let Ok(json) = serde_json::to_string_pretty(&chats) {
        let _ = std::fs::write(&path, json);
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
    let path = agent_chats_file_path(state);
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

    let path = agent_chats_file_path(state);
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
    let path = agent_chats_file_path(state);
    let _ = std::fs::remove_file(&path);
}
