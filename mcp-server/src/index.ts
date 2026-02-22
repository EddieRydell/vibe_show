#!/usr/bin/env node

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { VibeLightsClient, type ToolSchema } from "./vibelights-client.js";

const maybeClient = VibeLightsClient.discover();
if (!maybeClient) {
  console.error(
    "Could not find VibeLights API. Make sure VibeLights is running.\n" +
      "Set VIBELIGHTS_PORT env var or ensure the .vibelights-port file exists.",
  );
  process.exit(1);
}
const client: VibeLightsClient = maybeClient;

const server = new McpServer({
  name: "vibelights",
  version: "0.2.0",
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

server.resource("analysis", "vibelights://analysis", async () => {
  try {
    const summary = await client.getAnalysisSummary();
    return { contents: [{ uri: "vibelights://analysis", text: JSON.stringify(summary, null, 2), mimeType: "application/json" }] };
  } catch {
    return { contents: [{ uri: "vibelights://analysis", text: "No analysis available", mimeType: "text/plain" }] };
  }
});

server.resource("scripts", "vibelights://scripts", async () => {
  const scripts = await client.getScripts();
  return { contents: [{ uri: "vibelights://scripts", text: JSON.stringify(scripts, null, 2), mimeType: "application/json" }] };
});

server.resource("library", "vibelights://library", async () => {
  const [gradients, curves] = await Promise.all([
    client.getLibraryGradients(),
    client.getLibraryCurves(),
  ]);
  const library = { gradients, curves };
  return { contents: [{ uri: "vibelights://library", text: JSON.stringify(library, null, 2), mimeType: "application/json" }] };
});

// ── Dynamic tool registration from registry ──────────────────────

/**
 * Convert a JSON Schema property definition into a Zod schema.
 * Handles the subset of JSON Schema used by the tool registry.
 */
function jsonSchemaToZod(schema: Record<string, unknown>): z.ZodTypeAny {
  const type = schema.type as string | undefined;
  switch (type) {
    case "string": {
      const enumVals = schema.enum as string[] | undefined;
      if (enumVals && enumVals.length > 0) {
        return z.enum(enumVals as [string, ...string[]]);
      }
      return z.string();
    }
    case "number":
      return z.number();
    case "integer":
      return z.number().int();
    case "boolean":
      return z.boolean();
    case "array":
      return z.array(z.unknown());
    case "object":
      return z.record(z.string(), z.unknown());
    default:
      return z.unknown();
  }
}

/**
 * Build a Zod object schema from a tool's inputSchema.
 */
function buildZodSchema(inputSchema: Record<string, unknown>): Record<string, z.ZodTypeAny> {
  const properties = (inputSchema.properties ?? {}) as Record<string, Record<string, unknown>>;
  const required = (inputSchema.required ?? []) as string[];

  const shape: Record<string, z.ZodTypeAny> = {};
  for (const [key, propSchema] of Object.entries(properties)) {
    let zodType = jsonSchemaToZod(propSchema);
    if (!required.includes(key)) {
      zodType = zodType.optional();
    }
    shape[key] = zodType;
  }
  return shape;
}

/**
 * Register all tools from the VibeLights tool registry.
 * Falls back to hardcoded basic tools if the registry is unavailable.
 */
async function registerTools(): Promise<void> {
  let schemas: ToolSchema[];
  try {
    schemas = await client.getToolSchemas();
  } catch (err) {
    console.error("[vibelights-mcp] Failed to fetch tool schemas, using fallback:", err);
    registerFallbackTools();
    return;
  }

  for (const tool of schemas) {
    const zodShape = buildZodSchema(tool.inputSchema);

    server.tool(tool.name, tool.description, zodShape, async (params) => {
      try {
        const result = await client.executeTool(tool.name, params as Record<string, unknown>);
        return {
          content: [{ type: "text" as const, text: typeof result === "string" ? result : JSON.stringify(result, null, 2) }],
        };
      } catch (err) {
        return {
          content: [{ type: "text" as const, text: `Error: ${err instanceof Error ? err.message : String(err)}` }],
          isError: true,
        };
      }
    });
  }

  console.error(`[vibelights-mcp] Registered ${schemas.length} tools from registry`);
}

/**
 * Fallback: register a minimal set of tools if the registry endpoint is unavailable
 * (e.g., running against an older VibeLights version).
 */
function registerFallbackTools(): void {
  server.tool("describe_show", "Get a human-readable summary of the show state", {}, async () => {
    const show = await client.getShow();
    const playback = await client.getPlayback();
    return { content: [{ type: "text" as const, text: JSON.stringify({ show, playback }, null, 2) }] };
  });

  server.tool("play", "Start playback", {}, async () => {
    await client.play();
    return { content: [{ type: "text" as const, text: "Playing" }] };
  });

  server.tool("pause", "Pause playback", {}, async () => {
    await client.pause();
    return { content: [{ type: "text" as const, text: "Paused" }] };
  });

  server.tool("seek", "Seek to time", { time: z.number() }, async ({ time }) => {
    await client.seek(time);
    return { content: [{ type: "text" as const, text: `Seeked to ${time}s` }] };
  });

  server.tool("save", "Save current sequence", {}, async () => {
    await client.save();
    return { content: [{ type: "text" as const, text: "Saved" }] };
  });

  console.error("[vibelights-mcp] Registered 5 fallback tools");
}

// ── Start ────────────────────────────────────────────────────────

await registerTools();

const transport = new StdioServerTransport();
await server.connect(transport);
