use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

use super::color::Color;
use super::color_gradient::ColorGradient;
use super::curve::Curve;
use super::fixture::EffectTarget;

/// A time range within a sequence. Start must be < end, both in seconds.
/// Constructed via `TimeRange::new` which enforces this invariant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(try_from = "TimeRangeRaw")]
#[ts(export)]
pub struct TimeRange {
    start: f64,
    end: f64,
}

#[derive(Deserialize)]
struct TimeRangeRaw {
    start: f64,
    end: f64,
}

impl TryFrom<TimeRangeRaw> for TimeRange {
    type Error = String;
    fn try_from(raw: TimeRangeRaw) -> Result<Self, String> {
        TimeRange::new(raw.start, raw.end)
            .ok_or_else(|| format!("Invalid TimeRange: start={}, end={}", raw.start, raw.end))
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
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
    /// Saturating subtraction per channel.
    Subtract,
    /// Per-channel minimum.
    Min,
    /// Per-channel average.
    Average,
    /// Screen blend (complement of multiply).
    Screen,
    /// Where foreground is non-black, output black; else preserve background.
    Mask,
    /// Scale background brightness by foreground luminance.
    IntensityOverlay,
}

/// All known effect parameter keys. Compile-time checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ParamKey {
    Color,
    Colors,
    Gradient,
    MovementCurve,
    PulseCurve,
    IntensityCurve,
    ColorMode,
    Speed,
    PulseWidth,
    BackgroundLevel,
    Reverse,
    Spread,
    Saturation,
    Brightness,
    Rate,
    DutyCycle,
    Density,
    Offset,
    Direction,
    CenterX,
    CenterY,
    PassCount,
    WipeOn,
}

/// How gradient colors are applied across time/space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ColorMode {
    Static,
    GradientPerPulse,
    GradientThroughEffect,
    GradientAcrossItems,
}

/// Type-safe parameter values for effects.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ParamValue {
    Float(f64),
    Int(i32),
    Bool(bool),
    Color(Color),
    ColorList(Vec<Color>),
    Text(String),
    Curve(Curve),
    ColorGradient(ColorGradient),
    ColorMode(ColorMode),
}

impl ParamValue {
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ParamValue::Float(v) => Some(*v),
            ParamValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i32> {
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

    pub fn as_curve(&self) -> Option<&Curve> {
        match self {
            ParamValue::Curve(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_color_gradient(&self) -> Option<&ColorGradient> {
        match self {
            ParamValue::ColorGradient(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_color_mode(&self) -> Option<ColorMode> {
        match self {
            ParamValue::ColorMode(v) => Some(*v),
            _ => None,
        }
    }
}

/// Describes the type and constraints for an effect parameter, used to drive UI generation.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ParamType {
    Float { min: f64, max: f64, step: f64 },
    Int { min: i32, max: i32 },
    Bool,
    Color,
    ColorList { min_colors: usize, max_colors: usize },
    Curve,
    ColorGradient { min_stops: usize, max_stops: usize },
    ColorMode,
    Text { options: Vec<String> },
}

/// Schema entry for one effect parameter: key, label, type constraints, and default value.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ParamSchema {
    pub key: ParamKey,
    pub label: String,
    pub param_type: ParamType,
    pub default: ParamValue,
}

/// Named, typed parameters for an effect instance.
/// Serializes as a flat JSON object (transparent over the inner HashMap).
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct EffectParams(HashMap<ParamKey, ParamValue>);

impl EffectParams {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn set(mut self, key: ParamKey, value: ParamValue) -> Self {
        self.0.insert(key, value);
        self
    }

    pub fn set_mut(&mut self, key: ParamKey, value: ParamValue) {
        self.0.insert(key, value);
    }

    pub fn get(&self, key: ParamKey) -> Option<&ParamValue> {
        self.0.get(&key)
    }

    /// Get a float param with a default fallback.
    pub fn float_or(&self, key: ParamKey, default: f64) -> f64 {
        self.get(key).and_then(|v| v.as_float()).unwrap_or(default)
    }

    /// Get a color param with a default fallback.
    pub fn color_or(&self, key: ParamKey, default: Color) -> Color {
        self.get(key).and_then(|v| v.as_color()).unwrap_or(default)
    }

    /// Get a color list param with a default fallback.
    pub fn color_list_or<'a>(&'a self, key: ParamKey, default: &'a [Color]) -> &'a [Color] {
        self.get(key)
            .and_then(|v| v.as_color_list())
            .unwrap_or(default)
    }

    /// Get a bool param with a default fallback.
    pub fn bool_or(&self, key: ParamKey, default: bool) -> bool {
        self.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    /// Get a curve param with a default fallback.
    pub fn curve_or<'a>(&'a self, key: ParamKey, default: &'a Curve) -> &'a Curve {
        self.get(key)
            .and_then(|v| v.as_curve())
            .unwrap_or(default)
    }

    /// Get a color gradient param with a default fallback.
    pub fn gradient_or<'a>(&'a self, key: ParamKey, default: &'a ColorGradient) -> &'a ColorGradient {
        self.get(key)
            .and_then(|v| v.as_color_gradient())
            .unwrap_or(default)
    }

    /// Get a color mode param with a default fallback.
    pub fn color_mode_or(&self, key: ParamKey, default: ColorMode) -> ColorMode {
        self.get(key)
            .and_then(|v| v.as_color_mode())
            .unwrap_or(default)
    }

    /// Get a text param with a default fallback.
    pub fn text_or<'a>(&'a self, key: ParamKey, default: &'a str) -> &'a str {
        self.get(key)
            .and_then(|v| v.as_text())
            .unwrap_or(default)
    }
}

/// Which built-in effect type an instance uses.
/// Future: this will be extended with Custom(String) for user-defined effects.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum EffectKind {
    Solid,
    Chase,
    Rainbow,
    Strobe,
    Gradient,
    Twinkle,
    Fade,
    Wipe,
}

/// A placed effect on the timeline. Fully describes what happens, when, and to what.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EffectInstance {
    pub kind: EffectKind,
    pub params: EffectParams,
    pub time_range: TimeRange,
    pub blend_mode: BlendMode,
    /// Opacity multiplier (0.0 = transparent, 1.0 = fully opaque).
    pub opacity: f64,
}

/// A track targets a set of fixtures and contains a list of non-overlapping effect instances.
/// Tracks are layered bottom-to-top; blend mode lives on each EffectInstance.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Track {
    pub name: String,
    pub target: EffectTarget,
    pub effects: Vec<EffectInstance>,
}

/// A sequence is the top-level timeline container. One sequence per song/show.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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
