use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tower_http::cors::CorsLayer;

use crate::chat;
use crate::registry::catalog;
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

// ── Tool result payload ──────────────────────────────────────────

#[derive(Serialize)]
struct ToolResultPayload {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_file: Option<String>,
}

// ── Handlers ─────────────────────────────────────────────────────

async fn post_tool(
    Extension(state): Extension<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    match chat::execute_tool_api(&state, &name, &body) {
        Ok(result) => ok_json(ToolResultPayload {
            message: result.message,
            data_file: result.data_file,
        })
        .into_response(),
        Err(e) => err_json(StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn get_tools(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    let _lock = state.show.lock(); // consistent snapshot
    ok_json(catalog::to_json_schema())
}

async fn post_batch(
    Extension(state): Extension<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    match chat::execute_tool_api(&state, "batch_edit", &body) {
        Ok(result) => ok_json(ToolResultPayload {
            message: result.message,
            data_file: result.data_file,
        })
        .into_response(),
        Err(e) => err_json(StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn get_show(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    match chat::execute_tool_api(&state, "get_show", &serde_json::json!({})) {
        Ok(result) => ok_json(ToolResultPayload {
            message: result.message,
            data_file: result.data_file,
        })
        .into_response(),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_playback(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    match chat::execute_tool_api(&state, "get_playback", &serde_json::json!({})) {
        Ok(result) => ok_json(ToolResultPayload {
            message: result.message,
            data_file: result.data_file,
        })
        .into_response(),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_analysis_summary(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    match chat::execute_tool_api(&state, "get_analysis_summary", &serde_json::json!({})) {
        Ok(result) => ok_json(ToolResultPayload {
            message: result.message,
            data_file: result.data_file,
        })
        .into_response(),
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
