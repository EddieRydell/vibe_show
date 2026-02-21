use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::error::AppError;
use crate::model::analysis::{AnalysisFeatures, AudioAnalysis};
use crate::progress::emit_progress;

// ── Path helpers ──────────────────────────────────────────────────

/// Path where analysis results are cached alongside the media file.
/// e.g., `media/song.mp3` → `media/song.mp3.analysis.json`
pub fn analysis_path(media_dir: &Path, filename: &str) -> PathBuf {
    media_dir.join(format!("{filename}.analysis.json"))
}

/// Directory where Demucs stems are stored for a given media file.
/// e.g., `media/stems/song-mp3/`
pub fn stems_dir(media_dir: &Path, filename: &str) -> PathBuf {
    let slug = filename.replace('.', "-");
    media_dir.join("stems").join(slug)
}

// ── Disk I/O ──────────────────────────────────────────────────────

/// Load cached analysis results from disk.
pub fn load_analysis(path: &Path) -> Result<AudioAnalysis, AppError> {
    let data = std::fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| AppError::AnalysisError {
        message: format!("Failed to parse analysis JSON: {e}"),
    })
}

/// Save analysis results to disk as JSON.
pub fn save_analysis(path: &Path, analysis: &AudioAnalysis) -> Result<(), AppError> {
    let data = serde_json::to_string_pretty(analysis).map_err(|e| AppError::AnalysisError {
        message: format!("Failed to serialize analysis: {e}"),
    })?;
    std::fs::write(path, data)?;
    Ok(())
}

// ── HTTP client to sidecar ────────────────────────────────────────

/// Run audio analysis by POSTing to the Python sidecar and streaming SSE progress.
///
/// The sidecar returns SSE events during processing, with a final `result` event
/// containing the full `AudioAnalysis` JSON.
#[allow(clippy::too_many_arguments)]
pub async fn run_analysis(
    app: &AppHandle,
    port: u16,
    audio_path: &Path,
    output_dir: &Path,
    features: &AnalysisFeatures,
    models_dir: &Path,
    use_gpu: bool,
) -> Result<AudioAnalysis, AppError> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}/analyze");

    let body = serde_json::json!({
        "audio_path": audio_path.to_string_lossy(),
        "output_dir": output_dir.to_string_lossy(),
        "features": features,
        "models_dir": models_dir.to_string_lossy(),
        "gpu": use_gpu,
    });

    let response = client
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(3600)) // 1 hour timeout for long analyses
        .send()
        .await
        .map_err(|e| AppError::PythonError {
            message: format!("Failed to connect to sidecar: {e}"),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::PythonError {
            message: format!("Sidecar returned {status}: {text}"),
        });
    }

    // Read the SSE stream
    let text = response.text().await.map_err(|e| AppError::PythonError {
        message: format!("Failed to read response: {e}"),
    })?;

    // Parse SSE events from the response body
    let mut result: Option<AudioAnalysis> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            // Try to parse as progress event first
            if let Ok(progress) = serde_json::from_str::<SseProgressEvent>(data) {
                emit_progress(
                    app,
                    "analysis",
                    &progress.phase,
                    progress.progress,
                    progress.detail.as_deref(),
                );
            }
            // Try to parse as final result
            else if let Ok(analysis) = serde_json::from_str::<AudioAnalysis>(data) {
                result = Some(analysis);
            }
        }
    }

    result.ok_or_else(|| AppError::AnalysisError {
        message: "Sidecar did not return analysis results".into(),
    })
}

/// SSE progress event from the sidecar.
#[derive(serde::Deserialize)]
struct SseProgressEvent {
    phase: String,
    progress: f64,
    detail: Option<String>,
}
