use std::collections::HashMap;

use crate::effects::resolve_effect;
use crate::model::fixture::EffectTarget;
use crate::model::{Color, FixtureId, Show};

use super::mixer;

/// A single frame of output: colors for every pixel of every fixture.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Frame {
    /// Map from fixture ID to its pixel colors.
    pub fixtures: HashMap<u32, Vec<[u8; 4]>>,
}

/// Resolves which fixture IDs are targeted by an EffectTarget.
fn resolve_target(show: &Show, target: &EffectTarget) -> Vec<FixtureId> {
    match target {
        EffectTarget::All => show.fixtures.iter().map(|f| f.id).collect(),
        EffectTarget::Fixtures(ids) => ids.clone(),
        EffectTarget::Group(group_id) => show
            .groups
            .iter()
            .find(|g| g.id == *group_id)
            .map(|g| g.resolve_fixture_ids(&show.groups))
            .unwrap_or_default(),
    }
}

/// Get pixel count for a fixture by ID.
fn fixture_pixel_count(show: &Show, fixture_id: FixtureId) -> usize {
    show.fixtures
        .iter()
        .find(|f| f.id == fixture_id)
        .map(|f| f.pixel_count as usize)
        .unwrap_or(0)
}

/// Evaluate the full show at a given time, producing a Frame.
///
/// Pipeline:
/// 1. Start with all fixtures at BLACK
/// 2. For each track (bottom to top):
///    a. Find all EffectInstances active at time `t`
///    b. For each targeted fixture, evaluate the effect
///    c. Blend the result onto the accumulated frame using the track's blend mode
pub fn evaluate(show: &Show, sequence_index: usize, t: f64) -> Frame {
    let sequence = match show.sequences.get(sequence_index) {
        Some(s) => s,
        None => {
            return Frame {
                fixtures: HashMap::new(),
            }
        }
    };

    // Initialize all fixtures to black.
    let mut frame: HashMap<FixtureId, Vec<Color>> = show
        .fixtures
        .iter()
        .map(|f| (f.id, vec![Color::BLACK; f.pixel_count as usize]))
        .collect();

    // Evaluate tracks bottom-to-top.
    for track in &sequence.tracks {
        let target_fixtures = resolve_target(show, &track.target);

        // Find active effects at time t.
        for effect_instance in &track.effects {
            if !effect_instance.time_range.contains(t) {
                continue;
            }

            let effect = resolve_effect(&effect_instance.kind);
            let t_normalized = effect_instance.time_range.normalize(t);

            // Calculate total pixel count across all targeted fixtures for this effect.
            // This lets effects like chase/rainbow span across multiple fixtures seamlessly.
            let total_pixels: usize = target_fixtures
                .iter()
                .map(|id| fixture_pixel_count(show, *id))
                .sum();

            let mut global_pixel_offset = 0usize;

            for &fixture_id in &target_fixtures {
                let pixel_count = fixture_pixel_count(show, fixture_id);
                if pixel_count == 0 {
                    continue;
                }

                let pixels = frame.entry(fixture_id).or_insert_with(|| {
                    vec![Color::BLACK; pixel_count]
                });

                for (local_pixel, pixel) in pixels.iter_mut().enumerate() {
                    let global_pixel = global_pixel_offset + local_pixel;
                    let effect_color = effect.evaluate(
                        t_normalized,
                        global_pixel,
                        total_pixels,
                        &effect_instance.params,
                    );

                    *pixel = mixer::blend(*pixel, effect_color, track.blend_mode);
                }

                global_pixel_offset += pixel_count;
            }
        }
    }

    // Convert to serializable format.
    Frame {
        fixtures: frame
            .into_iter()
            .map(|(id, colors)| {
                (
                    id.0,
                    colors
                        .into_iter()
                        .map(|c| [c.r, c.g, c.b, c.a])
                        .collect(),
                )
            })
            .collect(),
    }
}
