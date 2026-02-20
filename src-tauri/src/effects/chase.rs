use std::sync::LazyLock;

use crate::model::{BlendMode, Color, ColorGradient, ColorMode, Curve, EffectParams, ParamKey, ParamSchema, ParamType, ParamValue};

use super::Effect;

static DEFAULT_MOVEMENT: LazyLock<Curve> = LazyLock::new(Curve::linear);
static DEFAULT_PULSE: LazyLock<Curve> = LazyLock::new(Curve::triangle);
static DEFAULT_WHITE_GRADIENT: LazyLock<ColorGradient> =
    LazyLock::new(|| ColorGradient::solid(Color::WHITE));

/// Batch evaluate: extract params once, loop over pixels.
pub fn evaluate_pixels_batch(
    t: f64,
    dest: &mut [Color],
    global_offset: usize,
    total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
    opacity: f64,
) {
    let gradient = params.gradient_or(ParamKey::Gradient, &DEFAULT_WHITE_GRADIENT);
    let movement_curve = params.curve_or(ParamKey::MovementCurve, &DEFAULT_MOVEMENT);
    let pulse_curve = params.curve_or(ParamKey::PulseCurve, &DEFAULT_PULSE);
    let color_mode = params.color_mode_or(ParamKey::ColorMode, ColorMode::Static);
    let speed = params.float_or(ParamKey::Speed, 1.0);
    let pulse_width = params.float_or(ParamKey::PulseWidth, 0.3).clamp(0.01, 1.0);
    let background_level = params.float_or(ParamKey::BackgroundLevel, 0.0).clamp(0.0, 1.0);
    let reverse = params.bool_or(ParamKey::Reverse, false);

    if total_pixels == 0 {
        return;
    }

    let head = movement_curve.evaluate((t * speed).fract());
    let head = if reverse { 1.0 - head } else { head };
    let inv_total = 1.0 / (total_pixels as f64);
    let inv_pulse = 1.0 / pulse_width;

    for (i, pixel) in dest.iter_mut().enumerate() {
        let pos = ((global_offset + i) as f64) * inv_total;

        // Circular distance from head
        let mut dist = head - pos;
        if dist < 0.0 {
            dist += 1.0;
        }

        let intensity = if dist < pulse_width {
            let pulse_pos = dist * inv_pulse;
            pulse_curve.evaluate(pulse_pos).max(background_level)
        } else {
            background_level
        };

        // Sample color based on color mode
        let color = match color_mode {
            ColorMode::GradientPerPulse if dist < pulse_width => {
                gradient.evaluate(dist * inv_pulse)
            }
            ColorMode::GradientThroughEffect => gradient.evaluate(t),
            ColorMode::GradientAcrossItems => gradient.evaluate(pos),
            ColorMode::Static => gradient.evaluate(0.0),
            // GradientPerPulse when outside the pulse — fall back to static
            ColorMode::GradientPerPulse => gradient.evaluate(0.0),
        };

        let effect_color = color.scale(intensity * opacity);
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

/// A moving pulse that travels across fixtures with configurable movement,
/// pulse shape, color gradient, and color modes.
pub struct ChaseEffect;

impl Effect for ChaseEffect {
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let gradient = params.gradient_or(ParamKey::Gradient, &DEFAULT_WHITE_GRADIENT);
        let movement_curve = params.curve_or(ParamKey::MovementCurve, &DEFAULT_MOVEMENT);
        let pulse_curve = params.curve_or(ParamKey::PulseCurve, &DEFAULT_PULSE);
        let color_mode = params.color_mode_or(ParamKey::ColorMode, ColorMode::Static);
        let speed = params.float_or(ParamKey::Speed, 1.0);
        let pulse_width = params.float_or(ParamKey::PulseWidth, 0.3).clamp(0.01, 1.0);
        let background_level = params.float_or(ParamKey::BackgroundLevel, 0.0).clamp(0.0, 1.0);
        let reverse = params.bool_or(ParamKey::Reverse, false);

        if pixel_count == 0 {
            return Color::BLACK;
        }

        let pos = (pixel_index as f64) / (pixel_count as f64);
        let head = movement_curve.evaluate((t * speed).fract());
        let head = if reverse { 1.0 - head } else { head };

        let mut dist = head - pos;
        if dist < 0.0 {
            dist += 1.0;
        }

        let intensity = if dist < pulse_width {
            let pulse_pos = dist / pulse_width;
            pulse_curve.evaluate(pulse_pos).max(background_level)
        } else {
            background_level
        };

        let color = match color_mode {
            ColorMode::GradientPerPulse if dist < pulse_width => {
                gradient.evaluate(dist / pulse_width)
            }
            ColorMode::GradientThroughEffect => gradient.evaluate(t),
            ColorMode::GradientAcrossItems => gradient.evaluate(pos),
            ColorMode::Static => gradient.evaluate(0.0),
            // GradientPerPulse when outside the pulse — fall back to static
            ColorMode::GradientPerPulse => gradient.evaluate(0.0),
        };

        color.scale(intensity)
    }

    fn name(&self) -> &'static str {
        "Chase"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema {
                key: ParamKey::Gradient,
                label: "Color Gradient".into(),
                param_type: ParamType::ColorGradient {
                    min_stops: 1,
                    max_stops: 16,
                },
                default: ParamValue::ColorGradient(ColorGradient::solid(Color::WHITE)),
            },
            ParamSchema {
                key: ParamKey::ColorMode,
                label: "Color Mode".into(),
                param_type: ParamType::ColorMode,
                default: ParamValue::ColorMode(ColorMode::Static),
            },
            ParamSchema {
                key: ParamKey::MovementCurve,
                label: "Movement Curve".into(),
                param_type: ParamType::Curve,
                default: ParamValue::Curve(Curve::linear()),
            },
            ParamSchema {
                key: ParamKey::PulseCurve,
                label: "Pulse Curve".into(),
                param_type: ParamType::Curve,
                default: ParamValue::Curve(Curve::triangle()),
            },
            ParamSchema {
                key: ParamKey::Speed,
                label: "Speed".into(),
                param_type: ParamType::Float {
                    min: 0.1,
                    max: 20.0,
                    step: 0.1,
                },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: ParamKey::PulseWidth,
                label: "Pulse Width".into(),
                param_type: ParamType::Float {
                    min: 0.01,
                    max: 1.0,
                    step: 0.01,
                },
                default: ParamValue::Float(0.3),
            },
            ParamSchema {
                key: ParamKey::BackgroundLevel,
                label: "Background Level".into(),
                param_type: ParamType::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                },
                default: ParamValue::Float(0.0),
            },
            ParamSchema {
                key: ParamKey::Reverse,
                label: "Reverse".into(),
                param_type: ParamType::Bool,
                default: ParamValue::Bool(false),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_bright_in_middle() {
        let effect = ChaseEffect;
        // At t=0, head at 0.0 (linear curve), pulse_width=0.3.
        // The default triangle pulse peaks at pulse_pos=0.5 → dist=0.15.
        // A pixel at pos=0.85 has dist = 0.0 - 0.85 + 1.0 = 0.15 → peak brightness.
        let params = EffectParams::new()
            .set(ParamKey::Speed, ParamValue::Float(1.0))
            .set(ParamKey::PulseWidth, ParamValue::Float(0.3))
            .set(ParamKey::BackgroundLevel, ParamValue::Float(0.0));
        let mid_pulse = effect.evaluate(0.0, 85, 100, &params);
        assert!(mid_pulse.r > 0 || mid_pulse.g > 0 || mid_pulse.b > 0);
    }

    #[test]
    fn background_level_outside_pulse() {
        let effect = ChaseEffect;
        // At t=0, head at 0.0, pulse_width=0.1. Pixel at pos=0.5 is outside pulse.
        let params = EffectParams::new()
            .set(ParamKey::Speed, ParamValue::Float(1.0))
            .set(ParamKey::PulseWidth, ParamValue::Float(0.1))
            .set(ParamKey::BackgroundLevel, ParamValue::Float(0.0));
        let far_away = effect.evaluate(0.0, 50, 100, &params);
        assert_eq!(far_away, Color::BLACK);
    }
}
