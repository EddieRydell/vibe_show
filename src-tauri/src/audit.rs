//! JSONL audit logging for chat sessions.
//!
//! Each chat interaction is logged as a series of JSONL entries in
//! `{app_config_dir}/chat-logs/YYYY-MM-DD.jsonl`. This lets you review
//! what the LLM tried, what tools it called, and where things went
//! wrong — across any provider.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

/// Unique identifier for a single chat session (one `send_message` call).
#[derive(Debug, Clone)]
pub struct SessionId(String);

impl SessionId {
    fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// The kind of event being logged.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    /// A new chat session started (user sent a message).
    SessionStart,
    /// An API request was sent to the LLM provider.
    ApiRequest,
    /// An API response was received from the LLM provider.
    ApiResponse,
    /// A tool was called by the LLM.
    ToolCall,
    /// A tool returned a result.
    ToolResult,
    /// The chat session completed successfully.
    SessionEnd,
    /// An error occurred during the session.
    Error,
}

/// A single audit log entry, serialized as one line of JSON.
#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub event: AuditEventKind,
    /// Iteration number within the agentic loop (0 = first request).
    pub iteration: u32,
    /// Event-specific payload. Structure depends on `event`:
    /// - `session_start`: `{ "user_message": "..." }`
    /// - `api_request`: `{ "message_count": N, "system_prompt_len": N }`
    /// - `api_response`: `{ "stop_reason": "...", "content_blocks": N, "raw": { ... } }`
    /// - `tool_call`: `{ "tool": "...", "input": { ... } }`
    /// - `tool_result`: `{ "tool": "...", "result": "...", "is_error": bool }`
    /// - `session_end`: `{ "total_iterations": N }`
    /// - `error`: `{ "message": "..." }`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Handle for logging events within a single chat session.
pub struct AuditLogger {
    session_id: SessionId,
    log_dir: PathBuf,
    provider: String,
    model: String,
    iteration: u32,
}

impl AuditLogger {
    /// Start a new audit session. Creates the log directory if needed.
    pub fn new(app_config_dir: &Path, provider: &str, model: &str) -> Self {
        let log_dir = app_config_dir.join("chat-logs");
        let _ = fs::create_dir_all(&log_dir);

        Self {
            session_id: SessionId::new(),
            log_dir,
            provider: provider.to_string(),
            model: model.to_string(),
            iteration: 0,
        }
    }

    /// Log the start of a chat session.
    pub fn log_session_start(&self, user_message: &str) {
        self.write_entry(AuditEventKind::SessionStart, Some(serde_json::json!({
            "user_message": user_message,
        })));
    }

    /// Log an outbound API request (we don't log the full messages to save space,
    /// just metadata).
    pub fn log_api_request(&self, message_count: usize, system_prompt_len: usize) {
        self.write_entry(AuditEventKind::ApiRequest, Some(serde_json::json!({
            "message_count": message_count,
            "system_prompt_len": system_prompt_len,
        })));
    }

    /// Log an API response.
    pub fn log_api_response(&self, stop_reason: &str, content_block_count: usize, raw: &Value) {
        self.write_entry(AuditEventKind::ApiResponse, Some(serde_json::json!({
            "stop_reason": stop_reason,
            "content_blocks": content_block_count,
            "raw": raw,
        })));
    }

    /// Log a tool call made by the LLM.
    pub fn log_tool_call(&self, tool_name: &str, input: &Value) {
        self.write_entry(AuditEventKind::ToolCall, Some(serde_json::json!({
            "tool": tool_name,
            "input": input,
        })));
    }

    /// Log the result of a tool execution.
    pub fn log_tool_result(&self, tool_name: &str, result: &str, is_error: bool) {
        self.write_entry(AuditEventKind::ToolResult, Some(serde_json::json!({
            "tool": tool_name,
            "result": result,
            "is_error": is_error,
        })));
    }

    /// Log successful session completion.
    pub fn log_session_end(&self) {
        self.write_entry(AuditEventKind::SessionEnd, Some(serde_json::json!({
            "total_iterations": self.iteration,
        })));
    }

    /// Log an error that occurred during the session.
    pub fn log_error(&self, message: &str) {
        self.write_entry(AuditEventKind::Error, Some(serde_json::json!({
            "message": message,
        })));
    }

    /// Advance the iteration counter (call at the start of each agentic loop iteration).
    pub fn next_iteration(&mut self) {
        self.iteration += 1;
    }

    /// Set iteration (0-indexed, called at loop start).
    pub fn set_iteration(&mut self, i: u32) {
        self.iteration = i;
    }

    /// Write a single JSONL entry to today's log file.
    fn write_entry(&self, event: AuditEventKind, data: Option<Value>) {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.as_str().to_string(),
            provider: self.provider.clone(),
            model: self.model.clone(),
            event,
            iteration: self.iteration,
            data,
        };

        let filename = Utc::now().format("%Y-%m-%d").to_string() + ".jsonl";
        let path = self.log_dir.join(filename);

        // Best-effort: don't let logging failures break chat
        if let Ok(json) = serde_json::to_string(&entry) {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                let _ = writeln!(file, "{json}");
            }
        }
    }
}
