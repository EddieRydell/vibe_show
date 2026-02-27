import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { cmd } from "../commands";
import { CHAT_TOOL_RESULT, CHAT_COMPLETE } from "../events";
import { Save, Sparkles, MessageSquare } from "lucide-react";
import { ScreenShell, useAppShell } from "../components/ScreenShell";
import { ScriptBrowser } from "../components/script/ScriptBrowser";
import { ScriptEditor } from "../components/script/ScriptEditor";
import { ScriptPreview } from "../components/script/ScriptPreview";
import { ParameterPlayground } from "../components/script/ParameterPlayground";
import { useScriptPreview } from "../hooks/useScriptPreview";
import type {
  ScriptCompileResult,
  ScriptParams,
  ScriptParamInfo,
} from "../types";
import { useToast } from "../hooks/useToast";

interface Props {
  initialScriptName: string | null;
  onBack: () => void;
  onOpenScript: (name: string) => void;
}

export function ScriptScreen({
  initialScriptName,
  onBack,
}: Props) {
  const { chatOpen, toggleChat, refreshRef } = useAppShell();
  const { showError } = useToast();

  const [currentScript, setCurrentScript] = useState<string | null>(
    initialScriptName,
  );
  const [source, setSource] = useState("");
  const [dirty, setDirty] = useState(false);
  const [compileResult, setCompileResult] =
    useState<ScriptCompileResult | null>(null);
  const [params, setParams] = useState<ScriptParamInfo[]>([]);
  const [paramValues, setParamValues] = useState<ScriptParams>({});
  const [browserRefreshKey, setBrowserRefreshKey] = useState(0);
  const sourceRef = useRef(source);
  sourceRef.current = source;

  const compiled = compileResult?.success ?? false;

  const preview = useScriptPreview({
    scriptName: compiled ? currentScript : null,
    compiled,
    params: paramValues as never,
    pixelCount: 50,
    timeSamples: 100,
  });

  // Register refresh handler for ChatPanel
  const handleChatRefresh = useCallback(() => {
    setBrowserRefreshKey((k) => k + 1);
  }, []);

  useEffect(() => {
    refreshRef.current = handleChatRefresh;
    return () => { refreshRef.current = null; };
  }, [handleChatRefresh, refreshRef]);

  // Load script source when current script changes
  useEffect(() => {
    if (!currentScript) {
      setSource("");
      setCompileResult(null);
      setParams([]);
      setParamValues({});
      return;
    }
    // Load from global library
    cmd.listGlobalScripts()
      .then((scripts) => {
        const match = scripts.find(([name]) => name === currentScript);
        if (match) {
          setSource(match[1]);
          setDirty(false);
        }
      })
      .catch(showError);
    // Also load params from cache
    cmd.getScriptParams(currentScript)
      .then((p) => {
        setParams(p);
        setParamValues(buildDefaults(p));
      })
      .catch(showError);
  }, [currentScript, showError]);

  // Listen for chat tool_result events — refresh script list & current source
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<{ tool: string; result: string }>(CHAT_TOOL_RESULT, () => {
      // Refresh browser whenever the AI modifies scripts
      setBrowserRefreshKey((k) => k + 1);
      // Reload current script source in case AI modified it
      if (currentScript) {
        cmd.listGlobalScripts()
          .then((scripts) => {
            const match = scripts.find(([name]) => name === currentScript);
            if (match) {
              setSource(match[1]);
              setDirty(false);
            }
          })
          .catch(showError);
        cmd.getScriptParams(currentScript)
          .then((p) => {
            setParams(p);
            setParamValues(buildDefaults(p));
          })
          .catch(showError);
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [currentScript]);

  // Listen for chat:complete to also check if a new script was created
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen(CHAT_COMPLETE, () => {
      // Refresh browser to pick up any newly created scripts
      setBrowserRefreshKey((k) => k + 1);
      // If no script is selected, check if one was just created and select it
      if (!currentScript) {
        cmd.listGlobalScripts()
          .then((scripts) => {
            if (scripts.length > 0) {
              // Select the last script (likely the one just created)
              const lastScript = scripts[scripts.length - 1]!;
              setCurrentScript(lastScript[0]);
            }
          })
          .catch(showError);
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [currentScript, showError]);

  const handleSourceChange = useCallback((newSource: string) => {
    setSource(newSource);
    setDirty(true);
  }, []);

  const handleCompile = useCallback(
    (result: ScriptCompileResult) => {
      setCompileResult(result);
      if (result.success && result.params) {
        setParams(result.params);
        // Merge new defaults, keeping user-set values
        setParamValues((prev) => {
          const merged: ScriptParams = { ...prev };
          for (const p of result.params ?? []) {
            if (!(p.name in merged) && p.default != null) {
              merged[p.name] = p.default;
            }
          }
          return merged;
        });
      }
    },
    [],
  );

  const handleSave = useCallback(async () => {
    if (!currentScript) return;
    const result = await cmd.compileGlobalScript(currentScript, sourceRef.current);
    setCompileResult(result);
    if (result.success) {
      setDirty(false);
      if (result.params) {
        setParams(result.params);
      }
      setBrowserRefreshKey((k) => k + 1);
    }
  }, [currentScript]);

  const handleNewScript = useCallback(async (name: string) => {
    const defaultSource = `@name "${name}"\n\nlet c = hsv(t * 360.0, 1.0, 1.0)\nc\n`;
    await cmd.compileGlobalScript(name, defaultSource);
    setBrowserRefreshKey((k) => k + 1);
    setCurrentScript(name);
  }, []);

  const handleSelectScript = useCallback((name: string) => {
    setCurrentScript(name);
  }, []);

  const handleParamChange = useCallback((values: ScriptParams) => {
    setParamValues(values);
  }, []);

  const subtitle = currentScript
    ? `: ${currentScript}${dirty ? " *" : ""}`
    : undefined;

  const toolbar = (
    <div className="border-border bg-surface flex select-none items-center gap-2 border-b px-4 py-1.5">
      <div className="flex-1" />
      <button
        onClick={() => { void handleSave(); }}
        disabled={!currentScript || !dirty}
        className="bg-primary hover:bg-primary/90 flex items-center gap-1 rounded px-2 py-0.5 text-[11px] text-white disabled:opacity-40"
      >
        <Save size={12} />
        Save
      </button>
    </div>
  );

  return (
    <ScreenShell title="Effect Studio" subtitle={subtitle} onBack={onBack} toolbar={toolbar}>
      {/* Main content */}
      <div className="flex min-h-0 flex-1">
        {/* Left: Script Browser */}
        <ScriptBrowser
          currentScript={currentScript}
          onSelectScript={handleSelectScript}
          onNewScript={(name) => { void handleNewScript(name); }}
          refreshKey={browserRefreshKey}
        />

        {/* Center: Code Editor or Welcome */}
        <div className="flex min-w-0 flex-1 flex-col">
          {currentScript ? (
            <ScriptEditor
              source={source}
              scriptName={currentScript}
              onSourceChange={handleSourceChange}
              onCompileResult={handleCompile}
              onSave={() => { void handleSave(); }}
            />
          ) : (
            <WelcomePanel
              onNewScript={(name) => { void handleNewScript(name); }}
              chatOpen={chatOpen}
              onOpenChat={toggleChat}
            />
          )}
        </div>

        {/* Right: Preview + Parameters (only when a script is selected) */}
        {currentScript && (
          <div className="border-border flex w-[280px] shrink-0 flex-col gap-3 overflow-y-auto border-l p-3">
            {/* Compile status */}
            <div className="flex items-center gap-2">
              <div
                className={`size-2  rounded-full ${
                  compileResult == null
                    ? "bg-text-2"
                    : compileResult.success
                      ? "bg-green-500"
                      : "bg-red-500"
                }`}
              />
              <span className="text-text-2 text-[10px]">
                {compileResult == null
                  ? "Not compiled"
                  : compileResult.success
                    ? "Compiled OK"
                    : `${compileResult.errors.length} error(s)`}
              </span>
            </div>

            {/* Errors */}
            {compileResult && !compileResult.success && compileResult.errors.length > 0 && (
              <div className="bg-red-500/10 rounded p-2">
                {compileResult.errors.map((err, i) => (
                  <div key={i} className="text-red-400 text-[10px]">
                    {err.message}
                  </div>
                ))}
              </div>
            )}

            {/* Preview */}
            <ScriptPreview
              heatmap={preview.heatmap}
              strip={preview.strip}
              currentTime={preview.currentTime}
              playing={preview.playing}
              pixelCount={50}
              duration={preview.duration}
              onScrub={preview.scrub}
              onTogglePlay={preview.togglePlay}
              onDurationChange={preview.setDuration}
            />

            {/* Parameters */}
            <ParameterPlayground
              params={params}
              values={paramValues}
              onChange={handleParamChange}
            />
          </div>
        )}
      </div>
    </ScreenShell>
  );
}

/** Welcome panel shown when no script is selected — AI-first experience. */
function WelcomePanel({
  onNewScript,
  chatOpen,
  onOpenChat,
}: {
  onNewScript: (name: string) => void;
  chatOpen: boolean;
  onOpenChat: () => void;
}) {
  const [showNameInput, setShowNameInput] = useState(false);
  const [nameValue, setNameValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (showNameInput) inputRef.current?.focus();
  }, [showNameInput]);

  const commit = useCallback(() => {
    const trimmed = nameValue.trim();
    if (!trimmed) {
      setShowNameInput(false);
      setNameValue("");
      return;
    }
    setShowNameInput(false);
    setNameValue("");
    onNewScript(trimmed);
  }, [nameValue, onNewScript]);

  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-6 p-8">
      <div className="flex flex-col items-center gap-2">
        <Sparkles size={32} className="text-primary" />
        <h2 className="text-text text-lg font-semibold">Effect Studio</h2>
        <p className="text-text-2 max-w-md text-center text-sm">
          Create custom light effects with AI or write them yourself.
          Describe what you want and the AI will generate the script for you.
        </p>
      </div>

      {showNameInput ? (
        <div className="flex items-center gap-2">
          <input
            ref={inputRef}
            type="text"
            value={nameValue}
            onChange={(e) => setNameValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") { setShowNameInput(false); setNameValue(""); }
            }}
            onBlur={commit}
            placeholder="Script name"
            className="border-border bg-surface-2 text-text placeholder:text-text-2 rounded border px-3 py-1.5 text-sm outline-none focus:border-primary"
          />
        </div>
      ) : (
        <div className="flex items-center gap-3">
          <button
            onClick={() => setShowNameInput(true)}
            className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-4 py-2 text-xs transition-colors"
          >
            New blank script
          </button>
          {!chatOpen && (
            <button
              onClick={onOpenChat}
              className="bg-primary hover:bg-primary/90 flex items-center gap-1.5 rounded px-4 py-2 text-xs text-white transition-colors"
            >
              <MessageSquare size={12} />
              Open AI Chat
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function buildDefaults(params: ScriptParamInfo[]): ScriptParams {
  const defaults: ScriptParams = {};
  for (const p of params) {
    if (p.default != null) {
      defaults[p.name] = p.default;
    }
  }
  return defaults;
}
