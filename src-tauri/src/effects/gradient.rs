use crate::model::{BlendMode, Color, EffectParams, ParamKey, ParamSchema, ParamType, ParamValue};

use super::Effect;

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
    let colors = params.color_list_or(ParamKey::Colors, &DEFAULT_COLORS);
    let offset = params.float_or(ParamKey::Offset, 0.0);

    if colors.is_empty() {
        let c = Color::BLACK;
        for pixel in dest.iter_mut() {
            *pixel = pixel.blend(c, blend_mode);
        }
        return;
    }
    if colors.len() == 1 {
        let c = colors[0];
        for pixel in dest.iter_mut() {
            *pixel = pixel.blend(c, blend_mode);
        }
        return;
    }

    let segment_count = colors.len() - 1;
    let inv_total = if total_pixels > 1 {
        1.0 / ((total_pixels - 1) as f64)
    } else {
        0.0
    };
    let time_offset = t * offset;

    for (i, pixel) in dest.iter_mut().enumerate() {
        let pos = if total_pixels > 1 {
            ((global_offset + i) as f64) * inv_total
        } else {
            0.5
        };
        let pos = (pos + time_offset).fract().abs();
        let scaled = pos * segment_count as f64;
        let segment = (scaled as usize).min(segment_count - 1);
        let frac = scaled - segment as f64;
        let effect_color = colors[segment].lerp(colors[segment + 1], frac);
        let effect_color = if opacity < 1.0 { effect_color.scale(opacity) } else { effect_color };
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

/// A spatial gradient across pixels, interpolating between colors.
pub struct GradientEffect;

static DEFAULT_COLORS: [Color; 2] = [Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)];

impl Effect for GradientEffect {
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let colors = params.color_list_or(ParamKey::Colors, &DEFAULT_COLORS);
        let offset = params.float_or(ParamKey::Offset, 0.0);

        if colors.is_empty() {
            return Color::BLACK;
        }
        if colors.len() == 1 {
            return colors[0];
        }

        let pos = if pixel_count > 1 {
            (pixel_index as f64) / ((pixel_count - 1) as f64)
        } else {
            0.5
        };

        // Apply time-based offset, wrapping.
        let pos = (pos + t * offset).fract().abs();

        // Map position to color segments.
        let segment_count = colors.len() - 1;
        let scaled = pos * segment_count as f64;
        let segment = (scaled as usize).min(segment_count - 1);
        let frac = scaled - segment as f64;

        colors[segment].lerp(colors[segment + 1], frac)
    }

    fn name(&self) -> &'static str {
        "Gradient"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema {
                key: ParamKey::Colors,
                label: "Colors".into(),
                param_type: ParamType::ColorList { min_colors: 2, max_colors: 16 },
                default: ParamValue::ColorList(vec![Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)]),
            },
            ParamSchema {
                key: ParamKey::Offset,
                label: "Offset".into(),
                param_type: ParamType::Float { min: -5.0, max: 5.0, step: 0.1 },
                default: ParamValue::Float(0.0),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_pixel_is_first_color() {
        let effect = GradientEffect;
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        let params = EffectParams::new()
            .set(ParamKey::Colors, ParamValue::ColorList(vec![red, blue]));

        let first = effect.evaluate(0.0, 0, 10, &params);
        assert_eq!(first, red);
    }

    #[test]
    fn gradient_progresses_spatially() {
        // Pixel near the end should have more blue than pixel near the start
        let effect = GradientEffect;
        let params = EffectParams::new()
            .set(ParamKey::Colors, ParamValue::ColorList(vec![
                Color::rgb(255, 0, 0),
                Color::rgb(0, 0, 255),
            ]));
        let near_start = effect.evaluate(0.0, 1, 10, &params);
        let near_end = effect.evaluate(0.0, 8, 10, &params);
        assert!(near_end.b > near_start.b);
        assert!(near_start.r > near_end.r);
    }

    #[test]
    fn middle_interpolates() {
        let effect = GradientEffect;
        let params = EffectParams::new()
            .set(ParamKey::Colors, ParamValue::ColorList(vec![Color::BLACK, Color::WHITE]));
        let mid = effect.evaluate(0.0, 5, 11, &params);
        assert!((mid.r as i16 - 127).abs() <= 1);
    }
}
