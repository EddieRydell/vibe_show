use std::collections::HashMap;

use crate::effects::resolve_effect;
use crate::engine::Frame;
use crate::model::{EffectInstance, Sequence, Show};
use crate::util::base64_decode;

/// Human-readable summary of the entire show: fixtures, groups, controllers, patches, layout.
pub fn describe_show(show: &Show) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "Show: {}",
        if show.name.is_empty() {
            "(untitled)"
        } else {
            &show.name
        }
    ));

    // Fixtures
    lines.push(format!("\nFixtures ({})", show.fixtures.len()));
    for f in &show.fixtures {
        lines.push(format!(
            "  - {} (id: {}, {} pixels, {:?})",
            f.name, f.id.0, f.pixel_count, f.color_model
        ));
    }

    // Groups
    if !show.groups.is_empty() {
        lines.push(format!("\nGroups ({})", show.groups.len()));
        for g in &show.groups {
            lines.push(format!("  - {} (id: {}, {} members)", g.name, g.id.0, g.members.len()));
        }
    }

    // Controllers
    if !show.controllers.is_empty() {
        lines.push(format!("\nControllers ({})", show.controllers.len()));
        for c in &show.controllers {
            lines.push(format!(
                "  - {} (id: {}, {:?})",
                c.name, c.id.0, c.protocol
            ));
        }
    }

    // Patches
    if !show.patches.is_empty() {
        lines.push(format!("\nPatches ({})", show.patches.len()));
        for p in &show.patches {
            lines.push(format!(
                "  - fixture {} -> {:?}",
                p.fixture_id.0,
                p.output
            ));
        }
    }

    // Layout
    let layout_count = show.layout.fixtures.len();
    if layout_count > 0 {
        lines.push(format!("\nLayout: {layout_count} fixtures positioned"));
    }

    // Sequences
    lines.push(format!("\nSequences ({})", show.sequences.len()));
    for (i, seq) in show.sequences.iter().enumerate() {
        lines.push(format!(
            "  [{}] {} ({:.1}s @ {}fps, {} tracks)",
            i, seq.name, seq.duration, seq.frame_rate, seq.tracks.len()
        ));
    }

    lines.join("\n")
}

/// Human-readable summary of a sequence: tracks, effects with params, timing.
pub fn describe_sequence(seq: &Sequence) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "Sequence: {} ({:.1}s @ {}fps)",
        seq.name, seq.duration, seq.frame_rate
    ));
    if let Some(ref audio) = seq.audio_file {
        lines.push(format!("Audio: {audio}"));
    }

    lines.push(format!("\nTracks ({})", seq.tracks.len()));
    for (i, track) in seq.tracks.iter().enumerate() {
        lines.push(format!(
            "  Track {}: \"{}\" (target: {:?}, {} effects)",
            i,
            track.name,
            track.target,
            track.effects.len()
        ));
        for (j, effect) in track.effects.iter().enumerate() {
            lines.push(format!(
                "    [{}] {}",
                j,
                describe_effect(effect)
            ));
        }
    }

    lines.join("\n")
}

/// Compact summary for LLM consumption. Gives the LLM enough context to
/// understand the show without dumping every track and effect.
pub fn summarize_show(show: &Show) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "Show: {}",
        if show.name.is_empty() { "(untitled)" } else { &show.name }
    ));
    lines.push(format!("Fixtures: {}", show.fixtures.len()));
    if !show.groups.is_empty() {
        lines.push(format!("Groups: {}", show.groups.len()));
    }

    for (i, seq) in show.sequences.iter().enumerate() {
        lines.push(format!(
            "\nSequence [{}]: \"{}\" ({:.1}s @ {}fps)",
            i, seq.name, seq.duration, seq.frame_rate
        ));
        if let Some(ref audio) = seq.audio_file {
            lines.push(format!("  Audio: {audio}"));
        }

        let total_effects: usize = seq.tracks.iter().map(|t| t.effects.len()).sum();
        lines.push(format!(
            "  Tracks: {}, Effects: {}",
            seq.tracks.len(),
            total_effects
        ));

        // Effect type distribution
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        for track in &seq.tracks {
            for effect in &track.effects {
                let key = format!("{}", effect.kind);
                *type_counts.entry(key).or_insert(0) += 1;
            }
        }
        if !type_counts.is_empty() {
            let mut sorted: Vec<_> = type_counts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            let parts: Vec<String> = sorted.iter().map(|(k, v)| format!("{k}: {v}")).collect();
            lines.push(format!("  Effect types: {}", parts.join(", ")));
        }

        // Sample of first few tracks (so LLM can see structure)
        let sample_count = seq.tracks.len().min(5);
        if sample_count > 0 {
            lines.push(format!("\n  First {sample_count} tracks:"));
            for (i, track) in seq.tracks.iter().take(sample_count).enumerate() {
                let effects_desc: Vec<String> = track.effects.iter().map(|e| {
                    format!("{} [{:.1}s-{:.1}s]", e.kind, e.time_range.start(), e.time_range.end())
                }).collect();
                lines.push(format!("    Track {}: \"{}\" (target: {:?}) — {}",
                    i, track.name, track.target,
                    if effects_desc.is_empty() { "no effects".to_string() } else { effects_desc.join(", ") }
                ));
            }
            if seq.tracks.len() > sample_count {
                lines.push(format!("    ... and {} more tracks", seq.tracks.len() - sample_count));
            }
            lines.push("  Use get_effect_detail({sequence_index, track_index, effect_index}) to inspect any effect.".to_string());
        }
    }

    lines.join("\n")
}

/// Human-readable summary of a frame: per-fixture color averages.
pub fn describe_frame(show: &Show, frame: &Frame) -> String {
    let mut lines = Vec::new();

    lines.push("Frame state:".to_string());

    for fixture in &show.fixtures {
        if let Some(b64) = frame.fixtures.get(&fixture.id.0) {
            let bytes = base64_decode(b64);
            let pixel_count = bytes.len() / 4;
            if pixel_count == 0 {
                lines.push(format!("  {} (id {}): no pixels", fixture.name, fixture.id.0));
                continue;
            }

            // chunks_exact(4) guarantees each `px` has exactly 4 elements
            #[allow(clippy::indexing_slicing)]
            let (r_sum, g_sum, b_sum) =
                bytes.chunks_exact(4).fold((0u64, 0u64, 0u64), |acc, px| {
                    (acc.0 + u64::from(px[0]), acc.1 + u64::from(px[1]), acc.2 + u64::from(px[2]))
                });
            let n = pixel_count as u64;
            lines.push(format!(
                "  {} (id {}): {} pixels, avg RGB({}, {}, {})",
                fixture.name,
                fixture.id.0,
                pixel_count,
                r_sum / n,
                g_sum / n,
                b_sum / n
            ));
        } else {
            lines.push(format!("  {} (id {}): black", fixture.name, fixture.id.0));
        }
    }

    lines.join("\n")
}

/// Human-readable summary of a single effect instance.
pub fn describe_effect(effect: &EffectInstance) -> String {
    let schema = resolve_effect(&effect.kind)
        .map_or_else(Vec::new, |e| e.param_schema());

    let mut param_strs = Vec::new();
    for s in &schema {
        if let Some(val) = effect.params.get(&s.key) {
            param_strs.push(format!("{}={}", s.key, val));
        }
    }

    let params_display = if param_strs.is_empty() {
        "defaults".to_string()
    } else {
        param_strs.join(", ")
    };

    format!(
        "{} [{:.1}s - {:.1}s] ({})",
        effect.kind,
        effect.time_range.start(),
        effect.time_range.end(),
        params_display
    )
}
