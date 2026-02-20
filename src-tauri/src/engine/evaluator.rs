use std::collections::HashMap;

use crate::effects::BuiltinEffect;
use crate::model::fixture::EffectTarget;
use crate::model::{Color, FixtureId, GroupId, Show};

/// A single frame of output: colors for every pixel of every fixture.
///
/// Only fixtures with non-black pixels are included. Pixel data is
/// base64-encoded RGBA bytes for compact serialization over IPC.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Frame {
    /// Map from fixture ID to base64-encoded RGBA pixel data.
    /// Each string decodes to `pixel_count * 4` bytes (R, G, B, A per pixel).
    /// Fixtures that are all-black are omitted.
    pub fixtures: HashMap<u32, String>,
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
            .map(|v| v.as_slice())
            .unwrap_or(&[]),
    }
}

/// Encode raw bytes as base64.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[(triple >> 18 & 0x3F) as usize] as char);
        result.push(CHARS[(triple >> 12 & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[(triple >> 6 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
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
        unsafe { std::slice::from_raw_parts(colors.as_ptr() as *const u8, colors.len() * 4) };
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
pub fn evaluate(show: &Show, sequence_index: usize, t: f64) -> Frame {
    let sequence = match show.sequences.get(sequence_index) {
        Some(s) => s,
        None => {
            return Frame {
                fixtures: HashMap::new(),
            }
        }
    };

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

    // Evaluate tracks bottom-to-top.
    for track in &sequence.tracks {
        // Phase 1C: Binary search for active effects.
        // Effects are sorted by start time. Skip effects that start after t.
        let end_idx = track.effects.partition_point(|e| e.time_range.start() <= t);

        // Collect active effects first — skip target resolution entirely if none active.
        // This avoids HashMap lookups and Vec clones for inactive tracks.
        let active: Vec<_> = track.effects[..end_idx]
            .iter()
            .filter(|e| e.time_range.contains(t))
            .collect();
        if active.is_empty() {
            continue;
        }

        let target_fixtures = resolve_target_cached(&track.target, &all_fixture_ids, &group_fixtures);

        for effect_instance in &active {
            // Phase 2A: Enum dispatch (no Box allocation).
            let effect = BuiltinEffect::from_kind(&effect_instance.kind);
            let t_normalized = effect_instance.time_range.normalize(t);

            // Phase 1A: Use precomputed pixel counts.
            let total_pixels: usize = target_fixtures
                .iter()
                .map(|id| pixel_counts.get(id).copied().unwrap_or(0))
                .sum();

            let mut global_pixel_offset = 0usize;

            for &fixture_id in target_fixtures {
                let pixel_count = pixel_counts.get(&fixture_id).copied().unwrap_or(0);
                if pixel_count == 0 {
                    continue;
                }

                let pixels = frame
                    .entry(fixture_id)
                    .or_insert_with(|| vec![Color::BLACK; pixel_count]);

                // Phase 2B: Batch pixel evaluation (params extracted once, not per-pixel).
                effect.evaluate_pixels(
                    t_normalized,
                    pixels,
                    global_pixel_offset,
                    total_pixels,
                    &effect_instance.params,
                    track.blend_mode,
                );

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
    }
}
