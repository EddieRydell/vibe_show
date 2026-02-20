use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dispatcher::EditCommand;
use crate::effects;
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

/// Tauri-specific emitter: forwards events to the frontend via AppHandle.
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
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

#[derive(Debug, Clone, Serialize)]
pub struct ChatHistoryEntry {
    pub role: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

// ── Tool definitions ─────────────────────────────────────────────

fn tool_definitions() -> Value {
    serde_json::json!([
        {
            "name": "add_effect",
            "description": "Add an effect to a track",
            "input_schema": {
                "type": "object",
                "properties": {
                    "track_index": { "type": "integer" },
                    "kind": { "type": "string", "enum": ["Solid", "Chase", "Rainbow", "Strobe", "Gradient", "Twinkle"] },
                    "start": { "type": "number", "description": "Start time in seconds" },
                    "end": { "type": "number", "description": "End time in seconds" }
                },
                "required": ["track_index", "kind", "start", "end"]
            }
        },
        {
            "name": "delete_effects",
            "description": "Delete effects by (track_index, effect_index) pairs",
            "input_schema": {
                "type": "object",
                "properties": {
                    "targets": {
                        "type": "array",
                        "items": { "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2 }
                    }
                },
                "required": ["targets"]
            }
        },
        {
            "name": "update_effect_param",
            "description": "Set a parameter on an effect",
            "input_schema": {
                "type": "object",
                "properties": {
                    "track_index": { "type": "integer" },
                    "effect_index": { "type": "integer" },
                    "key": { "type": "string" },
                    "value": { "description": "ParamValue - use {\"Color\":{\"r\":255,\"g\":0,\"b\":0,\"a\":255}} for colors, {\"Float\":1.0} for numbers, etc." }
                },
                "required": ["track_index", "effect_index", "key", "value"]
            }
        },
        {
            "name": "update_effect_time_range",
            "description": "Change the start/end time of an effect",
            "input_schema": {
                "type": "object",
                "properties": {
                    "track_index": { "type": "integer" },
                    "effect_index": { "type": "integer" },
                    "start": { "type": "number" },
                    "end": { "type": "number" }
                },
                "required": ["track_index", "effect_index", "start", "end"]
            }
        },
        {
            "name": "add_track",
            "description": "Create a new track targeting a fixture",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "fixture_id": { "type": "integer", "description": "Target fixture ID" },
                    "blend_mode": { "type": "string", "enum": ["Override", "Add", "Multiply", "Max", "Alpha"], "default": "Override" }
                },
                "required": ["name", "fixture_id"]
            }
        },
        {
            "name": "play",
            "description": "Start playback",
            "input_schema": { "type": "object", "properties": {} }
        },
        {
            "name": "pause",
            "description": "Pause playback",
            "input_schema": { "type": "object", "properties": {} }
        },
        {
            "name": "seek",
            "description": "Seek to a time in seconds",
            "input_schema": {
                "type": "object",
                "properties": {
                    "time": { "type": "number" }
                },
                "required": ["time"]
            }
        },
        {
            "name": "undo",
            "description": "Undo the last editing action",
            "input_schema": { "type": "object", "properties": {} }
        },
        {
            "name": "redo",
            "description": "Redo the last undone action",
            "input_schema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_show",
            "description": "Get the full show model including fixtures, tracks, and effects",
            "input_schema": { "type": "object", "properties": {} }
        }
    ])
}

// ── System prompt builder ────────────────────────────────────────

fn build_system_prompt(state: &Arc<AppState>) -> String {
    let show = state.show.lock().unwrap();
    let playback = state.playback.lock().unwrap();

    let mut lines = Vec::new();
    lines.push("You are a light show assistant integrated into VibeLights, a light show sequencer.".to_string());
    lines.push("You can control the sequencer by calling the available tools.".to_string());
    lines.push(String::new());

    // Current state summary
    lines.push(format!("Show: {}", if show.name.is_empty() { "(untitled)" } else { &show.name }));
    lines.push(format!("Fixtures: {}", show.fixtures.len()));
    for f in &show.fixtures {
        lines.push(format!("  - {} (id: {}, {} pixels)", f.name, f.id.0, f.pixel_count));
    }

    if let Some(seq) = show.sequences.first() {
        lines.push(format!("\nSequence: {} ({:.1}s @ {}fps)", seq.name, seq.duration, seq.frame_rate));
        lines.push(format!("Tracks: {}", seq.tracks.len()));
        for (i, t) in seq.tracks.iter().enumerate() {
            lines.push(format!("  Track {}: \"{}\" ({:?}, {} effects)", i, t.name, t.blend_mode, t.effects.len()));
            for (j, e) in t.effects.iter().enumerate() {
                lines.push(format!("    Effect {}: {:?} [{:.1}s - {:.1}s]", j, e.kind, e.time_range.start(), e.time_range.end()));
            }
        }
    }

    lines.push(format!("\nPlayback: {} at {:.1}s",
        if playback.playing { "playing" } else { "paused" },
        playback.current_time
    ));

    // Available effect types
    lines.push("\nAvailable effect types:".to_string());
    let kinds = [
        crate::model::EffectKind::Solid,
        crate::model::EffectKind::Chase,
        crate::model::EffectKind::Rainbow,
        crate::model::EffectKind::Strobe,
        crate::model::EffectKind::Gradient,
        crate::model::EffectKind::Twinkle,
        crate::model::EffectKind::Fade,
    ];
    for kind in &kinds {
        let effect = effects::resolve_effect(kind);
        let schemas = effect.param_schema();
        let params: Vec<String> = schemas.iter().map(|s| format!("{} ({})", s.key, s.label)).collect();
        lines.push(format!("  {:?}: params [{}]", kind, params.join(", ")));
    }

    lines.push("\nParam values use tagged format: {\"Float\": 1.0}, {\"Color\": {\"r\":255,\"g\":0,\"b\":0,\"a\":255}}, {\"Bool\": true}, {\"ColorList\": [{\"r\":255,\"g\":0,\"b\":0,\"a\":255}]}, {\"Int\": 5}, {\"Text\": \"hello\"}".to_string());

    lines.join("\n")
}

// ── Tool execution ───────────────────────────────────────────────

fn execute_tool(state: &Arc<AppState>, name: &str, input: &Value) -> Result<String, String> {
    match name {
        "add_effect" => {
            let cmd = EditCommand::AddEffect {
                sequence_index: 0,
                track_index: input["track_index"].as_u64().ok_or("Missing track_index")? as usize,
                kind: serde_json::from_value(input["kind"].clone()).map_err(|e| e.to_string())?,
                start: input["start"].as_f64().ok_or("Missing start")?,
                end: input["end"].as_f64().ok_or("Missing end")?,
            };
            let mut dispatcher = state.dispatcher.lock().unwrap();
            let mut show = state.show.lock().unwrap();
            let result = dispatcher.execute(&mut show, cmd)?;
            Ok(format!("{:?}", result))
        }
        "delete_effects" => {
            let targets: Vec<(usize, usize)> = input["targets"]
                .as_array()
                .ok_or("Missing targets")?
                .iter()
                .map(|pair| {
                    let arr = pair.as_array().ok_or("Invalid target pair")?;
                    Ok((
                        arr[0].as_u64().ok_or("Invalid track index")? as usize,
                        arr[1].as_u64().ok_or("Invalid effect index")? as usize,
                    ))
                })
                .collect::<Result<_, String>>()?;
            let cmd = EditCommand::DeleteEffects {
                sequence_index: 0,
                targets,
            };
            let mut dispatcher = state.dispatcher.lock().unwrap();
            let mut show = state.show.lock().unwrap();
            dispatcher.execute(&mut show, cmd)?;
            Ok("Deleted".to_string())
        }
        "update_effect_param" => {
            let cmd = EditCommand::UpdateEffectParam {
                sequence_index: 0,
                track_index: input["track_index"].as_u64().ok_or("Missing track_index")? as usize,
                effect_index: input["effect_index"].as_u64().ok_or("Missing effect_index")? as usize,
                key: input["key"].as_str().ok_or("Missing key")?.to_string(),
                value: serde_json::from_value(input["value"].clone()).map_err(|e| e.to_string())?,
            };
            let mut dispatcher = state.dispatcher.lock().unwrap();
            let mut show = state.show.lock().unwrap();
            dispatcher.execute(&mut show, cmd)?;
            Ok("Updated".to_string())
        }
        "update_effect_time_range" => {
            let cmd = EditCommand::UpdateEffectTimeRange {
                sequence_index: 0,
                track_index: input["track_index"].as_u64().ok_or("Missing track_index")? as usize,
                effect_index: input["effect_index"].as_u64().ok_or("Missing effect_index")? as usize,
                start: input["start"].as_f64().ok_or("Missing start")?,
                end: input["end"].as_f64().ok_or("Missing end")?,
            };
            let mut dispatcher = state.dispatcher.lock().unwrap();
            let mut show = state.show.lock().unwrap();
            dispatcher.execute(&mut show, cmd)?;
            Ok("Updated".to_string())
        }
        "add_track" => {
            let fixture_id = input["fixture_id"].as_u64().ok_or("Missing fixture_id")? as u32;
            let blend_mode = input.get("blend_mode")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(crate::model::BlendMode::Override);
            let cmd = EditCommand::AddTrack {
                sequence_index: 0,
                name: input["name"].as_str().ok_or("Missing name")?.to_string(),
                target: crate::model::EffectTarget::Fixtures(vec![crate::model::FixtureId(fixture_id)]),
                blend_mode,
            };
            let mut dispatcher = state.dispatcher.lock().unwrap();
            let mut show = state.show.lock().unwrap();
            let result = dispatcher.execute(&mut show, cmd)?;
            Ok(format!("{:?}", result))
        }
        "play" => {
            state.playback.lock().unwrap().playing = true;
            Ok("Playing".to_string())
        }
        "pause" => {
            state.playback.lock().unwrap().playing = false;
            Ok("Paused".to_string())
        }
        "seek" => {
            let time = input["time"].as_f64().ok_or("Missing time")?;
            state.playback.lock().unwrap().current_time = time.max(0.0);
            Ok(format!("Seeked to {:.1}s", time))
        }
        "undo" => {
            let mut dispatcher = state.dispatcher.lock().unwrap();
            let mut show = state.show.lock().unwrap();
            let desc = dispatcher.undo(&mut show)?;
            Ok(format!("Undone: {}", desc))
        }
        "redo" => {
            let mut dispatcher = state.dispatcher.lock().unwrap();
            let mut show = state.show.lock().unwrap();
            let desc = dispatcher.redo(&mut show)?;
            Ok(format!("Redone: {}", desc))
        }
        "get_show" => {
            let show = state.show.lock().unwrap();
            Ok(serde_json::to_string_pretty(&*show).unwrap_or_default())
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

// ── Chat manager ─────────────────────────────────────────────────

#[derive(Default)]
pub struct ChatManager {
    history: Vec<ChatMessage>,
    cancelled: bool,
}

impl ChatManager {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            cancelled: false,
        }
    }

    pub fn history_for_display(&self) -> Vec<ChatHistoryEntry> {
        let mut entries = Vec::new();
        for msg in &self.history {
            match &msg.content {
                ChatContent::Text(text) => {
                    entries.push(ChatHistoryEntry {
                        role: msg.role.clone(),
                        text: text.clone(),
                        tool_name: None,
                    });
                }
                ChatContent::Blocks(blocks) => {
                    for block in blocks {
                        match block {
                            ContentBlock::Text { text } => {
                                entries.push(ChatHistoryEntry {
                                    role: msg.role.clone(),
                                    text: text.clone(),
                                    tool_name: None,
                                });
                            }
                            ContentBlock::ToolUse { name, .. } => {
                                entries.push(ChatHistoryEntry {
                                    role: "assistant".to_string(),
                                    text: format!("Calling {}...", name),
                                    tool_name: Some(name.clone()),
                                });
                            }
                            ContentBlock::ToolResult { content, .. } => {
                                entries.push(ChatHistoryEntry {
                                    role: "tool".to_string(),
                                    text: content.clone(),
                                    tool_name: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        entries
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Send a message and run the agentic loop. Uses ChatEmitter for event output.
    pub async fn send_message(
        state: Arc<AppState>,
        emitter: &dyn ChatEmitter,
        message: String,
    ) -> Result<(), String> {
        let api_key = {
            let settings = state.settings.lock().unwrap();
            settings
                .as_ref()
                .and_then(|s| s.claude_api_key.clone())
                .ok_or("No Claude API key configured. Set it in Settings.")?
        };

        // Add user message
        {
            let mut chat = state.chat.lock().unwrap();
            chat.cancelled = false;
            chat.history.push(ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(message),
            });
        }

        let client = reqwest::Client::new();
        let max_iterations = 10;

        for _ in 0..max_iterations {
            // Check cancellation
            if state.chat.lock().unwrap().cancelled {
                return Ok(());
            }

            let system_prompt = build_system_prompt(&state);
            let messages: Vec<Value> = {
                let chat = state.chat.lock().unwrap();
                chat.history
                    .iter()
                    .map(|m| serde_json::to_value(m).unwrap())
                    .collect()
            };

            let body = serde_json::json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 4096,
                "system": system_prompt,
                "tools": tool_definitions(),
                "messages": messages,
            });

            emitter.emit_thinking(true);

            let response = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("API request failed: {}", e))?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                return Err(format!("API error {}: {}", status, text));
            }

            let response_json: Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            let stop_reason = response_json["stop_reason"].as_str().unwrap_or("");
            let content_blocks = response_json["content"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            // Process response blocks
            let mut has_tool_use = false;
            let mut assistant_blocks = Vec::new();
            let mut tool_results = Vec::new();

            for block in &content_blocks {
                let block_type = block["type"].as_str().unwrap_or("");
                match block_type {
                    "text" => {
                        let text = block["text"].as_str().unwrap_or("").to_string();
                        emitter.emit_token(&text);
                        assistant_blocks.push(ContentBlock::Text { text });
                    }
                    "tool_use" => {
                        has_tool_use = true;
                        let id = block["id"].as_str().unwrap_or("").to_string();
                        let name = block["name"].as_str().unwrap_or("").to_string();
                        let input = block["input"].clone();

                        emitter.emit_tool_call(&name);

                        assistant_blocks.push(ContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        });

                        // Execute the tool
                        let result = execute_tool(&state, &name, &input)
                            .unwrap_or_else(|e| format!("Error: {}", e));

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
                let mut chat = state.chat.lock().unwrap();
                chat.history.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: ChatContent::Blocks(assistant_blocks),
                });

                // If there were tool uses, add tool results
                if !tool_results.is_empty() {
                    chat.history.push(ChatMessage {
                        role: "user".to_string(),
                        content: ChatContent::Blocks(tool_results),
                    });
                }
            }

            // If no tool use, we're done
            if !has_tool_use || stop_reason == "end_turn" {
                break;
            }
        }

        emitter.emit_complete();
        Ok(())
    }

    /// Get the last assistant text response (useful for API/CLI returning results).
    pub fn last_assistant_text(&self) -> Option<String> {
        for msg in self.history.iter().rev() {
            if msg.role == "assistant" {
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
