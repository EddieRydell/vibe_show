import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronRight, Maximize2, Plus, Trash2 } from "lucide-react";
import { cmd } from "../commands";
import type { Color, ColorGradient, ColorStop, Curve, CurvePoint } from "../types";
import { colorToCSS, lerpColor } from "../utils/colorUtils";
import { GradientEditor } from "./controls/GradientEditor";
import { CurveEditor } from "./controls/CurveEditor";
import { ScriptEditorDialog } from "./ScriptEditorDialog";
import { CurveEditorDialog } from "./CurveEditorDialog";
import { GradientEditorDialog } from "./GradientEditorDialog";
import { useToast } from "../hooks/useToast";

interface Props {
  onClose?: () => void;
  onLibraryChange: () => void;
  showVersion: number;
}

// ── Gradient preview canvas ──────────────────────────────────────

function GradientPreview({ stops }: { stops: ColorStop[] }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const ctx = canvasRef.current?.getContext("2d");
    if (!ctx || stops.length === 0) return;
    const w = 200;
    const h = 16;
    const sorted = [...stops].sort((a, b) => a.position - b.position);
    for (let px = 0; px < w; px++) {
      const pos = px / (w - 1);
      let color: Color;
      if (sorted.length === 1) {
        color = sorted[0]!.color;
      } else if (pos <= sorted[0]!.position) {
        color = sorted[0]!.color;
      } else if (pos >= sorted[sorted.length - 1]!.position) {
        color = sorted[sorted.length - 1]!.color;
      } else {
        let idx = 0;
        for (let i = 1; i < sorted.length; i++) {
          if (sorted[i]!.position >= pos) { idx = i; break; }
        }
        const a = sorted[idx - 1]!;
        const b = sorted[idx]!;
        const dp = b.position - a.position;
        const t = dp > 0 ? (pos - a.position) / dp : 0;
        color = lerpColor(a.color, b.color, t);
      }
      ctx.fillStyle = colorToCSS(color);
      ctx.fillRect(px, 0, 1, h);
    }
  }, [stops]);

  return <canvas ref={canvasRef} width={200} height={16} className="rounded" />;
}

// ── Section accordion ────────────────────────────────────────────

function Section({
  title,
  count,
  defaultOpen,
  onAdd,
  children,
}: {
  title: string;
  count: number;
  defaultOpen?: boolean;
  onAdd: () => void;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen ?? true);
  return (
    <div className="border-border border-b">
      <div
        className="hover:bg-surface-2 flex cursor-pointer select-none items-center gap-1 px-3 py-1.5"
        role="button"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <span className="text-text text-[11px] font-semibold">{title}</span>
        <span className="text-text-2 text-[10px]">({count})</span>
        <div className="flex-1" />
        <button
          className="text-text-2 hover:text-text p-0.5"
          title={`New ${title.slice(0, -1)}`}
          onClick={(e) => { e.stopPropagation(); onAdd(); }}
        >
          <Plus size={12} />
        </button>
      </div>
      {open && <div className="px-3 pb-2">{children}</div>}
    </div>
  );
}

// ── Inline editable name ─────────────────────────────────────────

function EditableName({
  value,
  onRename,
}: {
  value: string;
  onRename: (newName: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) inputRef.current?.select();
  }, [editing]);

  if (!editing) {
    return (
      <span
        className="text-text cursor-pointer truncate text-[11px]"
        onDoubleClick={() => { setDraft(value); setEditing(true); }}
        title="Double-click to rename"
      >
        {value}
      </span>
    );
  }

  return (
    <input
      ref={inputRef}
      className="border-primary bg-surface-2 text-text w-full rounded border px-1 py-0.5 text-[11px] outline-none"
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        setEditing(false);
        if (draft.trim() && draft.trim() !== value) onRename(draft.trim());
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          setEditing(false);
          if (draft.trim() && draft.trim() !== value) onRename(draft.trim());
        }
        if (e.key === "Escape") { setEditing(false); setDraft(value); }
      }}
      autoFocus
    />
  );
}

// ── Main LibraryPanel ────────────────────────────────────────────

export function LibraryPanel({ onLibraryChange, showVersion }: Props) {
  const { showError } = useToast();
  const [gradients, setGradients] = useState<[string, ColorGradient][]>([]);
  const [curves, setCurves] = useState<[string, Curve][]>([]);
  const [scripts, setScripts] = useState<string[]>([]);
  const [expandedGradient, setExpandedGradient] = useState<string | null>(null);
  const [expandedCurve, setExpandedCurve] = useState<string | null>(null);
  const [scriptEditor, setScriptEditor] = useState<{ name: string | null; source: string } | null>(null);
  const [curveDialog, setCurveDialog] = useState<{ name: string; points: CurvePoint[] } | null>(null);
  const [gradientDialog, setGradientDialog] = useState<{ name: string; stops: ColorStop[] } | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [g, c, sPairs] = await Promise.all([
        cmd.listGlobalGradients(),
        cmd.listGlobalCurves(),
        cmd.listGlobalScripts(),
      ]);
      setGradients(g);
      setCurves(c);
      setScripts(sPairs.map(([n]) => n));
    } catch (e) {
      console.error("[VibeLights] Library refresh failed:", e);
    }
  }, []);

  // Re-fetch when the panel mounts or after any show mutation (including undo/redo)
  useEffect(() => { void refresh(); }, [refresh, showVersion]);

  // ── Gradient actions ─────────────────────────────────────────

  const addGradient = useCallback(async () => {
    const existing = gradients.map(([n]) => n);
    let idx = 1;
    while (existing.includes(`gradient_${idx}`)) idx++;
    const name = `gradient_${idx}`;
    const gradient: ColorGradient = {
      stops: [
        { position: 0, color: { r: 255, g: 0, b: 0, a: 255 } },
        { position: 1, color: { r: 0, g: 0, b: 255, a: 255 } },
      ],
    };
    try {
      await cmd.setGlobalGradient(name, gradient);
      onLibraryChange();
      await refresh();
      setExpandedGradient(name);
    } catch (e) {
      showError(e);
    }
  }, [gradients, onLibraryChange, refresh]);

  const deleteGradient = useCallback(async (name: string) => {
    try {
      await cmd.deleteGlobalGradient(name);
      onLibraryChange();
      await refresh();
      if (expandedGradient === name) setExpandedGradient(null);
    } catch (e) {
      showError(e);
    }
  }, [onLibraryChange, refresh, expandedGradient]);

  const renameGradient = useCallback(async (oldName: string, newName: string) => {
    try {
      await cmd.renameGlobalGradient(oldName, newName);
      onLibraryChange();
      await refresh();
      if (expandedGradient === oldName) setExpandedGradient(newName);
    } catch (e) {
      showError(e);
    }
  }, [onLibraryChange, refresh, expandedGradient]);

  const updateGradient = useCallback(async (name: string, stops: ColorStop[]) => {
    try {
      await cmd.setGlobalGradient(name, { stops });
      onLibraryChange();
      await refresh();
    } catch (e) {
      showError(e);
    }
  }, [onLibraryChange, refresh]);

  // ── Curve actions ────────────────────────────────────────────

  const addCurve = useCallback(async () => {
    const existing = curves.map(([n]) => n);
    let idx = 1;
    while (existing.includes(`curve_${idx}`)) idx++;
    const name = `curve_${idx}`;
    const curve: Curve = {
      points: [
        { x: 0, y: 0 },
        { x: 1, y: 1 },
      ],
    };
    try {
      await cmd.setGlobalCurve(name, curve);
      onLibraryChange();
      await refresh();
      setExpandedCurve(name);
    } catch (e) {
      showError(e);
    }
  }, [curves, onLibraryChange, refresh]);

  const deleteCurve = useCallback(async (name: string) => {
    try {
      await cmd.deleteGlobalCurve(name);
      onLibraryChange();
      await refresh();
      if (expandedCurve === name) setExpandedCurve(null);
    } catch (e) {
      showError(e);
    }
  }, [onLibraryChange, refresh, expandedCurve]);

  const renameCurve = useCallback(async (oldName: string, newName: string) => {
    try {
      await cmd.renameGlobalCurve(oldName, newName);
      onLibraryChange();
      await refresh();
      if (expandedCurve === oldName) setExpandedCurve(newName);
    } catch (e) {
      showError(e);
    }
  }, [onLibraryChange, refresh, expandedCurve]);

  const updateCurve = useCallback(async (name: string, points: CurvePoint[]) => {
    try {
      await cmd.setGlobalCurve(name, { points });
      onLibraryChange();
      await refresh();
    } catch (e) {
      showError(e);
    }
  }, [onLibraryChange, refresh]);

  // ── Script actions ───────────────────────────────────────────

  const openNewScript = useCallback(() => {
    setScriptEditor({ name: null, source: "" });
  }, []);

  const openScript = useCallback(async (name: string) => {
    try {
      const source = await cmd.getGlobalScriptSource(name);
      setScriptEditor({ name, source });
    } catch (e) {
      showError(e);
    }
  }, []);

  const deleteScript = useCallback(async (name: string) => {
    try {
      await cmd.deleteGlobalScript(name);
      onLibraryChange();
      await refresh();
    } catch (e) {
      showError(e);
    }
  }, [onLibraryChange, refresh]);

  return (
    <>
      <div className="border-border bg-surface flex size-full shrink-0 flex-col border-l">
        <div className="flex-1 overflow-y-auto">
          {/* Gradients */}
          <Section title="Gradients" count={gradients.length} onAdd={() => { void addGradient(); }}>
            {gradients.length === 0 && (
              <div className="text-text-2 py-2 text-center text-[10px]">No gradients yet</div>
            )}
            {gradients.map(([name, gradient]) => (
              <div key={name} className="mt-1">
                <div className="flex items-center gap-1">
                  <button
                    className="flex min-w-0 flex-1 items-center gap-1.5"
                    onClick={() => setExpandedGradient(expandedGradient === name ? null : name)}
                  >
                    {expandedGradient === name ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                    <EditableName value={name} onRename={(n) => { void renameGradient(name, n); }} />
                  </button>
                  <button
                    className="text-text-2 hover:text-primary shrink-0 p-0.5"
                    onClick={() => setGradientDialog({ name, stops: gradient.stops })}
                    title="Expand editor"
                  >
                    <Maximize2 size={10} />
                  </button>
                  <button
                    className="text-text-2 hover:text-error shrink-0 p-0.5"
                    onClick={() => { void deleteGradient(name); }}
                    title="Delete"
                  >
                    <Trash2 size={10} />
                  </button>
                </div>
                <div className="mt-1">
                  <GradientPreview stops={gradient.stops} />
                </div>
                {expandedGradient === name && (
                  <div className="mt-1.5">
                    <GradientEditor
                      label=""
                      value={gradient.stops}
                      minStops={2}
                      maxStops={16}
                      onChange={(stops) => { void updateGradient(name, stops); }}
                    />
                  </div>
                )}
              </div>
            ))}
          </Section>

          {/* Curves */}
          <Section title="Curves" count={curves.length} onAdd={() => { void addCurve(); }}>
            {curves.length === 0 && (
              <div className="text-text-2 py-2 text-center text-[10px]">No curves yet</div>
            )}
            {curves.map(([name, curve]) => (
              <div key={name} className="mt-1">
                <div className="flex items-center gap-1">
                  <button
                    className="flex min-w-0 flex-1 items-center gap-1.5"
                    onClick={() => setExpandedCurve(expandedCurve === name ? null : name)}
                  >
                    {expandedCurve === name ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                    <EditableName value={name} onRename={(n) => { void renameCurve(name, n); }} />
                  </button>
                  <span className="text-text-2 text-[9px]">{curve.points.length} pts</span>
                  <button
                    className="text-text-2 hover:text-primary shrink-0 p-0.5"
                    onClick={() => setCurveDialog({ name, points: curve.points })}
                    title="Expand editor"
                  >
                    <Maximize2 size={10} />
                  </button>
                  <button
                    className="text-text-2 hover:text-error shrink-0 p-0.5"
                    onClick={() => { void deleteCurve(name); }}
                    title="Delete"
                  >
                    <Trash2 size={10} />
                  </button>
                </div>
                {expandedCurve === name && (
                  <div className="mt-1.5">
                    <CurveEditor
                      label=""
                      value={curve.points}
                      onChange={(pts) => { void updateCurve(name, pts); }}
                    />
                  </div>
                )}
              </div>
            ))}
          </Section>

          {/* Scripts */}
          <Section title="Scripts" count={scripts.length} onAdd={openNewScript}>
            {scripts.length === 0 && (
              <div className="text-text-2 py-2 text-center text-[10px]">No scripts yet</div>
            )}
            {scripts.map((name) => (
              <div key={name} className="mt-1 flex items-center gap-1">
                <button
                  className="text-text min-w-0 flex-1 truncate text-left text-[11px] hover:underline"
                  onClick={() => { void openScript(name); }}
                >
                  {name}
                </button>
                <button
                  className="text-text-2 hover:text-error shrink-0 p-0.5"
                  onClick={() => { void deleteScript(name); }}
                  title="Delete"
                >
                  <Trash2 size={10} />
                </button>
              </div>
            ))}
          </Section>
        </div>
      </div>

      {/* Script Editor Dialog */}
      {scriptEditor && (
        <ScriptEditorDialog
          scriptName={scriptEditor.name}
          initialSource={scriptEditor.source}
          onSaved={() => {
            setScriptEditor(null);
            onLibraryChange();
            void refresh();
          }}
          onCancel={() => setScriptEditor(null)}
        />
      )}

      {/* Curve Editor Dialog */}
      {curveDialog && (
        <CurveEditorDialog
          initialValue={curveDialog.points}
          onApply={(pts) => {
            void updateCurve(curveDialog.name, pts);
            setCurveDialog(null);
          }}
          onCancel={() => setCurveDialog(null)}
        />
      )}

      {/* Gradient Editor Dialog */}
      {gradientDialog && (
        <GradientEditorDialog
          initialValue={gradientDialog.stops}
          minStops={2}
          maxStops={16}
          onApply={(stops) => {
            void updateGradient(gradientDialog.name, stops);
            setGradientDialog(null);
          }}
          onCancel={() => setGradientDialog(null)}
        />
      )}
    </>
  );
}
