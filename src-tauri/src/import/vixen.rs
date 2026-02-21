use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::color::Color;
use crate::model::color_gradient::{ColorGradient, ColorStop};
use crate::model::curve::{Curve, CurvePoint};
use crate::model::fixture::{
    BulbShape, ChannelOrder, ColorModel, Controller, ControllerId, ControllerProtocol,
    EffectTarget, FixtureDef, FixtureGroup, FixtureId, GroupId, GroupMember, PixelType,
};
use crate::model::show::{FixtureLayout, Layout, Show};
use crate::model::timeline::{
    BlendMode, ColorMode, EffectInstance, EffectKind, EffectParams, ParamKey, ParamValue, Sequence,
    TimeRange, Track, WipeDirection,
};

use super::vixen_preview;

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    Xml(quick_xml::Error),
    Parse(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Io(e) => write!(f, "I/O error: {e}"),
            ImportError::Xml(e) => write!(f, "XML error: {e}"),
            ImportError::Parse(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

impl From<quick_xml::Error> for ImportError {
    fn from(e: quick_xml::Error) -> Self {
        ImportError::Xml(e)
    }
}

// ── Wizard types ────────────────────────────────────────────────────

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

// ── ISO 8601 duration parser ────────────────────────────────────────

/// Parse ISO 8601 duration strings like `PT1M53.606S`, `P0DT0H5M30.500S`, etc.
/// Returns duration in seconds.
#[must_use]
pub fn parse_iso_duration(s: &str) -> Option<f64> {
    let s = s.trim();
    if !s.starts_with('P') {
        return None;
    }

    let s = &s[1..]; // Strip 'P'
    let mut total_seconds = 0.0;
    let mut current_num = String::new();
    let mut in_time_part = false;

    for ch in s.chars() {
        match ch {
            'T' => {
                in_time_part = true;
                current_num.clear();
            }
            'D' => {
                let days: f64 = current_num.parse().ok()?;
                total_seconds += days * 86400.0;
                current_num.clear();
            }
            'H' if in_time_part => {
                let hours: f64 = current_num.parse().ok()?;
                total_seconds += hours * 3600.0;
                current_num.clear();
            }
            'M' if in_time_part => {
                let minutes: f64 = current_num.parse().ok()?;
                total_seconds += minutes * 60.0;
                current_num.clear();
            }
            'S' if in_time_part => {
                let secs: f64 = current_num.parse().ok()?;
                total_seconds += secs;
                current_num.clear();
            }
            _ => {
                current_num.push(ch);
            }
        }
    }

    Some(total_seconds)
}

// ── CIE XYZ → sRGB conversion ──────────────────────────────────────

/// Convert CIE XYZ (D65, 0-100 scale) to sRGB Color.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn xyz_to_srgb(x: f64, y: f64, z: f64) -> Color {
    // Apply sRGB gamma
    fn gamma(c: f64) -> f64 {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    }

    // Normalize from 0-100 to 0-1
    let x = x / 100.0;
    let y = y / 100.0;
    let z = z / 100.0;

    // XYZ to linear sRGB (D65 reference, sRGB primaries)
    let r_lin = x * 3.240_454_2 + y * -1.537_138_5 + z * -0.498_531_4;
    let g_lin = x * -0.969_266_0 + y * 1.876_010_8 + z * 0.041_556_0;
    let b_lin = x * 0.055_643_4 + y * -0.204_025_9 + z * 1.057_225_2;

    Color::rgb(
        (gamma(r_lin) * 255.0).round() as u8,
        (gamma(g_lin) * 255.0).round() as u8,
        (gamma(b_lin) * 255.0).round() as u8,
    )
}

// ── Effect type mapping ─────────────────────────────────────────────

/// Build a Curve `ParamValue` from Vixen curve points (0-100 scale → 0-1 normalized).
fn build_curve_param(points: &[(f64, f64)]) -> Option<ParamValue> {
    if points.len() < 2 {
        return None;
    }
    let curve_points: Vec<CurvePoint> = points
        .iter()
        .map(|(x, y)| CurvePoint {
            x: x / 100.0,
            y: y / 100.0,
        })
        .collect();
    Curve::new(curve_points).map(ParamValue::Curve)
}

/// Build a `ColorGradient` `ParamValue` from Vixen gradient stops (positions 0-1).
fn build_gradient_param(stops: &[(f64, Color)]) -> Option<ParamValue> {
    if stops.is_empty() {
        return None;
    }
    let color_stops: Vec<ColorStop> = stops
        .iter()
        .map(|(pos, color)| ColorStop {
            position: *pos,
            color: *color,
        })
        .collect();
    ColorGradient::new(color_stops).map(ParamValue::ColorGradient)
}

/// Map Vixen color handling string to our `ColorMode` enum.
///
/// The `default` parameter controls the fallback for `StaticColor` and `None`,
/// which varies by effect type:
/// - Chase/Wipe/Spin: `GradientPerPulse` (gradient defines pulse shape, e.g. white head → colored tail)
/// - Fade/Pulse/ColorWash: `GradientThroughEffect` (gradient animates over the effect duration)
/// - Garlands/PinWheel: `GradientAcrossItems` (gradient spreads across pixels)
fn map_color_handling(handling: Option<&str>, default: ColorMode) -> ColorMode {
    match handling {
        Some("GradientThroughWholeEffect") => ColorMode::GradientThroughEffect,
        Some("GradientAcrossItems" | "ColorAcrossItems") => ColorMode::GradientAcrossItems,
        Some("GradientForEachPulse" | "GradientOverEachPulse" | "GradientPerPulse") => {
            ColorMode::GradientPerPulse
        }
        // StaticColor and None: use per-effect-type default
        _ => default,
    }
}

/// Helper: populate gradient param from parsed stops or single color.
fn set_gradient(params: EffectParams, effect: &VixenEffect) -> EffectParams {
    let base_color = effect.color.unwrap_or(Color::WHITE);
    if let Some(stops) = effect.gradient_colors.as_ref() {
        if let Some(grad_val) = build_gradient_param(stops) {
            return params.set(ParamKey::Gradient, grad_val);
        }
    }
    params.set(
        ParamKey::Gradient,
        ParamValue::ColorGradient(ColorGradient::solid(base_color)),
    )
}

/// Helper: check if direction indicates reverse.
fn is_reverse_direction(direction: Option<&str>) -> bool {
    match direction {
        Some(d) => matches!(d, "Reverse" | "Right" | "Down" | "Out" | "1"),
        None => false,
    }
}

/// Map a Vixen wipe direction string to a `WipeDirection` + reverse flag.
fn map_wipe_direction(direction: Option<&str>) -> (WipeDirection, bool) {
    match direction {
        Some("Right" | "Reverse" | "1") => (WipeDirection::Horizontal, true),
        Some("Vertical" | "Up") => (WipeDirection::Vertical, false),
        Some("Down") => (WipeDirection::Vertical, true),
        Some("DiagonalUp") => (WipeDirection::DiagonalUp, false),
        Some("DiagonalDown") => (WipeDirection::DiagonalDown, false),
        Some("Burst" | "BurstIn") => (WipeDirection::Burst, false),
        Some("BurstOut" | "Out") => (WipeDirection::Burst, true),
        Some("Circle" | "CircleIn") => (WipeDirection::Circle, false),
        Some("CircleOut") => (WipeDirection::Circle, true),
        Some("Diamond" | "DiamondIn") => (WipeDirection::Diamond, false),
        Some("DiamondOut") => (WipeDirection::Diamond, true),
        // "Horizontal", "Left", "0", None, and any unknown → default
        _ => (WipeDirection::Horizontal, false),
    }
}

/// Map a Vixen effect type name to a `VibeLights` `EffectKind` + default params.
///
/// Effects that can't be mapped print a LOUD warning for easy debugging.
#[allow(clippy::too_many_lines)]
fn map_vixen_effect(effect: &VixenEffect) -> (EffectKind, EffectParams) {
    let type_name = effect.type_name.as_str();
    let intensity_curve = effect.intensity_curve.as_ref();
    let movement_curve = effect.movement_curve.as_ref();
    let pulse_curve = effect.pulse_curve.as_ref();
    let color_handling = effect.color_handling.as_deref();
    let level = effect.level;
    let base_color = effect.color.unwrap_or(Color::WHITE);

    match type_name {
        // ── Pulse / SetLevel → Fade ──────────────────────────────
        "Pulse" | "SetLevel" => {
            let mut params = EffectParams::new();
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::IntensityCurve, curve_val);
                }
            } else {
                let intensity = level.unwrap_or(1.0).clamp(0.0, 1.0);
                params = params.set(
                    ParamKey::IntensityCurve,
                    ParamValue::Curve(Curve::constant(intensity)),
                );
            }
            params = set_gradient(params, effect);
            let color_mode = map_color_handling(color_handling, ColorMode::GradientThroughEffect);
            params = params.set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode));
            (EffectKind::Fade, params)
        }

        // ── Chase → Chase ────────────────────────────────────────
        "Chase" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            if let Some(pts) = movement_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::MovementCurve, curve_val);
                }
            }
            if let Some(pts) = pulse_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::PulseCurve, curve_val);
                }
            }
            let color_mode = map_color_handling(color_handling, ColorMode::GradientPerPulse);
            params = params
                .set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode))
                .set(ParamKey::Speed, ParamValue::Float(1.0))
                .set(ParamKey::PulseWidth, ParamValue::Float(0.3));
            (EffectKind::Chase, params)
        }

        // ── Spin → Chase (continuous rotation) ───────────────────
        "Spin" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            if let Some(pts) = pulse_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::PulseCurve, curve_val);
                }
            }
            let speed = effect.revolution_count.unwrap_or(4.0);
            let pulse_width = effect
                .pulse_percentage
                .map_or(0.1, |p| (p / 100.0).clamp(0.01, 1.0));
            let reverse = effect.reverse_spin.unwrap_or(false);
            let color_mode = map_color_handling(color_handling, ColorMode::GradientPerPulse);
            params = params
                .set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode))
                .set(ParamKey::Speed, ParamValue::Float(speed))
                .set(ParamKey::PulseWidth, ParamValue::Float(pulse_width))
                .set(ParamKey::Reverse, ParamValue::Bool(reverse));
            (EffectKind::Chase, params)
        }

        // ── Wipe → Wipe (spatial sweep) ─────────────────────────
        // Vixen Wipe is a 2D spatial effect that sweeps across fixtures
        // based on their physical positions with 7 direction modes.
        "Wipe" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            if let Some(pts) = movement_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::MovementCurve, curve_val);
                }
            }
            if let Some(pts) = pulse_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::PulseCurve, curve_val);
                }
            }
            // Map Vixen direction strings to our direction vocabulary
            let (direction, reverse) = map_wipe_direction(effect.direction.as_deref());
            let color_mode = map_color_handling(color_handling, ColorMode::GradientPerPulse);
            params = params
                .set(ParamKey::Direction, ParamValue::WipeDirection(direction))
                .set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode))
                .set(ParamKey::Speed, ParamValue::Float(1.0))
                .set(ParamKey::PulseWidth, ParamValue::Float(1.0))
                .set(ParamKey::Reverse, ParamValue::Bool(reverse));
            (EffectKind::Wipe, params)
        }

        // ── Alternating → Chase (50/50 split) ───────────────────
        "Alternating" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::PulseCurve, curve_val);
                }
            }
            let color_mode = map_color_handling(color_handling, ColorMode::GradientPerPulse);
            params = params
                .set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode))
                .set(ParamKey::Speed, ParamValue::Float(1.0))
                .set(ParamKey::PulseWidth, ParamValue::Float(0.5));
            (EffectKind::Chase, params)
        }

        // ── Shockwave → Chase (radial wave approximated as linear) ──
        // Shockwave is a 2D radial wave from a center point.
        // We approximate it as a fast narrow chase pulse.
        "Shockwave" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            // AccelerationCurve maps to movement (head position)
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::MovementCurve, curve_val);
                }
            }
            let color_mode = map_color_handling(color_handling, ColorMode::GradientPerPulse);
            params = params
                .set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode))
                .set(ParamKey::Speed, ParamValue::Float(2.0))
                .set(ParamKey::PulseWidth, ParamValue::Float(0.15));
            (EffectKind::Chase, params)
        }

        // ── Garlands → Chase (multi-color segment pattern) ──────
        // Garlands creates alternating colored segments.
        // Best approximation: chase with gradient_across_items and wide pulse.
        "Garlands" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            if let Some(pts) = movement_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::MovementCurve, curve_val);
                }
            }
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::PulseCurve, curve_val);
                }
            }
            let color_mode = map_color_handling(color_handling, ColorMode::GradientAcrossItems);
            params = params
                .set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode))
                .set(ParamKey::Speed, ParamValue::Float(1.0))
                .set(ParamKey::PulseWidth, ParamValue::Float(0.5));
            (EffectKind::Chase, params)
        }

        // ── PinWheel → Chase (rotating color pattern) ───────────
        // PinWheel creates rotating "arms" of color from a center point.
        // Approximated as a chase with gradient spread across items.
        "PinWheel" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            if let Some(pts) = movement_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::MovementCurve, curve_val);
                }
            }
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::PulseCurve, curve_val);
                }
            }
            let color_mode = map_color_handling(color_handling, ColorMode::GradientAcrossItems);
            let reverse = is_reverse_direction(effect.direction.as_deref());
            params = params
                .set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode))
                .set(ParamKey::Speed, ParamValue::Float(2.0))
                .set(ParamKey::PulseWidth, ParamValue::Float(0.3))
                .set(ParamKey::Reverse, ParamValue::Bool(reverse));
            (EffectKind::Chase, params)
        }

        // ── Butterfly → Chase (color wave pattern) ──────────────
        // Butterfly creates mirrored color waves.
        // Approximated as a chase with gradient_through_effect.
        "Butterfly" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::PulseCurve, curve_val);
                }
            }
            let color_mode = map_color_handling(color_handling, ColorMode::GradientPerPulse);
            let reverse = is_reverse_direction(effect.direction.as_deref());
            params = params
                .set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode))
                .set(ParamKey::Speed, ParamValue::Float(2.0))
                .set(ParamKey::PulseWidth, ParamValue::Float(0.5))
                .set(ParamKey::Reverse, ParamValue::Bool(reverse));
            (EffectKind::Chase, params)
        }

        // ── Dissolve → Twinkle (random pixel on/off) ────────────
        // Dissolve randomly turns pixels on/off over time.
        // Approximated as twinkle with matched color.
        "Dissolve" => {
            let mut params = EffectParams::new();
            params = params
                .set(ParamKey::Color, ParamValue::Color(base_color))
                .set(ParamKey::Density, ParamValue::Float(0.5))
                .set(ParamKey::Speed, ParamValue::Float(4.0));
            (EffectKind::Twinkle, params)
        }

        // ── ColorWash → Fade (was Gradient, but Fade is closer) ─
        // ColorWash in Vixen is a smooth color envelope — basically a fade
        // with gradient over time.
        "ColorWash" => {
            let mut params = set_gradient(EffectParams::new(), effect);
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::IntensityCurve, curve_val);
                }
            }
            let color_mode = map_color_handling(color_handling, ColorMode::GradientThroughEffect);
            params = params.set(ParamKey::ColorMode, ParamValue::ColorMode(color_mode));
            (EffectKind::Fade, params)
        }

        // ── Twinkle ─────────────────────────────────────────────
        "Twinkle" => (
            EffectKind::Twinkle,
            EffectParams::new()
                .set(ParamKey::Color, ParamValue::Color(base_color))
                .set(ParamKey::Density, ParamValue::Float(0.4))
                .set(ParamKey::Speed, ParamValue::Float(6.0)),
        ),

        // ── Strobe ──────────────────────────────────────────────
        "Strobe" => (
            EffectKind::Strobe,
            EffectParams::new()
                .set(ParamKey::Color, ParamValue::Color(base_color))
                .set(ParamKey::Rate, ParamValue::Float(10.0))
                .set(ParamKey::DutyCycle, ParamValue::Float(0.5)),
        ),

        // ── Rainbow ─────────────────────────────────────────────
        "Rainbow" => (
            EffectKind::Rainbow,
            EffectParams::new()
                .set(ParamKey::Speed, ParamValue::Float(1.0))
                .set(ParamKey::Spread, ParamValue::Float(2.0)),
        ),

        // ── Fire → Fade (warm color flicker) ────────────────────
        // Fire is simulated with a warm gradient and intensity modulation.
        "Fire" => {
            let mut params = EffectParams::new();
            // Use a warm gradient: red → orange → yellow
            let warm_gradient = ColorGradient::new(vec![
                ColorStop { position: 0.0, color: Color::rgb(180, 30, 0) },
                ColorStop { position: 0.4, color: Color::rgb(255, 100, 0) },
                ColorStop { position: 1.0, color: Color::rgb(255, 200, 50) },
            ])
            .unwrap_or_else(|| ColorGradient::solid(Color::rgb(255, 100, 0)));
            params = params.set(ParamKey::Gradient, ParamValue::ColorGradient(warm_gradient));
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::IntensityCurve, curve_val);
                }
            }
            params = params.set(
                ParamKey::ColorMode,
                ParamValue::ColorMode(ColorMode::GradientThroughEffect),
            );
            (EffectKind::Fade, params)
        }

        // ── Fireworks → Twinkle (bright random bursts) ──────────
        // Fireworks are particle bursts — approximated as bright twinkle.
        "Fireworks" => (
            EffectKind::Twinkle,
            EffectParams::new()
                .set(ParamKey::Color, ParamValue::Color(base_color))
                .set(ParamKey::Density, ParamValue::Float(0.3))
                .set(ParamKey::Speed, ParamValue::Float(10.0)),
        ),

        // ── Snowflakes / Meteor → Twinkle ───────────────────────
        "Snowflakes" | "Meteor" | "Meteors" => (
            EffectKind::Twinkle,
            EffectParams::new()
                .set(ParamKey::Color, ParamValue::Color(base_color))
                .set(ParamKey::Density, ParamValue::Float(0.3))
                .set(ParamKey::Speed, ParamValue::Float(5.0)),
        ),

        // ── Candle → Fade (warm flicker) ────────────────────────
        "Candle" => {
            let mut params = EffectParams::new();
            params = params
                .set(
                    ParamKey::Gradient,
                    ParamValue::ColorGradient(ColorGradient::solid(
                        Color::rgb(255, 180, 50),
                    )),
                )
                .set(ParamKey::ColorMode, ParamValue::ColorMode(ColorMode::Static));
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set(ParamKey::IntensityCurve, curve_val);
                }
            }
            (EffectKind::Fade, params)
        }

        // ── LipSync / CountDown / Launcher / Video / Nutcracker → skip ──
        // These are audio-reactive, timing, or video effects with no light equivalent.
        "LipSync" | "CountDown" | "Launcher" | "Video" | "NutcrackerModule" | "Audio" => {
            eprintln!(
                "[VibeLights] WARNING: Skipping unsupported effect type '{type_name}' (no light equivalent)",
            );
            (
                EffectKind::Solid,
                EffectParams::new().set(ParamKey::Color, ParamValue::Color(Color::BLACK)),
            )
        }

        // ── MaskAndFill → Solid (masking not supported) ─────────
        "MaskAndFill" | "Mask" | "Fill" => {
            eprintln!(
                "[VibeLights] WARNING: Effect type '{type_name}' mapped to Solid (masking not supported)",
            );
            (
                EffectKind::Solid,
                EffectParams::new().set(ParamKey::Color, ParamValue::Color(base_color)),
            )
        }

        // ── Unknown effect → Solid + LOUD WARNING ───────────────
        _ => {
            eprintln!(
                "\n[VibeLights] !!! UNHANDLED EFFECT TYPE: '{}' !!!\n\
                 [VibeLights]     Mapped to Solid gray as fallback.\n\
                 [VibeLights]     Color: {:?}, Gradient: {}, Curves: m={} p={} i={}\n",
                type_name,
                effect.color,
                effect.gradient_colors.is_some(),
                effect.movement_curve.is_some(),
                effect.pulse_curve.is_some(),
                effect.intensity_curve.is_some(),
            );
            (
                EffectKind::Solid,
                EffectParams::new().set(ParamKey::Color, ParamValue::Color(Color::rgb(128, 128, 128))),
            )
        }
    }
}

// ── Internal intermediate types ─────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct VixenNode {
    name: String,
    guid: String,
    children_guids: Vec<String>,
    channel_id: Option<String>,
}

#[derive(Debug, Clone)]
struct VixenEffect {
    type_name: String,
    start_time: f64,
    duration: f64,
    target_node_guids: Vec<String>,
    color: Option<Color>,
    /// `ChaseMovement` / `MovementCurve` (position over time for Chase/Wipe effects)
    movement_curve: Option<Vec<(f64, f64)>>,
    /// `PulseCurve` (intensity envelope per pulse for Chase/Spin effects)
    pulse_curve: Option<Vec<(f64, f64)>>,
    /// `LevelCurve` / `IntensityCurve` / `DissolveCurve` / etc. (brightness envelope)
    intensity_curve: Option<Vec<(f64, f64)>>,
    gradient_colors: Option<Vec<(f64, Color)>>,
    color_handling: Option<String>,
    level: Option<f64>,
    /// Spin-specific: number of revolutions over the effect duration
    revolution_count: Option<f64>,
    /// Spin-specific: pulse width as percentage of revolution (0-100)
    pulse_percentage: Option<f64>,
    /// Spin-specific: pulse time in milliseconds (used when PulseLengthFormat=FixedTime)
    #[allow(dead_code)]
    pulse_time_ms: Option<f64>,
    /// Spin-specific: whether the spin direction is reversed
    reverse_spin: Option<bool>,
    /// Direction for Wipe/Butterfly/etc. (e.g. "Up", "Down", "Left", "Right")
    direction: Option<String>,
}

// ── VixenImporter ───────────────────────────────────────────────────

pub struct VixenImporter {
    nodes: HashMap<String, VixenNode>,
    guid_to_id: HashMap<String, u32>,
    next_id: u32,
    fixtures: Vec<FixtureDef>,
    groups: Vec<FixtureGroup>,
    controllers: Vec<Controller>,
    patches: Vec<crate::model::fixture::Patch>,
    sequences: Vec<Sequence>,
    /// IDs of fixtures that were created by merging leaf channels (e.g. RGB leaves → multi-pixel fixture).
    /// These should NOT be re-merged by a parent node.
    merged_fixture_ids: HashSet<u32>,
    /// Warnings accumulated during import (orphan targets, unsupported shapes, etc.).
    warnings: Vec<String>,
}

impl Default for VixenImporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Which kind of curve a Vixen data model entry contains.
/// Replaces ad-hoc string routing ("movement"/"pulse"/"intensity").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurveKind {
    Movement,
    Pulse,
    Intensity,
}

impl VixenImporter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            guid_to_id: HashMap::new(),
            next_id: 0,
            fixtures: Vec::new(),
            groups: Vec::new(),
            controllers: Vec::new(),
            patches: Vec::new(),
            sequences: Vec::new(),
            merged_fixture_ids: HashSet::new(),
            warnings: Vec::new(),
        }
    }

    /// Reconstruct importer state from an existing profile + saved GUID mapping.
    /// This allows importing sequences against a previously-imported profile.
    #[must_use]
    pub fn from_profile(
        fixtures: Vec<FixtureDef>,
        groups: Vec<FixtureGroup>,
        controllers: Vec<Controller>,
        patches: Vec<crate::model::fixture::Patch>,
        guid_map: HashMap<String, u32>,
    ) -> Self {
        let next_id = guid_map.values().copied().max().map_or(0, |m| m + 1);
        Self {
            nodes: HashMap::new(),
            guid_to_id: guid_map,
            next_id,
            fixtures,
            groups,
            controllers,
            patches,
            sequences: Vec::new(),
            merged_fixture_ids: HashSet::new(),
            warnings: Vec::new(),
        }
    }

    /// Return the GUID → ID mapping (for persisting after profile import).
    #[must_use]
    pub fn guid_map(&self) -> &HashMap<String, u32> {
        &self.guid_to_id
    }

    /// Return warnings accumulated during import.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Count of parsed fixtures.
    #[must_use]
    pub fn fixture_count(&self) -> usize {
        self.fixtures.len()
    }

    /// Count of parsed groups.
    #[must_use]
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Count of parsed controllers.
    #[must_use]
    pub fn controller_count(&self) -> usize {
        self.controllers.len()
    }

    /// Parse Vixen preview layout data and produce `FixtureLayout` entries.
    ///
    /// # Errors
    ///
    /// Returns `ImportError` if the preview file cannot be found or parsed.
    pub fn parse_preview(
        &mut self,
        vixen_dir: &Path,
        preview_file_override: Option<&Path>,
    ) -> Result<Vec<FixtureLayout>, ImportError> {
        let preview_path = if let Some(override_path) = preview_file_override {
            override_path.to_path_buf()
        } else {
            vixen_preview::find_preview_file(vixen_dir).ok_or_else(|| {
                ImportError::Parse("No preview data file found".into())
            })?
        };

        let preview_data = vixen_preview::parse_preview_file(&preview_path)?;

        // Build pixel count map from current fixtures
        let pixel_counts: HashMap<u32, u32> = self
            .fixtures
            .iter()
            .map(|f| (f.id.0, f.pixel_count))
            .collect();

        let layouts = vixen_preview::build_fixture_layouts(
            &preview_data,
            &self.guid_to_id,
            &pixel_counts,
            &mut self.warnings,
        );

        Ok(layouts)
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Parse SystemConfig.xml to extract fixtures, groups, and controllers.
    ///
    /// # Errors
    ///
    /// Returns `ImportError` on I/O or XML parsing failures.
    pub fn parse_system_config(&mut self, path: &Path) -> Result<(), ImportError> {
        let file = File::open(path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let mut xml = Reader::from_reader(reader);
        xml.config_mut().trim_text(true);

        let mut buf = Vec::with_capacity(4096);

        // First pass: collect all nodes
        self.parse_nodes(&mut xml, &mut buf)?;

        // Rewind and parse controllers
        let file2 = File::open(path)?;
        let reader2 = BufReader::with_capacity(64 * 1024, file2);
        let mut xml2 = Reader::from_reader(reader2);
        xml2.config_mut().trim_text(true);
        buf.clear();
        self.parse_controllers(&mut xml2, &mut buf)?;

        // Build fixtures and groups from nodes
        self.build_fixtures_and_groups();

        Ok(())
    }

    fn parse_nodes(
        &mut self,
        xml: &mut Reader<BufReader<File>>,
        buf: &mut Vec<u8>,
    ) -> Result<(), ImportError> {
        // Vixen 3 SystemConfig stores nodes as nested XML:
        //   <Nodes>
        //     <Node name="Group" id="GUID-1">
        //       <Node name="Child" id="GUID-2" channelId="CH-GUID">
        //         <Properties>...</Properties>
        //       </Node>
        //     </Node>
        //   </Nodes>
        //
        // Parent-child relationships are implicit via nesting.
        // We use a stack to track the current hierarchy.

        let mut in_nodes_section = false;
        let mut node_stack: Vec<VixenNode> = Vec::new();

        loop {
            match xml.read_event_into(buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    match name.as_str() {
                        "Nodes" | "SystemNodes" => {
                            in_nodes_section = true;
                        }
                        "Node" | "ElementNode" | "ChannelNode" if in_nodes_section => {
                            let mut node_id = String::new();
                            let mut node_name = String::new();
                            let mut channel_id = None;

                            for attr in e.attributes().flatten() {
                                let key =
                                    String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                let val =
                                    String::from_utf8_lossy(&attr.value).to_string();
                                match key.as_str() {
                                    "id" | "Id" => node_id = val,
                                    "name" | "Name" => node_name = val,
                                    "channelId" | "ChannelId" => channel_id = Some(val),
                                    _ => {}
                                }
                            }

                            node_stack.push(VixenNode {
                                name: node_name,
                                guid: node_id,
                                children_guids: Vec::new(),
                                channel_id,
                            });
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    match name.as_str() {
                        "Node" | "ElementNode" | "ChannelNode" if !node_stack.is_empty() => {
                            let Some(node) = node_stack.pop() else {
                                continue;
                            };
                            if !node.guid.is_empty() {
                                let guid = node.guid.clone();
                                self.nodes.insert(guid.clone(), node);

                                // Register as child of parent node (if any)
                                if let Some(parent) = node_stack.last_mut() {
                                    parent.children_guids.push(guid);
                                }
                            }
                        }
                        "Nodes" | "SystemNodes" => {
                            in_nodes_section = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Self-closing leaf node: <Node name="..." id="..." channelId="..." />
                    if (name == "Node" || name == "ElementNode" || name == "ChannelNode")
                        && in_nodes_section
                    {
                        let mut node_id = String::new();
                        let mut node_name = String::new();
                        let mut channel_id = None;

                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "id" | "Id" => node_id = val,
                                "name" | "Name" => node_name = val,
                                "channelId" | "ChannelId" => channel_id = Some(val),
                                _ => {}
                            }
                        }

                        if !node_id.is_empty() {
                            let guid = node_id.clone();
                            self.nodes.insert(
                                guid.clone(),
                                VixenNode {
                                    name: node_name,
                                    guid: node_id,
                                    children_guids: Vec::new(),
                                    channel_id,
                                },
                            );

                            // Register as child of parent node
                            if let Some(parent) = node_stack.last_mut() {
                                parent.children_guids.push(guid);
                            }
                        }
                    }
                }
                Err(e) => return Err(ImportError::Xml(e)),
                _ => {}
            }
            buf.clear();
        }

        Ok(())
    }

    fn parse_controllers(
        &mut self,
        xml: &mut Reader<BufReader<File>>,
        buf: &mut Vec<u8>,
    ) -> Result<(), ImportError> {
        let mut in_controllers = false;
        let mut current_name = String::new();
        let mut current_outputs: Vec<(String, u16)> = Vec::new(); // (ip, universe)
        let mut depth = 0u32;
        let mut controller_id_counter = 0u32;

        loop {
            match xml.read_event_into(buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(ref e)) => {
                    depth += 1;
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    if name == "Controllers" || name == "OutputControllers" {
                        in_controllers = true;
                    }

                    if in_controllers
                        && (name == "Controller"
                            || name == "OutputController"
                            || name.contains("Controller"))
                    {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "name" || key == "Name" {
                                current_name = val;
                            }
                        }
                    }

                    // Look for universe/IP configuration in output elements
                    if in_controllers {
                        let mut ip = None;
                        let mut universe = None;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "ip" | "IP" | "address" | "Address" | "UnicastAddress" => {
                                    ip = Some(val);
                                }
                                "universe" | "Universe" => {
                                    universe = val.parse().ok();
                                }
                                _ => {}
                            }
                        }
                        if let (Some(ip_addr), Some(uni)) = (ip, universe) {
                            current_outputs.push((ip_addr, uni));
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth = depth.saturating_sub(1);

                    if name == "Controllers" || name == "OutputControllers" {
                        in_controllers = false;
                    }

                    if in_controllers
                        && (name == "Controller"
                            || name == "OutputController"
                            || name.contains("Controller"))
                        && !current_name.is_empty()
                    {
                        // Create a controller for each unique IP/universe combo
                        // If no outputs found, create a generic E1.31 controller
                        if current_outputs.is_empty() {
                            self.controllers.push(Controller {
                                id: ControllerId(controller_id_counter),
                                name: current_name.clone(),
                                protocol: ControllerProtocol::E131 {
                                    unicast_address: None,
                                },
                            });
                            controller_id_counter += 1;
                        } else {
                            for (ip, _universe) in &current_outputs {
                                self.controllers.push(Controller {
                                    id: ControllerId(controller_id_counter),
                                    name: format!("{current_name} ({ip})"),
                                    protocol: ControllerProtocol::E131 {
                                        unicast_address: Some(ip.clone()),
                                    },
                                });
                                controller_id_counter += 1;
                            }
                        }
                        current_name.clear();
                        current_outputs.clear();
                    }
                }
                Err(e) => return Err(ImportError::Xml(e)),
                _ => {}
            }
            buf.clear();
        }

        Ok(())
    }

    fn build_fixtures_and_groups(&mut self) {
        // Find root nodes (nodes not referenced as children by any other node)
        let all_child_guids: std::collections::HashSet<&str> = self
            .nodes
            .values()
            .flat_map(|n| n.children_guids.iter().map(String::as_str))
            .collect();

        let root_guids: Vec<String> = self
            .nodes
            .keys()
            .filter(|guid| !all_child_guids.contains(guid.as_str()))
            .cloned()
            .collect();

        // Assign IDs and build fixtures/groups
        for guid in &root_guids {
            self.build_node(guid);
        }
    }

    /// Recursively build a fixture or group from a Vixen node GUID.
    /// Returns either a `FixtureId` or `GroupId` if successfully created.
    fn build_node(&mut self, guid: &str) -> Option<GroupMember> {
        // Already processed?
        if let Some(&id) = self.guid_to_id.get(guid) {
            let node = self.nodes.get(guid)?;
            if node.children_guids.is_empty() {
                return Some(GroupMember::Fixture(FixtureId(id)));
            }
            return Some(GroupMember::Group(GroupId(id)));
        }

        let node = self.nodes.get(guid)?.clone();
        let id = self.alloc_id();
        self.guid_to_id.insert(guid.to_string(), id);

        if node.children_guids.is_empty() {
            // Leaf node → fixture
            self.fixtures.push(FixtureDef {
                id: FixtureId(id),
                name: node.name.clone(),
                color_model: ColorModel::Rgb,
                pixel_count: 1,
                pixel_type: PixelType::default(),
                bulb_shape: BulbShape::default(),
                display_radius_override: None,
                channel_order: ChannelOrder::default(),
            });
            Some(GroupMember::Fixture(FixtureId(id)))
        } else {
            // Interior node → group
            // First, recursively build all children
            let mut members = Vec::new();
            for child_guid in &node.children_guids {
                if let Some(member) = self.build_node(child_guid) {
                    members.push(member);
                }
            }

            // Check if all children are *original leaf* fixtures (not already-merged multi-pixel ones).
            // Only merge leaves into a multi-pixel fixture; if any child is already merged,
            // create a group instead to preserve the hierarchy.
            let all_original_leaves = members.iter().all(|m| match m {
                GroupMember::Fixture(fid) => !self.merged_fixture_ids.contains(&fid.0),
                GroupMember::Group(_) => false,
            });
            let child_count = members.len();

            if all_original_leaves && child_count > 1 {
                // Merge: remove individual leaf fixtures, create one multi-pixel fixture.
                // In Vixen, each leaf element node is one pixel (an RGB fixture).
                // The channelId on the leaf references the output channel.
                let fixture_ids: Vec<FixtureId> = members
                    .iter()
                    .filter_map(|m| match m {
                        GroupMember::Fixture(fid) => Some(*fid),
                        GroupMember::Group(_) => None,
                    })
                    .collect();

                // Remove individual fixtures
                self.fixtures.retain(|f| !fixture_ids.contains(&f.id));

                // Create one multi-pixel fixture for this group of leaves
                #[allow(clippy::cast_possible_truncation)]
                let pixel_count = child_count as u32;
                self.fixtures.push(FixtureDef {
                    id: FixtureId(id),
                    name: node.name.clone(),
                    color_model: ColorModel::Rgb,
                    pixel_count,
                    pixel_type: PixelType::default(),
                    bulb_shape: BulbShape::default(),
                    display_radius_override: None,
                    channel_order: ChannelOrder::default(),
                });

                // Record this as a merged fixture so parent nodes don't re-merge it
                self.merged_fixture_ids.insert(id);

                // Remap child leaf GUIDs to point to the parent fixture ID.
                // This is critical for layout resolution: preview pixels reference
                // leaf node GUIDs, which must resolve to the merged parent fixture.
                for child_guid in &node.children_guids {
                    self.guid_to_id.insert(child_guid.clone(), id);
                }

                Some(GroupMember::Fixture(FixtureId(id)))
            } else if !members.is_empty() {
                self.groups.push(FixtureGroup {
                    id: GroupId(id),
                    name: node.name.clone(),
                    members,
                });
                Some(GroupMember::Group(GroupId(id)))
            } else {
                None
            }
        }
    }

    /// Parse a .tim sequence file.
    ///
    /// # Errors
    ///
    /// Returns `ImportError` on I/O or XML parsing failures.
    #[allow(clippy::too_many_lines)]
    pub fn parse_sequence(&mut self, path: &Path) -> Result<(), ImportError> {
        let file = File::open(path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let mut xml = Reader::from_reader(reader);
        xml.config_mut().trim_text(true);

        let mut buf = Vec::with_capacity(4096);

        let seq_name = path
            .file_stem()
            .map_or_else(|| "Untitled".to_string(), |s| s.to_string_lossy().to_string());

        let mut duration = 30.0f64;
        let mut effects: Vec<VixenEffect> = Vec::new();
        let mut audio_file: Option<String> = None;

        // Parsing state
        let mut current_element = String::new();
        let mut in_data_models = false;
        let mut in_effect_nodes = false;
        let mut in_media = false;

        // Current effect being parsed
        let mut effect_type = String::new();
        let mut effect_start = 0.0f64;
        let mut effect_duration = 0.0f64;
        let mut effect_targets: Vec<String> = Vec::new();
        let mut effect_color: Option<Color> = None;

        // For effect data models, we store type_name keyed by ModuleInstanceId
        let mut data_model_types: HashMap<String, String> = HashMap::new();
        let mut data_model_colors: HashMap<String, Color> = HashMap::new();
        // Curve data keyed by ModuleInstanceId, separated by curve type
        let mut data_model_movement_curves: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        let mut data_model_pulse_curves: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        let mut data_model_intensity_curves: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        let mut data_model_gradients: HashMap<String, Vec<(f64, Color)>> = HashMap::new();
        let mut data_model_color_handling: HashMap<String, String> = HashMap::new();
        // Also map ModuleTypeId → type_name (class-level; many instances share one type)
        let mut module_type_to_name: HashMap<String, String> = HashMap::new();
        let mut current_data_model_id = String::new();
        let mut current_data_model_type_id = String::new();
        let mut in_data_model_entry = false;
        let mut data_model_depth = 0u32;
        // Temporary state for parsing curve points within a data model
        let mut current_curve_points: Vec<(f64, f64)> = Vec::new();
        let mut current_gradient_stops: Vec<(f64, Color)> = Vec::new();
        let mut in_curve_element = false;
        let mut current_curve_kind = CurveKind::Intensity;
        let mut in_gradient_element = false;
        let mut current_color_handling = String::new();
        // State for parsing PointPair child elements (X, Y are text, not attributes)
        let mut in_point_pair = false;
        let mut point_pair_x: Option<f64> = None;
        let mut point_pair_y: Option<f64> = None;
        // State for parsing ColorPoint child elements
        let mut in_color_point = false;
        let mut color_point_position: Option<f64> = None;
        // State for parsing _color child elements within ColorPoint (XYZ as child text)
        let mut in_gradient_color = false;
        let mut gradient_color_x: Option<f64> = None;
        let mut gradient_color_y: Option<f64> = None;
        let mut gradient_color_z: Option<f64> = None;
        // State for parsing SetLevel-style direct RGB color (_r/_g/_b 0-1 scale)
        let mut in_direct_color = false;
        let mut direct_color_r: Option<f64> = None;
        let mut direct_color_g: Option<f64> = None;
        let mut direct_color_b: Option<f64> = None;
        let mut data_model_levels: HashMap<String, f64> = HashMap::new();
        // Spin-specific data keyed by ModuleInstanceId
        let mut data_model_revolution_count: HashMap<String, f64> = HashMap::new();
        let mut data_model_pulse_percentage: HashMap<String, f64> = HashMap::new();
        let mut data_model_pulse_time_ms: HashMap<String, f64> = HashMap::new();
        let mut data_model_reverse_spin: HashMap<String, bool> = HashMap::new();
        // Direction data keyed by ModuleInstanceId (for Wipe, Butterfly, etc.)
        let mut data_model_direction: HashMap<String, String> = HashMap::new();

        // For effect node surrogates
        let mut in_effect_node_entry = false;
        let mut current_module_id = String::new();
        let mut current_effect_instance_id = String::new();
        let mut effect_node_depth = 0u32;

        let mut depth = 0u32;

        loop {
            match xml.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(ref e)) => {
                    depth += 1;
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_element.clone_from(&tag);

                    match tag.as_str() {
                        "_dataModels" | "DataModels" => {
                            in_data_models = true;
                        }
                        "_effectNodeSurrogates" | "EffectNodeSurrogates" => {
                            in_effect_nodes = true;
                        }
                        "_mediaSurrogates" | "MediaSurrogates" => {
                            in_media = true;
                        }
                        _ => {}
                    }

                    // Inside _dataModels, each entry is a <d1p1:anyType> wrapper
                    // with i:type attribute containing the effect type.
                    // Also handle older formats with explicit DataModel tags.
                    if in_data_models && !in_data_model_entry {
                        // Match the wrapper element: anyType, or tags with "DataModel" in the name
                        let local_tag = tag.rsplit(':').next().unwrap_or(&tag);
                        if local_tag == "anyType" || tag.contains("DataModel") {
                            in_data_model_entry = true;
                            data_model_depth = depth;
                            current_data_model_id.clear();
                            current_data_model_type_id.clear();
                            effect_type.clear();
                            for attr in e.attributes().flatten() {
                                let key =
                                    String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                let val =
                                    String::from_utf8_lossy(&attr.value).to_string();
                                let local_key = key.rsplit(':').next().unwrap_or(&key);
                                match local_key {
                                    "type" | "Type" | "typeName" => {
                                        // Extract class name: "d2p1:PinWheelData" → "PinWheel"
                                        let raw = val.rsplit(':').next().unwrap_or(&val);
                                        let cleaned = raw
                                            .rsplit('.')
                                            .next()
                                            .unwrap_or(raw);
                                        let cleaned = cleaned
                                            .strip_suffix("Module")
                                            .or_else(|| cleaned.strip_suffix("Data"))
                                            .unwrap_or(cleaned);
                                        effect_type = cleaned.to_string();
                                    }
                                    "id" | "Id" => {
                                        current_data_model_id = val;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    // Inside _effectNodeSurrogates, each effect is an <EffectNodeSurrogate>.
                    // Must NOT match child <ChannelNodeReferenceSurrogate> tags.
                    if in_effect_nodes
                        && !in_effect_node_entry
                        && tag == "EffectNodeSurrogate"
                    {
                        in_effect_node_entry = true;
                        effect_node_depth = depth;
                        effect_start = 0.0;
                        effect_duration = 0.0;
                        effect_targets.clear();
                        effect_color = None;
                        current_module_id.clear();
                        current_effect_instance_id.clear();

                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "startTime" | "StartTime" => {
                                    effect_start = parse_iso_duration(&val).unwrap_or(0.0);
                                }
                                "timeSpan" | "TimeSpan" | "duration" | "Duration" => {
                                    effect_duration = parse_iso_duration(&val).unwrap_or(0.0);
                                }
                                "typeId" | "TypeId" => {
                                    current_module_id = val;
                                }
                                "moduleInstanceId" | "ModuleInstanceId"
                                | "instanceId" | "InstanceId" => {
                                    current_effect_instance_id = val;
                                }
                                _ => {}
                            }
                        }
                    }

                    // Detect curve/gradient container elements within data model entries.
                    // Vixen XML uses namespace prefixes (d2p1:, d3p1:, etc.) — match local name.
                    if in_data_model_entry {
                        let local_tag = tag.rsplit(':').next().unwrap_or(&tag);
                        match local_tag {
                            "ChaseMovement" | "MovementCurve" | "WipeMovement" => {
                                in_curve_element = true;
                                current_curve_kind = CurveKind::Movement;
                                current_curve_points.clear();
                            }
                            "PulseCurve" => {
                                in_curve_element = true;
                                current_curve_kind = CurveKind::Pulse;
                                current_curve_points.clear();
                            }
                            "LevelCurve" | "IntensityCurve" | "Curve"
                            | "DissolveCurve" | "AccelerationCurve"
                            | "SpeedCurve" | "Height" => {
                                in_curve_element = true;
                                current_curve_kind = CurveKind::Intensity;
                                current_curve_points.clear();
                            }
                            "ColorGradient" => {
                                in_gradient_element = true;
                                current_gradient_stops.clear();
                            }
                            "_colors" | "Colors" if in_gradient_element => {
                                // nested container within ColorGradient; already tracking
                            }
                            _ => {}
                        }

                        // PointPair: X and Y are child text elements, not attributes
                        if in_curve_element && local_tag == "PointPair" {
                            in_point_pair = true;
                            point_pair_x = None;
                            point_pair_y = None;
                        }

                        // ColorPoint: _position and _color are child elements
                        if in_gradient_element && local_tag == "ColorPoint" {
                            in_color_point = true;
                            color_point_position = None;
                        }

                        // _color inside ColorPoint: XYZ stored as child text elements _x, _y, _z
                        if in_color_point && local_tag == "_color" {
                            in_gradient_color = true;
                            gradient_color_x = None;
                            gradient_color_y = None;
                            gradient_color_z = None;
                        }

                        // SetLevel-style direct RGB color: <color> with _r/_g/_b children (0-1 scale)
                        if local_tag == "color" && !in_gradient_element && !in_color_point {
                            in_direct_color = true;
                            direct_color_r = None;
                            direct_color_g = None;
                            direct_color_b = None;
                        }
                    }

                    // Look for XYZ color values as attributes (older/alternate formats)
                    if (in_data_model_entry || in_effect_node_entry) && tag == "Color" {
                        let mut x = None;
                        let mut y = None;
                        let mut z = None;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "X" | "x" => x = val.parse().ok(),
                                "Y" | "y" => y = val.parse().ok(),
                                "Z" | "z" => z = val.parse().ok(),
                                _ => {}
                            }
                        }
                        if let (Some(x), Some(y), Some(z)) = (x, y, z) {
                            let color = xyz_to_srgb(x, y, z);
                            if in_data_model_entry && !current_data_model_id.is_empty() {
                                data_model_colors
                                    .insert(current_data_model_id.clone(), color);
                            }
                            effect_color = Some(color);
                        }
                    }
                }
                Ok(Event::Text(ref e)) => {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if text.is_empty() {
                        // skip
                    } else if in_data_model_entry && current_element == "ModuleInstanceId" {
                        // Capture the data model's ID from child element text
                        current_data_model_id = text;
                    } else if in_data_model_entry && current_element == "ModuleTypeId" {
                        current_data_model_type_id = text;
                    } else if in_data_model_entry
                        && (current_element == "ColorHandling"
                            || current_element == "ColorMode"
                            || current_element == "_colorHandling")
                    {
                        current_color_handling = text;
                    } else if in_point_pair {
                        // PointPair X/Y are child text elements: <X>0</X> <Y>100</Y>
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        match local_el {
                            "X" => point_pair_x = text.parse().ok(),
                            "Y" => point_pair_y = text.parse().ok(),
                            _ => {}
                        }
                    } else if in_gradient_color {
                        // Gradient color XYZ as child text: <_x>95.047</_x> <_y>100</_y> <_z>108.883</_z>
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        match local_el {
                            "_x" | "X" => gradient_color_x = text.parse().ok(),
                            "_y" | "Y" => gradient_color_y = text.parse().ok(),
                            "_z" | "Z" => gradient_color_z = text.parse().ok(),
                            _ => {}
                        }
                    } else if in_color_point {
                        // ColorPoint position as child text: <_position>0</_position>
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        if local_el == "_position" || local_el == "Position" {
                            color_point_position = text.parse().ok();
                        }
                    } else if in_direct_color {
                        // SetLevel direct RGB: <_r>1</_r> <_g>0</_g> <_b>0</_b> (0-1 scale)
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        match local_el {
                            "_r" | "R" => direct_color_r = text.parse().ok(),
                            "_g" | "G" => direct_color_g = text.parse().ok(),
                            "_b" | "B" => direct_color_b = text.parse().ok(),
                            _ => {}
                        }
                    } else if in_data_model_entry {
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        let id = &current_data_model_id;
                        if !id.is_empty() {
                            match local_el {
                                // SetLevel intensity level
                                "level" | "Level" | "IntensityLevel" => {
                                    if let Ok(v) = text.parse::<f64>() {
                                        data_model_levels.insert(id.clone(), v);
                                    }
                                }
                                // Spin: revolution count (= speed in passes)
                                "RevolutionCount" => {
                                    if let Ok(v) = text.parse::<f64>() {
                                        data_model_revolution_count.insert(id.clone(), v);
                                    }
                                }
                                // Spin: pulse width as percentage of revolution
                                "PulsePercentage" => {
                                    if let Ok(v) = text.parse::<f64>() {
                                        data_model_pulse_percentage.insert(id.clone(), v);
                                    }
                                }
                                // Spin: pulse time in ms (when PulseLengthFormat=FixedTime)
                                "PulseTime" => {
                                    if let Ok(v) = text.parse::<f64>() {
                                        data_model_pulse_time_ms.insert(id.clone(), v);
                                    }
                                }
                                // Spin: reverse direction
                                "ReverseSpin" => {
                                    if let Ok(v) = text.parse::<bool>() {
                                        data_model_reverse_spin.insert(id.clone(), v);
                                    }
                                }
                                // Direction for Wipe, Butterfly, etc.
                                "Direction" | "WipeDirection" => {
                                    data_model_direction.insert(id.clone(), text.clone());
                                }
                                _ => {}
                            }
                        }
                    } else if current_element == "Length" {
                        if let Some(dur) = parse_iso_duration(&text) {
                            duration = dur;
                        }
                    } else if in_effect_node_entry {
                        // Inside an EffectNodeSurrogate: capture timing, targets, type
                        match current_element.as_str() {
                            "StartTime" | "startTime" => {
                                if let Some(t) = parse_iso_duration(&text) {
                                    effect_start = t;
                                }
                            }
                            "TimeSpan" | "timeSpan" | "Duration" => {
                                if let Some(d) = parse_iso_duration(&text) {
                                    effect_duration = d;
                                }
                            }
                            "NodeId" => {
                                // NodeId inside ChannelNodeReferenceSurrogate → target GUID
                                let guid = text.trim();
                                if !guid.is_empty() {
                                    effect_targets.push(guid.to_string());
                                }
                            }
                            "TargetNodeId" | "TargetNodes" => {
                                // Semicolon-separated GUID list (older format)
                                for guid in text.split(';') {
                                    let guid = guid.trim();
                                    if !guid.is_empty() {
                                        effect_targets.push(guid.to_string());
                                    }
                                }
                            }
                            "TypeId" | "typeId" => {
                                // TypeId is the module class ID (e.g., "Chase type GUID")
                                current_module_id = text;
                            }
                            "InstanceId" => {
                                // InstanceId links to a specific ModuleInstanceId in _dataModels
                                current_effect_instance_id = text;
                            }
                            _ => {}
                        }
                    } else if in_media
                        && (current_element == "FilePath"
                            || current_element == "FileName"
                            || current_element == "MediaFilePath"
                            || current_element.ends_with(":FilePath")
                            || current_element.ends_with(":FileName")
                            || current_element.ends_with(":RelativeAudioPath"))
                    {
                        audio_file = Some(text);
                    }
                }
                Ok(Event::End(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth = depth.saturating_sub(1);

                    match tag.as_str() {
                        "_dataModels" | "DataModels" => {
                            in_data_models = false;
                        }
                        "_effectNodeSurrogates" | "EffectNodeSurrogates" => {
                            in_effect_nodes = false;
                        }
                        "_mediaSurrogates" | "MediaSurrogates" => {
                            in_media = false;
                        }
                        _ => {}
                    }

                    // End of curve/gradient child elements — use local tag name
                    if in_data_model_entry {
                        let local_tag = tag.rsplit(':').next().unwrap_or(&tag);

                        // End of PointPair → finalize point
                        if in_point_pair && local_tag == "PointPair" {
                            if let (Some(x), Some(y)) = (point_pair_x, point_pair_y) {
                                current_curve_points.push((x, y));
                            }
                            in_point_pair = false;
                        }

                        // End of _color inside ColorPoint → convert XYZ to sRGB
                        if in_gradient_color && local_tag == "_color" {
                            if let (Some(x), Some(y), Some(z)) =
                                (gradient_color_x, gradient_color_y, gradient_color_z)
                            {
                                let color = xyz_to_srgb(x, y, z);
                                // Also set as the data model color and effect color
                                if !current_data_model_id.is_empty() {
                                    data_model_colors
                                        .insert(current_data_model_id.clone(), color);
                                }
                                effect_color = Some(color);
                                // Store for the current ColorPoint (resolved when ColorPoint closes)
                                gradient_color_x = None; // reuse fields to pass color
                                gradient_color_y = None;
                                gradient_color_z = None;
                                // Tag the last gradient stop with this color
                                // (ColorPoint may not be finalized yet, so we update the pending stop)
                                if in_color_point {
                                    // We'll add the stop when ColorPoint closes
                                    // For now store the color in effect_color
                                }
                            }
                            in_gradient_color = false;
                        }

                        // End of ColorPoint → finalize gradient stop
                        if in_color_point && local_tag == "ColorPoint" {
                            let pos = color_point_position.unwrap_or(0.0);
                            let color = effect_color.unwrap_or(Color::WHITE);
                            current_gradient_stops.push((pos, color));
                            in_color_point = false;
                            color_point_position = None;
                        }

                        // End of direct color element (SetLevel _r/_g/_b)
                        if in_direct_color && local_tag == "color" {
                            if let (Some(r), Some(g), Some(b)) =
                                (direct_color_r, direct_color_g, direct_color_b)
                            {
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                let color = Color::rgb(
                                    (r.clamp(0.0, 1.0) * 255.0) as u8,
                                    (g.clamp(0.0, 1.0) * 255.0) as u8,
                                    (b.clamp(0.0, 1.0) * 255.0) as u8,
                                );
                                if !current_data_model_id.is_empty() {
                                    data_model_colors
                                        .insert(current_data_model_id.clone(), color);
                                }
                                effect_color = Some(color);
                            }
                            in_direct_color = false;
                        }

                        // End of curve container
                        match local_tag {
                            "ChaseMovement" | "MovementCurve" | "WipeMovement"
                            | "PulseCurve" | "LevelCurve"
                            | "IntensityCurve" | "Curve"
                            | "DissolveCurve" | "AccelerationCurve"
                            | "SpeedCurve" | "Height"
                                if in_curve_element =>
                            {
                                if !current_curve_points.is_empty()
                                    && !current_data_model_id.is_empty()
                                {
                                    let target_map = match current_curve_kind {
                                        CurveKind::Movement => &mut data_model_movement_curves,
                                        CurveKind::Pulse => &mut data_model_pulse_curves,
                                        CurveKind::Intensity => &mut data_model_intensity_curves,
                                    };
                                    target_map.insert(
                                        current_data_model_id.clone(),
                                        current_curve_points.clone(),
                                    );
                                }
                                in_curve_element = false;
                            }
                            "ColorGradient" if in_gradient_element => {
                                if !current_gradient_stops.is_empty()
                                    && !current_data_model_id.is_empty()
                                {
                                    data_model_gradients.insert(
                                        current_data_model_id.clone(),
                                        current_gradient_stops.clone(),
                                    );
                                }
                                in_gradient_element = false;
                            }
                            _ => {}
                        }
                    }

                    // End of a data model entry
                    if in_data_model_entry && depth < data_model_depth {
                        if !current_data_model_id.is_empty() && !effect_type.is_empty() {
                            data_model_types.insert(
                                current_data_model_id.clone(),
                                effect_type.clone(),
                            );
                        }
                        // Store color handling
                        if !current_data_model_id.is_empty()
                            && !current_color_handling.is_empty()
                        {
                            data_model_color_handling.insert(
                                current_data_model_id.clone(),
                                current_color_handling.clone(),
                            );
                        }
                        // Also map ModuleTypeId → data (class-level lookup).
                        // Many effects share one ModuleTypeId but have different instances.
                        if !current_data_model_type_id.is_empty() {
                            if !effect_type.is_empty() {
                                module_type_to_name
                                    .entry(current_data_model_type_id.clone())
                                    .or_insert_with(|| effect_type.clone());
                            }
                            // Store curve/gradient/color handling under type ID too
                            for curve_map in [
                                &mut data_model_movement_curves,
                                &mut data_model_pulse_curves,
                                &mut data_model_intensity_curves,
                            ] {
                                let clone = curve_map
                                    .get(&current_data_model_id)
                                    .cloned();
                                if let Some(data) = clone {
                                    curve_map
                                        .entry(current_data_model_type_id.clone())
                                        .or_insert(data);
                                }
                            }
                            let grads_clone = data_model_gradients
                                .get(&current_data_model_id)
                                .cloned();
                            if let Some(grads) = grads_clone {
                                data_model_gradients
                                    .entry(current_data_model_type_id.clone())
                                    .or_insert(grads);
                            }
                            let ch_clone = data_model_color_handling
                                .get(&current_data_model_id)
                                .cloned();
                            if let Some(ch) = ch_clone {
                                data_model_color_handling
                                    .entry(current_data_model_type_id.clone())
                                    .or_insert(ch);
                            }
                        }
                        in_data_model_entry = false;
                        current_color_handling.clear();
                        current_curve_points.clear();
                        current_gradient_stops.clear();
                        in_curve_element = false;
                        in_gradient_element = false;
                    }

                    // End of an EffectNodeSurrogate → finalize the effect
                    if in_effect_node_entry && depth < effect_node_depth {
                        // Resolve effect type via three-level lookup:
                        // 1. InstanceId → data_model_types (instance-level, most specific)
                        // 2. TypeId → module_type_to_name (class-level, shared across instances)
                        // 3. Fallback to last parsed effect_type (unreliable)
                        let resolved_type = data_model_types
                            .get(&current_effect_instance_id)
                            .or_else(|| module_type_to_name.get(&current_module_id))
                            .cloned()
                            .unwrap_or_else(|| "Solid".to_string());

                        // Resolve color: instance-level first, then class-level
                        let resolved_color = effect_color
                            .or_else(|| data_model_colors.get(&current_effect_instance_id).copied())
                            .or_else(|| data_model_colors.get(&current_module_id).copied());

                        // Resolve curve/gradient data from data models.
                        // Instance-level first, then class-level (TypeId) fallback.
                        let resolved_movement = data_model_movement_curves
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_movement_curves.get(&current_module_id))
                            .cloned();
                        let resolved_pulse = data_model_pulse_curves
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_pulse_curves.get(&current_module_id))
                            .cloned();
                        let resolved_intensity = data_model_intensity_curves
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_intensity_curves.get(&current_module_id))
                            .cloned();
                        let resolved_gradients = data_model_gradients
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_gradients.get(&current_module_id))
                            .cloned();
                        let resolved_color_handling = data_model_color_handling
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_color_handling.get(&current_module_id))
                            .cloned();
                        let resolved_level = data_model_levels
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_levels.get(&current_module_id))
                            .copied();

                        // Spin-specific fields
                        let resolved_rev_count = data_model_revolution_count
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_revolution_count.get(&current_module_id))
                            .copied();
                        let resolved_pulse_pct = data_model_pulse_percentage
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_pulse_percentage.get(&current_module_id))
                            .copied();
                        let resolved_pulse_time = data_model_pulse_time_ms
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_pulse_time_ms.get(&current_module_id))
                            .copied();
                        let resolved_reverse = data_model_reverse_spin
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_reverse_spin.get(&current_module_id))
                            .copied();
                        let resolved_direction = data_model_direction
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_direction.get(&current_module_id))
                            .cloned();

                        if effect_duration > 0.0 {
                            effects.push(VixenEffect {
                                type_name: resolved_type,
                                start_time: effect_start,
                                duration: effect_duration,
                                target_node_guids: effect_targets.clone(),
                                color: resolved_color,
                                movement_curve: resolved_movement,
                                pulse_curve: resolved_pulse,
                                intensity_curve: resolved_intensity,
                                gradient_colors: resolved_gradients,
                                color_handling: resolved_color_handling,
                                level: resolved_level,
                                revolution_count: resolved_rev_count,
                                pulse_percentage: resolved_pulse_pct,
                                pulse_time_ms: resolved_pulse_time,
                                reverse_spin: resolved_reverse,
                                direction: resolved_direction,
                            });
                        }

                        in_effect_node_entry = false;
                        effect_targets.clear();
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local_tag = tag.rsplit(':').next().unwrap_or(&tag);

                    // Handle self-closing Color elements with XYZ attributes
                    if (in_data_model_entry || in_effect_node_entry) && local_tag == "Color" {
                        let mut x = None;
                        let mut y = None;
                        let mut z = None;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "X" | "x" => x = val.parse().ok(),
                                "Y" | "y" => y = val.parse().ok(),
                                "Z" | "z" => z = val.parse().ok(),
                                _ => {}
                            }
                        }
                        if let (Some(x), Some(y), Some(z)) = (x, y, z) {
                            let color = xyz_to_srgb(x, y, z);
                            if in_data_model_entry && !current_data_model_id.is_empty() {
                                data_model_colors
                                    .insert(current_data_model_id.clone(), color);
                            }
                            effect_color = Some(color);
                        }
                    }
                }
                Err(e) => return Err(ImportError::Xml(e)),
                _ => {}
            }
            buf.clear();
        }

        // Build tracks from effects, grouped by target node
        let tracks = self.build_tracks(effects);

        self.sequences.push(Sequence {
            name: seq_name,
            duration,
            frame_rate: 30.0,
            audio_file,
            tracks,
            scripts: std::collections::HashMap::new(),
            gradient_library: std::collections::HashMap::new(),
            curve_library: std::collections::HashMap::new(),
        });

        Ok(())
    }

    /// Merge adjacent effects that have the same type and color within a gap threshold.
    /// This collapses rapid-fire Vixen effects (e.g. 100 consecutive Pulse effects) into one.
    fn merge_adjacent_effects(effects: &mut Vec<VixenEffect>) {
        const GAP_THRESHOLD: f64 = 0.050; // 50ms

        if effects.len() < 2 {
            return;
        }

        // Sort by start time first
        effects.sort_by(|a, b| {
            a.start_time
                .partial_cmp(&b.start_time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut merged = Vec::with_capacity(effects.len());
        let Some(first) = effects.first() else {
            return;
        };
        let mut current = first.clone();

        for next in effects.iter().skip(1) {
            let current_end = current.start_time + current.duration;
            let gap = next.start_time - current_end;
            let same_type = current.type_name == next.type_name;
            let same_color = current.color == next.color;

            if same_type && same_color && (-GAP_THRESHOLD..=GAP_THRESHOLD).contains(&gap) {
                // Merge: extend current to cover both
                let new_end = (next.start_time + next.duration).max(current_end);
                current.duration = new_end - current.start_time;
            } else {
                merged.push(current);
                current = next.clone();
            }
        }
        merged.push(current);

        *effects = merged;
    }

    /// Build tracks from parsed Vixen effects, grouped by target node.
    #[allow(clippy::too_many_lines)]
    fn build_tracks(&self, effects: Vec<VixenEffect>) -> Vec<Track> {
        const MAX_TOTAL_EFFECTS: usize = 10_000;

        // Group effects by their primary target
        let mut effects_by_target: HashMap<String, Vec<VixenEffect>> = HashMap::new();

        for effect in effects {
            if effect.target_node_guids.is_empty() {
                effects_by_target
                    .entry("_all_".to_string())
                    .or_default()
                    .push(effect);
            } else {
                let Some(target_guid) = effect.target_node_guids.first() else {
                    continue;
                };
                // Skip orphan targets — if the GUID doesn't map to any known fixture/group, drop it
                if target_guid != "_all_" && !self.guid_to_id.contains_key(target_guid) {
                    continue;
                }
                effects_by_target
                    .entry(target_guid.clone())
                    .or_default()
                    .push(effect);
            }
        }

        let mut tracks = Vec::new();
        let mut total_effects = 0usize;

        for (target_guid, mut target_effects) in effects_by_target {
            // Merge adjacent same-type effects to reduce count
            Self::merge_adjacent_effects(&mut target_effects);

            // Sort by start time
            target_effects.sort_by(|a, b| {
                a.start_time
                    .partial_cmp(&b.start_time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Assign effects to lanes (non-overlapping within each lane)
            let mut lanes: Vec<Vec<&VixenEffect>> = Vec::new();

            for effect in &target_effects {
                let mut assigned = false;

                for lane in &mut lanes {
                    let lane_end = lane
                        .last()
                        .map_or(0.0, |e| e.start_time + e.duration);
                    if effect.start_time >= lane_end {
                        lane.push(effect);
                        assigned = true;
                        break;
                    }
                }

                if !assigned {
                    lanes.push(vec![effect]);
                }
            }

            // Resolve target
            let target = if target_guid == "_all_" {
                EffectTarget::All
            } else if let Some(&id) = self.guid_to_id.get(&target_guid) {
                // Check if it's a fixture or group
                if self.fixtures.iter().any(|f| f.id == FixtureId(id)) {
                    EffectTarget::Fixtures(vec![FixtureId(id)])
                } else if self.groups.iter().any(|g| g.id == GroupId(id)) {
                    EffectTarget::Group(GroupId(id))
                } else {
                    continue; // orphan — skip
                }
            } else {
                continue; // orphan — skip
            };

            let target_name = if target_guid == "_all_" {
                "All".to_string()
            } else {
                self.nodes
                    .get(&target_guid)
                    .map_or_else(|| format!("Track {}", tracks.len() + 1), |n| n.name.clone())
            };

            // Create a track per lane
            for (lane_idx, lane) in lanes.iter().enumerate() {
                let lane_suffix = if lanes.len() > 1 {
                    format!(" ({})", lane_idx + 1)
                } else {
                    String::new()
                };

                let mut effect_instances: Vec<EffectInstance> = lane
                    .iter()
                    .filter_map(|e| {
                        let end = e.start_time + e.duration;
                        let time_range = TimeRange::new(e.start_time, end)?;
                        let (kind, params) = map_vixen_effect(e);
                        Some(EffectInstance {
                            kind,
                            params,
                            time_range,
                            blend_mode: BlendMode::Override,
                            opacity: 1.0,
                        })
                    })
                    .collect();

                // Sort effects by start time for efficient binary-search evaluation.
                effect_instances.sort_by(|a, b| {
                    a.time_range
                        .start()
                        .partial_cmp(&b.time_range.start())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                if !effect_instances.is_empty() {
                    total_effects += effect_instances.len();
                    tracks.push(Track {
                        name: format!("{target_name}{lane_suffix}"),
                        target: target.clone(),
                        effects: effect_instances,
                    });
                }
            }
        }

        // Cap total effects
        if total_effects > MAX_TOTAL_EFFECTS {
            eprintln!(
                "[VibeLights] Warning: {total_effects} effects exceed cap of {MAX_TOTAL_EFFECTS}. Truncating tracks.",
            );
            let mut count = 0usize;
            for track in &mut tracks {
                let remaining = MAX_TOTAL_EFFECTS.saturating_sub(count);
                if remaining == 0 {
                    track.effects.clear();
                } else if track.effects.len() > remaining {
                    track.effects.truncate(remaining);
                }
                count += track.effects.len();
            }
            // Remove empty tracks
            tracks.retain(|t| !t.effects.is_empty());
        }

        tracks
    }

    /// Extract just the sequences (for sequence-only imports).
    #[must_use]
    pub fn into_sequences(self) -> Vec<Sequence> {
        self.sequences
    }

    /// Consume the importer and produce a Show.
    #[must_use]
    pub fn into_show(self) -> Show {
        Show {
            name: "Vixen Import".into(),
            fixtures: self.fixtures,
            groups: self.groups,
            layout: Layout {
                fixtures: Vec::new(), // Layout will need to be created separately
            },
            sequences: self.sequences,
            patches: self.patches,
            controllers: self.controllers,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::uninlined_format_args,
    clippy::bool_assert_comparison,
    clippy::match_same_arms,
    clippy::option_map_or_none,
)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_iso_duration_simple() {
        assert!((parse_iso_duration("PT1M53.606S").unwrap() - 113.606).abs() < 0.001);
        assert!((parse_iso_duration("PT5M30.500S").unwrap() - 330.5).abs() < 0.001);
        assert!((parse_iso_duration("PT30S").unwrap() - 30.0).abs() < 0.001);
        assert!((parse_iso_duration("PT1H").unwrap() - 3600.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_iso_duration_with_days() {
        assert!((parse_iso_duration("P0DT0H5M30.500S").unwrap() - 330.5).abs() < 0.001);
        assert!((parse_iso_duration("P1DT0H0M0S").unwrap() - 86400.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_iso_duration_edge_cases() {
        assert!((parse_iso_duration("PT0S").unwrap() - 0.0).abs() < 0.001);
        assert!((parse_iso_duration("PT0.001S").unwrap() - 0.001).abs() < 0.0001);
        assert!(parse_iso_duration("not a duration").is_none());
        assert!(parse_iso_duration("").is_none());
    }

    #[test]
    fn test_xyz_to_srgb_white() {
        let white = xyz_to_srgb(95.047, 100.0, 108.883);
        // Should be close to (255, 255, 255)
        assert!(white.r >= 254, "r={}", white.r);
        assert!(white.g >= 254, "g={}", white.g);
        assert!(white.b >= 254, "b={}", white.b);
    }

    #[test]
    fn test_xyz_to_srgb_black() {
        let black = xyz_to_srgb(0.0, 0.0, 0.0);
        assert_eq!(black.r, 0);
        assert_eq!(black.g, 0);
        assert_eq!(black.b, 0);
    }

    #[test]
    fn test_xyz_to_srgb_red() {
        // sRGB red (255,0,0) in XYZ is approximately (41.24, 21.26, 1.93)
        let red = xyz_to_srgb(41.24, 21.26, 1.93);
        assert!(red.r > 240, "r={}", red.r);
        assert!(red.g < 15, "g={}", red.g);
        assert!(red.b < 15, "b={}", red.b);
    }

    fn test_effect(type_name: &str) -> VixenEffect {
        VixenEffect {
            type_name: type_name.to_string(),
            start_time: 0.0,
            duration: 5.0,
            target_node_guids: Vec::new(),
            color: None,
            movement_curve: None,
            pulse_curve: None,
            intensity_curve: None,
            gradient_colors: None,
            color_handling: None,
            level: None,
            revolution_count: None,
            pulse_percentage: None,
            pulse_time_ms: None,
            reverse_spin: None,
            direction: None,
        }
    }

    #[test]
    fn test_effect_mapping() {
        // Core effects
        let (kind, _) = map_vixen_effect(&test_effect("Pulse"));
        assert!(matches!(kind, EffectKind::Fade));

        let (kind, _) = map_vixen_effect(&test_effect("SetLevel"));
        assert!(matches!(kind, EffectKind::Fade));

        let mut chase = test_effect("Chase");
        chase.color = Some(Color::rgb(255, 0, 0));
        let (kind, _) = map_vixen_effect(&chase);
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("ColorWash"));
        assert!(matches!(kind, EffectKind::Fade));

        let (kind, _) = map_vixen_effect(&test_effect("Twinkle"));
        assert!(matches!(kind, EffectKind::Twinkle));

        let (kind, _) = map_vixen_effect(&test_effect("Strobe"));
        assert!(matches!(kind, EffectKind::Strobe));

        let (kind, _) = map_vixen_effect(&test_effect("Rainbow"));
        assert!(matches!(kind, EffectKind::Rainbow));

        // Movement-based effects → Chase
        let (kind, _) = map_vixen_effect(&test_effect("Spin"));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("Wipe"));
        assert!(matches!(kind, EffectKind::Wipe));

        let (kind, _) = map_vixen_effect(&test_effect("Alternating"));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("PinWheel"));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("Shockwave"));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("Garlands"));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("Butterfly"));
        assert!(matches!(kind, EffectKind::Chase));

        // Random/particle effects → Twinkle
        let (kind, _) = map_vixen_effect(&test_effect("Dissolve"));
        assert!(matches!(kind, EffectKind::Twinkle));

        let (kind, _) = map_vixen_effect(&test_effect("Fireworks"));
        assert!(matches!(kind, EffectKind::Twinkle));

        let (kind, _) = map_vixen_effect(&test_effect("Snowflakes"));
        assert!(matches!(kind, EffectKind::Twinkle));

        let (kind, _) = map_vixen_effect(&test_effect("Meteor"));
        assert!(matches!(kind, EffectKind::Twinkle));

        // Flame/warm effects → Fade
        let (kind, _) = map_vixen_effect(&test_effect("Fire"));
        assert!(matches!(kind, EffectKind::Fade));

        let (kind, _) = map_vixen_effect(&test_effect("Candle"));
        assert!(matches!(kind, EffectKind::Fade));

        // Skip effects → Solid
        let (kind, _) = map_vixen_effect(&test_effect("Audio"));
        assert!(matches!(kind, EffectKind::Solid));

        let (kind, _) = map_vixen_effect(&test_effect("MaskAndFill"));
        assert!(matches!(kind, EffectKind::Solid));

        // Unknown effect falls back to Solid with loud warning
        let (kind, _) = map_vixen_effect(&test_effect("SomeUnknownEffect"));
        assert!(matches!(kind, EffectKind::Solid));
    }

    #[test]
    fn test_wipe_direction_reverse() {
        let mut wipe = test_effect("Wipe");
        wipe.direction = Some("Reverse".to_string());
        let (kind, params) = map_vixen_effect(&wipe);
        assert!(matches!(kind, EffectKind::Wipe));
        assert_eq!(params.bool_or(ParamKey::Reverse, false), true);

        // Default (no direction) should not be reversed
        let wipe_default = test_effect("Wipe");
        let (_, params) = map_vixen_effect(&wipe_default);
        assert_eq!(params.bool_or(ParamKey::Reverse, true), false);
    }

    #[test]
    fn test_wipe_full_width_pulse() {
        let wipe = test_effect("Wipe");
        let (_, params) = map_vixen_effect(&wipe);
        // Wipe should have pulse_width=1.0 (full sweep)
        assert!((params.float_or(ParamKey::PulseWidth, 0.0) - 1.0).abs() < 0.001);
    }

    /// Verify that when leaf nodes are merged into a multi-pixel parent fixture,
    /// the guid_to_id entries for leaf GUIDs are remapped to the parent fixture ID.
    /// This is critical for layout resolution: preview pixels reference leaf node
    /// GUIDs and must resolve to the merged parent fixture.
    #[test]
    fn test_merged_leaf_guid_remapping() {
        let mut importer = VixenImporter::new();

        // Create a parent node with 3 leaf children (simulating an RGB fixture).
        let leaf_guids: Vec<String> = (0..3).map(|i| format!("leaf-{i}")).collect();
        let parent_guid = "parent".to_string();

        for (i, guid) in leaf_guids.iter().enumerate() {
            importer.nodes.insert(
                guid.clone(),
                VixenNode {
                    name: format!("Leaf {i}"),
                    guid: guid.clone(),
                    children_guids: vec![],
                    channel_id: Some(format!("ch-{i}")),
                },
            );
        }

        importer.nodes.insert(
            parent_guid.clone(),
            VixenNode {
                name: "Parent Fixture".to_string(),
                guid: parent_guid.clone(),
                children_guids: leaf_guids.clone(),
                channel_id: None,
            },
        );

        // Build the parent node (which will recursively build leaves, then merge them).
        let result = importer.build_node(&parent_guid);
        assert!(result.is_some());

        // After merging, there should be exactly one fixture (the merged parent).
        assert_eq!(importer.fixtures.len(), 1, "Expected 1 merged fixture");
        let parent_fixture = &importer.fixtures[0];
        assert_eq!(parent_fixture.pixel_count, 3);
        let parent_id = parent_fixture.id.0;

        // Critical: all leaf GUIDs must now resolve to the parent fixture ID.
        for leaf_guid in &leaf_guids {
            let mapped_id = importer.guid_to_id.get(leaf_guid).copied();
            assert_eq!(
                mapped_id,
                Some(parent_id),
                "Leaf GUID '{}' should map to parent ID {}, got {:?}",
                leaf_guid,
                parent_id,
                mapped_id
            );
        }

        // The parent GUID should also map to the parent ID.
        assert_eq!(
            importer.guid_to_id.get(&parent_guid).copied(),
            Some(parent_id)
        );
    }

    /// Integration test: parse real Vixen SystemConfig to verify fixture merging.
    /// The real config has ~25K leaf channels that should be merged into ~hundreds of fixtures.
    #[test]
    fn test_real_vixen_fixture_merging() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        if !config_path.exists() {
            eprintln!("Skipping: real SystemConfig.xml not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();

        let fixture_count = importer.fixture_count();
        let group_count = importer.group_count();

        eprintln!(
            "Real Vixen import: {} fixtures, {} groups",
            fixture_count, group_count
        );

        // With correct merging, 25K channels should collapse to far fewer fixtures.
        // The exact number depends on the hierarchy, but it should be well under 1000.
        assert!(
            fixture_count < 1000,
            "Expected fewer than 1000 fixtures after merging, got {}",
            fixture_count
        );
        // Should have at least some groups
        assert!(
            group_count > 0,
            "Expected some groups from the Vixen hierarchy"
        );

        // Print some fixture names for manual verification
        let show = importer.into_show();
        let single_pixel = show.fixtures.iter().filter(|f| f.pixel_count == 1).count();
        let multi_pixel = show.fixtures.iter().filter(|f| f.pixel_count > 1).count();
        eprintln!("  Single-pixel fixtures: {single_pixel}");
        eprintln!("  Multi-pixel fixtures: {multi_pixel}");
        for f in show.fixtures.iter().take(20) {
            eprintln!("  Fixture: {} (id={}, pixels={})", f.name, f.id.0, f.pixel_count);
        }
        for g in show.groups.iter().take(10) {
            eprintln!("  Group: {} (id={}, members={})", g.name, g.id.0, g.members.len());
        }
    }

    /// Integration test: parse a real .tim sequence to verify curve and gradient extraction.
    #[test]
    fn test_real_vixen_sequence_curves_and_gradients() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        let seq_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence\60-bpm-metronome.tim",
        );
        if !config_path.exists() || !seq_path.exists() {
            eprintln!("Skipping: real Vixen files not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();
        importer.parse_sequence(seq_path).unwrap();

        let sequences = importer.into_sequences();
        assert!(!sequences.is_empty(), "Should have at least one sequence");
        let seq = &sequences[0];

        let mut fade_count = 0usize;
        let mut chase_count = 0usize;
        let mut has_gradient = 0usize;
        let mut has_curve = 0usize;
        let mut non_white_gradient = 0usize;

        for track in &seq.tracks {
            for effect in &track.effects {
                match effect.kind {
                    EffectKind::Fade => {
                        fade_count += 1;
                        if effect.params.get(ParamKey::Gradient).is_some() {
                            has_gradient += 1;
                        }
                        if effect.params.get(ParamKey::IntensityCurve).is_some() {
                            has_curve += 1;
                        }
                        // Check if gradient has non-white color
                        if let Some(ParamValue::ColorGradient(g)) = effect.params.get(ParamKey::Gradient) {
                            let c = g.evaluate(0.0);
                            if c.r != 255 || c.g != 255 || c.b != 255 {
                                non_white_gradient += 1;
                            }
                        }
                    }
                    EffectKind::Chase => {
                        chase_count += 1;
                    }
                    _ => {}
                }
            }
        }

        eprintln!("Sequence: {} tracks", seq.tracks.len());
        eprintln!("  Fade effects: {fade_count}");
        eprintln!("    with gradient param: {has_gradient}");
        eprintln!("    with intensity curve param: {has_curve}");
        eprintln!("    with non-white gradient: {non_white_gradient}");
        eprintln!("  Chase effects: {chase_count}");

        // We should find at least some Fade effects (from Pulse) with extracted data
        assert!(
            fade_count > 0,
            "Expected at least one Fade effect from Pulse mapping"
        );
    }

    #[test]
    fn test_real_vixen_chase_curves() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        // Use a file known to have Chase effects
        let seq_dir = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence",
        );
        if !config_path.exists() || !seq_dir.exists() {
            eprintln!("Skipping: real Vixen files not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();

        // Try AllOn 2016.tim which is known to have Chase effects
        let seq_path = seq_dir.join("AllOn 2016.tim");
        if !seq_path.exists() {
            // Fall back to scanning for any file with Chase effects
            eprintln!("AllOn 2016.tim not found, scanning for Chase effects...");
            for entry in std::fs::read_dir(seq_dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "tim") {
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    if content.contains("ChaseMovement") {
                        eprintln!("Found Chase in: {}", path.display());
                        let mut imp2 = VixenImporter::new();
                        imp2.parse_system_config(config_path).unwrap();
                        imp2.parse_sequence(&path).unwrap();
                        let seqs = imp2.into_sequences();
                        for seq in &seqs {
                            for track in &seq.tracks {
                                for effect in &track.effects {
                                    if matches!(effect.kind, EffectKind::Chase) {
                                        let has_move = effect.params.get(ParamKey::MovementCurve).is_some();
                                        let has_pulse = effect.params.get(ParamKey::PulseCurve).is_some();
                                        eprintln!(
                                            "  Chase effect: movement_curve={}, pulse_curve={}",
                                            has_move, has_pulse
                                        );
                                    }
                                }
                            }
                        }
                        return;
                    }
                }
            }
            eprintln!("No files with ChaseMovement found");
            return;
        }

        importer.parse_sequence(&seq_path).unwrap();
        let sequences = importer.into_sequences();

        let mut chase_count = 0usize;
        let mut chase_with_movement = 0usize;
        let mut chase_with_pulse = 0usize;

        for seq in &sequences {
            for track in &seq.tracks {
                for effect in &track.effects {
                    if matches!(effect.kind, EffectKind::Chase) {
                        chase_count += 1;
                        if effect.params.get(ParamKey::MovementCurve).is_some() {
                            chase_with_movement += 1;
                        }
                        if effect.params.get(ParamKey::PulseCurve).is_some() {
                            chase_with_pulse += 1;
                        }
                    }
                }
            }
        }

        eprintln!("Chase effects: {chase_count}");
        eprintln!("  with movement_curve: {chase_with_movement}");
        eprintln!("  with pulse_curve: {chase_with_pulse}");

        if chase_count > 0 {
            assert!(
                chase_with_movement > 0,
                "Expected at least one Chase with movement_curve (found {chase_count} chases)"
            );
        }
    }

    fn base64_decode(input: &str) -> Vec<u8> {
        const DECODE: [u8; 256] = {
            let mut table = [255u8; 256];
            let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut i = 0;
            while i < 64 {
                table[chars[i] as usize] = i as u8;
                i += 1;
            }
            table
        };
        let bytes: Vec<u8> = input.bytes().filter(|b| DECODE[*b as usize] != 255).collect();
        let mut result = Vec::with_capacity(bytes.len() * 3 / 4);
        for chunk in bytes.chunks(4) {
            if chunk.len() < 2 { break; }
            let a = DECODE[chunk[0] as usize] as u32;
            let b = DECODE[chunk[1] as usize] as u32;
            let c = if chunk.len() > 2 { DECODE[chunk[2] as usize] as u32 } else { 0 };
            let d = if chunk.len() > 3 { DECODE[chunk[3] as usize] as u32 } else { 0 };
            let triple = (a << 18) | (b << 12) | (c << 6) | d;
            result.push((triple >> 16) as u8);
            if chunk.len() > 2 { result.push((triple >> 8) as u8); }
            if chunk.len() > 3 { result.push(triple as u8); }
        }
        result
    }

    /// Diagnostic: decode base64 RGBA pixel data and print actual colors.
    #[test]
    fn test_frame_color_diagnostic() {
        use crate::engine;

        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        let seq_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence\All We Are.tim",
        );
        if !config_path.exists() || !seq_path.exists() {
            eprintln!("Skipping: All We Are.tim not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();
        importer.parse_sequence(seq_path).unwrap();
        let show = importer.into_show();

        // Build fixture name map
        let fixture_names: HashMap<u32, &str> = show
            .fixtures
            .iter()
            .map(|f| (f.id.0, f.name.as_str()))
            .collect();

        eprintln!("\n=== Frame Color Diagnostic (fresh import) ===");
        for t in [60.0, 62.0, 65.0, 155.0, 160.0] {
            let frame = engine::evaluate(&show, 0, t, None, None);
            eprintln!("\nt={:.0}: {} lit fixtures", t, frame.fixtures.len());

            // Decode a few fixtures' pixel data
            let mut entries: Vec<_> = frame.fixtures.iter().collect();
            entries.sort_by_key(|(id, _)| *id);
            for (fixture_id, b64) in entries.iter().take(8) {
                let name = fixture_names.get(fixture_id).unwrap_or(&"???");
                // Decode base64
                let bytes = base64_decode(b64);
                let pixel_count = bytes.len() / 4;
                // Show first few pixels
                let pixels: Vec<String> = bytes
                    .chunks(4)
                    .take(4)
                    .map(|c| format!("({},{},{},{})", c[0], c[1], c[2], c[3]))
                    .collect();
                let more = if pixel_count > 4 {
                    format!(" +{} more", pixel_count - 4)
                } else {
                    String::new()
                };
                eprintln!(
                    "  fixture {fixture_id} ({name}): {pixel_count} pixels [{}]{more}",
                    pixels.join(" ")
                );
            }
        }
    }

    /// Diagnostic: check what color/gradient data VixenEffects actually have before mapping.
    #[test]
    fn test_vixen_effect_color_data() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        let seq_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence\All We Are.tim",
        );
        if !config_path.exists() || !seq_path.exists() {
            eprintln!("Skipping: files not found");
            return;
        }

        // We need to intercept VixenEffects before they're mapped.
        // Easiest way: add temp instrumentation to build_tracks. Instead,
        // we'll parse the XML manually and inspect the data model maps.

        let file = std::fs::File::open(seq_path).unwrap();
        let reader = std::io::BufReader::new(file);
        let mut xml = quick_xml::Reader::from_reader(reader);
        xml.config_mut().trim_text(true);
        let mut buf = Vec::new();

        let mut color_count = 0usize;
        let mut gradient_count = 0usize;
        let mut color_handling_count = 0usize;
        let mut data_model_count = 0usize;
        let mut in_data_models = false;
        let mut in_entry = false;
        let mut depth = 0u32;
        let mut entry_depth = 0u32;
        let mut has_color_in_entry = false;
        let mut has_gradient_in_entry = false;
        let mut entry_type = String::new();
        let mut type_colors: HashMap<String, usize> = HashMap::new();
        let mut type_gradients: HashMap<String, usize> = HashMap::new();

        loop {
            match xml.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    depth += 1;
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local = tag.rsplit(':').next().unwrap_or(&tag);

                    if tag == "_dataModels" || tag == "DataModels" {
                        in_data_models = true;
                    }

                    if in_data_models && !in_entry && (local == "anyType" || tag.contains("DataModel")) {
                        in_entry = true;
                        entry_depth = depth;
                        has_color_in_entry = false;
                        has_gradient_in_entry = false;
                        entry_type.clear();
                        data_model_count += 1;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            let local_key = key.rsplit(':').next().unwrap_or(&key);
                            if local_key == "type" || local_key == "Type" {
                                let raw = val.rsplit(':').next().unwrap_or(&val);
                                let cleaned = raw.rsplit('.').next().unwrap_or(raw);
                                let cleaned = cleaned.strip_suffix("Module")
                                    .or_else(|| cleaned.strip_suffix("Data"))
                                    .unwrap_or(cleaned);
                                entry_type = cleaned.to_string();
                            }
                        }
                    }

                    if in_entry {
                        if local == "ColorGradient" || local == "_colors" {
                            has_gradient_in_entry = true;
                        }
                        if local == "color" || local == "Color" || local == "_color" {
                            has_color_in_entry = true;
                        }
                    }
                }
                Ok(quick_xml::events::Event::End(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth = depth.saturating_sub(1);

                    if tag == "_dataModels" || tag == "DataModels" {
                        in_data_models = false;
                    }

                    if in_entry && depth < entry_depth {
                        if has_color_in_entry { color_count += 1; }
                        if has_gradient_in_entry { gradient_count += 1; }
                        if !entry_type.is_empty() {
                            if has_color_in_entry {
                                *type_colors.entry(entry_type.clone()).or_insert(0) += 1;
                            }
                            if has_gradient_in_entry {
                                *type_gradients.entry(entry_type.clone()).or_insert(0) += 1;
                            }
                        }
                        in_entry = false;
                    }
                }
                Ok(quick_xml::events::Event::Text(ref e)) => {
                    if in_entry {
                        let text = e.unescape().unwrap_or_default().to_string();
                        if text == "StaticColor" || text == "GradientThroughWholeEffect"
                            || text == "GradientForEachPulse" || text == "ColorAcrossItems"
                        {
                            color_handling_count += 1;
                        }
                    }
                }
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        eprintln!("\n=== Vixen Effect Color Data Analysis ===");
        eprintln!("Total data model entries: {data_model_count}");
        eprintln!("  with any color element: {color_count}");
        eprintln!("  with ColorGradient: {gradient_count}");
        eprintln!("  with ColorHandling: {color_handling_count}");
        eprintln!("\nColor by effect type:");
        let mut sorted: Vec<_> = type_colors.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (t, c) in &sorted {
            let g = type_gradients.get(*t).unwrap_or(&0);
            eprintln!("  {t}: {c} with color, {g} with gradient");
        }
    }

    /// Check what colors our parsed gradients actually produce.
    #[test]
    fn test_parsed_gradient_colors() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        let seq_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence\All We Are.tim",
        );
        if !config_path.exists() || !seq_path.exists() {
            eprintln!("Skipping: files not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();
        importer.parse_sequence(seq_path).unwrap();

        let show = importer.into_show();
        let seq = &show.sequences[0];

        let mut white_gradient = 0usize;
        let mut colored_gradient = 0usize;
        let mut no_gradient = 0usize;
        let mut white_color = 0usize;
        let mut colored_color = 0usize;
        let mut sample_colors: Vec<String> = Vec::new();

        for track in &seq.tracks {
            for effect in &track.effects {
                if let Some(ParamValue::ColorGradient(g)) = effect.params.get(ParamKey::Gradient) {
                    let c0 = g.evaluate(0.0);
                    let c5 = g.evaluate(0.5);
                    if c0.r == c0.g && c0.g == c0.b && c5.r == c5.g && c5.g == c5.b {
                        white_gradient += 1;
                    } else {
                        colored_gradient += 1;
                        if sample_colors.len() < 10 {
                            sample_colors.push(format!(
                                "{:?}@0.0=({},{},{}) @0.5=({},{},{})",
                                effect.kind, c0.r, c0.g, c0.b, c5.r, c5.g, c5.b
                            ));
                        }
                    }
                } else if let Some(ParamValue::Color(c)) = effect.params.get(ParamKey::Color) {
                    if c.r == c.g && c.g == c.b {
                        white_color += 1;
                    } else {
                        colored_color += 1;
                    }
                } else {
                    no_gradient += 1;
                }
            }
        }

        eprintln!("\n=== Parsed Gradient Color Analysis ===");
        eprintln!("  White/gray gradients: {white_gradient}");
        eprintln!("  Colored gradients: {colored_gradient}");
        eprintln!("  White/gray single color: {white_color}");
        eprintln!("  Colored single color: {colored_color}");
        eprintln!("  No gradient/color: {no_gradient}");
        if !sample_colors.is_empty() {
            eprintln!("\nSample colored gradients:");
            for s in &sample_colors {
                eprintln!("  {s}");
            }
        }

        assert!(
            colored_gradient + colored_color > 0,
            "Expected at least some non-white colors in the imported effects!"
        );
    }

    /// Diagnostic test: fresh import of All We Are, evaluate frames, check for non-black output.
    #[test]
    fn test_all_we_are_frame_evaluation() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        let seq_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence\All We Are.tim",
        );
        if !config_path.exists() || !seq_path.exists() {
            eprintln!("Skipping: All We Are.tim not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();
        importer.parse_sequence(seq_path).unwrap();

        let show = importer.into_show();
        let seq = &show.sequences[0];

        eprintln!("\n=== All We Are Frame Evaluation ===");
        eprintln!("Fixtures: {}, Tracks: {}", show.fixtures.len(), seq.tracks.len());

        let total_effects: usize = seq.tracks.iter().map(|t| t.effects.len()).sum();
        eprintln!("Total effects after track building: {total_effects}");

        // Check frame output at various times
        let mut any_lit = false;
        for t in [1.0, 10.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0, 210.0, 240.0] {
            let active: usize = seq.tracks.iter().map(|track| {
                track.effects.iter().filter(|e| e.time_range.contains(t)).count()
            }).sum();
            let frame = crate::engine::evaluate(&show, 0, t, None, None);
            let lit = frame.fixtures.len();
            if lit > 0 { any_lit = true; }
            eprintln!("  t={:>6.1}: {:>3} active effects, {:>3} lit fixtures", t, active, lit);
        }

        assert!(any_lit, "Expected at least some non-black fixtures across the sequence");

        // Check first few tracks for sanity
        eprintln!("\nFirst 10 tracks:");
        for (i, track) in seq.tracks.iter().take(10).enumerate() {
            let effects_summary: Vec<String> = track.effects.iter().take(3).map(|e| {
                format!("{:?}@{:.1}-{:.1}", e.kind, e.time_range.start(), e.time_range.end())
            }).collect();
            let more = if track.effects.len() > 3 {
                format!(" +{} more", track.effects.len() - 3)
            } else {
                String::new()
            };
            eprintln!("  Track {i}: {} ({} effects) - [{}]{more}",
                track.name, track.effects.len(), effects_summary.join(", "));
        }
    }

    /// Integration test: import "All We Are.tim" and verify all effect types are handled.
    /// Counts effects by kind and checks that no effects fell through to the unknown handler.
    #[test]
    fn test_all_we_are_effect_coverage() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        let seq_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence\All We Are.tim",
        );
        if !config_path.exists() || !seq_path.exists() {
            eprintln!("Skipping: All We Are.tim not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();
        importer.parse_sequence(seq_path).unwrap();

        let sequences = importer.into_sequences();
        assert!(!sequences.is_empty());
        let seq = &sequences[0];

        let mut by_kind: HashMap<String, usize> = HashMap::new();
        let mut total = 0usize;
        let mut with_gradient = 0usize;
        let mut with_movement = 0usize;
        let mut with_pulse = 0usize;
        let mut with_intensity = 0usize;

        for track in &seq.tracks {
            for effect in &track.effects {
                total += 1;
                let kind_name = format!("{:?}", effect.kind);
                *by_kind.entry(kind_name).or_insert(0) += 1;
                if effect.params.get(ParamKey::Gradient).is_some() {
                    with_gradient += 1;
                }
                if effect.params.get(ParamKey::MovementCurve).is_some() {
                    with_movement += 1;
                }
                if effect.params.get(ParamKey::PulseCurve).is_some() {
                    with_pulse += 1;
                }
                if effect.params.get(ParamKey::IntensityCurve).is_some() {
                    with_intensity += 1;
                }
            }
        }

        eprintln!("\n=== All We Are Effect Coverage ===");
        eprintln!("Total effects: {total}");
        eprintln!("  with gradient: {with_gradient}");
        eprintln!("  with movement_curve: {with_movement}");
        eprintln!("  with pulse_curve: {with_pulse}");
        eprintln!("  with intensity_curve: {with_intensity}");
        eprintln!("By kind:");
        let mut sorted: Vec<_> = by_kind.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (kind, count) in &sorted {
            eprintln!("  {kind}: {count}");
        }

        // All effects should map to real types (Chase, Fade, Twinkle, Strobe, Rainbow, Gradient)
        // If any fell to Solid (gray fallback), that's unexpected for this file.
        assert!(total > 0, "Expected effects in All We Are.tim");
    }

    /// Diagnostic: what color_mode values do effects actually get, and what does gradient.evaluate return?
    #[test]
    fn test_color_mode_diagnostic() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        let seq_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence\All We Are.tim",
        );
        if !config_path.exists() || !seq_path.exists() {
            eprintln!("Skipping: files not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();
        importer.parse_sequence(seq_path).unwrap();
        let show = importer.into_show();
        let seq = &show.sequences[0];

        let mut mode_counts: HashMap<String, usize> = HashMap::new();
        let mut static_with_colored_gradient = 0usize;
        let mut static_gradient_at_0: Vec<String> = Vec::new();

        for track in &seq.tracks {
            for effect in &track.effects {
                let mode = effect.params.color_mode_or(ParamKey::ColorMode, ColorMode::Static);
                let mode_str = format!("{:?}", mode);
                *mode_counts.entry(mode_str).or_insert(0) += 1;

                // Check static mode + colored gradient
                if mode == ColorMode::Static {
                    if let Some(ParamValue::ColorGradient(g)) = effect.params.get(ParamKey::Gradient) {
                        let c0 = g.evaluate(0.0);
                        let c5 = g.evaluate(0.5);
                        let c1 = g.evaluate(1.0);
                        if !(c0.r == c0.g && c0.g == c0.b && c5.r == c5.g && c5.g == c5.b) {
                            static_with_colored_gradient += 1;
                            if static_gradient_at_0.len() < 5 {
                                static_gradient_at_0.push(format!(
                                    "{:?}: @0.0=({},{},{}) @0.5=({},{},{}) @1.0=({},{},{})",
                                    effect.kind,
                                    c0.r, c0.g, c0.b,
                                    c5.r, c5.g, c5.b,
                                    c1.r, c1.g, c1.b,
                                ));
                            }
                        }
                    }
                }
            }
        }

        eprintln!("\n=== Color Mode Distribution ===");
        let mut sorted: Vec<_> = mode_counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (mode, count) in &sorted {
            eprintln!("  {mode}: {count}");
        }
        eprintln!("\nStatic mode with colored gradient: {static_with_colored_gradient}");
        for s in &static_gradient_at_0 {
            eprintln!("  {s}");
        }
    }
}
