use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[cfg(feature = "tauri-app")]
use ts_rs::TS;

// ── Top-level analysis container ──────────────────────────────────

/// Complete audio analysis results. Each field is optional so analysis
/// can be run incrementally (e.g., beats only, or stems + lyrics).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct AudioAnalysis {
    /// Which features were successfully analyzed
    pub features: AnalysisFeatures,
    pub beats: Option<BeatAnalysis>,
    pub structure: Option<StructureAnalysis>,
    pub stems: Option<StemAnalysis>,
    pub lyrics: Option<LyricsAnalysis>,
    pub mood: Option<MoodAnalysis>,
    pub harmony: Option<HarmonyAnalysis>,
    pub low_level: Option<LowLevelFeatures>,
    pub pitch: Option<PitchAnalysis>,
    pub drums: Option<DrumAnalysis>,
    pub vocal_presence: Option<VocalPresence>,
}

// ── Feature flags ─────────────────────────────────────────────────

/// Boolean flags for which analysis features to run or were completed.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
#[serde(default)]
pub struct AnalysisFeatures {
    pub beats: bool,
    pub structure: bool,
    pub stems: bool,
    pub lyrics: bool,
    pub mood: bool,
    pub harmony: bool,
    pub low_level: bool,
    pub pitch: bool,
    pub drums: bool,
    pub vocal_presence: bool,
}

impl Default for AnalysisFeatures {
    fn default() -> Self {
        Self {
            beats: true,
            structure: true,
            stems: true,
            lyrics: true,
            mood: true,
            harmony: true,
            low_level: true,
            pitch: true,
            drums: true,
            vocal_presence: true,
        }
    }
}

// ── Beat analysis ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct BeatAnalysis {
    /// Beat times in seconds
    pub beats: Vec<f64>,
    /// Downbeat times in seconds (first beat of each measure)
    pub downbeats: Vec<f64>,
    /// Estimated tempo in BPM
    pub tempo: f64,
    /// Time signature numerator (e.g., 4 for 4/4)
    pub time_signature: u32,
    /// Per-beat confidence values (0.0 - 1.0)
    pub beat_confidences: Vec<f64>,
    /// Overall tempo confidence (0.0 - 1.0)
    pub tempo_confidence: f64,
}

// ── Song structure ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct StructureAnalysis {
    pub sections: Vec<SongSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SongSection {
    /// Section label: "intro", "verse", "chorus", "bridge", "outro", etc.
    pub label: String,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
    /// Confidence (0.0 - 1.0)
    pub confidence: f64,
}

// ── Source separation (stems) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct StemAnalysis {
    /// Relative path to vocals stem WAV
    pub vocals: String,
    /// Relative path to drums stem WAV
    pub drums: String,
    /// Relative path to bass stem WAV
    pub bass: String,
    /// Relative path to other/accompaniment stem WAV
    pub other: String,
}

// ── Lyrics (speech-to-text) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct LyricsAnalysis {
    /// Word-level timestamps
    pub words: Vec<LyricWord>,
    /// Full transcription text
    pub full_text: String,
    /// Detected language code (e.g., "en")
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct LyricWord {
    pub word: String,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
    /// Confidence (0.0 - 1.0)
    pub confidence: f64,
}

// ── Mood / high-level descriptors ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct MoodAnalysis {
    /// Valence: negative (sad) to positive (happy), 0.0 - 1.0
    pub valence: f64,
    /// Arousal: calm to energetic, 0.0 - 1.0
    pub arousal: f64,
    /// Danceability, 0.0 - 1.0
    pub danceability: f64,
    /// Predicted genres with confidence scores
    pub genres: HashMap<String, f64>,
}

// ── Harmony (key + chords) ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct HarmonyAnalysis {
    /// Detected key (e.g., "C major", "A minor")
    pub key: String,
    /// Key detection confidence (0.0 - 1.0)
    pub key_confidence: f64,
    /// Chord progression over time
    pub chords: Vec<ChordEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct ChordEvent {
    /// Chord label (e.g., "Cmaj", "Am7", "N" for no chord)
    pub label: String,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
}

// ── Low-level features ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct LowLevelFeatures {
    /// RMS energy curve (one value per time step)
    pub rms: Vec<f64>,
    /// Spectral centroid curve (Hz, one per time step)
    pub spectral_centroid: Vec<f64>,
    /// Onset strength curve (one per time step)
    pub onset_strength: Vec<f64>,
    /// Time step between samples in seconds
    pub time_step: f64,
    /// Chromagram: 12 rows (C, C#, ..., B) x N time steps, flattened row-major
    pub chromagram: Vec<f64>,
    /// Number of time steps (columns in the chromagram)
    pub chromagram_length: u32,
}

// ── Pitch / note detection ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct PitchAnalysis {
    pub notes: Vec<NoteEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct NoteEvent {
    /// MIDI note number (0-127)
    pub midi_note: u32,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
    /// Velocity / amplitude (0.0 - 1.0)
    pub velocity: f64,
}

// ── Drum onset detection ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct DrumAnalysis {
    /// Onset times in seconds
    pub onsets: Vec<f64>,
    /// Onset strengths (0.0 - 1.0), parallel to `onsets`
    pub strengths: Vec<f64>,
}

// ── Vocal presence detection ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct VocalPresence {
    pub segments: Vec<VocalSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct VocalSegment {
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
}

// ── Python environment status ─────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct PythonEnvStatus {
    pub uv_available: bool,
    pub python_installed: bool,
    pub venv_exists: bool,
    pub deps_installed: bool,
    pub installed_models: Vec<String>,
    pub sidecar_running: bool,
    pub sidecar_port: u32,
    pub gpu_available: bool,
}
