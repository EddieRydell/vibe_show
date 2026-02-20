use std::sync::LazyLock;

use crate::model::{BlendMode, Color, ColorGradient, Curve, EffectParams, ParamSchema, ParamType, ParamValue};

use super::Effect;

static DEFAULT_INTENSITY: LazyLock<Curve> = LazyLock::new(Curve::triangle);
static DEFAULT_GRADIENT: LazyLock<ColorGradient> =
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
    let intensity_curve = params.curve_or("intensity_curve", &DEFAULT_INTENSITY);
    let gradient = params.gradient_or("gradient", &DEFAULT_GRADIENT);
    let color_mode = params.text_or("color_mode", "gradient_through_effect");

    let intensity = intensity_curve.evaluate(t);
    let inv_total = if total_pixels > 0 {
        1.0 / (total_pixels as f64)
    } else {
        0.0
    };

    for (i, pixel) in dest.iter_mut().enumerate() {
        let pos = ((global_offset + i) as f64) * inv_total;

        let color = match color_mode {
            "gradient_across_items" => gradient.evaluate(pos),
            "static" => gradient.evaluate(0.0),
            _ => gradient.evaluate(t), // "gradient_through_effect"
        };

        let effect_color = color.scale(intensity);
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

/// A fade/pulse effect: intensity envelope over time with a color gradient.
/// Maps to Vixen's "Pulse" effect.
pub struct FadeEffect;

impl Effect for FadeEffect {
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let intensity_curve = params.curve_or("intensity_curve", &DEFAULT_INTENSITY);
        let gradient = params.gradient_or("gradient", &DEFAULT_GRADIENT);
        let color_mode = params.text_or("color_mode", "gradient_through_effect");

        let intensity = intensity_curve.evaluate(t);
        let pos = if pixel_count > 0 {
            (pixel_index as f64) / (pixel_count as f64)
        } else {
            0.0
        };

        let color = match color_mode {
            "gradient_across_items" => gradient.evaluate(pos),
            "static" => gradient.evaluate(0.0),
            _ => gradient.evaluate(t),
        };

        color.scale(intensity)
    }

    fn name(&self) -> &'static str {
        "Fade"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema {
                key: "intensity_curve".into(),
                label: "Intensity Curve".into(),
                param_type: ParamType::Curve,
                default: ParamValue::Curve(Curve::triangle()),
            },
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
                        "gradient_through_effect".into(),
                        "gradient_across_items".into(),
                    ],
                },
                default: ParamValue::Text("gradient_through_effect".into()),
            },
        ]
    }
}
