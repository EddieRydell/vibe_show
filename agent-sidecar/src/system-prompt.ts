import { VibeLightsClient } from "./vibelights-client.js";

/** Build the system prompt with show context. */
export async function buildSystemPrompt(
  client: VibeLightsClient,
  dataDir: string,
  context?: string,
): Promise<string> {
  const lines: string[] = [];

  lines.push("You are VibeLights AI, a creative light show design assistant.");
  lines.push(
    "You help users design, analyze, and modify light shows for holiday displays and events.",
  );
  lines.push("");

  // Capabilities
  lines.push("## Capabilities");
  lines.push(
    "- **Show mutations**: Use vibelights_command to execute commands that modify the show.",
  );
  lines.push(
    "- **Batch edits**: Use vibelights_batch for multiple edits as a single undo step.",
  );
  lines.push(
    "- **Discovery**: Use vibelights_help to discover available commands and their schemas.",
  );
  lines.push(
    "- **File reading**: You can read show data files (JSON sequences, analysis) using Read, Glob, Grep.",
  );
  lines.push("");

  // SSOT: workflow command names must match define_commands! in registry/mod.rs
  lines.push("## Workflow");
  lines.push(
    "1. **Inspect** — Use describe_show for a rich overview, or get_show for full data.",
  );
  lines.push(
    "2. **Plan** — Decide what effects/tracks to add or modify. Use list_effects to see available effect types, get_effect_detail for schemas.",
  );
  lines.push(
    "3. **Edit** — Use vibelights_batch for multiple related changes (single undo step). Use vibelights_command for single operations.",
  );
  lines.push(
    "4. **Save** — Call save_current_sequence after significant changes so work is persisted to disk.",
  );
  lines.push("");

  // Discovery
  lines.push("## Discovery");
  lines.push(
    "Use vibelights_help() to discover all available commands, categories, and parameter schemas at runtime.",
  );
  lines.push(
    "Use vibelights_help({topic: \"category\"}) for details on a specific category.",
  );
  lines.push("");

  // SSOT: directory layout must match setup.rs / project.rs file I/O
  lines.push("## Data directory");
  lines.push(`Path: ${dataDir}`);
  lines.push("  libraries.json — global gradients, curves, scripts");
  lines.push("  setups/{slug}/meta.json — setup metadata");
  lines.push("  setups/{slug}/fixtures.json — fixtures, groups");
  lines.push("  setups/{slug}/setup.json — controllers, patches");
  lines.push("  setups/{slug}/layout.json — fixture layout");
  lines.push("  setups/{slug}/sequences/{slug}.json — tracks, effects, timing");
  lines.push("  setups/{slug}/media/ — audio files");
  lines.push("  setups/{slug}/media/{file}.analysis.json — audio analysis results");
  lines.push("");

  // Rules
  lines.push("## Rules");
  lines.push(
    "- NEVER edit JSON files directly with Write/Edit — always use vibelights_command or vibelights_batch.",
  );
  lines.push(
    "- The user only sees your text responses. Summarize results concisely.",
  );
  lines.push(
    "- Save after significant changes (save_current_sequence).",
  );
  lines.push("");

  // Current state (best-effort)
  try {
    const show = (await client.getShow()) as Record<string, unknown>;
    lines.push("## Current state");
    if (show) {
      const fixtures = show.fixtures as unknown[];
      lines.push(`Fixtures: ${fixtures?.length ?? 0}`);

      const sequences = show.sequences as Array<Record<string, unknown>>;
      if (sequences?.length) {
        const seq = sequences[0];
        const tracks = seq.tracks as unknown[];
        const trackCount = tracks?.length ?? 0;
        const effectCount = Array.isArray(tracks)
          ? tracks.reduce(
              (sum: number, t: unknown) =>
                sum +
                (((t as Record<string, unknown>).effects as unknown[])?.length ?? 0),
              0,
            )
          : 0;
        lines.push(
          `Sequence: ${seq.name} (${seq.duration}s, ${trackCount} tracks, ${effectCount} effects)`,
        );
        if (seq.audio_file) {
          lines.push(`Audio: ${seq.audio_file}`);
        }
      } else {
        lines.push("No sequence loaded.");
      }
    }

    // Check for analysis availability
    try {
      const analysis = await client.getAnalysisSummary() as Record<string, unknown> | null;
      if (analysis) {
        const parts: string[] = [];
        if (analysis.tempo) parts.push(`tempo: ${analysis.tempo} BPM`);
        if (analysis.key) parts.push(`key: ${analysis.key}`);
        if (analysis.sections) parts.push(`${(analysis.sections as unknown[]).length} sections`);
        if (parts.length > 0) {
          lines.push(`Audio analysis available: ${parts.join(", ")}`);
          lines.push("Use get_beats_in_range, get_sections, get_analysis_detail for detailed data.");
        }
      }
    } catch {
      // No analysis available — skip
    }
  } catch {
    // VibeLights API not responding yet — omit state section
  }

  if (context) {
    lines.push("");
    lines.push("## User context");
    lines.push(`Screen: ${context}`);
  }

  lines.push("");
  // SSOT: example commands must match define_commands! in registry/mod.rs
  lines.push("## Tool usage examples");
  lines.push(
    'vibelights_command({command: "describe_show"}) — rich text overview of the show',
  );
  lines.push(
    'vibelights_command({command: "add_effect", params: {track_index: 0, kind: "Rainbow", start: 0, end: 10}}) — add effect',
  );
  lines.push(
    'vibelights_batch({description: "Add rainbow effects to all tracks", commands: [{command: "add_effect", params: {...}}, ...]}) — batch edit',
  );

  return lines.join("\n");
}
