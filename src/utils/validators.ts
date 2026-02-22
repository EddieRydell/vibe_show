interface ChatEntry {
  role: string;
  text: string;
}

function isChatEntry(x: unknown): x is ChatEntry {
  return (
    typeof x === "object" &&
    x !== null &&
    "role" in x &&
    typeof (x as ChatEntry).role === "string" &&
    "text" in x &&
    typeof (x as ChatEntry).text === "string"
  );
}

export function parseChatEntries(raw: unknown): ChatEntry[] {
  if (!Array.isArray(raw)) return [];
  return raw.filter(isChatEntry);
}
