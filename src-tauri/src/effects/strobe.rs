use crate::model::{Color, EffectParams, ParamSchema, ParamType, ParamValue};

use super::Effect;

/// Flashes between a color and black at a configurable rate.
///
/// Params:
/// - "color": Color (default: white)
/// - "rate": f64 - flashes per effect duration (default: 10.0)
/// - "duty_cycle": f64 - fraction of each cycle that is "on", 0.0-1.0 (default: 0.5)
pub struct StrobeEffect;

impl Effect for StrobeEffect {
    fn evaluate(
        &self,
        t: f64,
        _pixel_index: usize,
        _pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let color = params.color_or("color", Color::WHITE);
        let rate = params.float_or("rate", 10.0);
        let duty_cycle = params.float_or("duty_cycle", 0.5).clamp(0.0, 1.0);

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
                key: "color".into(),
                label: "Color".into(),
                param_type: ParamType::Color,
                default: ParamValue::Color(Color::WHITE),
            },
            ParamSchema {
                key: "rate".into(),
                label: "Rate".into(),
                param_type: ParamType::Float { min: 1.0, max: 50.0, step: 0.5 },
                default: ParamValue::Float(10.0),
            },
            ParamSchema {
                key: "duty_cycle".into(),
                label: "Duty Cycle".into(),
                param_type: ParamType::Float { min: 0.0, max: 1.0, step: 0.01 },
                default: ParamValue::Float(0.5),
            },
        ]
    }
}
