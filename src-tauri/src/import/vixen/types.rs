use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::color::Color;

// ── Wizard types (TS-exported) ──────────────────────────────────────

/// Discovery result from scanning a Vixen directory.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct VixenDiscovery {
    pub vixen_dir: String,
    pub fixtures_found: usize,
    pub groups_found: usize,
    pub controllers_found: usize,
    pub preview_available: bool,
    pub preview_item_count: usize,
    /// Path to the file containing preview data (if found).
    pub preview_file_path: Option<String>,
    pub sequences: Vec<VixenSequenceInfo>,
    pub media_files: Vec<VixenMediaInfo>,
}

/// Info about a discovered Vixen sequence file.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct VixenSequenceInfo {
    pub filename: String,
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
}

/// Info about a discovered Vixen media file.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct VixenMediaInfo {
    pub filename: String,
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
}

/// What the user selected for import (sent from frontend).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct VixenImportConfig {
    pub vixen_dir: String,
    pub profile_name: String,
    pub import_controllers: bool,
    pub import_layout: bool,
    /// Optional user-provided path to the file containing preview/layout data.
    /// When set, overrides auto-detection in `find_preview_file`.
    pub preview_file_override: Option<String>,
    pub sequence_paths: Vec<String>,
    pub media_filenames: Vec<String>,
}

/// Result returned after full import.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct VixenImportResult {
    pub profile_slug: String,
    pub fixtures_imported: usize,
    pub groups_imported: usize,
    pub controllers_imported: usize,
    pub layout_items_imported: usize,
    pub sequences_imported: usize,
    pub media_imported: usize,
    pub warnings: Vec<String>,
}

// ── Internal types ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(super) struct VixenNode {
    pub name: String,
    pub guid: String,
    pub children_guids: Vec<String>,
    pub channel_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct VixenEffect {
    pub type_name: String,
    pub start_time: f64,
    pub duration: f64,
    pub target_node_guids: Vec<String>,
    pub color: Option<Color>,
    /// `ChaseMovement` / `MovementCurve` (position over time for Chase/Wipe effects)
    pub movement_curve: Option<Vec<(f64, f64)>>,
    /// `PulseCurve` (intensity envelope per pulse for Chase/Spin effects)
    pub pulse_curve: Option<Vec<(f64, f64)>>,
    /// `LevelCurve` / `IntensityCurve` / `DissolveCurve` / etc. (brightness envelope)
    pub intensity_curve: Option<Vec<(f64, f64)>>,
    pub gradient_colors: Option<Vec<(f64, Color)>>,
    pub color_handling: Option<String>,
    pub level: Option<f64>,
    /// Spin-specific: number of revolutions over the effect duration
    pub revolution_count: Option<f64>,
    /// Spin-specific: pulse width as percentage of revolution (0-100)
    pub pulse_percentage: Option<f64>,
    /// Spin-specific: pulse time in milliseconds (used when PulseLengthFormat=FixedTime)
    #[allow(dead_code)]
    pub pulse_time_ms: Option<f64>,
    /// Spin-specific: whether the spin direction is reversed
    pub reverse_spin: Option<bool>,
    /// Direction for Wipe/Butterfly/etc. (e.g. "Up", "Down", "Left", "Right")
    pub direction: Option<String>,
}

/// Which kind of curve a Vixen data model entry contains.
/// Replaces ad-hoc string routing ("movement"/"pulse"/"intensity").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CurveKind {
    Movement,
    Pulse,
    Intensity,
}
