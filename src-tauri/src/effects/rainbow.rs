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
    let speed = params.float_or(ParamKey::Speed, 1.0);
    let spread = params.float_or(ParamKey::Spread, 1.0);
    let saturation = params.float_or(ParamKey::Saturation, 1.0).clamp(0.0, 1.0);
    let brightness = params.float_or(ParamKey::Brightness, 1.0).clamp(0.0, 1.0);

    let time_offset = t * speed * 360.0;
    let spatial_scale = if total_pixels > 1 {
        spread / (total_pixels as f64) * 360.0
    } else {
        0.0
    };

    for (i, pixel) in dest.iter_mut().enumerate() {
        let spatial = (global_offset + i) as f64 * spatial_scale;
        let hue = (time_offset + spatial) % 360.0;
        let effect_color = Color::from_hsv(hue, saturation, brightness);
        let effect_color = if opacity < 1.0 { effect_color.scale(opacity) } else { effect_color };
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

/// Cycles through HSV hue across space and/or time.
pub struct RainbowEffect;

impl Effect for RainbowEffect {
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let speed = params.float_or(ParamKey::Speed, 1.0);
        let spread = params.float_or(ParamKey::Spread, 1.0);
        let saturation = params.float_or(ParamKey::Saturation, 1.0).clamp(0.0, 1.0);
        let brightness = params.float_or(ParamKey::Brightness, 1.0).clamp(0.0, 1.0);

        let spatial = if pixel_count > 1 {
            (pixel_index as f64) / (pixel_count as f64) * spread
        } else {
            0.0
        };

        let hue = ((t * speed + spatial) * 360.0) % 360.0;
        Color::from_hsv(hue, saturation, brightness)
    }

    fn name(&self) -> &'static str {
        "Rainbow"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema {
                key: ParamKey::Speed,
                label: "Speed".into(),
                param_type: ParamType::Float { min: 0.1, max: 20.0, step: 0.1 },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: ParamKey::Spread,
                label: "Spread".into(),
                param_type: ParamType::Float { min: 0.1, max: 10.0, step: 0.1 },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: ParamKey::Saturation,
                label: "Saturation".into(),
                param_type: ParamType::Float { min: 0.0, max: 1.0, step: 0.01 },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: ParamKey::Brightness,
                label: "Brightness".into(),
                param_type: ParamType::Float { min: 0.0, max: 1.0, step: 0.01 },
                default: ParamValue::Float(1.0),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_pixels_get_different_hues() {
        let effect = RainbowEffect;
        let params = EffectParams::new();
        let c0 = effect.evaluate(0.0, 0, 10, &params);
        let c5 = effect.evaluate(0.0, 5, 10, &params);
        // Pixels at different positions should produce different colors
        assert_ne!(c0, c5);
    }

    #[test]
    fn single_pixel_produces_valid_color() {
        let effect = RainbowEffect;
        let params = EffectParams::new();
        let c = effect.evaluate(0.0, 0, 1, &params);
        assert_eq!(c.a, 255);
    }
}
