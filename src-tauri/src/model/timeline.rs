use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
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

impl BlendMode {
    pub const fn all() -> &'static [BlendMode] {
        &[
            BlendMode::Override,
            BlendMode::Add,
            BlendMode::Multiply,
            BlendMode::Max,
            BlendMode::Alpha,
            BlendMode::Subtract,
            BlendMode::Min,
            BlendMode::Average,
            BlendMode::Screen,
            BlendMode::Mask,
            BlendMode::IntensityOverlay,
        ]
    }
}

/// All known effect parameter keys. Compile-time checked.
/// Built-in keys serialize as snake_case; `Custom` keys serialize as their raw string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
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
    /// Custom parameter key for DSL-defined effects.
    Custom(String),
}

/// Which direction a wipe effect sweeps across fixtures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum WipeDirection {
    #[default]
    Horizontal,
    Vertical,
    DiagonalUp,
    DiagonalDown,
    Burst,
    Circle,
    Diamond,
}

impl WipeDirection {
    pub const fn all() -> &'static [WipeDirection] {
        &[
            WipeDirection::Horizontal,
            WipeDirection::Vertical,
            WipeDirection::DiagonalUp,
            WipeDirection::DiagonalDown,
            WipeDirection::Burst,
            WipeDirection::Circle,
            WipeDirection::Diamond,
        ]
    }
}

/// How gradient colors are applied across time/space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ColorMode {
    Static,
    GradientPerPulse,
    GradientThroughEffect,
    GradientAcrossItems,
}

impl ColorMode {
    pub const fn all() -> &'static [ColorMode] {
        &[
            ColorMode::Static,
            ColorMode::GradientPerPulse,
            ColorMode::GradientThroughEffect,
            ColorMode::GradientAcrossItems,
        ]
    }
}

/// Type-safe parameter values for effects.
#[derive(Debug, Clone, Serialize, Deserialize, TS, JsonSchema)]
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
    WipeDirection(WipeDirection),
    /// A variant of a DSL-defined enum type.
    EnumVariant(String),
    /// A set of selected flags from a DSL-defined flags type.
    FlagSet(Vec<String>),
    /// Reference to a gradient in the sequence's gradient_library by name.
    GradientRef(String),
    /// Reference to a curve in the sequence's curve_library by name.
    CurveRef(String),
}

impl ParamValue {
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ParamValue::Float(v) => Some(*v),
            ParamValue::Int(v) => Some(f64::from(*v)),
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

    /// Extract a `WipeDirection`. Also accepts `ParamValue::Text` for backward
    /// compatibility with existing serialized data and import pipelines.
    pub fn as_wipe_direction(&self) -> Option<WipeDirection> {
        match self {
            ParamValue::WipeDirection(d) => Some(*d),
            ParamValue::Text(s) => crate::util::from_serde_str(s),
            _ => None,
        }
    }

    pub fn as_enum_variant(&self) -> Option<&str> {
        match self {
            ParamValue::EnumVariant(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_flag_set(&self) -> Option<&[String]> {
        match self {
            ParamValue::FlagSet(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_gradient_ref(&self) -> Option<&str> {
        match self {
            ParamValue::GradientRef(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_curve_ref(&self) -> Option<&str> {
        match self {
            ParamValue::CurveRef(v) => Some(v),
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
    ColorMode { options: Vec<String> },
    WipeDirection { options: Vec<String> },
    Text { options: Vec<String> },
    /// DSL-defined enum: exclusive selection (dropdown in UI).
    Enum { options: Vec<String> },
    /// DSL-defined flags: multi-select (checkboxes in UI).
    Flags { options: Vec<String> },
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
pub struct EffectParams(
    #[ts(as = "HashMap<String, ParamValue>")]
    HashMap<ParamKey, ParamValue>,
);

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

    #[allow(clippy::needless_pass_by_value)]
    pub fn get(&self, key: ParamKey) -> Option<&ParamValue> {
        self.0.get(&key)
    }

    pub fn inner(&self) -> &HashMap<ParamKey, ParamValue> {
        &self.0
    }

    /// Get a float param with a default fallback.
    pub fn float_or(&self, key: ParamKey, default: f64) -> f64 {
        self.get(key).and_then(ParamValue::as_float).unwrap_or(default)
    }

    /// Get a color param with a default fallback.
    pub fn color_or(&self, key: ParamKey, default: Color) -> Color {
        self.get(key).and_then(ParamValue::as_color).unwrap_or(default)
    }

    /// Get a color list param with a default fallback.
    pub fn color_list_or<'a>(&'a self, key: ParamKey, default: &'a [Color]) -> &'a [Color] {
        self.get(key)
            .and_then(ParamValue::as_color_list)
            .unwrap_or(default)
    }

    /// Get a bool param with a default fallback.
    pub fn bool_or(&self, key: ParamKey, default: bool) -> bool {
        self.get(key).and_then(ParamValue::as_bool).unwrap_or(default)
    }

    /// Get a curve param with a default fallback.
    pub fn curve_or<'a>(&'a self, key: ParamKey, default: &'a Curve) -> &'a Curve {
        self.get(key)
            .and_then(ParamValue::as_curve)
            .unwrap_or(default)
    }

    /// Get a color gradient param with a default fallback.
    pub fn gradient_or<'a>(&'a self, key: ParamKey, default: &'a ColorGradient) -> &'a ColorGradient {
        self.get(key)
            .and_then(ParamValue::as_color_gradient)
            .unwrap_or(default)
    }

    /// Get a color mode param with a default fallback.
    pub fn color_mode_or(&self, key: ParamKey, default: ColorMode) -> ColorMode {
        self.get(key)
            .and_then(ParamValue::as_color_mode)
            .unwrap_or(default)
    }

    /// Get a text param with a default fallback.
    pub fn text_or<'a>(&'a self, key: ParamKey, default: &'a str) -> &'a str {
        self.get(key)
            .and_then(|v| v.as_text())
            .unwrap_or(default)
    }

    /// Get a wipe direction param with a default fallback.
    /// Also accepts `ParamValue::Text` for backward compatibility.
    pub fn wipe_direction_or(&self, key: ParamKey, default: WipeDirection) -> WipeDirection {
        self.get(key)
            .and_then(ParamValue::as_wipe_direction)
            .unwrap_or(default)
    }

    /// Get an enum variant param with a default fallback.
    pub fn enum_or<'a>(&'a self, key: ParamKey, default: &'a str) -> &'a str {
        self.get(key)
            .and_then(ParamValue::as_enum_variant)
            .unwrap_or(default)
    }

    /// Get a flag set param with a default fallback.
    pub fn flag_set_or<'a>(&'a self, key: ParamKey, default: &'a [String]) -> &'a [String] {
        self.get(key)
            .and_then(ParamValue::as_flag_set)
            .unwrap_or(default)
    }

    /// Returns true if any parameter value is a library reference.
    pub fn has_refs(&self) -> bool {
        self.0.values().any(|v| matches!(v, ParamValue::GradientRef(_) | ParamValue::CurveRef(_)))
    }

    /// Clone the params, resolving any `GradientRef` / `CurveRef` into inline values.
    /// Unknown refs are left as-is (the effect will fall back to its default).
    pub fn resolve_refs(
        &self,
        gradient_lib: &HashMap<String, ColorGradient>,
        curve_lib: &HashMap<String, Curve>,
    ) -> Self {
        let resolved = self.0.iter().map(|(k, v)| {
            let new_v = match v {
                ParamValue::GradientRef(name) => gradient_lib
                    .get(name)
                    .map(|g| ParamValue::ColorGradient(g.clone())),
                ParamValue::CurveRef(name) => curve_lib
                    .get(name)
                    .map(|c| ParamValue::Curve(c.clone())),
                _ => None,
            };
            (k.clone(), new_v.unwrap_or_else(|| v.clone()))
        }).collect();
        Self(resolved)
    }

    /// Mutable iterator over all parameter values.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut ParamValue> {
        self.0.values_mut()
    }
}

/// Which effect type an instance uses.
/// Built-in effects are enum variants; DSL scripts use `Script(name)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
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
    /// A DSL-scripted effect. The string is the script name (key into Show::scripts).
    Script(String),
}

impl EffectKind {
    /// All built-in effect kinds (excludes Script).
    pub const fn all_builtin() -> &'static [EffectKind] {
        &[
            EffectKind::Solid,
            EffectKind::Chase,
            EffectKind::Rainbow,
            EffectKind::Strobe,
            EffectKind::Gradient,
            EffectKind::Twinkle,
            EffectKind::Fade,
            EffectKind::Wipe,
        ]
    }
}

/// A placed effect on the timeline. Fully describes what happens, when, and to what.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EffectInstance {
    pub kind: EffectKind,
    pub params: EffectParams,
    pub time_range: TimeRange,
    pub blend_mode: BlendMode,
    /// Opacity of this effect (0.0 = transparent, 1.0 = fully opaque).
    /// Values outside [0.0, 1.0] are safe: `Color::scale()` clamps the factor,
    /// and the evaluator uses opacity only via `scale()`.
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
    /// DSL effect scripts. Key = script name, Value = source code.
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    /// Reusable color gradients. Key = library item name.
    #[serde(default)]
    pub gradient_library: HashMap<String, ColorGradient>,
    /// Reusable curves. Key = library item name.
    #[serde(default)]
    pub curve_library: HashMap<String, Curve>,
}

impl Sequence {
    /// Validates and clamps sequence parameters to safe ranges.
    /// Duration and frame rate are forced to positive, finite values;
    /// NaN and non-positive values fall back to sensible defaults.
    ///
    /// Callers loading or creating sequences should chain `.validated()` to
    /// guarantee the invariants the evaluator depends on.
    #[must_use]
    pub fn validated(mut self) -> Self {
        if self.duration <= 0.0 || self.duration.is_nan() {
            self.duration = 30.0;
        }
        if self.frame_rate <= 0.0 || self.frame_rate.is_nan() {
            self.frame_rate = 30.0;
        }
        self
    }
}

// ── Display impls ──────────────────────────────────────────────────
// Used by describe.rs for stable, human-readable output (fed to the LLM).
// Prefer these over {:?} Debug formatting which is unstable across Rust versions.

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solid => f.write_str("Solid"),
            Self::Chase => f.write_str("Chase"),
            Self::Rainbow => f.write_str("Rainbow"),
            Self::Strobe => f.write_str("Strobe"),
            Self::Gradient => f.write_str("Gradient"),
            Self::Twinkle => f.write_str("Twinkle"),
            Self::Fade => f.write_str("Fade"),
            Self::Wipe => f.write_str("Wipe"),
            Self::Script(name) => write!(f, "Script({name})"),
        }
    }
}

impl fmt::Display for ParamKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Color => f.write_str("Color"),
            Self::Colors => f.write_str("Colors"),
            Self::Gradient => f.write_str("Gradient"),
            Self::MovementCurve => f.write_str("MovementCurve"),
            Self::PulseCurve => f.write_str("PulseCurve"),
            Self::IntensityCurve => f.write_str("IntensityCurve"),
            Self::ColorMode => f.write_str("ColorMode"),
            Self::Speed => f.write_str("Speed"),
            Self::PulseWidth => f.write_str("PulseWidth"),
            Self::BackgroundLevel => f.write_str("BackgroundLevel"),
            Self::Reverse => f.write_str("Reverse"),
            Self::Spread => f.write_str("Spread"),
            Self::Saturation => f.write_str("Saturation"),
            Self::Brightness => f.write_str("Brightness"),
            Self::Rate => f.write_str("Rate"),
            Self::DutyCycle => f.write_str("DutyCycle"),
            Self::Density => f.write_str("Density"),
            Self::Offset => f.write_str("Offset"),
            Self::Direction => f.write_str("Direction"),
            Self::CenterX => f.write_str("CenterX"),
            Self::CenterY => f.write_str("CenterY"),
            Self::PassCount => f.write_str("PassCount"),
            Self::WipeOn => f.write_str("WipeOn"),
            Self::Custom(name) => write!(f, "{name}"),
        }
    }
}

impl fmt::Display for ParamValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Float(v) => write!(f, "{v:.2}"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Color(c) => write!(f, "rgba({},{},{},{})", c.r, c.g, c.b, c.a),
            Self::ColorList(colors) => write!(f, "[{} colors]", colors.len()),
            Self::Text(s) => write!(f, "\"{s}\""),
            Self::Curve(c) => write!(f, "Curve({} pts)", c.points().len()),
            Self::ColorGradient(g) => write!(f, "Gradient({} stops)", g.stops().len()),
            Self::ColorMode(m) => write!(f, "{m:?}"),
            Self::WipeDirection(d) => write!(f, "{d:?}"),
            Self::EnumVariant(v) => write!(f, "{v}"),
            Self::FlagSet(flags) => write!(f, "[{}]", flags.join(", ")),
            Self::GradientRef(name) => write!(f, "GradientRef(\"{name}\")"),
            Self::CurveRef(name) => write!(f, "CurveRef(\"{name}\")"),
        }
    }
}
