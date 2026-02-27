use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::Value;
use tower_http::cors::CorsLayer;

use crate::chat;
use crate::registry::{catalog, CommandOutput, CommandResult};
use crate::state::AppState;

// ── Response types ───────────────────────────────────────────────

#[derive(Serialize)]
struct ApiOk<T: Serialize> {
    ok: bool,
    data: T,
}

#[derive(Serialize)]
struct ApiErr {
    ok: bool,
    error: String,
}

fn ok_json<T: Serialize>(data: T) -> impl IntoResponse {
    Json(ApiOk { ok: true, data })
}

fn err_json(status: StatusCode, msg: String) -> impl IntoResponse {
    (status, Json(ApiErr { ok: false, error: msg }))
}

// ── API command response ─────────────────────────────────────────

/// Full typed response from command execution. Includes the human-readable
/// `message`, the typed `result` (discriminated union the sidecar can narrow),
/// and an optional `data_file` path for large payloads.
///
/// Exported via ts-rs so the sidecar imports the generated type — no manual
/// mirroring, no drift.
#[derive(Serialize)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CommandResponse {
    pub message: String,
    pub result: CommandResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_file: Option<String>,
}

// ── Scratch file helpers ─────────────────────────────────────────

/// Threshold in bytes — data larger than this gets written to a scratch file.
const SCRATCH_THRESHOLD: usize = 500;

/// Write a JSON value to a scratch file for agent file-based exploration.
/// Returns the absolute path to the written file, or `None` on failure.
fn write_scratch_file(state: &AppState, filename: &str, data: &Value) -> Option<String> {
    let data_dir = crate::state::get_data_dir(state).ok()?;
    let dir = crate::paths::scratch_dir(&data_dir);
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(filename);
    let json = serde_json::to_string_pretty(data).ok()?;
    crate::project::atomic_write(&path, json.as_bytes()).ok()?;
    Some(path.to_string_lossy().to_string())
}

/// Build an `CommandResponse` from a `CommandOutput`, writing large
/// payloads to a scratch file when they exceed the threshold.
fn build_response(state: &AppState, name: &str, output: CommandOutput) -> CommandResponse {
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

    CommandResponse {
        message: output.message,
        result: output.result,
        data_file,
    }
}

// ── Handlers ─────────────────────────────────────────────────────

async fn post_tool(
    Extension(state): Extension<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    match chat::execute_tool_api(&state, &name, &body) {
        Ok(output) => ok_json(build_response(&state, &name, output)).into_response(),
        Err(e) => err_json(StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn get_tools(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    let _lock = state.show.lock(); // consistent snapshot
    ok_json(catalog::to_json_schema())
}

async fn post_batch(
    Extension(state): Extension<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    match chat::execute_tool_api(&state, "batch_edit", &body) {
        Ok(output) => ok_json(build_response(&state, "batch_edit", output)).into_response(),
        Err(e) => err_json(StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn get_show(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    match chat::execute_tool_api(&state, "get_show", &serde_json::json!({})) {
        Ok(output) => ok_json(build_response(&state, "get_show", output)).into_response(),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_playback(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    match chat::execute_tool_api(&state, "get_playback", &serde_json::json!({})) {
        Ok(output) => ok_json(build_response(&state, "get_playback", output)).into_response(),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_analysis_summary(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    match chat::execute_tool_api(&state, "get_analysis_summary", &serde_json::json!({})) {
        Ok(output) => {
            ok_json(build_response(&state, "get_analysis_summary", output)).into_response()
        }
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ── Server startup ───────────────────────────────────────────────

/// Start the internal HTTP API on a random port. Returns the port.
pub async fn start_api_server(state: Arc<AppState>) -> Result<u16, String> {
    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/api/tools/{name}", post(post_tool))
        .route("/api/tools", get(get_tools))
        .route("/api/batch", post(post_batch))
        .route("/api/show", get(get_show))
        .route("/api/playback", get(get_playback))
        .route("/api/analysis/summary", get(get_analysis_summary))
        .layer(cors)
        .layer(Extension(state));

    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind API server: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get API server port: {e}"))?
        .port();

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("[VibeLights] API server error: {e}");
        }
    });

    Ok(port)
}
