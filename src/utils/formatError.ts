/**
 * Extract a human-readable message from a Tauri IPC error.
 *
 * Tauri commands that return `Result<T, AppError>` send the error as a JSON
 * object matching the `AppError` serde representation:
 *   { code: "PythonError", detail: { message: "..." } }
 *   { code: "PythonNotReady" }
 *   { code: "NotFound", detail: { what: "..." } }
 *
 * `String(e)` on these objects produces "[object Object]", so we need to
 * dig into the structure.
 */
export function formatTauriError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === "object") {
    const obj = e as Record<string, unknown>;

    // Direct message field (some error shapes)
    if (typeof obj.message === "string") return obj.message;

    // AppError tagged enum: { code: "...", detail: { message: "..." } }
    if (obj.detail && typeof obj.detail === "object") {
      const detail = obj.detail as Record<string, unknown>;
      if (typeof detail.message === "string") return detail.message;
      if (typeof detail.model === "string")
        return `Required model not installed: ${detail.model}`;
      if (typeof detail.what === "string") return `${detail.what} not found`;
    }

    // Bare code with no detail (e.g., "PythonNotReady", "NoSetup")
    if (typeof obj.code === "string") {
      // Convert PascalCase to readable: "PythonNotReady" -> "Python not ready"
      return obj.code.replace(/([A-Z])/g, " $1").trim();
    }

    // Last resort: JSON
    try {
      return JSON.stringify(e);
    } catch {
      return "Unknown error";
    }
  }
  return "Unknown error";
}
