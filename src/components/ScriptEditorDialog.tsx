import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ScriptCompileResult } from "../types";

interface Props {
  scriptName: string | null; // null = new script
  initialSource: string;
  onSaved: () => void;
  onCancel: () => void;
  /** Tauri command for compiling (default: "compile_script") */
  compileCommand?: string;
  /** Tauri command for listing script names (default: "list_scripts") */
  listCommand?: string;
}

export function ScriptEditorDialog({
  scriptName,
  initialSource,
  onSaved,
  onCancel,
  compileCommand = "compile_script",
  listCommand = "list_scripts",
}: Props) {
  const [name, setName] = useState(scriptName ?? "");
  const [source, setSource] = useState(initialSource);
  const [compileResult, setCompileResult] = useState<ScriptCompileResult | null>(null);
  const [compiling, setCompiling] = useState(false);
  const [modified, setModified] = useState(false);
  const [saving, setSaving] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Auto-generate name for new scripts
  useEffect(() => {
    if (scriptName !== null) return;
    invoke<string[] | [string, string][]>(listCommand).then((result) => {
      // list_scripts returns string[], list_profile_scripts returns [name, source][]
      const names = result.map((r) => (Array.isArray(r) ? r[0] : r));
      let idx = 1;
      while (names.includes(`script_${idx}`)) idx++;
      setName(`script_${idx}`);
    }).catch(console.error);
  }, [scriptName, listCommand]);

  // Auto-compile on change (debounced)
  useEffect(() => {
    if (!name.trim() || !source.trim()) return;
    setModified(true);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setCompiling(true);
      invoke<ScriptCompileResult>(compileCommand, { name: name.trim(), source })
        .then((result) => {
          setCompileResult(result);
          if (result.success) setModified(false);
        })
        .catch(console.error)
        .finally(() => setCompiling(false));
    }, 500);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [source, name, compileCommand]);

  const handleSave = useCallback(async () => {
    if (!name.trim()) return;
    setSaving(true);
    try {
      if (modified || !compileResult?.success) {
        const result = await invoke<ScriptCompileResult>(compileCommand, {
          name: name.trim(),
          source,
        });
        setCompileResult(result);
        if (!result.success) {
          setSaving(false);
          return;
        }
      }
      onSaved();
    } catch (e) {
      console.error("[VibeLights] Script save failed:", e);
    } finally {
      setSaving(false);
    }
  }, [name, source, modified, compileResult, compileCommand, onSaved]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
      if (e.key === "s" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        handleSave();
      }
    },
    [onCancel, handleSave],
  );

  const status = compiling
    ? "Compiling..."
    : compileResult?.success
      ? "Compiled"
      : compileResult && !compileResult.success
        ? "Errors"
        : modified
          ? "Modified"
          : "";

  const statusColor = compiling
    ? "text-text-2"
    : compileResult?.success
      ? "text-green-400"
      : compileResult && !compileResult.success
        ? "text-red-400"
        : "text-text-2";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={handleKeyDown}
    >
      <div className="bg-surface border-border flex w-[560px] flex-col rounded-lg border shadow-xl">
        {/* Header */}
        <div className="border-border flex items-center justify-between border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">
            {scriptName ? "Edit Script" : "New Script"}
          </h3>
          <button onClick={onCancel} className="text-text-2 hover:text-text text-sm">
            &times;
          </button>
        </div>

        {/* Body */}
        <div className="space-y-3 px-5 py-4">
          {/* Script name */}
          <label className="block">
            <span className="text-text-2 mb-1 block text-xs">Name</span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
              className="border-border bg-surface-2 text-text focus:border-primary w-full rounded border px-3 py-1.5 text-sm outline-none"
            />
          </label>

          {/* Code textarea */}
          <label className="block">
            <div className="mb-1 flex items-center justify-between">
              <span className="text-text-2 text-xs">Source</span>
              {status && <span className={`text-[10px] ${statusColor}`}>{status}</span>}
            </div>
            <textarea
              value={source}
              onChange={(e) => setSource(e.target.value)}
              spellCheck={false}
              className="border-border bg-bg text-text focus:border-primary h-[300px] w-full resize-y rounded border p-3 font-mono text-xs leading-relaxed outline-none"
              placeholder="// Write your effect script here..."
            />
          </label>

          {/* Error panel */}
          {compileResult && !compileResult.success && compileResult.errors.length > 0 && (
            <div className="bg-red-500/10 max-h-24 overflow-y-auto rounded p-2">
              {compileResult.errors.map((err, i) => (
                <div key={i} className="text-[11px] text-red-400">
                  <span className="text-red-300 font-mono">@{err.offset}</span>{" "}
                  {err.message}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          <button
            onClick={onCancel}
            className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-4 py-1.5 text-xs transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving || !name.trim()}
            className="bg-primary hover:bg-primary/90 rounded px-4 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-50"
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
