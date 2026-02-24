import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { cmd } from "../commands";
import type { ConversationSummary } from "../types";
import { MessageSquare, Send, X, Trash2, Loader2, Check, History, Plus } from "lucide-react";
import Markdown from "react-markdown";
import type { AppScreen } from "../screens";
import { parseChatEntries } from "../utils/validators";

interface ChatEntry {
  role: string;
  text: string;
}

interface ToolActivity {
  tool: string;
  status: "running" | "done";
  result?: string;
}

interface ChatPanelProps {
  open: boolean;
  onClose: () => void;
  onRefresh: () => void;
  screen: AppScreen;
}

/** Build a context string describing the user's current screen location. */
function buildScreenContext(screen: AppScreen): string {
  switch (screen.kind) {
    case "editor":
      return `editor (setup: "${screen.setupSlug}", sequence: "${screen.sequenceSlug}")`;
    case "script":
      return `script editor (script: "${screen.scriptName ?? "new"}")`;
    case "analysis":
      return `analysis (setup: "${screen.setupSlug}", file: "${screen.filename}")`;
    case "home":
      return "home screen";
    case "settings":
      return "settings";
    default:
      return screen.kind;
  }
}

export function ChatPanel({ open, onClose, onRefresh, screen }: ChatPanelProps) {
  const [messages, setMessages] = useState<ChatEntry[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [streaming, setStreaming] = useState("");
  const [toolActivity, setToolActivity] = useState<ToolActivity[]>([]);
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const refreshConversations = useCallback(() => {
    cmd.listAgentConversations().then(setConversations).catch(console.warn);
  }, []);

  // Load persisted chat history on mount.
  useEffect(() => {
    refreshConversations();
    cmd.getAgentChatHistory().then((entries) => {
      setMessages(parseChatEntries(entries));
    }).catch(console.warn);
    setStreaming("");
    setSending(false);
  }, [refreshConversations]);

  // Listen for chat events.
  // Uses cancelled-flag pattern to avoid stale listener accumulation.
  useEffect(() => {
    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    const reg = <T,>(event: string, handler: (payload: T) => void) => {
      listen<T>(event, (e) => handler(e.payload)).then((fn) => {
        if (cancelled) fn();
        else unlisteners.push(fn);
      });
    };

    reg<string>("chat:token", (text) => {
      setStreaming((prev) => prev + text);
    });

    reg<string>("chat:tool_call", (tool) => {
      setToolActivity((prev) => [...prev, { tool, status: "running" }]);
    });

    reg<{ tool: string; result: string }>("chat:tool_result", (payload) => {
      setToolActivity((prev) => {
        let idx = -1;
        for (let i = prev.length - 1; i >= 0; i--) {
          if (prev[i].status === "running") { idx = i; break; }
        }
        if (idx === -1) return prev;
        const updated = [...prev];
        updated[idx] = { ...updated[idx], status: "done", result: payload.result };
        return updated;
      });
      onRefresh();
    });

    reg<boolean>("chat:complete", () => {
      setSending(false);
      setToolActivity([]);
      // Capture streamed text as a message before clearing
      setStreaming((prev) => {
        if (prev.trim()) {
          setMessages((msgs) => [...msgs, { role: "assistant", text: prev }]);
        }
        return "";
      });
      // Refresh messages from agent backend
      cmd.getAgentChatHistory().then((entries) => {
        const parsed = parseChatEntries(entries);
        if (parsed.length > 0) setMessages(parsed);
      }).catch(console.warn);
      onRefresh();
    });

    reg<boolean>("chat:thinking", () => {
      // Don't clear streaming — let text accumulate
    });

    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => fn());
    };
  }, [onRefresh]);

  // Auto-scroll
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streaming]);

  const handleSend = useCallback(async () => {
    if (!input.trim() || sending) return;
    const msg = input.trim();
    setInput("");
    setSending(true);
    setMessages((prev) => [...prev, { role: "user", text: msg }]);

    const context = buildScreenContext(screen);

    try {
      await cmd.sendAgentMessage(msg, context);
    } catch (e: unknown) {
      const text =
        typeof e === "string"
          ? e
          : e && typeof e === "object" && "detail" in e
            ? String((e as { detail?: { message?: string } }).detail?.message ?? e)
            : JSON.stringify(e);
      setMessages((prev) => [
        ...prev,
        { role: "error", text },
      ]);
      setSending(false);
    }
  }, [input, sending, screen]);

  const handleClear = useCallback(async () => {
    await cmd.newAgentConversation().catch(console.warn);
    await cmd.clearAgentSession().catch(console.warn);
    refreshConversations();
    setMessages([]);
    setStreaming("");
    setToolActivity([]);
  }, [refreshConversations]);

  const handleStop = useCallback(async () => {
    await cmd.cancelAgentMessage().catch(console.warn);
    setSending(false);
  }, []);

  const handleSwitchConversation = useCallback(async (id: string) => {
    await cmd.switchAgentConversation(id).catch(console.warn);
    const entries = await cmd.getAgentChatHistory().catch(() => []);
    setMessages(parseChatEntries(entries));
    setStreaming("");
    setToolActivity([]);
    setShowHistory(false);
    refreshConversations();
  }, [refreshConversations]);

  const handleDeleteConversation = useCallback(async (id: string) => {
    await cmd.deleteAgentConversation(id).catch(console.warn);
    refreshConversations();
    // If we deleted the active one, reload messages
    const active = conversations.find((c) => c.is_active);
    if (active?.id === id) {
      setMessages([]);
      setStreaming("");
    }
  }, [conversations, refreshConversations]);

  return (
    <div className={`border-border bg-surface flex h-full w-80 shrink-0 flex-col border-l ${open ? "" : "hidden"}`}>
      {/* Header */}
      <div className="border-border flex items-center justify-between border-b px-3 py-2">
        <div className="flex items-center gap-2">
          <MessageSquare size={14} className="text-primary" />
          <span className="text-text text-sm font-medium">Chat</span>
          <span className="bg-primary/15 text-primary rounded px-1.5 py-0.5 text-[10px] font-medium">
            Agent
          </span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setShowHistory((v) => !v)}
            className={`rounded p-1 transition-colors ${showHistory ? "text-primary bg-primary/10" : "text-text-2 hover:text-text"}`}
            aria-label="Conversation history"
            title="Conversation history"
          >
            <History size={14} />
          </button>
          <button
            onClick={handleClear}
            className="text-text-2 hover:text-text rounded p-1 transition-colors"
            aria-label="New conversation"
            title="New conversation"
          >
            <Plus size={14} />
          </button>
          <button
            onClick={onClose}
            className="text-text-2 hover:text-text rounded p-1 transition-colors"
            aria-label="Close chat"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Conversation History */}
      {showHistory && (
        <div className="border-border border-b overflow-y-auto max-h-48">
          {conversations.length === 0 ? (
            <div className="text-text-2 p-3  text-center text-xs">No conversations yet</div>
          ) : (
            conversations.map((conv) => (
              <div
                key={conv.id}
                className={`flex items-center gap-2 px-3 py-1.5 cursor-pointer text-xs transition-colors ${
                  conv.is_active ? "bg-primary/10 text-primary" : "text-text hover:bg-surface-2"
                }`}
                onClick={() => handleSwitchConversation(conv.id)}
              >
                <div className="flex-1 min-w-0">
                  <div className="truncate font-medium">{conv.title}</div>
                  <div className="text-text-2 text-[10px]">{conv.message_count} messages</div>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDeleteConversation(conv.id);
                  }}
                  className="text-text-2 hover:text-error shrink-0 rounded p-0.5 transition-colors"
                  aria-label="Delete conversation"
                  title="Delete conversation"
                >
                  <Trash2 size={11} />
                </button>
              </div>
            ))
          )}
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-3 py-2 space-y-2" aria-live="polite">
        {messages.map((msg, i) => (
          <div key={i} className={`text-xs ${msg.role === "user" ? "text-right" : ""}`}>
            {msg.role === "user" ? (
              <div className="bg-primary/10 text-text inline-block max-w-[90%] rounded-lg px-3 py-2 text-left">
                {msg.text}
              </div>
            ) : msg.role === "error" ? (
              <div className="bg-error/10 text-error rounded-lg px-3 py-2">
                {msg.text}
              </div>
            ) : (
              <div className="bg-surface-2 text-text max-w-[90%] rounded-lg px-3 py-2 prose-chat">
                <Markdown>{msg.text}</Markdown>
              </div>
            )}
          </div>
        ))}

        {streaming && (
          <div className="bg-surface-2 text-text rounded-lg px-3 py-2 text-xs prose-chat">
            <Markdown>{streaming}</Markdown>
            <span className="bg-text-2 ml-0.5 inline-block h-3 w-1 animate-pulse" />
          </div>
        )}

        {sending && !streaming && toolActivity.length === 0 && (
          <div className="text-text-2 flex items-center gap-2 py-1 text-xs">
            <Loader2 size={12} className="animate-spin" />
            Thinking...
          </div>
        )}

        {toolActivity.length > 0 && (
          <div className="space-y-0.5 py-1">
            {toolActivity.map((activity, i) => (
              <div key={i} className="text-text-2 flex items-center gap-1.5 text-[11px] py-0.5">
                {activity.status === "running" ? (
                  <Loader2 size={10} className="animate-spin shrink-0" />
                ) : (
                  <Check size={10} className="shrink-0" />
                )}
                <span className="font-mono">{activity.tool}</span>
                {activity.status === "done" && activity.result && (
                  <span className="truncate opacity-60">— {activity.result.slice(0, 80)}</span>
                )}
              </div>
            ))}
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="border-border border-t p-2">
        <div className="flex gap-1.5">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
            placeholder="Ask AI to edit your show..."
            disabled={sending}
            className="border-border bg-bg text-text placeholder:text-text-2 flex-1 rounded border px-2 py-1.5 text-xs outline-none focus:border-primary disabled:opacity-50"
          />
          {sending ? (
            <button
              onClick={handleStop}
              aria-label="Stop generating"
              className="border-border bg-surface text-text-2 hover:text-text rounded border px-2 py-1.5 text-xs transition-colors"
            >
              Stop
            </button>
          ) : (
            <button
              onClick={handleSend}
              disabled={!input.trim()}
              aria-label="Send message"
              className="bg-primary text-white rounded px-2 py-1.5 transition-colors hover:opacity-90 disabled:opacity-50"
            >
              <Send size={12} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
