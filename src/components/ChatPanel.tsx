import { useCallback, useEffect, useRef, useState } from "react";
import { cmd } from "../commands";
import { CHAT_TOKEN, CHAT_TOOL_CALL, CHAT_TOOL_RESULT, CHAT_COMPLETE, CHAT_THINKING, CHAT_ERROR } from "../events";
import { useTauriListener } from "../hooks/useTauriListener";
import type { ChatHistoryEntry, ConversationSummary } from "../types";
import { MessageSquare, Send, X, Trash2, Loader2, Check, History, Plus } from "lucide-react";
import Markdown from "react-markdown";
import type { AppScreen } from "../screens";
import { parseChatEntries } from "../utils/validators";
import { useToast } from "../hooks/useToast";

/** Display message: backend chat entries plus frontend-only roles. */
type DisplayMessage =
  | ChatHistoryEntry
  | { role: "error"; text: string }
  | { role: "tool_call"; id: string; tool: string; status: "running" | "done"; result?: string };

type KeyedMessage = DisplayMessage & { key: string };

let msgKeyCounter = 0;
function withKey(msg: DisplayMessage): KeyedMessage {
  return { ...msg, key: `msg-${++msgKeyCounter}` };
}
function withKeys(msgs: DisplayMessage[]): KeyedMessage[] {
  return msgs.map(withKey);
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
  const [messages, setMessages] = useState<KeyedMessage[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [streaming, setStreaming] = useState("");
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const { showError } = useToast();

  // Derived: any tool still running?
  const hasRunningTools = messages.some((m) => m.role === "tool_call" && m.status === "running");

  const refreshConversations = useCallback(() => {
    cmd.listAgentConversations().then(setConversations).catch(showError);
  }, [showError]);

  // Load persisted chat history on mount.
  useEffect(() => {
    refreshConversations();
    cmd.getAgentChatHistory().then((entries) => {
      setMessages(withKeys(parseChatEntries(entries)));
    }).catch(showError);
    setStreaming("");
    setSending(false);
  }, [refreshConversations, showError]);

  // Listen for chat events
  useTauriListener<string>(CHAT_TOKEN, (text) => {
    setStreaming((prev) => prev + text);
  });

  // Tool call: flush any accumulated text as a message, then add the tool inline
  useTauriListener<{ id: string; tool: string }>(CHAT_TOOL_CALL, (payload) => {
    setStreaming((prev) => {
      if (prev.trim()) {
        setMessages((msgs) => [...msgs, withKey({ role: "assistant", text: prev })]);
      }
      return "";
    });
    setMessages((msgs) => [...msgs, withKey({
      role: "tool_call", id: payload.id, tool: payload.tool, status: "running",
    })]);
  });

  // Tool result: update the matching tool message in-place
  useTauriListener<{ id: string; tool: string; result: string }>(CHAT_TOOL_RESULT, (payload) => {
    setMessages((msgs) => {
      const idx = msgs.findIndex((m) => m.role === "tool_call" && m.id === payload.id);
      if (idx === -1) return msgs;
      const updated = [...msgs];
      const prev = updated[idx]! as KeyedMessage & { role: "tool_call" };
      updated[idx] = { ...prev, status: "done", result: payload.result };
      return updated;
    });
    onRefresh();
  }, [onRefresh]);

  // Complete: flush remaining text
  useTauriListener<boolean>(CHAT_COMPLETE, () => {
    setSending(false);
    setStreaming((prev) => {
      if (prev.trim()) {
        setMessages((msgs) => [...msgs, withKey({ role: "assistant", text: prev })]);
      }
      return "";
    });
    onRefresh();
  }, [onRefresh]);

  useTauriListener<boolean>(CHAT_THINKING, () => {
    // Don't clear streaming — let text accumulate
  });

  useTauriListener<string>(CHAT_ERROR, (text) => {
    setMessages((prev) => [...prev, withKey({ role: "error", text })]);
  });

  // Auto-scroll only when already near the bottom
  const isNearBottom = useCallback(() => {
    const el = scrollContainerRef.current;
    if (!el) return true;
    return el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  }, []);

  useEffect(() => {
    if (isNearBottom()) {
      messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages, isNearBottom]);

  // Debounced scroll for streaming tokens
  const scrollTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => {
    if (!streaming) return;
    clearTimeout(scrollTimerRef.current);
    scrollTimerRef.current = setTimeout(() => {
      if (isNearBottom()) {
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
      }
    }, 100);
    return () => clearTimeout(scrollTimerRef.current);
  }, [streaming, isNearBottom]);

  const handleSend = useCallback(async () => {
    if (!input.trim() || sending) return;
    const msg = input.trim();
    setInput("");
    setSending(true);
    setMessages((prev) => [...prev, withKey({ role: "user", text: msg })]);

    const context = buildScreenContext(screen);

    try {
      await cmd.sendAgentMessage(msg, context);
    } catch (e: unknown) {
      const text =
        typeof e === "string"
          ? e
          : e && typeof e === "object" && "detail" in e
            ? ((e as { detail?: { message?: string } }).detail?.message ?? JSON.stringify(e))
            : JSON.stringify(e);
      setMessages((prev) => [
        ...prev,
        withKey({ role: "error", text }),
      ]);
      setSending(false);
    }
  }, [input, sending, screen]);

  const handleClear = useCallback(async () => {
    await cmd.newAgentConversation().catch(showError);
    await cmd.clearAgentSession().catch(showError);
    refreshConversations();
    setMessages([]);
    setStreaming("");
  }, [refreshConversations]);

  const handleStop = useCallback(async () => {
    await cmd.cancelAgentMessage().catch(showError);
    setSending(false);
  }, [showError]);

  const handleSwitchConversation = useCallback(async (id: string) => {
    // Cancel any in-flight query before switching
    if (sending) {
      await cmd.cancelAgentMessage().catch(showError);
    }
    await cmd.switchAgentConversation(id).catch(showError);
    try {
      const entries = await cmd.getAgentChatHistory();
      setMessages(withKeys(parseChatEntries(entries)));
    } catch (e) {
      showError(e);
    }
    setStreaming("");
    setSending(false);
    setShowHistory(false);
    refreshConversations();
  }, [sending, showError, refreshConversations]);

  const handleDeleteConversation = useCallback(async (id: string) => {
    await cmd.deleteAgentConversation(id).catch(showError);
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
            onClick={() => { void handleClear(); }}
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
                onClick={() => { void handleSwitchConversation(conv.id); }}
              >
                <div className="flex-1 min-w-0">
                  <div className="truncate font-medium">{conv.title}</div>
                  <div className="text-text-2 text-[10px]">{conv.message_count} messages</div>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    void handleDeleteConversation(conv.id);
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
      <div ref={scrollContainerRef} className="flex-1 overflow-y-auto px-3 py-2 space-y-2" aria-live="polite">
        {messages.map((msg) => (
          <div key={msg.key} className={`text-xs ${msg.role === "user" ? "text-right" : ""}`}>
            {msg.role === "user" ? (
              <div className="bg-primary/10 text-text inline-block max-w-[90%] rounded-lg px-3 py-2 text-left">
                {msg.text}
              </div>
            ) : msg.role === "error" ? (
              <div className="bg-error/10 text-error rounded-lg px-3 py-2">
                {msg.text}
              </div>
            ) : msg.role === "tool_call" ? (
              <div className="text-text-2 flex items-center gap-1.5 text-[11px] py-0.5">
                {msg.status === "running" ? (
                  <Loader2 size={10} className="animate-spin shrink-0" />
                ) : (
                  <Check size={10} className="shrink-0" />
                )}
                <span className="font-mono">{msg.tool}</span>
                {msg.status === "done" && msg.result && (
                  <span className="truncate opacity-60">— {msg.result.slice(0, 80)}</span>
                )}
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

        {sending && !streaming && !hasRunningTools && (
          <div className="text-text-2 flex items-center gap-2 py-1 text-xs">
            <Loader2 size={12} className="animate-spin" />
            Thinking...
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
                void handleSend();
              }
            }}
            placeholder="Ask AI to edit your show..."
            disabled={sending}
            className="border-border bg-bg text-text placeholder:text-text-2 flex-1 rounded border px-2 py-1.5 text-xs outline-none focus:border-primary disabled:opacity-50"
          />
          {sending ? (
            <button
              onClick={() => { void handleStop(); }}
              aria-label="Stop generating"
              className="border-border bg-surface text-text-2 hover:text-text rounded border px-2 py-1.5 text-xs transition-colors"
            >
              Stop
            </button>
          ) : (
            <button
              onClick={() => { void handleSend(); }}
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
