/**
 * Single source of truth for event names shared between Rust and TypeScript.
 * The Rust mirror is `src-tauri/src/events.rs` — a sync test verifies they match.
 */

export const CHAT_TOKEN = "chat:token" as const;
export const CHAT_TOOL_CALL = "chat:tool_call" as const;
export const CHAT_TOOL_RESULT = "chat:tool_result" as const;
export const CHAT_COMPLETE = "chat:complete" as const;
export const CHAT_THINKING = "chat:thinking" as const;
export const CHAT_ERROR = "chat:error" as const;
export const PROGRESS = "progress" as const;
export const SHOW_REFRESHED = "show-refreshed" as const;
export const SELECTION_CHANGED = "selection-changed" as const;
