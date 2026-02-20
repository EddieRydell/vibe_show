use crate::effects::resolve_effect;
use crate::engine::Frame;
use crate::model::{EffectInstance, Sequence, Show};

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
        lines.push(format!("\nLayout: {} fixtures positioned", layout_count));
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
        lines.push(format!("Audio: {}", audio));
    }

    lines.push(format!("\nTracks ({})", seq.tracks.len()));
    for (i, track) in seq.tracks.iter().enumerate() {
        lines.push(format!(
            "  Track {}: \"{}\" (target: {:?}, blend: {:?}, {} effects)",
            i,
            track.name,
            track.target,
            track.blend_mode,
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

/// Decode base64-encoded RGBA pixel data into raw bytes.
fn base64_decode(s: &str) -> Vec<u8> {
    fn val(c: u8) -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => 0,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            break;
        }
        let a = val(chunk[0]) as u32;
        let b = val(chunk[1]) as u32;
        let c = if chunk.len() > 2 && chunk[2] != b'=' { val(chunk[2]) as u32 } else { 0 };
        let d = if chunk.len() > 3 && chunk[3] != b'=' { val(chunk[3]) as u32 } else { 0 };
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        out.push((triple >> 16) as u8);
        if chunk.len() > 2 && chunk[2] != b'=' {
            out.push((triple >> 8 & 0xFF) as u8);
        }
        if chunk.len() > 3 && chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    out
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

            let (r_sum, g_sum, b_sum) =
                bytes.chunks_exact(4).fold((0u64, 0u64, 0u64), |acc, px| {
                    (acc.0 + px[0] as u64, acc.1 + px[1] as u64, acc.2 + px[2] as u64)
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
    let resolved = resolve_effect(&effect.kind);
    let schema = resolved.param_schema();

    let mut param_strs = Vec::new();
    for s in &schema {
        if let Some(val) = effect.params.get(&s.key) {
            param_strs.push(format!("{}={:?}", s.key, val));
        }
    }

    let params_display = if param_strs.is_empty() {
        "defaults".to_string()
    } else {
        param_strs.join(", ")
    };

    format!(
        "{:?} [{:.1}s - {:.1}s] ({})",
        effect.kind,
        effect.time_range.start(),
        effect.time_range.end(),
        params_display
    )
}
