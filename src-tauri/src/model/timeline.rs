use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use ts_rs::TS;

use super::color::Color;
use super::color_gradient::ColorGradient;
use super::curve::Curve;
use super::fixture::EffectTarget;
use super::motion_path::MotionPath;

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

const TIME_EPSILON: f64 = 1e-9;

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
    /// Uses a small epsilon tolerance to avoid single-frame gaps at effect boundaries
    /// caused by floating-point precision.
    pub fn contains(&self, t: f64) -> bool {
        t >= self.start - TIME_EPSILON && t < self.end + TIME_EPSILON
    }

    /// Raw normalization — may return values outside [0, 1].
    pub fn normalize_unclamped(&self, t: f64) -> f64 {
        (t - self.start) / self.duration()
    }

    /// Clamped normalization for effect evaluation.
    pub fn normalize(&self, t: f64) -> f64 {
        self.normalize_unclamped(t).clamp(0.0, 1.0)
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
/// Built-in keys serialize as their variant name; `Custom` keys serialize as their raw string.
/// Unknown strings deserialize as `Custom(s)` so script params round-trip through JSON.
#[derive(Debug, Clone, PartialEq, Eq, Hash, TS, JsonSchema)]
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

impl ParamKey {
    /// Parse a string into a `ParamKey`, falling back to `Custom` for unknown keys.
    fn from_string(s: &str) -> Self {
        match s {
            "Color" => Self::Color,
            "Colors" => Self::Colors,
            "Gradient" => Self::Gradient,
            "MovementCurve" => Self::MovementCurve,
            "PulseCurve" => Self::PulseCurve,
            "IntensityCurve" => Self::IntensityCurve,
            "ColorMode" => Self::ColorMode,
            "Speed" => Self::Speed,
            "PulseWidth" => Self::PulseWidth,
            "BackgroundLevel" => Self::BackgroundLevel,
            "Reverse" => Self::Reverse,
            "Spread" => Self::Spread,
            "Saturation" => Self::Saturation,
            "Brightness" => Self::Brightness,
            "Rate" => Self::Rate,
            "DutyCycle" => Self::DutyCycle,
            "Density" => Self::Density,
            "Offset" => Self::Offset,
            "Direction" => Self::Direction,
            "CenterX" => Self::CenterX,
            "CenterY" => Self::CenterY,
            "PassCount" => Self::PassCount,
            "WipeOn" => Self::WipeOn,
            other => Self::Custom(other.to_string()),
        }
    }
}

impl Serialize for ParamKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ParamKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_string(&s))
    }
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
    /// Reference to a motion path in the sequence's motion_paths by name.
    PathRef(String),
}

/// Generate a `ParamValue::as_*` accessor that copies a `Copy` inner value.
macro_rules! param_copy {
    ($method:ident, $variant:ident, $ty:ty) => {
        pub fn $method(&self) -> Option<$ty> {
            match self {
                ParamValue::$variant(v) => Some(*v),
                _ => None,
            }
        }
    };
}

/// Generate a `ParamValue::as_*` accessor that borrows an inner value as a slice.
macro_rules! param_slice {
    ($method:ident, $variant:ident, $elem:ty) => {
        pub fn $method(&self) -> Option<&[$elem]> {
            match self {
                ParamValue::$variant(v) => Some(v),
                _ => None,
            }
        }
    };
}

/// Generate a `ParamValue::as_*` accessor that borrows an inner value as `&str`.
macro_rules! param_str {
    ($method:ident, $variant:ident) => {
        pub fn $method(&self) -> Option<&str> {
            match self {
                ParamValue::$variant(v) => Some(v),
                _ => None,
            }
        }
    };
}

/// Generate a `ParamValue::as_*` accessor that borrows a reference to the inner value.
macro_rules! param_ref {
    ($method:ident, $variant:ident, $ty:ty) => {
        pub fn $method(&self) -> Option<&$ty> {
            match self {
                ParamValue::$variant(v) => Some(v),
                _ => None,
            }
        }
    };
}

impl ParamValue {
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ParamValue::Float(v) => Some(*v),
            ParamValue::Int(v) => Some(f64::from(*v)),
            _ => None,
        }
    }

    param_copy!(as_int, Int, i32);
    param_copy!(as_bool, Bool, bool);
    param_copy!(as_color, Color, Color);
    param_copy!(as_color_mode, ColorMode, ColorMode);

    param_slice!(as_color_list, ColorList, Color);
    param_slice!(as_flag_set, FlagSet, String);

    param_str!(as_text, Text);
    param_str!(as_enum_variant, EnumVariant);
    param_str!(as_gradient_ref, GradientRef);
    param_str!(as_curve_ref, CurveRef);
    param_str!(as_path_ref, PathRef);

    param_ref!(as_curve, Curve, Curve);
    param_ref!(as_color_gradient, ColorGradient, ColorGradient);

    /// Extract a `WipeDirection`. Also accepts `ParamValue::Text` for backward
    /// compatibility with existing serialized data and import pipelines.
    pub fn as_wipe_direction(&self) -> Option<WipeDirection> {
        match self {
            ParamValue::WipeDirection(d) => Some(*d),
            ParamValue::Text(s) => crate::util::from_serde_str(s),
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
    /// Motion path: dropdown of sequence motion paths.
    Path,
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

/// Generate an owned-value `*_or` accessor on `EffectParams`.
macro_rules! owned_or {
    ($method:ident, $accessor:ident, $ty:ty) => {
        pub fn $method(&self, key: ParamKey, default: $ty) -> $ty {
            self.get(key).and_then(ParamValue::$accessor).unwrap_or(default)
        }
    };
}

/// Generate a borrowed-value `*_or` accessor on `EffectParams`.
macro_rules! ref_or {
    ($method:ident, $accessor:ident, $ty:ty) => {
        pub fn $method<'a>(&'a self, key: ParamKey, default: &'a $ty) -> &'a $ty {
            self.get(key).and_then(ParamValue::$accessor).unwrap_or(default)
        }
    };
}

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

    owned_or!(float_or, as_float, f64);
    owned_or!(bool_or, as_bool, bool);
    owned_or!(color_or, as_color, Color);
    owned_or!(color_mode_or, as_color_mode, ColorMode);
    owned_or!(wipe_direction_or, as_wipe_direction, WipeDirection);

    ref_or!(color_list_or, as_color_list, [Color]);
    ref_or!(curve_or, as_curve, Curve);
    ref_or!(gradient_or, as_color_gradient, ColorGradient);
    ref_or!(flag_set_or, as_flag_set, [String]);

    pub fn text_or<'a>(&'a self, key: ParamKey, default: &'a str) -> &'a str {
        self.get(key).and_then(|v| v.as_text()).unwrap_or(default)
    }

    pub fn enum_or<'a>(&'a self, key: ParamKey, default: &'a str) -> &'a str {
        self.get(key).and_then(ParamValue::as_enum_variant).unwrap_or(default)
    }

    /// Returns true if any parameter value is a library reference.
    pub fn has_refs(&self) -> bool {
        self.0.values().any(|v| matches!(v, ParamValue::GradientRef(_) | ParamValue::CurveRef(_) | ParamValue::PathRef(_)))
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
    /// Named motion paths. Key = path name.
    #[serde(default)]
    pub motion_paths: HashMap<String, MotionPath>,
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
            Self::PathRef(name) => write!(f, "PathRef(\"{name}\")"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TimeRange boundary tests ──────────────────────────────────

    #[test]
    fn time_range_valid() {
        assert!(TimeRange::new(0.0, 1.0).is_some());
        assert!(TimeRange::new(0.0, 0.001).is_some());
        assert!(TimeRange::new(5.0, 10.0).is_some());
    }

    #[test]
    fn time_range_equal_start_end_is_none() {
        assert!(TimeRange::new(0.0, 0.0).is_none());
        assert!(TimeRange::new(5.0, 5.0).is_none());
    }

    #[test]
    fn time_range_reversed_is_none() {
        assert!(TimeRange::new(5.0, 1.0).is_none());
    }

    #[test]
    fn time_range_negative_start_is_none() {
        assert!(TimeRange::new(-1.0, 5.0).is_none());
    }

    #[test]
    fn time_range_both_negative_is_none() {
        assert!(TimeRange::new(-5.0, -1.0).is_none());
    }

    #[test]
    fn time_range_nan_is_none() {
        assert!(TimeRange::new(f64::NAN, 5.0).is_none());
        assert!(TimeRange::new(0.0, f64::NAN).is_none());
    }

    #[test]
    fn time_range_contains_boundaries() {
        let tr = TimeRange::new(1.0, 3.0).expect("valid range");
        assert!(tr.contains(1.0));
        assert!(tr.contains(2.0));
        // end is exclusive but with epsilon tolerance
        assert!(tr.contains(3.0));
        assert!(!tr.contains(0.0));
        assert!(!tr.contains(4.0));
    }

    #[test]
    fn time_range_normalize_boundaries() {
        let tr = TimeRange::new(2.0, 4.0).expect("valid range");
        let tol = 1e-10;
        assert!((tr.normalize(2.0) - 0.0).abs() < tol);
        assert!((tr.normalize(3.0) - 0.5).abs() < tol);
        assert!((tr.normalize(4.0) - 1.0).abs() < tol);
        // Outside range: clamped to [0, 1]
        assert!((tr.normalize(0.0) - 0.0).abs() < tol);
        assert!((tr.normalize(10.0) - 1.0).abs() < tol);
    }

    #[test]
    fn time_range_normalize_unclamped_outside() {
        let tr = TimeRange::new(2.0, 4.0).expect("valid range");
        assert!(tr.normalize_unclamped(0.0) < 0.0);
        assert!(tr.normalize_unclamped(6.0) > 1.0);
    }

    #[test]
    fn time_range_duration() {
        let tr = TimeRange::new(1.0, 3.5).expect("valid range");
        assert!((tr.duration() - 2.5).abs() < 1e-10);
    }

    // ── EffectParams boundary tests ──────────────────────────────

    #[test]
    fn effect_params_empty_returns_defaults() {
        let params = EffectParams::new();
        assert_eq!(params.float_or(ParamKey::Speed, 1.0), 1.0);
        assert_eq!(params.bool_or(ParamKey::Reverse, false), false);
        assert_eq!(params.color_or(ParamKey::Color, Color::WHITE), Color::WHITE);
    }

    #[test]
    fn effect_params_type_mismatch_returns_fallback() {
        let params = EffectParams::new().set(ParamKey::Speed, ParamValue::Bool(true));
        // Speed is stored as Bool, but requesting it as float should return fallback
        assert_eq!(params.float_or(ParamKey::Speed, 2.0), 2.0);
    }

    #[test]
    fn effect_params_correct_type_returns_value() {
        let params = EffectParams::new().set(ParamKey::Speed, ParamValue::Float(5.0));
        assert_eq!(params.float_or(ParamKey::Speed, 1.0), 5.0);
    }

    #[test]
    fn effect_params_int_coerces_to_float() {
        let params = EffectParams::new().set(ParamKey::Speed, ParamValue::Int(3));
        assert_eq!(params.float_or(ParamKey::Speed, 1.0), 3.0);
    }

    #[test]
    fn effect_params_has_refs() {
        let params = EffectParams::new()
            .set(ParamKey::Speed, ParamValue::Float(1.0));
        assert!(!params.has_refs());

        let params_with_ref = EffectParams::new()
            .set(ParamKey::Gradient, ParamValue::GradientRef("my_grad".to_string()));
        assert!(params_with_ref.has_refs());
    }

    #[test]
    fn effect_params_resolve_refs_substitutes_known() {
        use crate::model::ColorStop;

        let mut gradient_lib = HashMap::new();
        let grad = ColorGradient::new(vec![
            ColorStop { position: 0.0, color: Color::rgb(255, 0, 0) },
            ColorStop { position: 1.0, color: Color::rgb(0, 0, 255) },
        ]).expect("valid gradient");
        gradient_lib.insert("my_grad".to_string(), grad);
        let curve_lib = HashMap::new();

        let params = EffectParams::new()
            .set(ParamKey::Gradient, ParamValue::GradientRef("my_grad".to_string()));

        let resolved = params.resolve_refs(&gradient_lib, &curve_lib);
        assert!(resolved.get(ParamKey::Gradient).expect("should exist").as_color_gradient().is_some());
    }

    #[test]
    fn effect_params_resolve_refs_unknown_stays() {
        let gradient_lib = HashMap::new();
        let curve_lib = HashMap::new();

        let params = EffectParams::new()
            .set(ParamKey::Gradient, ParamValue::GradientRef("missing".to_string()));

        let resolved = params.resolve_refs(&gradient_lib, &curve_lib);
        assert!(resolved.get(ParamKey::Gradient).expect("should exist").as_gradient_ref().is_some());
    }

    // ── ParamValue accessor boundary tests ───────────────────────

    #[test]
    fn param_value_as_float_wrong_type() {
        assert_eq!(ParamValue::Bool(true).as_float(), None);
        assert_eq!(ParamValue::Text("hi".to_string()).as_float(), None);
    }

    #[test]
    fn param_value_as_color_wrong_type() {
        assert_eq!(ParamValue::Float(1.0).as_color(), None);
    }

    #[test]
    fn param_value_as_bool_wrong_type() {
        assert_eq!(ParamValue::Float(1.0).as_bool(), None);
    }

    // ── ParamKey round-trip test ─────────────────────────────────

    #[test]
    fn param_key_builtin_roundtrip() {
        let key = ParamKey::Speed;
        let json = serde_json::to_string(&key).expect("serialize");
        assert_eq!(json, "\"Speed\"");
        let back: ParamKey = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, ParamKey::Speed);
    }

    #[test]
    fn param_key_custom_roundtrip() {
        let key = ParamKey::Custom("myParam".to_string());
        let json = serde_json::to_string(&key).expect("serialize");
        assert_eq!(json, "\"myParam\"");
        let back: ParamKey = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, ParamKey::Custom("myParam".to_string()));
    }

    #[test]
    fn param_key_unknown_deserializes_as_custom() {
        let back: ParamKey = serde_json::from_str("\"UnknownKey\"").expect("deserialize");
        assert_eq!(back, ParamKey::Custom("UnknownKey".to_string()));
    }
}
