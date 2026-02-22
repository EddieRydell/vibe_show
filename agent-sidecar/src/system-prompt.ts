import { VibeLightsClient } from "./vibelights-client.js";

/** Build the system prompt with show context. */
export async function buildSystemPrompt(
  client: VibeLightsClient,
  dataDir: string,
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
    "- **File reading**: You can read show data files (JSON sequences, analysis results) using Read, Glob, and Grep tools.",
  );
  lines.push(
    "- **Show mutations**: Use the vibelights_command MCP tool to execute commands that modify the show.",
  );
  lines.push(
    "- **Batch edits**: Use vibelights_batch for multiple edits as a single undo step.",
  );
  lines.push(
    "- **Discovery**: Use vibelights_help to discover available commands and their schemas.",
  );
  lines.push("");

  // Param format
  lines.push("## Param value format");
  lines.push(
    'Effect params use tagged format: {"Float": 1.0}, {"Color": {"r":255,"g":0,"b":0,"a":255}}, {"Bool": true}, {"Int": 5}',
  );
  lines.push(
    '- Curve: {"Curve":{"points":[{"x":0,"y":0},{"x":1,"y":1}]}}',
  );
  lines.push(
    '- Gradient: {"ColorGradient":{"stops":[{"position":0,"color":{"r":255,"g":0,"b":0,"a":255}},{"position":1,"color":{"r":0,"g":0,"b":255,"a":255}}]}}',
  );
  lines.push('- Library refs: {"GradientRef":"name"} or {"CurveRef":"name"}');
  lines.push("");

  // Data directory layout
  lines.push("## Data directory");
  lines.push(`Data directory: ${dataDir}`);
  lines.push("Layout:");
  lines.push("  profiles/{slug}/profile.json — fixture definitions, groups, layout");
  lines.push("  profiles/{slug}/sequences/{slug}.json — sequence data (tracks, effects, timing)");
  lines.push("  profiles/{slug}/media/ — audio files");
  lines.push("  profiles/{slug}/media/{file}.analysis.json — audio analysis results");
  lines.push("");

  // Rules
  lines.push("## Rules");
  lines.push(
    "- NEVER edit JSON files directly with Write/Edit tools — always use vibelights_command or vibelights_batch for mutations.",
  );
  lines.push(
    "- Use Read/Glob/Grep for analysis: reading sequence files, searching for patterns, aggregating data.",
  );
  lines.push("- The user only sees your text responses. Summarize results concisely.");
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
                ((t as Record<string, unknown>).effects as unknown[])?.length ??
                0,
              0,
            )
          : 0;
        lines.push(
          `Sequence: ${seq.name} (${seq.duration}s, ${trackCount} tracks, ${effectCount} effects)`,
        );
        if (seq.audio_file) {
          lines.push(`Audio: ${seq.audio_file}`);
        }
      }
    }
  } catch {
    // VibeLights API not responding yet — omit state section
  }

  lines.push("");
  lines.push("## Tool usage examples");
  lines.push(
    'vibelights_command({command: "get_show"}) — commands without parameters',
  );
  lines.push(
    'vibelights_command({command: "open_sequence", params: {slug: "my-sequence"}}) — commands with parameters',
  );
  lines.push(
    'vibelights_batch({description: "...", commands: [{command: "add_effect", params: {...}}, ...]}) — multiple edits as one undo step',
  );

  return lines.join("\n");
}
