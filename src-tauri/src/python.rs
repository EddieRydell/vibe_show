use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::AppError;
use crate::model::PythonEnvStatus;
use crate::paths;
use crate::progress::emit_progress;
use crate::state::{check_cancelled, AppState};

// ── Path helpers ──────────────────────────────────────────────────

/// Resolve the Tauri resource directory.
fn resource_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

// ── Environment status check ──────────────────────────────────────

/// Probe the filesystem and return current Python environment status.
pub async fn check_env_status(
    app_config_dir: &Path,
    app: &AppHandle,
    state: &AppState,
) -> PythonEnvStatus {
    let uv_path = paths::uv_binary_path(&resource_dir(app));
    let uv_available = uv_path.exists();

    let venv_dir = paths::python_env_dir(app_config_dir);
    let venv_exists = venv_dir.exists();

    let py_exe = paths::python_exe(app_config_dir);
    let python_installed = py_exe.exists();

    // Check if deps are installed by looking for a marker file
    let deps_installed = paths::deps_installed_marker(app_config_dir).exists();

    // Check for known model directories
    let models = paths::models_dir(app_config_dir);
    let mut installed_models = Vec::new();
    for model_name in &["htdemucs", "whisper-turbo", "allin1", "essentia"] {
        if models.join(model_name).exists() {
            installed_models.push((*model_name).to_string());
        }
    }

    let sidecar_port = state.python_port.load(Ordering::Relaxed);
    let sidecar_running = sidecar_port > 0;

    // GPU detection: check if nvidia-smi exists
    let gpu_available = tokio::process::Command::new("nvidia-smi")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success());

    PythonEnvStatus {
        uv_available,
        python_installed,
        venv_exists,
        deps_installed,
        installed_models,
        sidecar_running,
        sidecar_port: u32::from(sidecar_port),
        gpu_available,
    }
}

// ── Bootstrap Python environment ──────────────────────────────────

/// Install Python 3.12 via uv, create a venv, and install all dependencies.
/// Emits progress events throughout. Checks the `cancel_flag` between steps.
pub async fn bootstrap_python(
    app: &AppHandle,
    app_config_dir: &Path,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<(), AppError> {
    let res_dir = resource_dir(app);
    let uv = paths::uv_binary_path(&res_dir);
    if !uv.exists() {
        return Err(AppError::PythonError {
            message: format!("uv binary not found at {}", uv.display()),
        });
    }

    let venv_dir = paths::python_env_dir(app_config_dir);
    let reqs = paths::requirements_path(&res_dir);

    // Step 1: Install Python 3.12
    check_cancelled(cancel_flag, "python_setup")?;
    emit_progress(app, "python_setup", "Installing Python 3.12...", 0.1, None);
    let mut child = tokio::process::Command::new(&uv)
        .args(["python", "install", "3.12"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::PythonError {
            message: format!("Failed to run uv python install: {e}"),
        })?;

    let status = wait_or_cancel(&mut child, cancel_flag, "python_setup").await?;
    if !status.success() {
        return Err(AppError::PythonError {
            message: "uv python install 3.12 failed".into(),
        });
    }

    // Step 2: Create venv
    check_cancelled(cancel_flag, "python_setup")?;
    emit_progress(app, "python_setup", "Creating virtual environment...", 0.25, None);
    let mut child = tokio::process::Command::new(&uv)
        .args([
            "venv",
            &venv_dir.to_string_lossy(),
            "--python",
            "3.12",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::PythonError {
            message: format!("Failed to create venv: {e}"),
        })?;

    let status = wait_or_cancel(&mut child, cancel_flag, "python_setup").await?;
    if !status.success() {
        return Err(AppError::PythonError {
            message: "uv venv creation failed".into(),
        });
    }

    // Step 3: Install dependencies
    check_cancelled(cancel_flag, "python_setup")?;
    emit_progress(
        app,
        "python_setup",
        "Installing Python dependencies (this may take several minutes)...",
        0.35,
        None,
    );

    if reqs.exists() {
        let mut child = tokio::process::Command::new(&uv)
            .args([
                "pip",
                "install",
                "-r",
                &reqs.to_string_lossy(),
                "--python",
                &paths::python_exe(app_config_dir).to_string_lossy(),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AppError::PythonError {
                message: format!("Failed to install dependencies: {e}"),
            })?;

        let status = wait_or_cancel(&mut child, cancel_flag, "python_setup").await?;
        if !status.success() {
            return Err(AppError::PythonError {
                message: "pip install failed — check requirements.txt".into(),
            });
        }
    }

    // Write marker file
    let marker = paths::deps_installed_marker(app_config_dir);
    let _ = tokio::fs::write(&marker, "1").await;

    emit_progress(app, "python_setup", "Python environment ready", 1.0, None);
    Ok(())
}

/// Wait for a child process to finish, but kill it if the cancel flag is set.
async fn wait_or_cancel(
    child: &mut tokio::process::Child,
    cancel_flag: &AtomicBool,
    operation: &str,
) -> Result<std::process::ExitStatus, AppError> {
    tokio::select! {
        result = child.wait() => {
            result.map_err(|e| AppError::PythonError {
                message: format!("Process error: {e}"),
            })
        }
        () = crate::state::wait_for_cancel(cancel_flag) => {
            let _ = child.kill().await;
            Err(AppError::Cancelled { operation: operation.to_string() })
        }
    }
}

// ── Sidecar lifecycle ─────────────────────────────────────────────

/// Start the Python FastAPI sidecar. Returns the child process and the port it's listening on.
pub async fn start_sidecar(
    app_config_dir: &Path,
    app: &AppHandle,
) -> Result<(tokio::process::Child, u16), AppError> {
    let py = paths::python_exe(app_config_dir);
    if !py.exists() {
        return Err(AppError::PythonNotReady);
    }

    let script = paths::sidecar_script_path(&resource_dir(app));
    if !script.exists() {
        return Err(AppError::PythonError {
            message: format!("Sidecar script not found at {}", script.display()),
        });
    }

    // Find a free port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| AppError::PythonError {
        message: format!("Failed to find free port: {e}"),
    })?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::PythonError {
            message: format!("Failed to get port: {e}"),
        })?
        .port();
    drop(listener);

    let models = paths::models_dir(app_config_dir);
    let _ = std::fs::create_dir_all(&models);

    let mut child = tokio::process::Command::new(&py)
        .arg(&script)
        .arg("--port")
        .arg(port.to_string())
        .arg("--models-dir")
        .arg(models.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::PythonError {
            message: format!("Failed to spawn sidecar: {e}"),
        })?;

    // Read stderr in background to log Python output
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[Python] {line}");
            }
        });
    }

    // Wait for health check (up to 30 seconds)
    let client = reqwest::Client::new();
    let health_url = format!("http://127.0.0.1:{port}/health");
    let mut attempts = 0;
    loop {
        if attempts >= 60 {
            // Kill the process since it didn't start in time
            let _ = child.kill().await;
            return Err(AppError::PythonError {
                message: "Sidecar failed to start within 30 seconds".into(),
            });
        }

        // Check if the process has exited
        if let Ok(Some(exit_status)) = child.try_wait() {
            return Err(AppError::PythonError {
                message: format!("Sidecar process exited immediately with status {exit_status}"),
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

/// Gracefully stop the sidecar. POSTs to /shutdown, then waits, then kills.
pub async fn stop_sidecar(
    child: &mut tokio::process::Child,
    port: u16,
) -> Result<(), AppError> {
    if port > 0 {
        let client = reqwest::Client::new();
        let shutdown_url = format!("http://127.0.0.1:{port}/shutdown");
        // Best-effort shutdown request
        let _ = client
            .post(&shutdown_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;

        // Wait a bit for graceful shutdown
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    // Kill if still running
    let _ = child.kill().await;
    Ok(())
}

/// Ensure the sidecar is running. If already running, returns the port.
/// Otherwise starts it.
pub async fn ensure_sidecar(
    state: &Arc<AppState>,
    app: &AppHandle,
) -> Result<u16, AppError> {
    let current_port = state.python_port.load(Ordering::Relaxed);
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
        state.python_port.store(0, Ordering::Relaxed);
    }

    let (child, port) = start_sidecar(&state.app_config_dir, app).await?;
    *state.python_sidecar.lock() = Some(child);
    state.python_port.store(port, Ordering::Relaxed);
    Ok(port)
}
