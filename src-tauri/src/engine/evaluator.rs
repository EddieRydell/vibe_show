use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::dsl::compiler::CompiledScript;
use crate::effects;
use crate::model::fixture::EffectTarget;
use crate::model::show::Position2D;
use crate::model::color_gradient::ColorGradient;
use crate::model::curve::Curve;
use crate::model::{Color, EffectKind, FixtureId, GroupId, Show};
use crate::util::base64_encode;

/// A single frame of output: colors for every pixel of every fixture.
///
/// Only fixtures with non-black pixels are included. Pixel data is
/// base64-encoded RGBA bytes for compact serialization over IPC.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Frame {
    /// Map from fixture ID to base64-encoded RGBA pixel data.
    /// Each string decodes to `pixel_count * 4` bytes (R, G, B, A per pixel).
    /// Fixtures that are all-black are omitted.
    pub fixtures: HashMap<u32, String>,
    /// Diagnostic warnings when the frame is empty for a known reason
    /// (e.g. missing sequence, no tracks). `None` when there is nothing to report.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub warnings: Option<Vec<String>>,
}

/// Resolve target fixtures using pre-computed group cache.
/// Returns a borrowed slice to avoid cloning per-track.
fn resolve_target_cached<'a>(
    target: &'a EffectTarget,
    all_fixtures: &'a [FixtureId],
    group_cache: &'a HashMap<GroupId, Vec<FixtureId>>,
) -> &'a [FixtureId] {
    match target {
        EffectTarget::All => all_fixtures,
        EffectTarget::Fixtures(ids) => ids,
        EffectTarget::Group(group_id) => group_cache
            .get(group_id)
            .map_or(&[], |v| v.as_slice()),
    }
}

/// Convert a Color slice to base64-encoded RGBA bytes.
///
/// # Safety
/// Color is `#[repr(C)]` with fields `{r: u8, g: u8, b: u8, a: u8}`,
/// identical layout to `[u8; 4]`.
fn colors_to_base64(colors: &[Color]) -> String {
    const _: () = assert!(std::mem::size_of::<Color>() == 4);
    const _: () = assert!(std::mem::align_of::<Color>() == 1);

    // SAFETY: Color is #[repr(C)] {r: u8, g: u8, b: u8, a: u8} = 4 bytes.
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(colors.as_ptr().cast::<u8>(), colors.len() * 4) };
    base64_encode(bytes)
}

/// Check if all pixels are BLACK (r=0, g=0, b=0, a=255).
fn is_all_black(colors: &[Color]) -> bool {
    colors.iter().all(|c| *c == Color::BLACK)
}

/// Evaluate the full show at a given time, producing a Frame.
///
/// Pipeline:
/// 1. Start with all fixtures at BLACK
/// 2. For each track (bottom to top):
///    a. Find all EffectInstances active at time `t`
///    b. For each targeted fixture, evaluate the effect
///    c. Blend the result onto the accumulated frame using the track's blend mode
/// 3. Encode only non-black fixtures as base64 for compact IPC transfer
///
/// If `effect_filter` is provided, only the specified (track_index, effect_index)
/// pairs are evaluated. All other effects are skipped.
#[allow(clippy::cast_precision_loss, clippy::implicit_hasher)]
pub fn evaluate(
    show: &Show,
    sequence_index: usize,
    t: f64,
    effect_filter: Option<&[(usize, usize)]>,
    script_cache: Option<&HashMap<String, Arc<CompiledScript>>>,
    gradient_lib: &HashMap<String, ColorGradient>,
    curve_lib: &HashMap<String, Curve>,
) -> Frame {
    let Some(sequence) = show.sequences.get(sequence_index) else {
        return Frame {
            fixtures: HashMap::new(),
            warnings: Some(vec![format!(
                "Sequence not found (index {sequence_index}, show has {})",
                show.sequences.len()
            )]),
        };
    };

    if sequence.tracks.is_empty() {
        return Frame {
            fixtures: HashMap::new(),
            warnings: Some(vec!["No tracks in sequence".to_string()]),
        };
    }

    let motion_path_lib = &sequence.motion_paths;

    // Phase 1A: Build fixture pixel count lookup (eliminates O(N) scans).
    let pixel_counts: HashMap<FixtureId, usize> = show
        .fixtures
        .iter()
        .map(|f| (f.id, f.pixel_count as usize))
        .collect();

    // Phase 1B: Pre-resolve group targets (eliminates repeated recursive resolution).
    let group_fixtures: HashMap<GroupId, Vec<FixtureId>> = show
        .groups
        .iter()
        .map(|g| (g.id, g.resolve_fixture_ids(&show.groups)))
        .collect();

    // Pre-build all-fixtures list for EffectTarget::All resolution (avoids per-track collect).
    let all_fixture_ids: Vec<FixtureId> = show.fixtures.iter().map(|f| f.id).collect();

    // Initialize only targeted fixtures to black (lazy via HashMap).
    let mut frame: HashMap<FixtureId, Vec<Color>> = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();

    // Evaluate tracks bottom-to-top.
    for (track_idx, track) in sequence.tracks.iter().enumerate() {
        // If a filter is active, skip tracks that have no entries in the filter.
        if let Some(filter) = effect_filter {
            if !filter.iter().any(|&(ti, _)| ti == track_idx) {
                continue;
            }
        }

        // Phase 1C: Binary search for active effects.
        // Effects are sorted by start time. Skip effects that start after t.
        let end_idx = track.effects.partition_point(|e| e.time_range.start() <= t);

        // Collect active effects first — skip target resolution entirely if none active.
        // This avoids HashMap lookups and Vec clones for inactive tracks.
        let active: Vec<_> = track.effects.get(..end_idx).unwrap_or(&track.effects)
            .iter()
            .enumerate()
            .filter(|&(ei, e)| {
                e.time_range.contains(t)
                    && effect_filter
                        .is_none_or(|f| f.iter().any(|&(ti, fi)| ti == track_idx && fi == ei))
            })
            .map(|(_, e)| e)
            .collect();
        if active.is_empty() {
            continue;
        }

        let target_fixtures = resolve_target_cached(&track.target, &all_fixture_ids, &group_fixtures);

        // Warn on dangling references (missing groups or fixtures that resolve to nothing).
        if target_fixtures.is_empty() {
            match &track.target {
                EffectTarget::Group(gid) => {
                    if !group_fixtures.contains_key(gid) {
                        warnings.push(format!(
                            "Track \"{}\" references non-existent group {:?}",
                            track.name, gid
                        ));
                    }
                }
                EffectTarget::Fixtures(ids) if !ids.is_empty() => {
                    let missing: Vec<_> = ids
                        .iter()
                        .filter(|id| !pixel_counts.contains_key(id))
                        .collect();
                    if !missing.is_empty() {
                        warnings.push(format!(
                            "Track \"{}\" references non-existent fixture(s): {:?}",
                            track.name, missing
                        ));
                    }
                }
                _ => {}
            }
        }

        // Compute total pixel count once per track (same for all effects on this track).
        let total_pixels: usize = target_fixtures
            .iter()
            .map(|id| pixel_counts.get(id).copied().unwrap_or(0))
            .sum();

        for effect_instance in &active {
            let t_normalized = effect_instance.time_range.normalize(t);
            let spatial = effects::needs_positions(&effect_instance.kind);

            // Resolve library references once per effect (outside per-fixture loop).
            // Use Cow to avoid cloning when there are no refs to resolve.
            let resolved_params: Cow<'_, _> = if effect_instance.params.has_refs() {
                Cow::Owned(effect_instance.params.resolve_refs(gradient_lib, curve_lib))
            } else {
                Cow::Borrowed(&effect_instance.params)
            };

            // Build flat position vector for spatial effects (e.g. Wipe).
            // Non-spatial effects skip this entirely (zero overhead).
            let positions: Option<Vec<Position2D>> = if spatial {
                // Build a HashMap of fixture_id → pixel_positions for fast lookup
                let layout_map: HashMap<FixtureId, &[Position2D]> = show
                    .layout
                    .fixtures
                    .iter()
                    .map(|fl| (fl.fixture_id, fl.pixel_positions.as_slice()))
                    .collect();

                let mut pos_vec = Vec::with_capacity(total_pixels);
                for &fid in target_fixtures {
                    let pc = pixel_counts.get(&fid).copied().unwrap_or(0);
                    if let Some(positions) = layout_map.get(&fid) {
                        if positions.len() == pc {
                            pos_vec.extend_from_slice(positions);
                        } else {
                            // Layout mismatch: fall back to evenly-spaced horizontal
                            eprintln!(
                                "Layout mismatch for fixture {:?}: expected {} positions, got {} — using fallback",
                                fid, pc, positions.len()
                            );
                            for i in 0..pc {
                                let x = if pc > 1 { i as f32 / (pc - 1) as f32 } else { 0.5 };
                                pos_vec.push(Position2D { x, y: 0.5 });
                            }
                        }
                    } else {
                        // No layout data: fall back to evenly-spaced horizontal
                        for i in 0..pc {
                            let x = if pc > 1 { i as f32 / (pc - 1) as f32 } else { 0.5 };
                            pos_vec.push(Position2D { x, y: 0.5 });
                        }
                    }
                }
                Some(pos_vec)
            } else {
                None
            };

            let mut global_pixel_offset = 0usize;

            for &fixture_id in target_fixtures {
                let pixel_count = pixel_counts.get(&fixture_id).copied().unwrap_or(0);
                if pixel_count == 0 {
                    continue;
                }

                let pixels = frame
                    .entry(fixture_id)
                    .or_insert_with(|| vec![Color::BLACK; pixel_count]);

                // Slice positions for this fixture (spatial effects only).
                let fixture_positions = positions
                    .as_ref()
                    .and_then(|p| p.get(global_pixel_offset..global_pixel_offset + pixel_count));

                // Phase 2: Batch pixel evaluation (params extracted once, not per-pixel).
                let handled = effects::evaluate_pixels(
                    &effect_instance.kind,
                    t_normalized,
                    pixels,
                    global_pixel_offset,
                    total_pixels,
                    &resolved_params,
                    effect_instance.blend_mode,
                    effect_instance.opacity,
                    fixture_positions,
                );

                // Fall through to DSL VM for Script effects.
                if !handled {
                    if let EffectKind::Script(ref script_name) = effect_instance.kind {
                        if let Some(compiled) = script_cache
                            .and_then(|cache| cache.get(script_name))
                        {
                            effects::script::evaluate_pixels_batch(
                                compiled,
                                t_normalized,
                                t,
                                pixels,
                                global_pixel_offset,
                                total_pixels,
                                &resolved_params,
                                effect_instance.blend_mode,
                                effect_instance.opacity,
                                fixture_positions,
                                Some(motion_path_lib),
                            );
                        }
                    }
                }

                global_pixel_offset += pixel_count;
            }
        }
    }

    // Only encode non-black fixtures as base64 for compact IPC transfer.
    Frame {
        fixtures: frame
            .into_iter()
            .filter(|(_, colors)| !is_all_black(colors))
            .map(|(id, colors)| (id.0, colors_to_base64(&colors)))
            .collect(),
        warnings: if warnings.is_empty() {
            None
        } else {
            Some(warnings)
        },
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
)]
mod tests {
    use super::*;
    use crate::model::fixture::{
        BulbShape, ChannelOrder, ColorModel, EffectTarget, FixtureDef, FixtureGroup, GroupMember,
        PixelType,
    };
    use crate::model::show::{Layout, Show};
    use crate::model::timeline::{
        BlendMode, EffectInstance, EffectKind, EffectParams, ParamKey, ParamValue, Sequence, TimeRange, Track,
    };

    fn fixture(id: u32, pixels: u32) -> FixtureDef {
        FixtureDef {
            id: FixtureId(id),
            name: format!("Fixture {id}"),
            color_model: ColorModel::Rgb,
            pixel_count: pixels,
            pixel_type: PixelType::Smart,
            bulb_shape: BulbShape::LED,
            display_radius_override: None,
            channel_order: ChannelOrder::Rgb,
        }
    }

    fn solid_effect(start: f64, end: f64, color: Color) -> EffectInstance {
        EffectInstance {
            kind: EffectKind::Solid,
            params: EffectParams::new().set(ParamKey::Color, ParamValue::Color(color)),
            time_range: TimeRange::new(start, end).unwrap(),
            blend_mode: BlendMode::Override,
            opacity: 1.0,
        }
    }

    fn solid_effect_blended(start: f64, end: f64, color: Color, blend_mode: BlendMode, opacity: f64) -> EffectInstance {
        EffectInstance {
            kind: EffectKind::Solid,
            params: EffectParams::new().set(ParamKey::Color, ParamValue::Color(color)),
            time_range: TimeRange::new(start, end).unwrap(),
            blend_mode,
            opacity,
        }
    }

    fn simple_show(fixtures: Vec<FixtureDef>, tracks: Vec<Track>) -> Show {
        Show {
            name: "Test".into(),
            fixtures,
            groups: vec![],
            layout: Layout { fixtures: vec![] },
            sequences: vec![Sequence {
                name: "Seq".into(),
                duration: 10.0,
                frame_rate: 30.0,
                audio_file: None,
                tracks,
                motion_paths: std::collections::HashMap::new(),
            }],
            patches: vec![],
            controllers: vec![],
        }
    }

    /// Decode base64 RGBA back to Color vec for assertions.
    fn decode_fixture_colors(frame: &Frame, fixture_id: u32) -> Option<Vec<Color>> {
        let b64 = frame.fixtures.get(&fixture_id)?;
        let bytes = base64_decode(b64);
        Some(
            bytes
                .chunks_exact(4)
                .map(|c| Color::rgba(c[0], c[1], c[2], c[3]))
                .collect(),
        )
    }

    fn base64_decode(input: &str) -> Vec<u8> {
        const TABLE: [u8; 128] = {
            let mut t = [255u8; 128];
            let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut i = 0;
            while i < 64 {
                t[chars[i] as usize] = i as u8;
                i += 1;
            }
            t
        };
        let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
        let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
        for chunk in bytes.chunks(4) {
            let vals: Vec<u8> = chunk.iter().map(|&b| TABLE[b as usize]).collect();
            if vals.len() >= 2 {
                out.push((vals[0] << 2) | (vals[1] >> 4));
            }
            if vals.len() >= 3 {
                out.push((vals[1] << 4) | (vals[2] >> 2));
            }
            if vals.len() >= 4 {
                out.push((vals[2] << 6) | vals[3]);
            }
        }
        out
    }

    #[test]
    fn single_solid_effect_produces_correct_output() {
        let red = Color::rgb(255, 0, 0);
        let show = simple_show(
            vec![fixture(1, 5)],
            vec![Track {
                name: "T1".into(),
                target: EffectTarget::All,
                effects: vec![solid_effect(0.0, 5.0, red)],
            }],
        );
        let frame = evaluate(&show, 0, 2.5, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).expect("fixture should be in frame");
        assert_eq!(colors.len(), 5);
        for c in &colors {
            assert_eq!(c.r, 255);
            assert_eq!(c.g, 0);
            assert_eq!(c.b, 0);
        }
    }

    #[test]
    fn effect_only_active_during_time_range() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![Track {
                name: "T1".into(),
                target: EffectTarget::All,
                effects: vec![solid_effect(2.0, 4.0, Color::WHITE)],
            }],
        );
        // Before range
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        assert!(frame.fixtures.is_empty());

        // Inside range
        let frame = evaluate(&show, 0, 3.0, None, None, &HashMap::new(), &HashMap::new());
        assert!(frame.fixtures.contains_key(&1));

        // Well past the end (beyond epsilon tolerance)
        let frame = evaluate(&show, 0, 4.1, None, None, &HashMap::new(), &HashMap::new());
        assert!(frame.fixtures.is_empty());
    }

    #[test]
    fn two_tracks_override_blend() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(255, 0, 0))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(0, 255, 0))],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        assert_eq!(colors[0], Color::rgb(0, 255, 0));
    }

    #[test]
    fn two_tracks_add_blend_saturates() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(200, 100, 0))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect_blended(0.0, 5.0, Color::rgb(200, 200, 50), BlendMode::Add, 1.0)],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        assert_eq!(colors[0].r, 255); // saturated
        assert_eq!(colors[0].g, 255); // 100+200 saturated
        assert_eq!(colors[0].b, 50);
    }

    #[test]
    fn two_tracks_multiply_blend() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(255, 128, 0))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect_blended(0.0, 5.0, Color::WHITE, BlendMode::Multiply, 1.0)],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        // Multiply with white is identity
        assert_eq!(colors[0].r, 255);
        assert_eq!(colors[0].g, 128);
        assert_eq!(colors[0].b, 0);
    }

    #[test]
    fn effects_span_across_fixtures_seamlessly() {
        // Gradient across 2 fixtures (5 pixels each = 10 total)
        // Should be continuous, not restart per fixture
        let show = simple_show(
            vec![fixture(1, 5), fixture(2, 5)],
            vec![Track {
                name: "T1".into(),
                target: EffectTarget::All,
                effects: vec![EffectInstance {
                    kind: EffectKind::Gradient,
                    params: EffectParams::new().set(
                        ParamKey::Colors,
                        ParamValue::ColorList(vec![Color::rgb(0, 0, 0), Color::rgb(255, 255, 255)]),
                    ),
                    time_range: TimeRange::new(0.0, 5.0).unwrap(),
                    blend_mode: BlendMode::Override,
                    opacity: 1.0,
                }],
            }],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let f2 = decode_fixture_colors(&frame, 2).unwrap();
        // Fixture 2 starts at global pixel 5. With 10 total pixels, pixel 5 has pos=5/9≈0.56.
        // Fixture 2 pixel 3 has global pos 8/9≈0.89 → should be bright.
        // If the gradient restarted per fixture, pixel 0 of f2 would be dark.
        // Instead it should be mid-brightness (continuous from fixture 1).
        assert!(
            f2[0].r > 100,
            "first pixel of fixture 2 should be mid-brightness (continuous), got r={}",
            f2[0].r
        );
        // Later pixels in fixture 2 should be brighter than earlier ones (monotonic gradient)
        assert!(f2[3].r > f2[0].r);
    }

    #[test]
    fn empty_show_produces_empty_frame() {
        let show = Show::empty();
        let frame = evaluate(&show, 0, 0.0, None, None, &HashMap::new(), &HashMap::new());
        assert!(frame.fixtures.is_empty());
    }

    #[test]
    fn zero_pixel_fixtures_are_skipped() {
        let show = simple_show(
            vec![fixture(1, 0), fixture(2, 3)],
            vec![Track {
                name: "T1".into(),
                target: EffectTarget::All,
                effects: vec![solid_effect(0.0, 5.0, Color::WHITE)],
            }],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        assert!(!frame.fixtures.contains_key(&1));
        assert!(frame.fixtures.contains_key(&2));
    }

    #[test]
    fn black_fixtures_excluded_from_output() {
        let show = simple_show(
            vec![fixture(1, 3)],
            vec![Track {
                name: "T1".into(),
                target: EffectTarget::All,
                effects: vec![solid_effect(0.0, 5.0, Color::BLACK)],
            }],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        assert!(frame.fixtures.is_empty());
    }

    #[test]
    fn group_targeting_resolves_fixtures() {
        let mut show = simple_show(
            vec![fixture(1, 3), fixture(2, 3), fixture(3, 3)],
            vec![Track {
                name: "T1".into(),
                target: EffectTarget::Group(GroupId(10)),
                effects: vec![solid_effect(0.0, 5.0, Color::rgb(255, 0, 0))],
            }],
        );
        show.groups.push(FixtureGroup {
            id: GroupId(10),
            name: "Group A".into(),
            members: vec![
                GroupMember::Fixture(FixtureId(1)),
                GroupMember::Fixture(FixtureId(3)),
            ],
        });
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        assert!(frame.fixtures.contains_key(&1));
        assert!(!frame.fixtures.contains_key(&2)); // not in group
        assert!(frame.fixtures.contains_key(&3));
    }

    #[test]
    fn effect_filter_limits_evaluation() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "T0".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(255, 0, 0))],
                },
                Track {
                    name: "T1".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(0, 255, 0))],
                },
            ],
        );
        // Only evaluate track 0, effect 0
        let filter = [(0usize, 0usize)];
        let frame = evaluate(&show, 0, 1.0, Some(&filter), None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        assert_eq!(colors[0], Color::rgb(255, 0, 0)); // track 1 was skipped
    }

    #[test]
    fn subtract_blend_mode() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(200, 150, 100))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect_blended(0.0, 5.0, Color::rgb(50, 200, 30), BlendMode::Subtract, 1.0)],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        assert_eq!(colors[0].r, 150); // 200 - 50
        assert_eq!(colors[0].g, 0);   // 150 - 200 saturates to 0
        assert_eq!(colors[0].b, 70);  // 100 - 30
    }

    #[test]
    fn min_blend_mode() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(200, 50, 100))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect_blended(0.0, 5.0, Color::rgb(100, 150, 80), BlendMode::Min, 1.0)],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        assert_eq!(colors[0].r, 100);
        assert_eq!(colors[0].g, 50);
        assert_eq!(colors[0].b, 80);
    }

    #[test]
    fn average_blend_mode() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(200, 100, 0))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect_blended(0.0, 5.0, Color::rgb(100, 50, 200), BlendMode::Average, 1.0)],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        assert_eq!(colors[0].r, 150); // (200+100)/2
        assert_eq!(colors[0].g, 75);  // (100+50)/2
        assert_eq!(colors[0].b, 100); // (0+200)/2
    }

    #[test]
    fn screen_blend_mode() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(128, 0, 255))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect_blended(0.0, 5.0, Color::rgb(128, 128, 0), BlendMode::Screen, 1.0)],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        // screen(128,128) = 255 - (127*127)/255 = 255 - 63 = 192
        assert_eq!(colors[0].r, 192);
        // screen(0,128) = 255 - (255*127)/255 = 255 - 127 = 128
        assert_eq!(colors[0].g, 128);
        // screen(255,0) = 255 - (0*255)/255 = 255
        assert_eq!(colors[0].b, 255);
    }

    #[test]
    fn mask_blend_mode() {
        let show = simple_show(
            vec![fixture(1, 3)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(255, 128, 64))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    // fg is non-black → mask produces black
                    effects: vec![solid_effect_blended(0.0, 5.0, Color::rgb(10, 0, 0), BlendMode::Mask, 1.0)],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        // All pixels should be black (masked out), so frame is empty
        assert!(frame.fixtures.is_empty());
    }

    #[test]
    fn intensity_overlay_blend_mode() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![
                Track {
                    name: "Bottom".into(),
                    target: EffectTarget::All,
                    effects: vec![solid_effect(0.0, 5.0, Color::rgb(200, 100, 50))],
                },
                Track {
                    name: "Top".into(),
                    target: EffectTarget::All,
                    // Pure white fg has brightness ~1.0, so bg is preserved
                    effects: vec![solid_effect_blended(0.0, 5.0, Color::WHITE, BlendMode::IntensityOverlay, 1.0)],
                },
            ],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        assert_eq!(colors[0].r, 200);
        assert_eq!(colors[0].g, 100);
        assert_eq!(colors[0].b, 50);
    }

    #[test]
    fn opacity_half_produces_dimmed_output() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![Track {
                name: "T1".into(),
                target: EffectTarget::All,
                effects: vec![solid_effect_blended(0.0, 5.0, Color::rgb(200, 100, 50), BlendMode::Override, 0.5)],
            }],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        let colors = decode_fixture_colors(&frame, 1).unwrap();
        assert_eq!(colors[0].r, 100);
        assert_eq!(colors[0].g, 50);
        assert_eq!(colors[0].b, 25);
    }

    #[test]
    fn opacity_zero_produces_no_output() {
        let show = simple_show(
            vec![fixture(1, 1)],
            vec![Track {
                name: "T1".into(),
                target: EffectTarget::All,
                effects: vec![solid_effect_blended(0.0, 5.0, Color::WHITE, BlendMode::Override, 0.0)],
            }],
        );
        let frame = evaluate(&show, 0, 1.0, None, None, &HashMap::new(), &HashMap::new());
        // opacity=0 means all black, so frame should be empty
        assert!(frame.fixtures.is_empty());
    }
}
