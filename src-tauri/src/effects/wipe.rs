use std::sync::LazyLock;

use crate::model::show::Position2D;
use crate::model::{
    BlendMode, Color, ColorGradient, ColorMode, Curve, EffectParams, ParamKey, ParamSchema,
    ParamType, ParamValue,
};

use super::Effect;

static DEFAULT_MOVEMENT: LazyLock<Curve> = LazyLock::new(Curve::linear);
static DEFAULT_PULSE: LazyLock<Curve> = LazyLock::new(Curve::linear);
static DEFAULT_WHITE_GRADIENT: LazyLock<ColorGradient> =
    LazyLock::new(|| ColorGradient::solid(Color::WHITE));

/// Project a 2D position onto a 1D scalar [0, 1] based on direction.
fn project_position(pos: Position2D, direction: &str, cx: f32, cy: f32) -> f64 {
    let x = pos.x as f64;
    let y = pos.y as f64;
    let cx = cx as f64;
    let cy = cy as f64;

    match direction {
        "horizontal" => x,
        "vertical" => y,
        "diagonal_up" => ((x + (1.0 - y)) * 0.5).clamp(0.0, 1.0),
        "diagonal_down" => ((x + y) * 0.5).clamp(0.0, 1.0),
        "burst" => {
            // Chebyshev distance (max of |dx|, |dy|) from center
            let dx = (x - cx).abs();
            let dy = (y - cy).abs();
            let max_possible = 1.0f64.max(cx.max(1.0 - cx)).max(cy.max(1.0 - cy));
            (dx.max(dy) / max_possible).clamp(0.0, 1.0)
        }
        "circle" => {
            // Euclidean distance from center
            let dx = x - cx;
            let dy = y - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            // Max possible distance from center in unit square
            let max_dist = ((cx.max(1.0 - cx)).powi(2) + (cy.max(1.0 - cy)).powi(2)).sqrt();
            if max_dist > 0.0 {
                (dist / max_dist).clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        "diamond" => {
            // Manhattan distance from center
            let dx = (x - cx).abs();
            let dy = (y - cy).abs();
            let max_dist = cx.max(1.0 - cx) + cy.max(1.0 - cy);
            if max_dist > 0.0 {
                ((dx + dy) / max_dist).clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        // Default: horizontal
        _ => x,
    }
}

/// Batch evaluate: extract params once, loop over pixels with spatial positions.
pub fn evaluate_pixels_batch(
    t: f64,
    dest: &mut [Color],
    global_offset: usize,
    total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
    opacity: f64,
    positions: Option<&[Position2D]>,
) {
    let gradient = params.gradient_or(ParamKey::Gradient, &DEFAULT_WHITE_GRADIENT);
    let movement_curve = params.curve_or(ParamKey::MovementCurve, &DEFAULT_MOVEMENT);
    let pulse_curve = params.curve_or(ParamKey::PulseCurve, &DEFAULT_PULSE);
    let color_mode = params.color_mode_or(ParamKey::ColorMode, ColorMode::Static);
    let speed = params.float_or(ParamKey::Speed, 1.0);
    let pulse_width = params.float_or(ParamKey::PulseWidth, 1.0).clamp(0.01, 1.0);
    let reverse = params.bool_or(ParamKey::Reverse, false);
    let direction = params.text_or(ParamKey::Direction, "horizontal");
    let center_x = params.float_or(ParamKey::CenterX, 0.5) as f32;
    let center_y = params.float_or(ParamKey::CenterY, 0.5) as f32;
    let pass_count = params.float_or(ParamKey::PassCount, 1.0).max(0.1);
    let wipe_on = params.bool_or(ParamKey::WipeOn, true);

    if total_pixels == 0 {
        return;
    }

    // Head position: sweeps from 0 to 1 over the effect duration, repeated by pass_count
    let head = movement_curve.evaluate((t * pass_count * speed).fract());
    let head = if reverse { 1.0 - head } else { head };

    // Expand the sweep range so the pulse fully enters and exits
    // head_pos ranges from -pulse_width to 1.0, mapped from head [0, 1]
    let head_pos = head * (1.0 + pulse_width) - pulse_width;

    let inv_total = 1.0 / (total_pixels as f64);
    let inv_pulse = 1.0 / pulse_width;

    for (i, pixel) in dest.iter_mut().enumerate() {
        // Get spatial position for this pixel
        let spatial_pos = if let Some(positions) = positions {
            // Use actual 2D position
            project_position(positions[i], direction, center_x, center_y)
        } else {
            // Fallback: use linear index as horizontal position
            ((global_offset + i) as f64) * inv_total
        };

        // Distance from sweep head to this pixel
        let dist = spatial_pos - head_pos;

        // Wipe: pixels behind the head are lit, edge uses pulse_curve
        let intensity = if wipe_on {
            // Revealing: pixels at or behind the head are lit
            if dist <= 0.0 {
                // Fully revealed
                1.0
            } else if dist < pulse_width {
                // In the transition zone — pulse_curve controls falloff
                let edge_t = dist * inv_pulse;
                1.0 - pulse_curve.evaluate(edge_t)
            } else {
                // Not yet reached
                0.0
            }
        } else {
            // Concealing: pixels at or behind the head are dark
            if dist <= 0.0 {
                0.0
            } else if dist < pulse_width {
                let edge_t = dist * inv_pulse;
                pulse_curve.evaluate(edge_t)
            } else {
                1.0
            }
        };

        if intensity <= 0.0 {
            continue;
        }

        // Sample color based on color mode
        let color = match color_mode {
            ColorMode::GradientPerPulse => {
                if dist > 0.0 && dist < pulse_width {
                    gradient.evaluate(dist * inv_pulse)
                } else {
                    gradient.evaluate(0.0)
                }
            }
            ColorMode::GradientThroughEffect => gradient.evaluate(t),
            ColorMode::GradientAcrossItems => gradient.evaluate(spatial_pos),
            ColorMode::Static => gradient.evaluate(0.0),
        };

        let effect_color = color.scale(intensity * opacity);
        *pixel = pixel.blend(effect_color, blend_mode);
    }
}

pub struct WipeEffect;

impl Effect for WipeEffect {
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        pixel_count: usize,
        params: &EffectParams,
    ) -> Color {
        // Fallback: use index-based horizontal position
        let pos = if pixel_count > 0 {
            pixel_index as f64 / pixel_count as f64
        } else {
            0.0
        };

        let gradient = params.gradient_or(ParamKey::Gradient, &DEFAULT_WHITE_GRADIENT);
        let movement_curve = params.curve_or(ParamKey::MovementCurve, &DEFAULT_MOVEMENT);
        let pulse_curve = params.curve_or(ParamKey::PulseCurve, &DEFAULT_PULSE);
        let speed = params.float_or(ParamKey::Speed, 1.0);
        let pulse_width = params.float_or(ParamKey::PulseWidth, 1.0).clamp(0.01, 1.0);
        let reverse = params.bool_or(ParamKey::Reverse, false);
        let pass_count = params.float_or(ParamKey::PassCount, 1.0).max(0.1);
        let wipe_on = params.bool_or(ParamKey::WipeOn, true);

        let head = movement_curve.evaluate((t * pass_count * speed).fract());
        let head = if reverse { 1.0 - head } else { head };
        let head_pos = head * (1.0 + pulse_width) - pulse_width;
        let dist = pos - head_pos;

        let intensity = if wipe_on {
            if dist <= 0.0 {
                1.0
            } else if dist < pulse_width {
                1.0 - pulse_curve.evaluate(dist / pulse_width)
            } else {
                0.0
            }
        } else if dist <= 0.0 {
            0.0
        } else if dist < pulse_width {
            pulse_curve.evaluate(dist / pulse_width)
        } else {
            1.0
        };

        gradient.evaluate(0.0).scale(intensity)
    }

    fn name(&self) -> &'static str {
        "Wipe"
    }

    fn param_schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema {
                key: ParamKey::Direction,
                label: "Direction".into(),
                param_type: ParamType::Text {
                    options: vec![
                        "horizontal".into(),
                        "vertical".into(),
                        "diagonal_up".into(),
                        "diagonal_down".into(),
                        "burst".into(),
                        "circle".into(),
                        "diamond".into(),
                    ],
                },
                default: ParamValue::Text("horizontal".into()),
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
                default: ParamValue::ColorMode(ColorMode::Static),
            },
            ParamSchema {
                key: ParamKey::MovementCurve,
                label: "Movement Curve".into(),
                param_type: ParamType::Curve,
                default: ParamValue::Curve(Curve::linear()),
            },
            ParamSchema {
                key: ParamKey::PulseCurve,
                label: "Edge Curve".into(),
                param_type: ParamType::Curve,
                default: ParamValue::Curve(Curve::linear()),
            },
            ParamSchema {
                key: ParamKey::Speed,
                label: "Speed".into(),
                param_type: ParamType::Float {
                    min: 0.1,
                    max: 20.0,
                    step: 0.1,
                },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: ParamKey::PulseWidth,
                label: "Edge Width".into(),
                param_type: ParamType::Float {
                    min: 0.01,
                    max: 1.0,
                    step: 0.01,
                },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: ParamKey::Reverse,
                label: "Reverse".into(),
                param_type: ParamType::Bool,
                default: ParamValue::Bool(false),
            },
            ParamSchema {
                key: ParamKey::CenterX,
                label: "Center X".into(),
                param_type: ParamType::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                },
                default: ParamValue::Float(0.5),
            },
            ParamSchema {
                key: ParamKey::CenterY,
                label: "Center Y".into(),
                param_type: ParamType::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                },
                default: ParamValue::Float(0.5),
            },
            ParamSchema {
                key: ParamKey::PassCount,
                label: "Pass Count".into(),
                param_type: ParamType::Float {
                    min: 0.1,
                    max: 10.0,
                    step: 0.1,
                },
                default: ParamValue::Float(1.0),
            },
            ParamSchema {
                key: ParamKey::WipeOn,
                label: "Wipe On (Reveal)".into(),
                param_type: ParamType::Bool,
                default: ParamValue::Bool(true),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: f32, y: f32) -> Position2D {
        Position2D { x, y }
    }

    #[test]
    fn project_horizontal_corners() {
        assert!((project_position(pos(0.0, 0.5), "horizontal", 0.5, 0.5) - 0.0).abs() < 1e-10);
        assert!((project_position(pos(1.0, 0.5), "horizontal", 0.5, 0.5) - 1.0).abs() < 1e-10);
        assert!((project_position(pos(0.5, 0.0), "horizontal", 0.5, 0.5) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn project_vertical_corners() {
        assert!((project_position(pos(0.5, 0.0), "vertical", 0.5, 0.5) - 0.0).abs() < 1e-10);
        assert!((project_position(pos(0.5, 1.0), "vertical", 0.5, 0.5) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn project_diagonal_up() {
        // Bottom-left (0,1): (0 + 0)/2 = 0
        assert!(
            (project_position(pos(0.0, 1.0), "diagonal_up", 0.5, 0.5) - 0.0).abs() < 1e-10
        );
        // Top-right (1,0): (1 + 1)/2 = 1
        assert!(
            (project_position(pos(1.0, 0.0), "diagonal_up", 0.5, 0.5) - 1.0).abs() < 1e-10
        );
        // Center (0.5,0.5): (0.5 + 0.5)/2 = 0.5
        assert!(
            (project_position(pos(0.5, 0.5), "diagonal_up", 0.5, 0.5) - 0.5).abs() < 1e-10
        );
    }

    #[test]
    fn project_diagonal_down() {
        // Top-left (0,0): (0 + 0)/2 = 0
        assert!(
            (project_position(pos(0.0, 0.0), "diagonal_down", 0.5, 0.5) - 0.0).abs() < 1e-10
        );
        // Bottom-right (1,1): (1 + 1)/2 = 1
        assert!(
            (project_position(pos(1.0, 1.0), "diagonal_down", 0.5, 0.5) - 1.0).abs() < 1e-10
        );
    }

    #[test]
    fn project_circle_center_is_zero() {
        assert!(
            (project_position(pos(0.5, 0.5), "circle", 0.5, 0.5) - 0.0).abs() < 1e-10
        );
    }

    #[test]
    fn project_circle_corner_is_one() {
        // Corner (1, 1) should be at distance 1.0 from center (0.5, 0.5)
        let val = project_position(pos(1.0, 1.0), "circle", 0.5, 0.5);
        assert!((val - 1.0).abs() < 1e-6);
    }

    #[test]
    fn project_diamond_center_is_zero() {
        assert!(
            (project_position(pos(0.5, 0.5), "diamond", 0.5, 0.5) - 0.0).abs() < 1e-10
        );
    }

    #[test]
    fn project_burst_center_is_zero() {
        assert!(
            (project_position(pos(0.5, 0.5), "burst", 0.5, 0.5) - 0.0).abs() < 1e-10
        );
    }

    #[test]
    fn horizontal_wipe_half_lit() {
        // At t=0.5 with linear movement, head should be at ~0.5
        // Pixels at x < 0.5 should be fully lit, pixels at x > 0.5 should be dark
        let positions: Vec<Position2D> = (0..10)
            .map(|i| pos(i as f32 / 9.0, 0.5))
            .collect();
        let mut dest = vec![Color::BLACK; 10];
        let params = EffectParams::new()
            .set(ParamKey::Direction, ParamValue::Text("horizontal".into()))
            .set(ParamKey::PulseWidth, ParamValue::Float(0.05))
            .set(ParamKey::WipeOn, ParamValue::Bool(true));

        evaluate_pixels_batch(0.5, &mut dest, 0, 10, &params, BlendMode::Override, 1.0, Some(&positions));

        // First pixel (x=0.0) should be lit
        assert!(dest[0].r > 200, "first pixel should be bright, got r={}", dest[0].r);
        // Last pixel (x=1.0) should be dark
        assert!(dest[9].r < 50, "last pixel should be dark, got r={}", dest[9].r);
    }

    #[test]
    fn circle_wipe_center_lit_first() {
        // At t=0.0 with circle direction, only center should be lit (head at edge of entry)
        // Actually at t near 0, head_pos is near -pulse_width, so nothing is lit yet.
        // At t=0.5, center (distance 0) is behind the head → lit.
        let positions = vec![
            pos(0.5, 0.5), // center
            pos(0.0, 0.0), // corner
            pos(1.0, 1.0), // corner
        ];
        let mut dest = vec![Color::BLACK; 3];
        let params = EffectParams::new()
            .set(ParamKey::Direction, ParamValue::Text("circle".into()))
            .set(ParamKey::PulseWidth, ParamValue::Float(0.1))
            .set(ParamKey::WipeOn, ParamValue::Bool(true));

        evaluate_pixels_batch(0.3, &mut dest, 0, 3, &params, BlendMode::Override, 1.0, Some(&positions));

        // Center (distance=0) should be lit before corners
        assert!(dest[0].r > dest[1].r, "center should be brighter than corner");
        assert!(dest[0].r > dest[2].r, "center should be brighter than corner");
    }

    #[test]
    fn wipe_off_inverts() {
        let positions: Vec<Position2D> = (0..10)
            .map(|i| pos(i as f32 / 9.0, 0.5))
            .collect();

        let mut dest_on = vec![Color::BLACK; 10];
        let params_on = EffectParams::new()
            .set(ParamKey::Direction, ParamValue::Text("horizontal".into()))
            .set(ParamKey::PulseWidth, ParamValue::Float(0.05))
            .set(ParamKey::WipeOn, ParamValue::Bool(true));
        evaluate_pixels_batch(0.5, &mut dest_on, 0, 10, &params_on, BlendMode::Override, 1.0, Some(&positions));

        let mut dest_off = vec![Color::BLACK; 10];
        let params_off = EffectParams::new()
            .set(ParamKey::Direction, ParamValue::Text("horizontal".into()))
            .set(ParamKey::PulseWidth, ParamValue::Float(0.05))
            .set(ParamKey::WipeOn, ParamValue::Bool(false));
        evaluate_pixels_batch(0.5, &mut dest_off, 0, 10, &params_off, BlendMode::Override, 1.0, Some(&positions));

        // First pixel: wipe_on=bright, wipe_off=dark
        assert!(dest_on[0].r > dest_off[0].r);
        // Last pixel: wipe_on=dark, wipe_off=bright
        assert!(dest_off[9].r > dest_on[9].r);
    }

    #[test]
    fn fallback_without_positions() {
        // When positions=None, should use index-based fallback (like horizontal)
        let mut dest = vec![Color::BLACK; 10];
        let params = EffectParams::new()
            .set(ParamKey::PulseWidth, ParamValue::Float(0.05));
        evaluate_pixels_batch(0.5, &mut dest, 0, 10, &params, BlendMode::Override, 1.0, None);
        // Should not panic, and should produce some output
        assert!(dest.iter().any(|c| c.r > 0));
    }
}
