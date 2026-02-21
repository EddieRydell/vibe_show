use crate::model::{BlendMode, Color, EffectParams, ParamKey, ParamSchema, ParamType, ParamValue};

use super::Effect;

const DEFAULT_DENSITY: f64 = 0.3;
const DEFAULT_SPEED: f64 = 5.0;

/// Batch evaluate: extract params once, loop over pixels.
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn evaluate_pixels_batch(
    t: f64,
    dest: &mut [Color],
    global_offset: usize,
    _total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
    opacity: f64,
) {
    let color = params.color_or(ParamKey::Color, Color::WHITE);
    let density = params.float_or(ParamKey::Density, DEFAULT_DENSITY).clamp(0.0, 1.0);
    let speed = params.float_or(ParamKey::Speed, DEFAULT_SPEED);

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
            let intensity = (brightness - threshold) * inv_density * opacity;
            color.scale(intensity)
        } else {
            Color::BLACK
        };
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

/// Random per-pixel twinkling. Uses a deterministic hash so the same
/// (time, pixel) always produces the same result (no actual RNG needed).
pub struct TwinkleEffect;

/// Simple deterministic hash for reproducible "randomness" without state.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn hash_pixel(pixel: usize, time_slot: u64) -> f64 {
    let mut x = (pixel as u64).wrapping_mul(2_654_435_761) ^ time_slot.wrapping_mul(2_246_822_519);
    x = x.wrapping_mul(x).wrapping_add(x);
    x ^= x >> 16;
    (x & 0xFFFF) as f64 / 65535.0
}

impl Effect for TwinkleEffect {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        _pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let color = params.color_or(ParamKey::Color, Color::WHITE);
        let density = params.float_or(ParamKey::Density, DEFAULT_DENSITY).clamp(0.0, 1.0);
        let speed = params.float_or(ParamKey::Speed, DEFAULT_SPEED);

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
                key: ParamKey::Color,
                label: "Color".into(),
                param_type: ParamType::Color,
                default: ParamValue::Color(Color::WHITE),
            },
            ParamSchema {
                key: ParamKey::Density,
                label: "Density".into(),
                param_type: ParamType::Float { min: 0.0, max: 1.0, step: 0.01 },
                default: ParamValue::Float(DEFAULT_DENSITY),
            },
            ParamSchema {
                key: ParamKey::Speed,
                label: "Speed".into(),
                param_type: ParamType::Float { min: 0.5, max: 30.0, step: 0.5 },
                default: ParamValue::Float(DEFAULT_SPEED),
            },
        ]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_inputs_same_output() {
        let effect = TwinkleEffect;
        let params = EffectParams::new();
        let a = effect.evaluate(0.3, 5, 100, &params);
        let b = effect.evaluate(0.3, 5, 100, &params);
        assert_eq!(a, b);
    }

    #[test]
    fn spatial_variation_exists() {
        let effect = TwinkleEffect;
        let params = EffectParams::new()
            .set(ParamKey::Density, ParamValue::Float(0.5));
        // Check several pixels — at least some should differ
        let colors: Vec<_> = (0..20)
            .map(|i| effect.evaluate(0.0, i, 20, &params))
            .collect();
        let all_same = colors.windows(2).all(|w| w[0] == w[1]);
        assert!(!all_same, "twinkle should produce spatial variation");
    }
}
