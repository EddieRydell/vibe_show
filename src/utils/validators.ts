import type { ChatHistoryEntry } from "../types";

function isChatHistoryEntry(x: unknown): x is ChatHistoryEntry {
  if (typeof x !== "object" || x === null) return false;
  const obj = x as Record<string, unknown>;
  if (typeof obj["role"] !== "string" || typeof obj["text"] !== "string") return false;
  return obj["role"] === "user" || obj["role"] === "assistant" || obj["role"] === "tool";
}

export function parseChatEntries(raw: unknown): ChatHistoryEntry[] {
  if (!Array.isArray(raw)) return [];
  return raw.filter(isChatHistoryEntry);
}
