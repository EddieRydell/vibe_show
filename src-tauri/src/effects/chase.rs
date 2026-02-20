use std::sync::LazyLock;

use crate::model::{BlendMode, Color, ColorGradient, Curve, EffectParams, ParamSchema, ParamType, ParamValue};

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
) {
    let fallback_gradient;
    let gradient_default = if params.get("color").is_some() {
        fallback_gradient = default_gradient(params);
        &fallback_gradient
    } else {
        &*DEFAULT_WHITE_GRADIENT
    };

    let gradient = params.gradient_or("gradient", gradient_default);
    let movement_curve = params.curve_or("movement_curve", &DEFAULT_MOVEMENT);
    let pulse_curve = params.curve_or("pulse_curve", &DEFAULT_PULSE);
    let color_mode = params.text_or("color_mode", "static");
    let speed = params.float_or("speed", 1.0);
    let pulse_width = params.float_or("pulse_width", 0.3).clamp(0.01, 1.0);
    let background_level = params.float_or("background_level", 0.0).clamp(0.0, 1.0);
    let reverse = params.bool_or("reverse", false);

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
            "gradient_per_pulse" if dist < pulse_width => {
                gradient.evaluate(dist * inv_pulse)
            }
            "gradient_through_effect" => gradient.evaluate(t),
            "gradient_across_items" => gradient.evaluate(pos),
            _ => gradient.evaluate(0.0), // "static"
        };

        let effect_color = color.scale(intensity);
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

/// Backward compat: if no `gradient` param, construct from old `color` param.
fn default_gradient(params: &EffectParams) -> ColorGradient {
    if let Some(ParamValue::Color(c)) = params.get("color") {
        ColorGradient::solid(*c)
    } else {
        ColorGradient::solid(Color::WHITE)
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
        let fallback_gradient;
        let gradient_default = if params.get("color").is_some() {
            fallback_gradient = default_gradient(params);
            &fallback_gradient
        } else {
            &*DEFAULT_WHITE_GRADIENT
        };

        let gradient = params.gradient_or("gradient", gradient_default);
        let movement_curve = params.curve_or("movement_curve", &DEFAULT_MOVEMENT);
        let pulse_curve = params.curve_or("pulse_curve", &DEFAULT_PULSE);
        let color_mode = params.text_or("color_mode", "static");
        let speed = params.float_or("speed", 1.0);
        let pulse_width = params.float_or("pulse_width", 0.3).clamp(0.01, 1.0);
        let background_level = params.float_or("background_level", 0.0).clamp(0.0, 1.0);
        let reverse = params.bool_or("reverse", false);

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
            "gradient_per_pulse" if dist < pulse_width => {
                gradient.evaluate(dist / pulse_width)
            }
            "gradient_through_effect" => gradient.evaluate(t),
            "gradient_across_items" => gradient.evaluate(pos),
            _ => gradient.evaluate(0.0),
        };

        color.scale(intensity)
    }

    fn name(&self) -> &'static str {
        "Chase"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema {
                key: "gradient".into(),
                label: "Color Gradient".into(),
                param_type: ParamType::ColorGradient {
                    min_stops: 1,
                    max_stops: 16,
                },
                default: ParamValue::ColorGradient(ColorGradient::solid(Color::WHITE)),
            },
            ParamSchema {
                key: "color_mode".into(),
                label: "Color Mode".into(),
                param_type: ParamType::Select {
                    options: vec![
                        "static".into(),
                        "gradient_per_pulse".into(),
                        "gradient_through_effect".into(),
                        "gradient_across_items".into(),
                    ],
                },
                default: ParamValue::Text("static".into()),
            },
            ParamSchema {
                key: "movement_curve".into(),
                label: "Movement Curve".into(),
                param_type: ParamType::Curve,
                default: ParamValue::Curve(Curve::linear()),
            },
            ParamSchema {
                key: "pulse_curve".into(),
                label: "Pulse Curve".into(),
                param_type: ParamType::Curve,
                default: ParamValue::Curve(Curve::triangle()),
            },
            ParamSchema {
                key: "speed".into(),
                label: "Speed".into(),
                param_type: ParamType::Float {
                    min: 0.1,
                    max: 20.0,
                    step: 0.1,
                },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: "pulse_width".into(),
                label: "Pulse Width".into(),
                param_type: ParamType::Float {
                    min: 0.01,
                    max: 1.0,
                    step: 0.01,
                },
                default: ParamValue::Float(0.3),
            },
            ParamSchema {
                key: "background_level".into(),
                label: "Background Level".into(),
                param_type: ParamType::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                },
                default: ParamValue::Float(0.0),
            },
            ParamSchema {
                key: "reverse".into(),
                label: "Reverse".into(),
                param_type: ParamType::Bool,
                default: ParamValue::Bool(false),
            },
        ]
    }
}
