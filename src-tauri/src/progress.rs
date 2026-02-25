use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

#[derive(Clone, Serialize, TS)]
#[ts(export)]
pub struct ProgressEvent {
    pub operation: String,
    pub phase: String,
    pub progress: f64,
    pub detail: Option<String>,
}

pub fn emit_progress(app: &AppHandle, op: &str, phase: &str, progress: f64, detail: Option<&str>) {
    let event = ProgressEvent {
        operation: op.to_string(),
        phase: phase.to_string(),
        progress,
        detail: detail.map(ToString::to_string),
    };
    let _ = app.emit(crate::events::PROGRESS, &event);
}
