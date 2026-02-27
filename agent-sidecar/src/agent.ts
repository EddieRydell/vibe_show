import { query, type McpSdkServerConfigWithInstance } from "@anthropic-ai/claude-agent-sdk";
import type { ServerResponse } from "node:http";
import { existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { VibeLightsClient } from "./vibelights-client.js";
import { createVibeLightsMcpServer } from "./mcp-tools.js";
import { buildSystemPrompt } from "./system-prompt.js";

/**
 * Resolve paths for compiled binary mode.
 * Returns pathToClaudeCodeExecutable and executable override if needed.
 */
function resolveCompileOverrides(): {
  pathToClaudeCodeExecutable?: string;
  executable?: string;
  envOverrides?: Record<string, string>;
} {
  // Check for cli.js alongside the executable (compiled binary)
  const exeDir = dirname(process.execPath);
  const adjacent = join(exeDir, "cli.js");
  if (existsSync(adjacent)) {
    return {
      pathToClaudeCodeExecutable: adjacent,
      // Use the compiled binary itself as the Bun runtime for spawning cli.js.
      // BUN_BE_BUN=1 tells the compiled binary to act as a normal bun runtime.
      executable: process.execPath,
      envOverrides: { BUN_BE_BUN: "1" },
    };
  }
  // Dev mode — let the SDK resolve everything normally
  return {};
}

export interface AgentConfig {
  vibelightsPort: number;
  dataDir: string;
  model?: string;
}

interface ActiveQuery {
  abort: AbortController;
}

// Active query for cancellation
let activeQuery: ActiveQuery | null = null;

// Session ID for resume
let currentSessionId: string | undefined;

/** Cancel the in-flight query. */
export function cancelQuery(): void {
  if (activeQuery) {
    activeQuery.abort.abort();
    activeQuery = null;
  }
}

/** Clear the session (reset conversation context). */
export function clearSession(): void {
  currentSessionId = undefined;
  cancelQuery();
}

/** Get current session ID. */
export function getSessionId(): string | undefined {
  return currentSessionId;
}

/**
 * Run an agent query and stream results as SSE events to the response.
 *
 * SSE event types:
 * - `token`: text chunk from the assistant
 * - `tool_call`: tool name being called
 * - `tool_result`: JSON { tool, result }
 * - `thinking`: boolean - whether the agent is thinking
 * - `complete`: boolean - query finished
 * - `error`: error message
 */
export async function runAgentQuery(
  config: AgentConfig,
  message: string,
  res: ServerResponse,
  sessionIdOverride?: string,
  context?: string,
): Promise<void> {
  const client = new VibeLightsClient(config.vibelightsPort);
  const mcpServer = createVibeLightsMcpServer(client);

  const systemPrompt = await buildSystemPrompt(client, config.dataDir, context);
  const overrides = resolveCompileOverrides();

  const abort = new AbortController();
  activeQuery = { abort };

  // Set up SSE headers
  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache",
    Connection: "keep-alive",
  });

  const sendEvent = (event: string, data: unknown) => {
    res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
  };

  try {
    const stream = query({
      prompt: message,
      options: {
        systemPrompt,
        allowedTools: ["Read", "Glob", "Grep"],
        permissionMode: "bypassPermissions",
        allowDangerouslySkipPermissions: true,
        cwd: config.dataDir,
        maxTurns: 30,
        model: config.model || undefined,
        abortController: abort,
        resume: sessionIdOverride ?? currentSessionId,
        includePartialMessages: true,
        ...(overrides.pathToClaudeCodeExecutable
          ? { pathToClaudeCodeExecutable: overrides.pathToClaudeCodeExecutable }
          : {}),
        ...(overrides.executable
          ? { executable: overrides.executable as "bun" | "node" }
          : {}),
        ...(overrides.envOverrides
          ? { env: { ...process.env, ...overrides.envOverrides } }
          : {}),
        mcpServers: {
          vibelights: mcpServer as McpSdkServerConfigWithInstance,
        },
      },
    });

    let streamed = false;
    // Track tool_use id -> name so we can label tool_result events correctly.
    // Also serves as dedup: only emit tool_call once per ID.
    const toolUseNames = new Map<string, string>();
    // Track which tool results we've already emitted (dedup for includePartialMessages)
    const emittedResults = new Set<string>();

    for await (const msg of stream) {
      if (abort.signal.aborted) break;

      switch (msg.type) {
        case "system": {
          if (msg.subtype === "init") {
            currentSessionId = msg.session_id;
            sendEvent("session_id", currentSessionId);
          }
          break;
        }

        case "assistant": {
          // Full assistant message — emit tool_call for any new tool_use blocks.
          // This is the single source of truth for tool calls (reliable id + name).
          const apiMsg = msg.message;
          if (apiMsg.content && Array.isArray(apiMsg.content)) {
            for (const block of apiMsg.content) {
              if (block.type === "tool_use") {
                if (typeof block.id !== "string" || typeof block.name !== "string") continue;
                const toolId = block.id;
                const toolName = block.name;
                if (toolId && !toolUseNames.has(toolId)) {
                  toolUseNames.set(toolId, toolName);
                  sendEvent("tool_call", { id: toolId, tool: toolName });
                }
              }
            }
          }
          break;
        }

        case "stream_event": {
          // Only handle text streaming — tool events come from "assistant" messages.
          const event = msg.event;
          if (event.type === "content_block_delta") {
            if ("delta" in event && event.delta && typeof event.delta === "object") {
              const delta = event.delta as Record<string, unknown>;
              if (delta.type === "text_delta" && typeof delta.text === "string") {
                sendEvent("token", delta.text);
                streamed = true;
              }
            }
          }
          break;
        }

        case "user": {
          const apiMsg = msg.message;
          if (apiMsg.content && Array.isArray(apiMsg.content)) {
            for (const block of apiMsg.content) {
              if (block.type === "tool_result") {
                if (typeof block.tool_use_id !== "string") continue;
                const toolId = block.tool_use_id;
                if (!toolId || emittedResults.has(toolId)) continue;
                emittedResults.add(toolId);
                const toolName = toolUseNames.get(toolId);
                if (!toolName) {
                  console.warn(`[VibeLights] tool_result for unknown tool_use_id: ${toolId}`);
                }
                const resultText =
                  typeof block.content === "string"
                    ? block.content
                    : Array.isArray(block.content)
                      ? (block.content as unknown[])
                          .filter((c: unknown): c is { type: string; text: string } =>
                            c != null && typeof c === "object" && "type" in c && (c as Record<string, unknown>).type === "text" && "text" in c && typeof (c as Record<string, unknown>).text === "string"
                          )
                          .map((c) => c.text)
                          .join("")
                      : JSON.stringify(block.content);
                sendEvent("tool_result", {
                  id: toolId,
                  tool: toolName ?? "unknown",
                  result: resultText.slice(0, 2000),
                });
              }
            }
          }
          break;
        }

        case "result": {
          if (msg.subtype === "success") {
            if (!streamed && msg.result) {
              sendEvent("token", msg.result);
            }
          } else {
            if ("errors" in msg && Array.isArray(msg.errors)) {
              const errors = msg.errors as string[];
              if (errors.length) sendEvent("error", errors.join("; "));
            }
          }
          break;
        }
      }
    }
  } catch (err) {
    if (!abort.signal.aborted) {
      sendEvent("error", err instanceof Error ? err.message : String(err));
    }
  } finally {
    activeQuery = null;
    sendEvent("complete", true);
    res.end();
  }
}
