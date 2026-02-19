use crate::model::{Color, EffectParams, ParamSchema, ParamType, ParamValue};

use super::Effect;

/// A moving pixel/group that travels across the fixture with an optional tail.
///
/// Params:
/// - "color": Color (default: white)
/// - "speed": f64 - number of complete passes over the effect duration (default: 1.0)
/// - "tail_length": f64 - fraction of the strip for the tail, 0.0-1.0 (default: 0.3)
/// - "reverse": bool - travel right to left (default: false)
pub struct ChaseEffect;

impl Effect for ChaseEffect {
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        let color = params.color_or("color", Color::WHITE);
        let speed = params.float_or("speed", 1.0);
        let tail_length = params.float_or("tail_length", 0.3).clamp(0.01, 1.0);
        let reverse = params.bool_or("reverse", false);

        if pixel_count == 0 {
            return Color::BLACK;
        }

        let pos = (pixel_index as f64) / (pixel_count as f64);
        let head = (t * speed).fract();
        let head = if reverse { 1.0 - head } else { head };

        // Distance from the head, wrapping around.
        let mut dist = head - pos;
        if dist < 0.0 {
            dist += 1.0;
        }

        if dist < tail_length {
            let brightness = 1.0 - (dist / tail_length);
            color.scale(brightness)
        } else {
            Color::BLACK
        }
    }

    fn name(&self) -> &'static str {
        "Chase"
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
                key: "speed".into(),
                label: "Speed".into(),
                param_type: ParamType::Float { min: 0.1, max: 20.0, step: 0.1 },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: "tail_length".into(),
                label: "Tail Length".into(),
                param_type: ParamType::Float { min: 0.01, max: 1.0, step: 0.01 },
                default: ParamValue::Float(0.3),
            },
            ParamSchema {
                key: "reverse".into(),
                label: "Reverse".into(),
                param_type: ParamType::Bool,
                default: ParamValue::Bool(false),
            },
        ]
    }
}
