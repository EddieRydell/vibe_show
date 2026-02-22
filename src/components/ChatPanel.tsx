import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cmd } from "../commands";
import { MessageSquare, Send, X, Trash2, Loader2, Key } from "lucide-react";
import { useAppShell } from "./ScreenShell";
import type { ChatMode } from "../types";

interface ChatEntry {
  role: string;
  text: string;
}

interface ChatPanelProps {
  open: boolean;
  onClose: () => void;
  onRefresh: () => void;
}

export function ChatPanel({ open, onClose, onRefresh }: ChatPanelProps) {
  const { openSettings } = useAppShell();
  const [messages, setMessages] = useState<ChatEntry[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [streaming, setStreaming] = useState("");
  const [hasApiKey, setHasApiKey] = useState(false);
  const [chatMode, setChatMode] = useState<ChatMode>("Basic");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Check API key and chat mode on mount
  useEffect(() => {
    cmd.getLlmConfig().then((config) => {
      setHasApiKey(config.has_api_key);
      setChatMode(config.chat_mode);
    }).catch(() => {});
  }, [open]);

  // Listen for chat events (both backends emit the same events)
  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<string>("chat:token", (event) => {
      setStreaming((prev) => prev + event.payload);
    }).then((unlisten) => unlisteners.push(unlisten));

    listen<string>("chat:tool_call", () => {
      // Tool calls happen behind the scenes — just clear streaming text
      // so the "Thinking..." spinner shows while tools execute
      setStreaming("");
    }).then((unlisten) => unlisteners.push(unlisten));

    listen<{ tool: string; result: string }>("chat:tool_result", () => {
      setStreaming("");
      onRefresh();
    }).then((unlisten) => unlisteners.push(unlisten));

    listen<boolean>("chat:complete", () => {
      setSending(false);
      setStreaming("");
      // Refresh messages from backend (Basic mode only — Agent mode doesn't persist history)
      cmd.getChatHistory().then((entries) => setMessages(entries as ChatEntry[])).catch(() => {});
      onRefresh();
    }).then((unlisten) => unlisteners.push(unlisten));

    listen<boolean>("chat:thinking", () => {
      setStreaming("");
    }).then((unlisten) => unlisteners.push(unlisten));

    return () => {
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

    try {
      if (chatMode === "Agent") {
        await invoke("send_agent_message", { message: msg });
      } else {
        await invoke("send_chat_message", { message: msg });
      }
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
  }, [input, sending, chatMode]);

  const handleClear = useCallback(async () => {
    if (chatMode === "Agent") {
      await invoke("clear_agent_session").catch(() => {});
    }
    await cmd.clearChat();
    setMessages([]);
    setStreaming("");
  }, [chatMode]);

  const handleStop = useCallback(async () => {
    if (chatMode === "Agent") {
      await invoke("cancel_agent_message").catch(() => {});
    }
    await cmd.stopChat();
    setSending(false);
  }, [chatMode]);

  if (!open) return null;

  return (
    <div className="border-border bg-surface flex h-full w-80 flex-shrink-0 flex-col border-l">
      {/* Header */}
      <div className="border-border flex items-center justify-between border-b px-3 py-2">
        <div className="flex items-center gap-2">
          <MessageSquare size={14} className="text-primary" />
          <span className="text-text text-sm font-medium">Chat</span>
          {chatMode === "Agent" && (
            <span className="bg-primary/15 text-primary rounded px-1.5 py-0.5 text-[10px] font-medium">
              Agent
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={handleClear}
            className="text-text-2 hover:text-text rounded p-1 transition-colors"
            title="Clear conversation"
          >
            <Trash2 size={14} />
          </button>
          <button
            onClick={onClose}
            className="text-text-2 hover:text-text rounded p-1 transition-colors"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-3 py-2 space-y-2">
        {!hasApiKey && chatMode !== "Agent" && (
          <div className="bg-surface-2 border-border rounded border p-3 text-center">
            <Key size={20} className="text-text-2 mx-auto mb-2" />
            <p className="text-text-2 mb-2 text-xs">
              Set your API key in Settings to use the chat.
            </p>
            <button
              onClick={openSettings}
              className="bg-primary text-white rounded px-3 py-1 text-xs hover:opacity-90"
            >
              Open Settings
            </button>
          </div>
        )}

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
              <div className="bg-surface-2 text-text max-w-[90%] rounded-lg px-3 py-2 whitespace-pre-wrap">
                {msg.text}
              </div>
            )}
          </div>
        ))}

        {streaming && (
          <div className="bg-surface-2 text-text rounded-lg px-3 py-2 text-xs whitespace-pre-wrap">
            {streaming}
            <span className="bg-text-2 ml-0.5 inline-block h-3 w-1 animate-pulse" />
          </div>
        )}

        {sending && !streaming && (
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
                handleSend();
              }
            }}
            placeholder={hasApiKey ? "Ask AI to edit your show..." : "API key required"}
            disabled={!hasApiKey || sending}
            className="border-border bg-bg text-text placeholder:text-text-2 flex-1 rounded border px-2 py-1.5 text-xs outline-none focus:border-primary disabled:opacity-50"
          />
          {sending ? (
            <button
              onClick={handleStop}
              className="border-border bg-surface text-text-2 hover:text-text rounded border px-2 py-1.5 text-xs transition-colors"
            >
              Stop
            </button>
          ) : (
            <button
              onClick={handleSend}
              disabled={!hasApiKey || !input.trim()}
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
