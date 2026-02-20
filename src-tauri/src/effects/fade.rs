use std::sync::LazyLock;

use crate::model::{BlendMode, Color, ColorGradient, ColorMode, Curve, EffectParams, ParamKey, ParamSchema, ParamType, ParamValue};

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
    opacity: f64,
) {
    let intensity_curve = params.curve_or(ParamKey::IntensityCurve, &DEFAULT_INTENSITY);
    let gradient = params.gradient_or(ParamKey::Gradient, &DEFAULT_GRADIENT);
    let color_mode = params.color_mode_or(ParamKey::ColorMode, ColorMode::GradientThroughEffect);

    let intensity = intensity_curve.evaluate(t);
    let inv_total = if total_pixels > 0 {
        1.0 / (total_pixels as f64)
    } else {
        0.0
    };

    for (i, pixel) in dest.iter_mut().enumerate() {
        let pos = ((global_offset + i) as f64) * inv_total;

        let color = match color_mode {
            ColorMode::GradientAcrossItems => gradient.evaluate(pos),
            ColorMode::Static => gradient.evaluate(0.0),
            _ => gradient.evaluate(t),
        };

        let effect_color = color.scale(intensity * opacity);
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
        let intensity_curve = params.curve_or(ParamKey::IntensityCurve, &DEFAULT_INTENSITY);
        let gradient = params.gradient_or(ParamKey::Gradient, &DEFAULT_GRADIENT);
        let color_mode = params.color_mode_or(ParamKey::ColorMode, ColorMode::GradientThroughEffect);

        let intensity = intensity_curve.evaluate(t);
        let pos = if pixel_count > 0 {
            (pixel_index as f64) / (pixel_count as f64)
        } else {
            0.0
        };

        let color = match color_mode {
            ColorMode::GradientAcrossItems => gradient.evaluate(pos),
            ColorMode::Static => gradient.evaluate(0.0),
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
                key: ParamKey::IntensityCurve,
                label: "Intensity Curve".into(),
                param_type: ParamType::Curve,
                default: ParamValue::Curve(Curve::triangle()),
            },
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
                default: ParamValue::ColorMode(ColorMode::GradientThroughEffect),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intensity_follows_default_triangle_curve() {
        let effect = FadeEffect;
        let params = EffectParams::new();
        // Default curve is triangle: 0→1→0
        // At t=0.0, intensity is 0 → output should be black
        let at_start = effect.evaluate(0.0, 0, 10, &params);
        assert_eq!(at_start, Color::BLACK);

        // At t=0.5, intensity is 1.0 → output should be white (default gradient)
        let at_peak = effect.evaluate(0.5, 0, 10, &params);
        assert_eq!(at_peak, Color::WHITE);
    }

    #[test]
    fn zero_intensity_at_end() {
        let effect = FadeEffect;
        let params = EffectParams::new();
        let at_end = effect.evaluate(1.0, 0, 10, &params);
        assert_eq!(at_end, Color::BLACK);
    }
}
