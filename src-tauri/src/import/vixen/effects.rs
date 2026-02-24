use crate::model::color::Color;
use crate::model::color_gradient::{ColorGradient, ColorStop};
use crate::model::curve::{Curve, CurvePoint};
use crate::model::timeline::{
    ColorMode, EffectKind, EffectParams, ParamKey, ParamValue, WipeDirection,
};

use super::constants::{vixen_color_handling, vixen_direction, vixen_effect};
use super::types::VixenEffect;

// ── Curve / gradient param builders ─────────────────────────────────

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

// ── Color handling / direction helpers ───────────────────────────────

/// Map Vixen color handling string to our `ColorMode` enum.
///
/// The `default` parameter controls the fallback for `StaticColor` and `None`,
/// which varies by effect type:
/// - Chase/Wipe/Spin: `GradientPerPulse` (gradient defines pulse shape, e.g. white head → colored tail)
/// - Fade/Pulse/ColorWash: `GradientThroughEffect` (gradient animates over the effect duration)
/// - Garlands/PinWheel: `GradientAcrossItems` (gradient spreads across pixels)
fn map_color_handling(handling: Option<&str>, default: ColorMode) -> ColorMode {
    use vixen_color_handling::{
        COLOR_ACROSS_ITEMS, GRADIENT_ACROSS_ITEMS, GRADIENT_FOR_EACH_PULSE,
        GRADIENT_OVER_EACH_PULSE, GRADIENT_PER_PULSE, GRADIENT_THROUGH_WHOLE_EFFECT,
    };
    match handling {
        Some(GRADIENT_THROUGH_WHOLE_EFFECT) => ColorMode::GradientThroughEffect,
        Some(GRADIENT_ACROSS_ITEMS | COLOR_ACROSS_ITEMS) => ColorMode::GradientAcrossItems,
        Some(GRADIENT_FOR_EACH_PULSE | GRADIENT_OVER_EACH_PULSE | GRADIENT_PER_PULSE) => {
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
    use vixen_direction::{DOWN, OUT, REVERSE, RIGHT};
    match direction {
        Some(d) => matches!(d, REVERSE | RIGHT | DOWN | OUT | "1"),
        None => false,
    }
}

/// Map a Vixen wipe direction string to a `WipeDirection` + reverse flag.
fn map_wipe_direction(direction: Option<&str>) -> (WipeDirection, bool) {
    use vixen_direction::{
        BURST, BURST_IN, BURST_OUT, CIRCLE, CIRCLE_IN, CIRCLE_OUT, DIAGONAL_DOWN,
        DIAGONAL_UP, DIAMOND, DIAMOND_IN, DIAMOND_OUT, DOWN, OUT, REVERSE, RIGHT, UP,
        VERTICAL,
    };
    match direction {
        Some(RIGHT | REVERSE | "1") => (WipeDirection::Horizontal, true),
        Some(VERTICAL | UP) => (WipeDirection::Vertical, false),
        Some(DOWN) => (WipeDirection::Vertical, true),
        Some(DIAGONAL_UP) => (WipeDirection::DiagonalUp, false),
        Some(DIAGONAL_DOWN) => (WipeDirection::DiagonalDown, false),
        Some(BURST | BURST_IN) => (WipeDirection::Burst, false),
        Some(BURST_OUT | OUT) => (WipeDirection::Burst, true),
        Some(CIRCLE | CIRCLE_IN) => (WipeDirection::Circle, false),
        Some(CIRCLE_OUT) => (WipeDirection::Circle, true),
        Some(DIAMOND | DIAMOND_IN) => (WipeDirection::Diamond, false),
        Some(DIAMOND_OUT) => (WipeDirection::Diamond, true),
        // "Horizontal", "Left", "0", None, and any unknown → default
        _ => (WipeDirection::Horizontal, false),
    }
}

// ── Effect type mapping ─────────────────────────────────────────────

/// Map a Vixen effect type name to a `VibeLights` `EffectKind` + default params.
///
/// Effects that can't be mapped print a LOUD warning for easy debugging.
#[allow(clippy::too_many_lines)]
pub(super) fn map_vixen_effect(effect: &VixenEffect) -> (EffectKind, EffectParams) {
    let type_name = effect.type_name.as_str();
    let intensity_curve = effect.intensity_curve.as_ref();
    let movement_curve = effect.movement_curve.as_ref();
    let pulse_curve = effect.pulse_curve.as_ref();
    let color_handling = effect.color_handling.as_deref();
    let level = effect.level;
    let base_color = effect.color.unwrap_or(Color::WHITE);

    use vixen_effect::{
        ALTERNATING, BUTTERFLY, CHASE, COLOR_WASH, DISSOLVE, GARLANDS, PIN_WHEEL, PULSE,
        RAINBOW, SET_LEVEL, SHOCKWAVE, SPIN, STROBE, TWINKLE, WIPE,
    };

    match type_name {
        // ── Pulse / SetLevel → Fade ──────────────────────────────
        PULSE | SET_LEVEL => {
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
        CHASE => {
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
        SPIN => {
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
        WIPE => {
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
        ALTERNATING => {
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
        SHOCKWAVE => {
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
        GARLANDS => {
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
        PIN_WHEEL => {
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
        BUTTERFLY => {
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
        DISSOLVE => {
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
        COLOR_WASH => {
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
        TWINKLE => (
            EffectKind::Twinkle,
            EffectParams::new()
                .set(ParamKey::Color, ParamValue::Color(base_color))
                .set(ParamKey::Density, ParamValue::Float(0.4))
                .set(ParamKey::Speed, ParamValue::Float(6.0)),
        ),

        // ── Strobe ──────────────────────────────────────────────
        STROBE => (
            EffectKind::Strobe,
            EffectParams::new()
                .set(ParamKey::Color, ParamValue::Color(base_color))
                .set(ParamKey::Rate, ParamValue::Float(10.0))
                .set(ParamKey::DutyCycle, ParamValue::Float(0.5)),
        ),

        // ── Rainbow ─────────────────────────────────────────────
        RAINBOW => (
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

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::bool_assert_comparison,
)]
mod tests {
    use super::*;

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
}
