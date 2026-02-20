use crate::model::{BlendMode, Color, EffectParams, ParamKey, ParamSchema, ParamType, ParamValue};

use super::Effect;

/// Batch evaluate: extract params once, compute single color, blend all pixels.
pub fn evaluate_pixels_batch(
    t: f64,
    dest: &mut [Color],
    _global_offset: usize,
    _total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
    opacity: f64,
) {
    let color = params.color_or(ParamKey::Color, Color::WHITE);
    let rate = params.float_or(ParamKey::Rate, 10.0);
    let duty_cycle = params.float_or(ParamKey::DutyCycle, 0.5).clamp(0.0, 1.0);

    let phase = (t * rate).fract();
    let effect_color = if phase < duty_cycle { color } else { Color::BLACK };
    let effect_color = if opacity < 1.0 { effect_color.scale(opacity) } else { effect_color };

    for pixel in dest.iter_mut() {
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

/// Flashes between a color and black at a configurable rate.
pub struct StrobeEffect;

impl Effect for StrobeEffect {
    fn evaluate(
        &self,
        t: f64,
        _pixel_index: usize,
        _pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let color = params.color_or(ParamKey::Color, Color::WHITE);
        let rate = params.float_or(ParamKey::Rate, 10.0);
        let duty_cycle = params.float_or(ParamKey::DutyCycle, 0.5).clamp(0.0, 1.0);

        let phase = (t * rate).fract();
        if phase < duty_cycle {
            color
        } else {
            Color::BLACK
        }
    }

    fn name(&self) -> &'static str {
        "Strobe"
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
                key: ParamKey::Rate,
                label: "Rate".into(),
                param_type: ParamType::Float { min: 1.0, max: 50.0, step: 0.5 },
                default: ParamValue::Float(10.0),
            },
            ParamSchema {
                key: ParamKey::DutyCycle,
                label: "Duty Cycle".into(),
                param_type: ParamType::Float { min: 0.0, max: 1.0, step: 0.01 },
                default: ParamValue::Float(0.5),
            },
        ]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn on_during_first_half_of_cycle() {
        let effect = StrobeEffect;
        // rate=1.0, duty_cycle=0.5: on for t in [0, 0.5), off for [0.5, 1.0)
        let params = EffectParams::new()
            .set(ParamKey::Rate, ParamValue::Float(1.0))
            .set(ParamKey::DutyCycle, ParamValue::Float(0.5));

        let on = effect.evaluate(0.0, 0, 1, &params);
        assert_eq!(on, Color::WHITE); // default color

        let off = effect.evaluate(0.5, 0, 1, &params);
        assert_eq!(off, Color::BLACK);
    }

    #[test]
    fn custom_color_strobes() {
        let effect = StrobeEffect;
        let red = Color::rgb(255, 0, 0);
        let params = EffectParams::new()
            .set(ParamKey::Color, ParamValue::Color(red))
            .set(ParamKey::Rate, ParamValue::Float(1.0))
            .set(ParamKey::DutyCycle, ParamValue::Float(0.5));
        assert_eq!(effect.evaluate(0.25, 0, 1, &params), red);
    }
}
