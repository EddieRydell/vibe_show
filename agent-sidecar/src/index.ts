import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { runAgentQuery, cancelQuery, clearSession, getSessionId, type AgentConfig } from "./agent.js";

// ── Config from env vars ─────────────────────────────────────────

const AGENT_PORT = parseInt(process.env.AGENT_PORT || "0", 10);
const VIBELIGHTS_PORT = parseInt(process.env.VIBELIGHTS_PORT || "0", 10);
const DATA_DIR = process.env.VIBELIGHTS_DATA_DIR || "";
const MODEL = process.env.VIBELIGHTS_MODEL || "";

if (!VIBELIGHTS_PORT) {
  console.error("VIBELIGHTS_PORT env var is required");
  process.exit(1);
}

const config: AgentConfig = {
  vibelightsPort: VIBELIGHTS_PORT,
  dataDir: DATA_DIR,
  model: MODEL || undefined,
};

// ── HTTP helpers ─────────────────────────────────────────────────

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf-8")));
    req.on("error", reject);
  });
}

function jsonResponse(res: ServerResponse, status: number, data: unknown) {
  res.writeHead(status, { "Content-Type": "application/json" });
  res.end(JSON.stringify(data));
}

// ── HTTP server ──────────────────────────────────────────────────

const server = createServer(async (req, res) => {
  const url = req.url ?? "/";
  const method = req.method ?? "GET";

  // CORS headers for local dev
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type");

  if (method === "OPTIONS") {
    res.writeHead(204);
    res.end();
    return;
  }

  try {
    if (method === "GET" && url === "/health") {
      jsonResponse(res, 200, { ok: true });
      return;
    }

    if (method === "POST" && url === "/chat") {
      const body = await readBody(req);
      let parsed: { message?: string; sessionId?: string; context?: string };
      try {
        parsed = JSON.parse(body);
      } catch {
        jsonResponse(res, 400, { error: "Invalid JSON" });
        return;
      }

      const message = parsed.message;
      if (!message || typeof message !== "string") {
        jsonResponse(res, 400, { error: "Missing 'message' field" });
        return;
      }

      // Run the agent query — streams SSE events
      await runAgentQuery(config, message, res, parsed.sessionId, parsed.context);
      return;
    }

    if (method === "POST" && url === "/cancel") {
      cancelQuery();
      jsonResponse(res, 200, { ok: true });
      return;
    }

    if (method === "POST" && url === "/clear") {
      clearSession();
      jsonResponse(res, 200, { ok: true });
      return;
    }

    if (method === "GET" && url === "/session") {
      jsonResponse(res, 200, { sessionId: getSessionId() ?? null });
      return;
    }

    if (method === "POST" && url === "/shutdown") {
      jsonResponse(res, 200, { ok: true });
      // Graceful shutdown after response is sent
      setTimeout(() => process.exit(0), 100);
      return;
    }

    jsonResponse(res, 404, { error: "Not found" });
  } catch (err) {
    console.error("[agent-sidecar] Error:", err);
    if (!res.headersSent) {
      jsonResponse(res, 500, {
        error: err instanceof Error ? err.message : "Internal error",
      });
    }
  }
});

// ── Start server ─────────────────────────────────────────────────

server.listen(AGENT_PORT, "127.0.0.1", () => {
  const addr = server.address();
  const port = typeof addr === "object" && addr ? addr.port : AGENT_PORT;
  // Print port to stdout (first line) — Rust reads this to find the port
  console.log(port);
  console.error(`[agent-sidecar] Listening on http://127.0.0.1:${port}`);
});
