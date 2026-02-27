use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::chat::{ChatEmitter, ChatHistoryEntry, ChatRole};
use crate::error::AppError;
use crate::state::AppState;

// ── Runtime detection ────────────────────────────────────────────

/// Find a JS runtime: prefer bun, fall back to node.
async fn find_runtime() -> Option<&'static str> {
    for cmd in &["bun", "node"] {
        let ok = tokio::process::Command::new(cmd)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success());
        if ok {
            return Some(cmd);
        }
    }
    None
}

// ── Sidecar lifecycle ───────────────────────────────────────────

/// Resolve the sidecar: returns (command, args, working_dir).
/// In production: compiled binary in resources (no runtime needed).
/// In dev: bun/node running the TS source or esbuild bundle.
fn resolve_sidecar(
    app: &AppHandle,
) -> Result<(String, Vec<String>, Option<std::path::PathBuf>), AppError> {
    #[cfg(target_os = "windows")]
    let binary_name = "agent-sidecar.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "agent-sidecar";

    // Dev builds: always run TS source via bun — no stale binaries.
    if cfg!(debug_assertions) {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(std::path::Path::to_path_buf)
            .ok_or_else(|| AppError::AgentError {
                message: "Cannot determine project root from CARGO_MANIFEST_DIR".into(),
            })?;
        let sidecar_dir = project_root.join("agent-sidecar");
        let src_entry = sidecar_dir.join("src").join("index.ts");

        if src_entry.exists() {
            return Ok((
                "runtime".to_string(),
                vec![src_entry.to_string_lossy().to_string()],
                Some(sidecar_dir),
            ));
        }

        return Err(AppError::AgentError {
            message: format!(
                "Agent sidecar source not found: {}",
                src_entry.display()
            ),
        });
    }

    // Production: use compiled binary from Tauri resources
    let resource_dir = app
        .path()
        .resource_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    let binary_path = resource_dir
        .join("resources")
        .join("agent-sidecar")
        .join(binary_name);

    if binary_path.exists() {
        return Ok((
            binary_path.to_string_lossy().to_string(),
            vec![],
            None,
        ));
    }

    Err(AppError::AgentError {
        message: format!(
            "Agent sidecar not found: {}",
            binary_path.display(),
        ),
    })
}

/// Start the agent sidecar. Returns the child process and the port it's listening on.
pub async fn start_agent_sidecar(
    state: &Arc<AppState>,
    app: &AppHandle,
) -> Result<(tokio::process::Child, u16), AppError> {
    let (command, args, working_dir) = resolve_sidecar(app)?;

    // If command is "runtime", we need a JS runtime (bun or node)
    let command = if command == "runtime" {
        find_runtime().await.ok_or_else(|| AppError::AgentError {
            message: "Neither bun nor Node.js found in PATH. Install one to use Agent mode."
                .into(),
        })?.to_string()
    } else {
        command
    };

    // Start the internal HTTP API server if not already running
    let api_port = state.api_port.load(Ordering::Relaxed);
    let api_port = if api_port == 0 {
        let port = crate::api::start_api_server(Arc::clone(state))
            .await
            .map_err(|e| AppError::AgentError {
                message: format!("Failed to start API server: {e}"),
            })?;
        state.api_port.store(port, Ordering::Relaxed);
        port
    } else {
        api_port
    };

    // Gather env vars
    let api_key = state
        .settings
        .lock()
        .as_ref()
        .and_then(|s| s.llm.api_key.clone());

    let data_dir = crate::state::get_data_dir(state)
        .map_err(|e| AppError::AgentError {
            message: format!("Cannot start agent: {e}"),
        })?
        .to_string_lossy()
        .to_string();

    let model = state
        .settings
        .lock()
        .as_ref()
        .and_then(|s| s.llm.model.clone());

    let mut cmd = tokio::process::Command::new(&command);
    for arg in &args {
        cmd.arg(arg);
    }

    // Set working directory (needed in dev so node_modules is found)
    if let Some(ref cwd) = working_dir {
        cmd.current_dir(cwd);
    }

    cmd.env("AGENT_PORT", "0") // Let OS pick a free port
        .env("VIBELIGHTS_PORT", api_port.to_string())
        .env("VIBELIGHTS_DATA_DIR", &data_dir);

    // Only pass API key / model if explicitly configured; otherwise let the
    // SDK use the user's existing Claude Code OAuth credentials from ~/.claude/
    if let Some(ref key) = api_key {
        cmd.env("ANTHROPIC_API_KEY", key);
    }
    if let Some(ref m) = model {
        cmd.env("VIBELIGHTS_MODEL", m);
    }

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::AgentError {
            message: format!("Failed to spawn agent sidecar: {e}"),
        })?;

    // Read stderr in background to log output
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[Agent] {line}");
            }
        });
    }

    // Read the port from the first line of stdout
    let stdout = child.stdout.take().ok_or_else(|| AppError::AgentError {
        message: "Failed to capture agent sidecar stdout".into(),
    })?;
    let mut stdout_reader = BufReader::new(stdout);
    let mut port_line = String::new();

    // Wait up to 15 seconds for the port line
    let port = tokio::time::timeout(
        tokio::time::Duration::from_secs(15),
        stdout_reader.read_line(&mut port_line),
    )
    .await
    .map_err(|_| AppError::AgentError {
        message: "Agent sidecar didn't print port within 15 seconds".into(),
    })?
    .map_err(|e| AppError::AgentError {
        message: format!("Failed to read port from agent sidecar: {e}"),
    })?;

    if port == 0 {
        // Check if process exited
        if let Ok(Some(exit_status)) = child.try_wait() {
            return Err(AppError::AgentError {
                message: format!("Agent sidecar exited immediately with status {exit_status}"),
            });
        }
        return Err(AppError::AgentError {
            message: "Agent sidecar didn't report a port".into(),
        });
    }

    let port: u16 = port_line.trim().parse().map_err(|_| AppError::AgentError {
        message: format!("Invalid port from agent sidecar: {}", port_line.trim()),
    })?;

    // Continue reading remaining stdout in background
    tokio::spawn(async move {
        let mut lines_reader = stdout_reader;
        let mut line = String::new();
        loop {
            line.clear();
            match lines_reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => eprintln!("[Agent] {}", line.trim_end()),
            }
        }
    });

    // Health check
    let client = reqwest::Client::new();
    let health_url = format!("http://127.0.0.1:{port}/health");
    let mut attempts = 0;
    loop {
        if attempts >= 30 {
            let _ = child.kill().await;
            return Err(AppError::AgentError {
                message: "Agent sidecar failed health check within 15 seconds".into(),
            });
        }
        if let Ok(Some(exit_status)) = child.try_wait() {
            return Err(AppError::AgentError {
                message: format!("Agent sidecar exited with status {exit_status}"),
            });
        }
        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => break,
            _ => {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                attempts += 1;
            }
        }
    }

    Ok((child, port))
}

/// Gracefully stop the agent sidecar.
pub async fn stop_agent_sidecar(
    child: &mut tokio::process::Child,
    port: u16,
) -> Result<(), AppError> {
    if port > 0 {
        let client = reqwest::Client::new();
        let shutdown_url = format!("http://127.0.0.1:{port}/shutdown");
        let _ = client
            .post(&shutdown_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
    let _ = child.kill().await;
    Ok(())
}

/// Ensure the agent sidecar is running. Returns the port.
pub async fn ensure_agent_sidecar(
    state: &Arc<AppState>,
    app: &AppHandle,
) -> Result<u16, AppError> {
    let current_port = state.agent_port.load(Ordering::Relaxed);
    if current_port > 0 {
        // Quick health check
        let client = reqwest::Client::new();
        let health_url = format!("http://127.0.0.1:{current_port}/health");
        if let Ok(resp) = client
            .get(&health_url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            if resp.status().is_success() {
                return Ok(current_port);
            }
        }
        // Sidecar died — clean up
        state.agent_port.store(0, Ordering::Relaxed);
    }

    let (child, port) = start_agent_sidecar(state, app).await?;
    *state.agent_sidecar.lock() = Some(child);
    state.agent_port.store(port, Ordering::Relaxed);
    Ok(port)
}

// ── SSE stream reader ───────────────────────────────────────────

/// Send a message to the agent sidecar and re-emit SSE events as Tauri events.
pub async fn send_message(
    state: &Arc<AppState>,
    app: &AppHandle,
    emitter: &dyn ChatEmitter,
    message: String,
    context: Option<&str>,
) -> Result<(), AppError> {
    let port = ensure_agent_sidecar(state, app).await?;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}/chat");

    let session_id = state.agent_session_id.lock().clone();

    let mut body = serde_json::json!({ "message": message });
    if let Some(sid) = &session_id {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("sessionId".to_string(), serde_json::json!(sid));
        }
    }
    if let Some(ctx) = context {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("context".to_string(), serde_json::json!(ctx));
        }
    }

    // Ensure there's an active conversation
    {
        let chats = state.agent_chats.lock();
        if chats.active_id.is_none() {
            drop(chats);
            crate::chat::new_agent_conversation(state);
        }
    }

    // Push user message to display history
    state.agent_display_messages.lock().push(ChatHistoryEntry {
        role: ChatRole::User,
        text: message.clone(),
    });

    // Update conversation title from first user message
    {
        let mut chats = state.agent_chats.lock();
        let active_id = chats.active_id.clone();
        if let Some(active_id) = active_id {
            if let Some(conv) = chats.conversations.iter_mut().find(|c| c.id == active_id) {
                if conv.title == "New conversation" {
                    let trimmed = message.trim();
                    conv.title = if trimmed.len() <= 60 {
                        trimmed.to_string()
                    } else {
                        let truncated: String = trimmed.chars().take(57).collect();
                        format!("{truncated}...")
                    };
                }
            }
        }
    }

    // From this point on, the user message is in memory — we must save on ALL exit paths
    // so that errors, cancellations, and crashes don't lose conversation history.

    emitter.emit_thinking(true);

    let result = stream_agent_response(state, emitter, &client, &url, &body).await;

    // Always persist, even on error — the user message and any partial response are valuable
    crate::chat::save_agent_chats(state);

    result
}

/// Inner streaming logic, separated so the caller can always save chats regardless of outcome.
async fn stream_agent_response(
    state: &Arc<AppState>,
    emitter: &dyn ChatEmitter,
    client: &reqwest::Client,
    url: &str,
    body: &serde_json::Value,
) -> Result<(), AppError> {
    let response = client
        .post(url)
        .json(body)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| {
            // If the sidecar crashed, clear the port
            state.agent_port.store(0, Ordering::Relaxed);
            AppError::AgentError {
                message: format!("Failed to connect to agent sidecar: {e}"),
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_else(|_| String::new());
        return Err(AppError::AgentError {
            message: format!("Agent sidecar error {status}: {text}"),
        });
    }

    // Read SSE stream from the response body
    let stream = response.bytes_stream();
    use futures_util::StreamExt;

    let mut event_name = String::new();
    let mut data_buf = String::new();
    let mut leftover = String::new();
    let mut assistant_text = String::new();

    tokio::pin!(stream);

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| AppError::AgentError {
            message: format!("Error reading agent stream: {e}"),
        })?;

        leftover.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete lines from the buffer
        while let Some(newline_pos) = leftover.find('\n') {
            let line: String = leftover.drain(..=newline_pos).collect();
            let line = line.trim_end_matches('\n').trim_end_matches('\r');

            if line.is_empty() {
                // Empty line = end of SSE event
                if !event_name.is_empty() && !data_buf.is_empty() {
                    process_sse_event(state, emitter, &event_name, &data_buf, &mut assistant_text);
                }
                event_name.clear();
                data_buf.clear();
                continue;
            }

            if let Some(name) = line.strip_prefix("event: ") {
                event_name = name.to_string();
            } else if let Some(data) = line.strip_prefix("data: ") {
                if !data_buf.is_empty() {
                    data_buf.push('\n');
                }
                data_buf.push_str(data);
            }
        }
    }

    // Process any remaining event
    if !event_name.is_empty() && !data_buf.is_empty() {
        process_sse_event(state, emitter, &event_name, &data_buf, &mut assistant_text);
    }

    // Push assistant message to display history
    if !assistant_text.trim().is_empty() {
        state.agent_display_messages.lock().push(ChatHistoryEntry {
            role: ChatRole::Assistant,
            text: assistant_text,
        });
    }

    Ok(())
}

fn process_sse_event(
    state: &Arc<AppState>,
    emitter: &dyn ChatEmitter,
    event: &str,
    data: &str,
    assistant_text: &mut String,
) {
    match event {
        "token" => {
            // Data is a JSON string
            if let Ok(text) = serde_json::from_str::<String>(data) {
                emitter.emit_token(&text);
                assistant_text.push_str(&text);
            }
        }
        "tool_call" => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                let id = val.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
                let tool = val.get("tool").and_then(serde_json::Value::as_str).unwrap_or("tool");
                emitter.emit_tool_call(id, tool);
            }
        }
        "tool_result" => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                let id = val.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
                let tool = val
                    .get("tool")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("tool");
                let result = val
                    .get("result")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                emitter.emit_tool_result(id, tool, result);
            }
        }
        "thinking" => {
            if let Ok(val) = serde_json::from_str::<bool>(data) {
                emitter.emit_thinking(val);
            }
        }
        "session_id" => {
            if let Ok(sid) = serde_json::from_str::<String>(data) {
                *state.agent_session_id.lock() = Some(sid);
            }
        }
        "complete" => {
            emitter.emit_complete();
        }
        "error" => {
            if let Ok(msg) = serde_json::from_str::<String>(data) {
                emitter.emit_error(&msg);
            }
            emitter.emit_complete();
        }
        _ => {}
    }
}

/// Cancel the in-flight agent query. Saves any partial conversation first.
pub async fn cancel_message(state: &Arc<AppState>) -> Result<(), AppError> {
    // Save whatever we have so far — partial responses are better than nothing
    crate::chat::save_agent_chats(state);

    let port = state.agent_port.load(Ordering::Relaxed);
    if port > 0 {
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{port}/cancel");
        let _ = client
            .post(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
    }
    Ok(())
}

/// Clear the agent session — archives the current conversation and starts a fresh one.
pub async fn clear_session(state: &Arc<AppState>) -> Result<(), AppError> {
    crate::chat::new_agent_conversation(state);
    let port = state.agent_port.load(Ordering::Relaxed);
    if port > 0 {
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{port}/clear");
        let _ = client
            .post(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
    }
    Ok(())
}
