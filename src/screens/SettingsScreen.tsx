import { useCallback, useEffect, useState } from "react";
import { cmd } from "../commands";
import { Sun, Moon, Monitor, RotateCcw, Key, Eye, EyeOff, Globe, Bot, MessageSquare } from "lucide-react";
import { useUISettings, type ThemeMode } from "../hooks/useUISettings";
import { ScreenShell } from "../components/ScreenShell";
import type { ChatMode, LlmProvider } from "../types";

interface Props {
  onBack: () => void;
}

const THEME_OPTIONS: { mode: ThemeMode; label: string; Icon: typeof Sun }[] = [
  { mode: "light", label: "Light", Icon: Sun },
  { mode: "dark", label: "Dark", Icon: Moon },
  { mode: "system", label: "System", Icon: Monitor },
];

const ACCENT_PRESETS = [
  { label: "Blue", value: "#3B82F6" },
  { label: "Violet", value: "#7C5CFF" },
  { label: "Teal", value: "#14B8A6" },
  { label: "Green", value: "#22C55E" },
  { label: "Orange", value: "#F97316" },
  { label: "Rose", value: "#F43F5E" },
];

const PROVIDER_OPTIONS: { value: LlmProvider; label: string }[] = [
  { value: "Anthropic", label: "Anthropic (Claude)" },
  { value: "OpenAiCompatible", label: "OpenAI Compatible" },
];

const CHAT_MODE_OPTIONS: { value: ChatMode; label: string; description: string; Icon: typeof Bot }[] = [
  {
    value: "Basic",
    label: "Basic Chat",
    description: "Direct API calls. Works with all providers.",
    Icon: MessageSquare,
  },
  {
    value: "Agent",
    label: "Claude Agent",
    description: "Full Claude Code tooling. Requires Node.js. Claude only.",
    Icon: Bot,
  },
];

export function SettingsScreen({ onBack }: Props) {
  const { settings, update, reset, defaults } = useUISettings();
  const [provider, setProvider] = useState<LlmProvider>("Anthropic");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [chatMode, setChatMode] = useState<ChatMode>("Basic");
  const [showKey, setShowKey] = useState(false);
  const [keySaved, setKeySaved] = useState(false);

  // Load existing LLM config on mount
  useEffect(() => {
    cmd.getLlmConfig().then((config) => {
      setProvider(config.provider);
      if (config.has_api_key) setApiKey("********");
      setBaseUrl(config.base_url ?? "");
      setModel(config.model ?? "");
      setChatMode(config.chat_mode);
    }).catch(() => {});
  }, []);

  const handleSaveLlmConfig = useCallback(async () => {
    const keyToSave = apiKey === "********" ? "" : apiKey;
    try {
      await cmd.setLlmConfig({
        provider,
        apiKey: keyToSave,
        baseUrl: baseUrl || null,
        model: model || null,
        chatMode,
      });
      setKeySaved(true);
      if (keyToSave) setApiKey("********");
      setTimeout(() => setKeySaved(false), 1500);
    } catch (e) {
      console.error("[VibeLights] Failed to save LLM config:", e);
    }
  }, [provider, apiKey, baseUrl, model, chatMode]);

  const handleClearApiKey = useCallback(async () => {
    try {
      await cmd.setLlmConfig({
        provider,
        apiKey: "",
        baseUrl: baseUrl || null,
        model: model || null,
      });
      setApiKey("");
      setKeySaved(false);
    } catch (e) {
      console.error("[VibeLights] Failed to clear API key:", e);
    }
  }, [provider, baseUrl, model]);

  const handleChatModeChange = useCallback(async (mode: ChatMode) => {
    setChatMode(mode);
    setKeySaved(false);
    // Save immediately since this is a standalone toggle
    try {
      await cmd.setLlmConfig({
        provider,
        apiKey: apiKey === "********" ? "" : apiKey,
        baseUrl: baseUrl || null,
        model: model || null,
        chatMode: mode,
      });
    } catch (e) {
      console.error("[VibeLights] Failed to save chat mode:", e);
    }
  }, [provider, apiKey, baseUrl, model]);

  return (
    <ScreenShell title="Settings" onBack={onBack} hideSettings>
      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-lg space-y-8 p-6">
          {/* ── Theme ── */}
          <Section title="Theme">
            <div className="flex gap-2">
              {THEME_OPTIONS.map(({ mode, label, Icon }) => (
                <button
                  key={mode}
                  onClick={() => update({ theme: mode })}
                  className={`flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium transition-colors ${
                    settings.theme === mode
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
                  }`}
                >
                  <Icon size={14} />
                  {label}
                </button>
              ))}
            </div>
            <p className="text-text-2 mt-2 text-xs">
              {settings.theme === "system"
                ? "Follows your operating system preference."
                : `Always use ${settings.theme} mode.`}
            </p>
          </Section>

          {/* ── Accent Color ── */}
          <Section title="Accent Color">
            <div className="flex flex-wrap gap-2">
              {ACCENT_PRESETS.map((preset) => (
                <button
                  key={preset.value}
                  onClick={() => update({ accentColor: preset.value })}
                  className={`flex items-center gap-2 rounded-lg border px-3 py-1.5 text-xs transition-colors ${
                    settings.accentColor.toLowerCase() === preset.value.toLowerCase()
                      ? "border-primary bg-primary/10 text-text"
                      : "border-border bg-surface text-text-2 hover:bg-surface-2"
                  }`}
                >
                  <span
                    className="size-3  rounded-full"
                    style={{ backgroundColor: preset.value }}
                  />
                  {preset.label}
                </button>
              ))}
            </div>
            <div className="mt-3 flex items-center gap-3">
              <label className="text-text-2 text-xs">Custom</label>
              <div className="relative">
                <div
                  className="border-border size-8  rounded border"
                  style={{ backgroundColor: settings.accentColor }}
                />
                <input
                  type="color"
                  className="absolute inset-0 cursor-pointer opacity-0"
                  value={settings.accentColor}
                  onChange={(e) => update({ accentColor: e.target.value })}
                />
              </div>
              <input
                type="text"
                className="border-border bg-surface text-text w-24 rounded border px-2 py-1 font-mono text-xs uppercase outline-none focus:border-primary"
                value={settings.accentColor}
                onChange={(e) => {
                  const v = e.target.value;
                  if (/^#[0-9a-fA-F]{6}$/.test(v)) {
                    update({ accentColor: v });
                  }
                }}
              />
            </div>
          </Section>

          {/* ── UI Scale ── */}
          <Section title="UI Scale">
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={75}
                max={150}
                step={5}
                value={settings.uiScale}
                onChange={(e) => update({ uiScale: Number(e.target.value) })}
                className="accent-primary h-1.5 flex-1 cursor-pointer appearance-none rounded-full bg-surface-2"
              />
              <span className="text-text w-12 text-right text-sm font-medium tabular-nums">
                {settings.uiScale}%
              </span>
            </div>
            <p className="text-text-2 mt-2 text-xs">
              Scales the entire interface. Default is 100%.
            </p>
          </Section>

          {/* ── AI Provider ── */}
          <Section title="AI Provider">
            {/* Provider selector */}
            <div className="flex gap-2">
              {PROVIDER_OPTIONS.map(({ value, label }) => (
                <button
                  key={value}
                  onClick={() => {
                    setProvider(value);
                    setKeySaved(false);
                  }}
                  className={`flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium transition-colors ${
                    provider === value
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
                  }`}
                >
                  {value === "Anthropic" ? <Key size={14} /> : <Globe size={14} />}
                  {label}
                </button>
              ))}
            </div>

            {/* API Key */}
            <div className="mt-3 flex items-center gap-2">
              <Key size={14} className="text-text-2 shrink-0" />
              <div className="relative flex-1">
                <input
                  type={showKey ? "text" : "password"}
                  className="border-border bg-surface text-text w-full rounded border px-2 py-1.5 pr-8 font-mono text-xs outline-none focus:border-primary"
                  placeholder={provider === "Anthropic" ? "sk-ant-..." : "sk-..."}
                  value={apiKey === "********" ? "********" : apiKey}
                  onChange={(e) => {
                    setApiKey(e.target.value);
                    setKeySaved(false);
                  }}
                  onFocus={() => {
                    if (apiKey === "********") setApiKey("");
                  }}
                />
                <button
                  type="button"
                  onClick={() => setShowKey((s) => !s)}
                  className="text-text-2 hover:text-text absolute right-2 top-1/2 -translate-y-1/2"
                >
                  {showKey ? <EyeOff size={12} /> : <Eye size={12} />}
                </button>
              </div>
              {apiKey && (
                <button
                  onClick={handleClearApiKey}
                  className="text-text-2 hover:text-error text-xs transition-colors"
                >
                  Clear
                </button>
              )}
            </div>

            {/* Base URL (only for OpenAI Compatible) */}
            {provider === "OpenAiCompatible" && (
              <div className="mt-3">
                <label className="text-text-2 mb-1 block text-xs">Base URL</label>
                <input
                  type="text"
                  className="border-border bg-surface text-text w-full rounded border px-2 py-1.5 font-mono text-xs outline-none focus:border-primary"
                  placeholder="https://api.openai.com/v1"
                  value={baseUrl}
                  onChange={(e) => {
                    setBaseUrl(e.target.value);
                    setKeySaved(false);
                  }}
                />
              </div>
            )}

            {/* Model override */}
            <div className="mt-3">
              <label className="text-text-2 mb-1 block text-xs">Model (optional override)</label>
              <input
                type="text"
                className="border-border bg-surface text-text w-full rounded border px-2 py-1.5 font-mono text-xs outline-none focus:border-primary"
                placeholder={provider === "Anthropic" ? "claude-sonnet-4-20250514" : "gpt-4o"}
                value={model}
                onChange={(e) => {
                  setModel(e.target.value);
                  setKeySaved(false);
                }}
              />
            </div>

            {/* Chat mode (only shown for Anthropic) */}
            {provider === "Anthropic" && (
              <div className="mt-4">
                <label className="text-text-2 mb-2 block text-xs font-medium">Chat Mode</label>
                <div className="flex gap-2">
                  {CHAT_MODE_OPTIONS.map(({ value, label, Icon }) => (
                    <button
                      key={value}
                      onClick={() => handleChatModeChange(value)}
                      className={`flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-medium transition-colors ${
                        chatMode === value
                          ? "border-primary bg-primary/10 text-primary"
                          : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
                      }`}
                    >
                      <Icon size={12} />
                      {label}
                    </button>
                  ))}
                </div>
                <p className="text-text-2 mt-2 text-xs">
                  {chatMode === "Agent"
                    ? "Agent mode uses Claude Code tools (file reading, search) for deep analysis. Requires Node.js 20+ in PATH."
                    : "Basic mode uses direct API calls with 3 meta-tools. Works with all providers."}
                </p>
              </div>
            )}

            {/* Save button */}
            <div className="mt-3 flex items-center gap-2">
              <button
                onClick={handleSaveLlmConfig}
                disabled={chatMode !== "Agent" && (!apiKey || apiKey === "********")}
                className={`rounded border px-3 py-1.5 text-xs transition-colors ${
                  keySaved
                    ? "border-green-500/30 bg-green-500/10 text-green-400"
                    : "border-primary bg-primary/10 text-primary hover:bg-primary/20"
                } disabled:opacity-50`}
              >
                {keySaved ? "Saved" : "Save"}
              </button>
            </div>

            <p className="text-text-2 mt-2 text-xs">
              Your API key is stored locally and used for the embedded chat panel.
              {provider === "Anthropic"
                ? " Get one at console.anthropic.com"
                : " Enter the base URL and key for your OpenAI-compatible provider."}
            </p>
          </Section>

          {/* ── Reset ── */}
          <div className="border-border border-t pt-6">
            <button
              onClick={reset}
              className="text-text-2 hover:text-text flex items-center gap-1.5 text-xs transition-colors"
            >
              <RotateCcw size={12} />
              Reset all settings to defaults
            </button>
            {settings.accentColor !== defaults.accentColor && (
              <p className="text-text-2 mt-1 text-[10px]">
                Default accent: {defaults.accentColor}
              </p>
            )}
          </div>
        </div>
      </div>
    </ScreenShell>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h3 className="text-text mb-3 text-sm font-medium">{title}</h3>
      {children}
    </div>
  );
}
