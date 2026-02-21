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

    // Resource libraries
    if !seq.scripts.is_empty() {
        lines.push(format!(
            "\nScripts ({}): {}",
            seq.scripts.len(),
            seq.scripts.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if !seq.gradient_library.is_empty() {
        let items: Vec<String> = seq
            .gradient_library
            .iter()
            .map(|(name, g)| format!("{name} ({} stops)", g.stops().len()))
            .collect();
        lines.push(format!(
            "\nGradient Library ({}): {}",
            seq.gradient_library.len(),
            items.join(", ")
        ));
    }
    if !seq.curve_library.is_empty() {
        let items: Vec<String> = seq
            .curve_library
            .iter()
            .map(|(name, c)| format!("{name} ({} pts)", c.points().len()))
            .collect();
        lines.push(format!(
            "\nCurve Library ({}): {}",
            seq.curve_library.len(),
            items.join(", ")
        ));
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
        if let Some(val) = effect.params.get(s.key.clone()) {
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
