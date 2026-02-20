import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";

/** HTTP client for the VibeLights API. */
export class VibeLightsClient {
  private baseUrl: string;

  constructor(port: number) {
    this.baseUrl = `http://127.0.0.1:${port}`;
  }

  /** Discover the VibeLights API port from the .vibelights-port file. */
  static discover(): VibeLightsClient | null {
    // Check common config dirs
    const candidates: string[] = [];

    if (process.platform === "win32") {
      const appData = process.env.APPDATA;
      if (appData) {
        candidates.push(path.join(appData, "com.vibelights.app", ".vibelights-port"));
      }
    } else if (process.platform === "darwin") {
      candidates.push(
        path.join(os.homedir(), "Library", "Application Support", "com.vibelights.app", ".vibelights-port"),
      );
    } else {
      const configHome = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config");
      candidates.push(path.join(configHome, "com.vibelights.app", ".vibelights-port"));
    }

    // Also check env var
    const envPort = process.env.VIBELIGHTS_PORT;
    if (envPort) {
      const port = parseInt(envPort, 10);
      if (port > 0) return new VibeLightsClient(port);
    }

    for (const candidate of candidates) {
      try {
        const content = fs.readFileSync(candidate, "utf-8").trim();
        const port = parseInt(content, 10);
        if (port > 0) return new VibeLightsClient(port);
      } catch {
        // File doesn't exist, try next
      }
    }

    return null;
  }

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const options: RequestInit = {
      method,
      headers: { "Content-Type": "application/json" },
    };
    if (body !== undefined) {
      options.body = JSON.stringify(body);
    }
    const res = await fetch(url, options);
    const json = await res.json() as { ok: boolean; data?: T; error?: string; description?: string };
    if (!json.ok) {
      throw new Error(json.error || "Unknown API error");
    }
    return json.data as T;
  }

  async getShow(): Promise<unknown> {
    return this.request("GET", "/api/show");
  }

  async getEffects(): Promise<unknown> {
    return this.request("GET", "/api/effects");
  }

  async getPlayback(): Promise<unknown> {
    return this.request("GET", "/api/playback");
  }

  async getEffectDetail(seq: number, track: number, idx: number): Promise<unknown> {
    return this.request("GET", `/api/effect/${seq}/${track}/${idx}`);
  }

  async getUndoState(): Promise<unknown> {
    return this.request("GET", "/api/undo-state");
  }

  async executeCommand(cmd: unknown): Promise<unknown> {
    return this.request("POST", "/api/command", cmd);
  }

  async undo(): Promise<unknown> {
    return this.request("POST", "/api/undo");
  }

  async redo(): Promise<unknown> {
    return this.request("POST", "/api/redo");
  }

  async play(): Promise<void> {
    await this.request("POST", "/api/play");
  }

  async pause(): Promise<void> {
    await this.request("POST", "/api/pause");
  }

  async seek(time: number): Promise<void> {
    await this.request("POST", "/api/seek", { time });
  }

  async save(): Promise<void> {
    await this.request("POST", "/api/save");
  }
}
