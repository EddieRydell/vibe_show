#!/usr/bin/env node

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { VibeLightsClient } from "./vibelights-client.js";

const client = VibeLightsClient.discover();
if (!client) {
  console.error(
    "Could not find VibeLights API. Make sure VibeLights is running.\n" +
      "Set VIBELIGHTS_PORT env var or ensure the .vibelights-port file exists.",
  );
  process.exit(1);
}

const server = new McpServer({
  name: "vibelights",
  version: "0.1.0",
});

// ── Resources ─────────────────────────────────────────────────────

server.resource("show", "vibelights://show", async () => {
  const show = await client.getShow();
  return { contents: [{ uri: "vibelights://show", text: JSON.stringify(show, null, 2), mimeType: "application/json" }] };
});

server.resource("effects", "vibelights://effects", async () => {
  const effects = await client.getEffects();
  return { contents: [{ uri: "vibelights://effects", text: JSON.stringify(effects, null, 2), mimeType: "application/json" }] };
});

server.resource("playback", "vibelights://playback", async () => {
  const playback = await client.getPlayback();
  return { contents: [{ uri: "vibelights://playback", text: JSON.stringify(playback, null, 2), mimeType: "application/json" }] };
});

// ── Tools: Core editing ──────────────────────────────────────────

server.tool(
  "add_effect",
  "Add an effect to a track in the current sequence",
  { track_index: z.number(), kind: z.string(), start: z.number(), end: z.number() },
  async ({ track_index, kind, start, end }) => {
    const result = await client.executeCommand({
      AddEffect: { sequence_index: 0, track_index, kind, start, end },
    });
    return { content: [{ type: "text" as const, text: JSON.stringify(result) }] };
  },
);

server.tool(
  "delete_effects",
  "Delete effects by (track_index, effect_index) pairs",
  { targets: z.array(z.tuple([z.number(), z.number()])) },
  async ({ targets }) => {
    const result = await client.executeCommand({
      DeleteEffects: { sequence_index: 0, targets },
    });
    return { content: [{ type: "text" as const, text: JSON.stringify(result) }] };
  },
);

server.tool(
  "update_effect_param",
  "Set a parameter on an effect",
  {
    track_index: z.number(),
    effect_index: z.number(),
    key: z.string(),
    value: z.unknown(),
  },
  async ({ track_index, effect_index, key, value }) => {
    const result = await client.executeCommand({
      UpdateEffectParam: { sequence_index: 0, track_index, effect_index, key, value },
    });
    return { content: [{ type: "text" as const, text: JSON.stringify(result) }] };
  },
);

server.tool(
  "update_effect_time_range",
  "Change the start/end time of an effect",
  { track_index: z.number(), effect_index: z.number(), start: z.number(), end: z.number() },
  async ({ track_index, effect_index, start, end }) => {
    const result = await client.executeCommand({
      UpdateEffectTimeRange: { sequence_index: 0, track_index, effect_index, start, end },
    });
    return { content: [{ type: "text" as const, text: JSON.stringify(result) }] };
  },
);

server.tool(
  "add_track",
  "Create a new track targeting a fixture or group",
  {
    name: z.string(),
    fixture_id: z.number().optional(),
    group_id: z.number().optional(),
    blend_mode: z.string().default("Override"),
  },
  async ({ name, fixture_id, group_id, blend_mode }) => {
    let target: unknown;
    if (fixture_id !== undefined) {
      target = { Fixtures: [fixture_id] };
    } else if (group_id !== undefined) {
      target = { Group: group_id };
    } else {
      target = "All";
    }
    const result = await client.executeCommand({
      AddTrack: { sequence_index: 0, name, target, blend_mode },
    });
    return { content: [{ type: "text" as const, text: JSON.stringify(result) }] };
  },
);

server.tool(
  "update_sequence_settings",
  "Update sequence name, duration, or frame rate",
  {
    name: z.string().optional(),
    duration: z.number().optional(),
    frame_rate: z.number().optional(),
  },
  async ({ name, duration, frame_rate }) => {
    const result = await client.executeCommand({
      UpdateSequenceSettings: { sequence_index: 0, name, duration, frame_rate },
    });
    return { content: [{ type: "text" as const, text: JSON.stringify(result) }] };
  },
);

// ── Tools: Convenience ───────────────────────────────────────────

server.tool(
  "create_effect_on_fixture",
  "Find or create a track for a fixture and add an effect to it. Optionally set params.",
  {
    fixture_id: z.number(),
    kind: z.string(),
    start: z.number(),
    end: z.number(),
    params: z.record(z.string(), z.unknown()).optional(),
  },
  async ({ fixture_id, kind, start, end, params }) => {
    // Get current show to find existing track
    const show = (await client.getShow()) as {
      sequences: Array<{ tracks: Array<{ target: unknown; name: string }> }>;
      fixtures: Array<{ id: number; name: string }>;
    };
    const sequence = show.sequences[0];
    if (!sequence) throw new Error("No sequence loaded");

    let trackIndex = sequence.tracks.findIndex((t) => {
      const target = t.target as Record<string, unknown>;
      return (
        typeof target === "object" &&
        target !== null &&
        "Fixtures" in target &&
        Array.isArray(target.Fixtures) &&
        target.Fixtures.length === 1 &&
        target.Fixtures[0] === fixture_id
      );
    });

    if (trackIndex === -1) {
      const fixture = show.fixtures.find((f) => f.id === fixture_id);
      const trackName = fixture ? fixture.name : `Fixture ${fixture_id}`;
      const addResult = (await client.executeCommand({
        AddTrack: {
          sequence_index: 0,
          name: trackName,
          target: { Fixtures: [fixture_id] },
          blend_mode: "Override",
        },
      })) as { result: string };
      trackIndex = parseInt(addResult.result, 10);
    }

    const addEffectResult = (await client.executeCommand({
      AddEffect: { sequence_index: 0, track_index: trackIndex, kind, start, end },
    })) as { result: string };
    const effectIndex = parseInt(addEffectResult.result, 10);

    // Set params if provided
    if (params) {
      for (const [key, value] of Object.entries(params)) {
        await client.executeCommand({
          UpdateEffectParam: {
            sequence_index: 0,
            track_index: trackIndex,
            effect_index: effectIndex,
            key,
            value,
          },
        });
      }
    }

    return {
      content: [
        {
          type: "text" as const,
          text: `Added ${kind} effect on track ${trackIndex} (effect index ${effectIndex})${params ? " with params" : ""}`,
        },
      ],
    };
  },
);

server.tool(
  "set_all_params",
  "Batch-set multiple params on an effect at once",
  {
    track_index: z.number(),
    effect_index: z.number(),
    params: z.record(z.string(), z.unknown()),
  },
  async ({ track_index, effect_index, params }) => {
    const commands = Object.entries(params).map(([key, value]) => ({
      UpdateEffectParam: {
        sequence_index: 0,
        track_index,
        effect_index,
        key,
        value,
      },
    }));
    const result = await client.executeCommand({
      Batch: { description: `Set ${commands.length} params`, commands },
    });
    return { content: [{ type: "text" as const, text: JSON.stringify(result) }] };
  },
);

// ── Tools: Playback / utility ────────────────────────────────────

server.tool("play", "Start playback", {}, async () => {
  await client.play();
  return { content: [{ type: "text" as const, text: "Playing" }] };
});

server.tool("pause", "Pause playback", {}, async () => {
  await client.pause();
  return { content: [{ type: "text" as const, text: "Paused" }] };
});

server.tool("seek", "Seek to a specific time in seconds", { time: z.number() }, async ({ time }) => {
  await client.seek(time);
  return { content: [{ type: "text" as const, text: `Seeked to ${time}s` }] };
});

server.tool("undo", "Undo the last editing action", {}, async () => {
  const result = await client.undo();
  return { content: [{ type: "text" as const, text: `Undone: ${JSON.stringify(result)}` }] };
});

server.tool("redo", "Redo the last undone action", {}, async () => {
  const result = await client.redo();
  return { content: [{ type: "text" as const, text: `Redone: ${JSON.stringify(result)}` }] };
});

server.tool("save", "Save the current sequence to disk", {}, async () => {
  await client.save();
  return { content: [{ type: "text" as const, text: "Saved" }] };
});

server.tool("describe_show", "Get a human-readable summary of the current show state", {}, async () => {
  const show = (await client.getShow()) as {
    name: string;
    fixtures: Array<{ id: number; name: string; pixel_count: number }>;
    groups: Array<{ id: number; name: string }>;
    sequences: Array<{
      name: string;
      duration: number;
      frame_rate: number;
      audio_file: string | null;
      tracks: Array<{
        name: string;
        target: unknown;
        blend_mode: string;
        effects: Array<{ kind: string; time_range: { start: number; end: number } }>;
      }>;
    }>;
  };
  const playback = (await client.getPlayback()) as {
    playing: boolean;
    current_time: number;
    duration: number;
  };

  const lines: string[] = [];
  lines.push(`Show: ${show.name || "(untitled)"}`);
  lines.push(`Fixtures: ${show.fixtures.length}`);
  for (const f of show.fixtures) {
    lines.push(`  - ${f.name} (id: ${f.id}, ${f.pixel_count} pixels)`);
  }
  if (show.groups.length > 0) {
    lines.push(`Groups: ${show.groups.length}`);
    for (const g of show.groups) {
      lines.push(`  - ${g.name} (id: ${g.id})`);
    }
  }
  const seq = show.sequences[0];
  if (seq) {
    lines.push(`\nSequence: ${seq.name}`);
    lines.push(`  Duration: ${seq.duration}s @ ${seq.frame_rate}fps`);
    if (seq.audio_file) lines.push(`  Audio: ${seq.audio_file}`);
    lines.push(`  Tracks: ${seq.tracks.length}`);
    for (let i = 0; i < seq.tracks.length; i++) {
      const t = seq.tracks[i];
      lines.push(`  Track ${i}: "${t.name}" (${t.blend_mode}, ${t.effects.length} effects)`);
      for (let j = 0; j < t.effects.length; j++) {
        const e = t.effects[j];
        lines.push(`    Effect ${j}: ${e.kind} [${e.time_range.start.toFixed(1)}s - ${e.time_range.end.toFixed(1)}s]`);
      }
    }
  }
  lines.push(`\nPlayback: ${playback.playing ? "playing" : "paused"} at ${playback.current_time.toFixed(1)}s`);

  return { content: [{ type: "text" as const, text: lines.join("\n") }] };
});

// ── Start ────────────────────────────────────────────────────────

const transport = new StdioServerTransport();
await server.connect(transport);
