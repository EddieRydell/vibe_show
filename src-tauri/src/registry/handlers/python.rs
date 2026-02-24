#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::error::AppError;
use crate::model::PythonEnvStatus;
use crate::progress::emit_progress;
use crate::python;
use crate::registry::{CommandOutput, CommandResult};
use crate::state::AppState;

#[cfg(feature = "tauri-app")]
pub async fn get_python_status(
    state: Arc<AppState>,
    app: Option<tauri::AppHandle>,
) -> Result<CommandOutput, AppError> {
    let status = if let Some(ref app_handle) = app {
        python::check_env_status(&state.app_config_dir, app_handle, &state).await
    } else {
        PythonEnvStatus {
            uv_available: false,
            python_installed: false,
            venv_exists: false,
            deps_installed: false,
            installed_models: Vec::new(),
            sidecar_running: state.python_port.load(Ordering::Relaxed) > 0,
            sidecar_port: 0,
            gpu_available: false,
        }
    };
    Ok(CommandOutput::new(
        format!("Python status: deps_installed={}", status.deps_installed),
        CommandResult::GetPythonStatus(status),
    ))
}

#[cfg(feature = "tauri-app")]
pub async fn setup_python_env(
    state: Arc<AppState>,
    app: Option<tauri::AppHandle>,
) -> Result<CommandOutput, AppError> {
    let app_handle = app.ok_or_else(|| AppError::ApiError {
        message: "AppHandle required for setup_python_env".into(),
    })?;
    let cancel_flag = state.cancellation.register("python_setup");

    let result = tokio::time::timeout(
        tokio::time::Duration::from_secs(900),
        python::bootstrap_python(&app_handle, &state.app_config_dir, &cancel_flag),
    )
    .await;

    state.cancellation.unregister("python_setup");

    match result {
        Ok(inner) => inner?,
        Err(_elapsed) => {
            emit_progress(&app_handle, "python_setup", "Setup timed out", 1.0, None);
            return Err(AppError::PythonError {
                message: "Python setup timed out after 15 minutes".into(),
            });
        }
    }

    Ok(CommandOutput::new(
        "Python environment set up.",
        CommandResult::SetupPythonEnv,
    ))
}

#[cfg(feature = "tauri-app")]
pub async fn start_python_sidecar(
    state: Arc<AppState>,
    app: Option<tauri::AppHandle>,
) -> Result<CommandOutput, AppError> {
    let app_handle = app.ok_or_else(|| AppError::ApiError {
        message: "AppHandle required for start_python_sidecar".into(),
    })?;
    let port = python::ensure_sidecar(&state, &app_handle).await?;
    Ok(CommandOutput::new(
        format!("Python sidecar started on port {port}."),
        CommandResult::StartPythonSidecar(port),
    ))
}

#[cfg(feature = "tauri-app")]
pub async fn stop_python_sidecar(
    state: Arc<AppState>,
    _app: Option<tauri::AppHandle>,
) -> Result<CommandOutput, AppError> {
    let port = state.python_port.load(Ordering::Relaxed);
    let mut child_opt = state.python_sidecar.lock().take();
    if let Some(ref mut child) = child_opt {
        python::stop_sidecar(child, port).await?;
    }
    state.python_port.store(0, Ordering::Relaxed);
    Ok(CommandOutput::new(
        "Python sidecar stopped.",
        CommandResult::StopPythonSidecar,
    ))
}
