use crate::model::{BlendMode, Color, EffectParams, ParamSchema, ParamType, ParamValue};

use super::Effect;

/// Batch evaluate: extract params once, loop over pixels.
pub fn evaluate_pixels_batch(
    t: f64,
    dest: &mut [Color],
    global_offset: usize,
    _total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
) {
    let color = params.color_or("color", Color::WHITE);
    let density = params.float_or("density", 0.3).clamp(0.0, 1.0);
    let speed = params.float_or("speed", 5.0);

    let slot = (t * speed) as u64;
    let next_slot = slot + 1;
    let frac = (t * speed).fract();
    let inv_frac = 1.0 - frac;
    let threshold = 1.0 - density;
    let inv_density = if density > 0.0 { 1.0 / density } else { 0.0 };

    for (i, pixel) in dest.iter_mut().enumerate() {
        let pixel_index = global_offset + i;
        let brightness_current = hash_pixel(pixel_index, slot);
        let brightness_next = hash_pixel(pixel_index, next_slot);
        let brightness = brightness_current * inv_frac + brightness_next * frac;

        let effect_color = if brightness > threshold {
            let intensity = (brightness - threshold) * inv_density;
            color.scale(intensity)
        } else {
            Color::BLACK
        };
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

/// Random per-pixel twinkling. Uses a deterministic hash so the same
/// (time, pixel) always produces the same result (no actual RNG needed).
///
/// Params:
/// - "color": Color (default: white)
/// - "density": f64 - fraction of pixels lit at any moment, 0.0-1.0 (default: 0.3)
/// - "speed": f64 - how fast twinkles change, cycles per duration (default: 5.0)
pub struct TwinkleEffect;

/// Simple deterministic hash for reproducible "randomness" without state.
fn hash_pixel(pixel: usize, time_slot: u64) -> f64 {
    let mut x = (pixel as u64).wrapping_mul(2654435761) ^ time_slot.wrapping_mul(2246822519);
    x = x.wrapping_mul(x).wrapping_add(x);
    x ^= x >> 16;
    (x & 0xFFFF) as f64 / 65535.0
}

impl Effect for TwinkleEffect {
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        _pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let color = params.color_or("color", Color::WHITE);
        let density = params.float_or("density", 0.3).clamp(0.0, 1.0);
        let speed = params.float_or("speed", 5.0);

        // Discrete time slots for twinkling.
        let slot = (t * speed) as u64;
        let next_slot = slot + 1;

        // Hash determines brightness at each slot.
        let brightness_current = hash_pixel(pixel_index, slot);
        let brightness_next = hash_pixel(pixel_index, next_slot);

        // Interpolate between slots for smooth transitions.
        let frac = (t * speed).fract();
        let brightness = brightness_current * (1.0 - frac) + brightness_next * frac;

        // Threshold by density - only some fraction of pixels are lit.
        if brightness > (1.0 - density) {
            let intensity = (brightness - (1.0 - density)) / density;
            color.scale(intensity)
        } else {
            Color::BLACK
        }
    }

    fn name(&self) -> &'static str {
        "Twinkle"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema {
                key: "color".into(),
                label: "Color".into(),
                param_type: ParamType::Color,
                default: ParamValue::Color(Color::WHITE),
            },
            ParamSchema {
                key: "density".into(),
                label: "Density".into(),
                param_type: ParamType::Float { min: 0.0, max: 1.0, step: 0.01 },
                default: ParamValue::Float(0.3),
            },
            ParamSchema {
                key: "speed".into(),
                label: "Speed".into(),
                param_type: ParamType::Float { min: 0.5, max: 30.0, step: 0.5 },
                default: ParamValue::Float(5.0),
            },
        ]
    }
}
