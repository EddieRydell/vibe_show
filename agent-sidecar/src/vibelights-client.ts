/** Simplified HTTP client for the VibeLights API. Accepts port directly. */
export class VibeLightsClient {
  private baseUrl: string;

  constructor(port: number) {
    this.baseUrl = `http://127.0.0.1:${port}`;
  }

  private async request<T>(
    method: string,
    urlPath: string,
    body?: unknown,
  ): Promise<T> {
    const url = `${this.baseUrl}${urlPath}`;
    const options: RequestInit = {
      method,
      headers: { "Content-Type": "application/json" },
    };
    if (body !== undefined) {
      options.body = JSON.stringify(body);
    }
    const res = await fetch(url, options);
    const json = (await res.json()) as {
      ok: boolean;
      data?: T;
      error?: string;
      description?: string;
    };
    if (!json.ok) {
      throw new Error(json.error || "Unknown API error");
    }
    return json.data as T;
  }

  /** Execute a tool by name via the generic dispatch endpoint. */
  async executeTool(
    name: string,
    params: Record<string, unknown>,
  ): Promise<unknown> {
    return this.request(
      "POST",
      `/api/tools/${encodeURIComponent(name)}`,
      params,
    );
  }

  /** Fetch all tool schemas from the registry. */
  async getToolSchemas(): Promise<ToolSchema[]> {
    return this.request("GET", "/api/tools");
  }

  /** Batch edit via the batch endpoint. */
  async batchEdit(commands: unknown): Promise<unknown> {
    return this.request("POST", "/api/batch", commands);
  }

  /** Get show state. */
  async getShow(): Promise<unknown> {
    return this.request("GET", "/api/show");
  }

  /** Get playback state. */
  async getPlayback(): Promise<unknown> {
    return this.request("GET", "/api/playback");
  }

  /** Get analysis summary. */
  async getAnalysisSummary(): Promise<unknown> {
    return this.request("GET", "/api/analysis/summary");
  }
}

export interface ToolSchema {
  name: string;
  description: string;
  category: string;
  inputSchema: Record<string, unknown>;
}
