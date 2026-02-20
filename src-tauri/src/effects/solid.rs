use crate::model::{BlendMode, Color, EffectParams, ParamKey, ParamSchema, ParamType, ParamValue};

use super::Effect;

/// Batch evaluate: extract color once, blend all pixels.
pub fn evaluate_pixels_batch(
    _t: f64,
    dest: &mut [Color],
    _global_offset: usize,
    _total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
    opacity: f64,
) {
    let color = params.color_or(ParamKey::Color, Color::WHITE);
    let color = if opacity < 1.0 { color.scale(opacity) } else { color };
    for pixel in dest.iter_mut() {
        *pixel = pixel.blend(color, blend_mode);
    }
}

/// Fills all pixels with a single solid color.
pub struct SolidEffect;

impl Effect for SolidEffect {
    fn evaluate(
        &self,
        _t: f64,
        _pixel_index: usize,
        _pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        params.color_or(ParamKey::Color, Color::WHITE)
    }

    fn name(&self) -> &'static str {
        "Solid"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![ParamSchema {
            key: ParamKey::Color,
            label: "Color".into(),
            param_type: ParamType::Color,
            default: ParamValue::Color(Color::WHITE),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_pixels_same_color() {
        let effect = SolidEffect;
        let red = Color::rgb(255, 0, 0);
        let params = EffectParams::new().set(ParamKey::Color, ParamValue::Color(red));
        for i in 0..10 {
            assert_eq!(effect.evaluate(0.5, i, 10, &params), red);
        }
    }

    #[test]
    fn default_is_white() {
        let effect = SolidEffect;
        let params = EffectParams::new();
        assert_eq!(effect.evaluate(0.0, 0, 1, &params), Color::WHITE);
    }
}
