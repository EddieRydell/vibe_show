use crate::model::{BlendMode, Color, EffectParams, ParamSchema, ParamType, ParamValue};

use super::Effect;

/// Batch evaluate: extract color once, blend all pixels.
pub fn evaluate_pixels_batch(
    _t: f64,
    dest: &mut [Color],
    _global_offset: usize,
    _total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
) {
    let color = params.color_or("color", Color::WHITE);
    for pixel in dest.iter_mut() {
        *pixel = pixel.blend(color, blend_mode);
    }
}

/// Fills all pixels with a single solid color.
///
/// Params:
/// - "color": Color (default: white)
pub struct SolidEffect;

impl Effect for SolidEffect {
    fn evaluate(
        &self,
        _t: f64,
        _pixel_index: usize,
        _pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        params.color_or("color", Color::WHITE)
    }

    fn name(&self) -> &'static str {
        "Solid"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![ParamSchema {
            key: "color".into(),
            label: "Color".into(),
            param_type: ParamType::Color,
            default: ParamValue::Color(Color::WHITE),
        }]
    }
}
