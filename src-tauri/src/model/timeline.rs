use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::color::Color;
use super::fixture::EffectTarget;

/// A time range within a sequence. Start must be < end, both in seconds.
/// Constructed via `TimeRange::new` which enforces this invariant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeRange {
    start: f64,
    end: f64,
}

impl TimeRange {
    /// Create a time range. Returns None if start >= end or either is negative.
    pub fn new(start: f64, end: f64) -> Option<Self> {
        if start >= 0.0 && end > start {
            Some(Self { start, end })
        } else {
            None
        }
    }

    pub fn start(&self) -> f64 {
        self.start
    }

    pub fn end(&self) -> f64 {
        self.end
    }

    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    /// Returns true if the given time falls within this range (inclusive start, exclusive end).
    pub fn contains(&self, t: f64) -> bool {
        t >= self.start && t < self.end
    }

    /// Normalize a time value to [0, 1] within this range. Clamps to bounds.
    pub fn normalize(&self, t: f64) -> f64 {
        ((t - self.start) / self.duration()).clamp(0.0, 1.0)
    }
}

/// How multiple effect layers combine their output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    /// Top layer fully replaces the layer below.
    Override,
    /// Additive blend (clamped at 255 per channel).
    Add,
    /// Multiplicative blend.
    Multiply,
    /// Per-channel maximum.
    Max,
    /// Alpha composite (foreground over background).
    Alpha,
}

/// Type-safe parameter values for effects. Extensible without stringly-typed nonsense.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    Color(Color),
    ColorList(Vec<Color>),
    /// A text value (for future DSL expressions, etc.)
    Text(String),
}

impl ParamValue {
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ParamValue::Float(v) => Some(*v),
            ParamValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            ParamValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<Color> {
        match self {
            ParamValue::Color(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_color_list(&self) -> Option<&[Color]> {
        match self {
            ParamValue::ColorList(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            ParamValue::Text(v) => Some(v),
            _ => None,
        }
    }
}

/// Describes the type and constraints for an effect parameter, used to drive UI generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamType {
    Float { min: f64, max: f64, step: f64 },
    Int { min: i64, max: i64 },
    Bool,
    Color,
    ColorList { min_colors: usize, max_colors: usize },
}

/// Schema entry for one effect parameter: key, label, type constraints, and default value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSchema {
    pub key: String,
    pub label: String,
    pub param_type: ParamType,
    pub default: ParamValue,
}

/// Named, typed parameters for an effect instance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EffectParams {
    params: HashMap<String, ParamValue>,
}

impl EffectParams {
    pub fn new() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    pub fn set(mut self, key: impl Into<String>, value: ParamValue) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    pub fn set_mut(&mut self, key: impl Into<String>, value: ParamValue) {
        self.params.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&ParamValue> {
        self.params.get(key)
    }

    /// Get a float param with a default fallback.
    pub fn float_or(&self, key: &str, default: f64) -> f64 {
        self.get(key).and_then(|v| v.as_float()).unwrap_or(default)
    }

    /// Get a color param with a default fallback.
    pub fn color_or(&self, key: &str, default: Color) -> Color {
        self.get(key).and_then(|v| v.as_color()).unwrap_or(default)
    }

    /// Get a color list param with a default fallback.
    pub fn color_list_or<'a>(&'a self, key: &str, default: &'a [Color]) -> &'a [Color] {
        self.get(key)
            .and_then(|v| v.as_color_list())
            .unwrap_or(default)
    }

    /// Get a bool param with a default fallback.
    pub fn bool_or(&self, key: &str, default: bool) -> bool {
        self.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }
}

/// Which built-in effect type an instance uses.
/// Future: this will be extended with Custom(String) for user-defined effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EffectKind {
    Solid,
    Chase,
    Rainbow,
    Strobe,
    Gradient,
    Twinkle,
}

/// A placed effect on the timeline. Fully describes what happens, when, and to what.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectInstance {
    pub kind: EffectKind,
    pub params: EffectParams,
    pub time_range: TimeRange,
}

/// A track targets a set of fixtures and contains a list of non-overlapping effect instances.
/// Tracks are layered bottom-to-top with a blend mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub target: EffectTarget,
    pub effects: Vec<EffectInstance>,
    pub blend_mode: BlendMode,
}

/// A sequence is the top-level timeline container. One sequence per song/show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence {
    pub name: String,
    /// Duration in seconds.
    pub duration: f64,
    /// Target frames per second for evaluation.
    pub frame_rate: f64,
    /// Audio file path, if any.
    pub audio_file: Option<String>,
    /// Tracks layered bottom (index 0) to top.
    pub tracks: Vec<Track>,
}
