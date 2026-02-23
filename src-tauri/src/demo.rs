use crate::model::fixture::{BulbShape, ChannelOrder, ColorModel, EffectTarget, FixtureGroup, GroupMember, PixelType};
use crate::model::show::{FixtureLayout, Layout, LayoutShape, Position2D};
use crate::model::timeline::TimeRange;
use crate::model::{
    BlendMode, Color, EffectInstance, EffectKind, EffectParams, FixtureDef, FixtureId, GroupId,
    ParamKey, ParamValue, Sequence, Show, Track,
};

/// Creates a demo show with 100 RGB pixels in a grid, multiple tracks with different effects.
#[allow(clippy::unwrap_used)] // hardcoded valid TimeRange constants
#[allow(clippy::cast_precision_loss)]
pub fn create_demo_show() -> Show {
    let pixel_count = 100u32;
    let cols = 20u32;
    let rows = pixel_count / cols;

    // Create fixtures: 5 strings of 20 pixels each (simulating 5 roof lines).
    let mut fixtures = Vec::new();
    let mut layout_fixtures = Vec::new();

    for row in 0..rows {
        let fixture_id = FixtureId(row);
        fixtures.push(FixtureDef {
            id: fixture_id,
            name: format!("String {}", row + 1),
            color_model: ColorModel::Rgb,
            pixel_count: cols,
            pixel_type: PixelType::default(),
            bulb_shape: BulbShape::default(),
            display_radius_override: None,
            channel_order: ChannelOrder::default(),
        });

        let pixel_positions: Vec<Position2D> = (0..cols)
            .map(|col| Position2D {
                x: (col as f32 + 0.5) / cols as f32,
                y: (row as f32 + 0.5) / rows as f32,
            })
            .collect();

        layout_fixtures.push(FixtureLayout {
            fixture_id,
            pixel_positions,
            shape: LayoutShape::default(),
        });
    }

    let groups = vec![FixtureGroup {
        id: GroupId(0),
        name: "All Strings".into(),
        members: (0..rows).map(|i| GroupMember::Fixture(FixtureId(i))).collect(),
    }];

    // Build a demo sequence: 30 seconds, multiple layered effects.
    let sequence = Sequence {
        name: "Demo Sequence".into(),
        duration: 30.0,
        frame_rate: 30.0,
        audio_file: None,
        scripts: std::collections::HashMap::new(),
        gradient_library: std::collections::HashMap::new(),
        curve_library: std::collections::HashMap::new(),
        motion_paths: std::collections::HashMap::new(),
        tracks: vec![
            // Base layer: slow rainbow across all strings.
            Track {
                name: "Rainbow Base".into(),
                target: EffectTarget::Group(GroupId(0)),
                effects: vec![EffectInstance {
                    kind: EffectKind::Rainbow,
                    params: EffectParams::new()
                        .set(ParamKey::Speed, ParamValue::Float(0.5))
                        .set(ParamKey::Spread, ParamValue::Float(2.0))
                        .set(ParamKey::Brightness, ParamValue::Float(0.4)),
                    time_range: TimeRange::new(0.0, 30.0).unwrap(),
                    blend_mode: BlendMode::Override,
                    opacity: 1.0,
                }],
            },
            // Chase on top strings, additive.
            Track {
                name: "Chase Top".into(),
                target: EffectTarget::Fixtures(vec![FixtureId(0), FixtureId(1)]),
                effects: vec![EffectInstance {
                    kind: EffectKind::Chase,
                    params: EffectParams::new()
                        .set(ParamKey::Gradient, ParamValue::ColorGradient(
                            crate::model::ColorGradient::solid(Color::rgb(0, 100, 255)),
                        ))
                        .set(ParamKey::Speed, ParamValue::Float(3.0))
                        .set(ParamKey::PulseWidth, ParamValue::Float(0.4)),
                    time_range: TimeRange::new(0.0, 20.0).unwrap(),
                    blend_mode: BlendMode::Add,
                    opacity: 1.0,
                }],
            },
            // Twinkle overlay on bottom strings.
            Track {
                name: "Twinkle Bottom".into(),
                target: EffectTarget::Fixtures(vec![FixtureId(3), FixtureId(4)]),
                effects: vec![EffectInstance {
                    kind: EffectKind::Twinkle,
                    params: EffectParams::new()
                        .set(ParamKey::Color, ParamValue::Color(Color::rgb(255, 255, 200)))
                        .set(ParamKey::Density, ParamValue::Float(0.4))
                        .set(ParamKey::Speed, ParamValue::Float(8.0)),
                    time_range: TimeRange::new(0.0, 30.0).unwrap(),
                    blend_mode: BlendMode::Add,
                    opacity: 1.0,
                }],
            },
            // Strobe burst in the middle, 15-20 seconds.
            Track {
                name: "Strobe Burst".into(),
                target: EffectTarget::Fixtures(vec![FixtureId(2)]),
                effects: vec![EffectInstance {
                    kind: EffectKind::Strobe,
                    params: EffectParams::new()
                        .set(ParamKey::Color, ParamValue::Color(Color::rgb(255, 50, 50)))
                        .set(ParamKey::Rate, ParamValue::Float(8.0))
                        .set(ParamKey::DutyCycle, ParamValue::Float(0.3)),
                    time_range: TimeRange::new(15.0, 20.0).unwrap(),
                    blend_mode: BlendMode::Max,
                    opacity: 1.0,
                }],
            },
            // Gradient sweep at the end.
            Track {
                name: "Gradient Finale".into(),
                target: EffectTarget::Group(GroupId(0)),
                effects: vec![EffectInstance {
                    kind: EffectKind::Gradient,
                    params: EffectParams::new()
                        .set(
                            ParamKey::Colors,
                            ParamValue::ColorList(vec![
                                Color::rgb(255, 0, 100),
                                Color::rgb(0, 255, 100),
                                Color::rgb(100, 0, 255),
                            ]),
                        )
                        .set(ParamKey::Offset, ParamValue::Float(0.5)),
                    time_range: TimeRange::new(20.0, 30.0).unwrap(),
                    blend_mode: BlendMode::Alpha,
                    opacity: 1.0,
                }],
            },
        ],
    };

    Show {
        name: "Demo Show".into(),
        fixtures,
        groups,
        layout: Layout {
            fixtures: layout_fixtures,
        },
        sequences: vec![sequence],
        patches: Vec::new(),
        controllers: Vec::new(),
    }
}
