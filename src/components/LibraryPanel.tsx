import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronRight, Maximize2, Plus, Trash2 } from "lucide-react";
import { cmd } from "../commands";
import type { Color, ColorGradient, ColorStop, Curve, CurvePoint } from "../types";
import { GradientEditor } from "./controls/GradientEditor";
import { CurveEditor } from "./controls/CurveEditor";
import { ScriptEditorDialog } from "./ScriptEditorDialog";
import { CurveEditorDialog } from "./CurveEditorDialog";
import { GradientEditorDialog } from "./GradientEditorDialog";
import { useShowVersion } from "../hooks/useShowVersion";

interface Props {
  onClose: () => void;
  onLibraryChange: () => void;
}

const PANEL_WIDTH = 260;

// ── Gradient preview canvas ──────────────────────────────────────

function colorToCSS(c: Color): string {
  return `rgb(${c.r},${c.g},${c.b})`;
}

function lerpColor(a: Color, b: Color, t: number): Color {
  return {
    r: Math.round(a.r + (b.r - a.r) * t),
    g: Math.round(a.g + (b.g - a.g) * t),
    b: Math.round(a.b + (b.b - a.b) * t),
    a: 255,
  };
}

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
        color = sorted[0].color;
      } else if (pos <= sorted[0].position) {
        color = sorted[0].color;
      } else if (pos >= sorted[sorted.length - 1].position) {
        color = sorted[sorted.length - 1].color;
      } else {
        let idx = 0;
        for (let i = 1; i < sorted.length; i++) {
          if (sorted[i].position >= pos) { idx = i; break; }
        }
        const a = sorted[idx - 1];
        const b = sorted[idx];
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

export function LibraryPanel({ onClose, onLibraryChange }: Props) {
  const showVersion = useShowVersion();
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
      const [g, c, s] = await Promise.all([
        cmd.listLibraryGradients(),
        cmd.listLibraryCurves(),
        cmd.listScripts(),
      ]);
      setGradients(g);
      setCurves(c);
      setScripts(s);
    } catch (e) {
      console.error("[VibeLights] Library refresh failed:", e);
    }
  }, []);

  // Re-fetch when the panel mounts or after any show mutation (including undo/redo)
  useEffect(() => { refresh(); }, [refresh, showVersion]);

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
      await cmd.setLibraryGradient(name, gradient.stops);
      onLibraryChange();
      refresh();
      setExpandedGradient(name);
    } catch (e) {
      console.error("[VibeLights] Add gradient failed:", e);
    }
  }, [gradients, onLibraryChange, refresh]);

  const deleteGradient = useCallback(async (name: string) => {
    try {
      await cmd.deleteLibraryGradient(name);
      onLibraryChange();
      refresh();
      if (expandedGradient === name) setExpandedGradient(null);
    } catch (e) {
      console.error("[VibeLights] Delete gradient failed:", e);
    }
  }, [onLibraryChange, refresh, expandedGradient]);

  const renameGradient = useCallback(async (oldName: string, newName: string) => {
    try {
      await cmd.renameLibraryGradient(oldName, newName);
      onLibraryChange();
      refresh();
      if (expandedGradient === oldName) setExpandedGradient(newName);
    } catch (e) {
      console.error("[VibeLights] Rename gradient failed:", e);
    }
  }, [onLibraryChange, refresh, expandedGradient]);

  const updateGradient = useCallback(async (name: string, stops: ColorStop[]) => {
    try {
      await cmd.setLibraryGradient(name, stops);
      onLibraryChange();
      refresh();
    } catch (e) {
      console.error("[VibeLights] Update gradient failed:", e);
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
      await cmd.setLibraryCurve(name, curve.points);
      onLibraryChange();
      refresh();
      setExpandedCurve(name);
    } catch (e) {
      console.error("[VibeLights] Add curve failed:", e);
    }
  }, [curves, onLibraryChange, refresh]);

  const deleteCurve = useCallback(async (name: string) => {
    try {
      await cmd.deleteLibraryCurve(name);
      onLibraryChange();
      refresh();
      if (expandedCurve === name) setExpandedCurve(null);
    } catch (e) {
      console.error("[VibeLights] Delete curve failed:", e);
    }
  }, [onLibraryChange, refresh, expandedCurve]);

  const renameCurve = useCallback(async (oldName: string, newName: string) => {
    try {
      await cmd.renameLibraryCurve(oldName, newName);
      onLibraryChange();
      refresh();
      if (expandedCurve === oldName) setExpandedCurve(newName);
    } catch (e) {
      console.error("[VibeLights] Rename curve failed:", e);
    }
  }, [onLibraryChange, refresh, expandedCurve]);

  const updateCurve = useCallback(async (name: string, points: CurvePoint[]) => {
    try {
      await cmd.setLibraryCurve(name, points);
      onLibraryChange();
      refresh();
    } catch (e) {
      console.error("[VibeLights] Update curve failed:", e);
    }
  }, [onLibraryChange, refresh]);

  // ── Script actions ───────────────────────────────────────────

  const openNewScript = useCallback(() => {
    setScriptEditor({ name: null, source: "" });
  }, []);

  const openScript = useCallback(async (name: string) => {
    try {
      const source = await cmd.getScriptSource(name);
      setScriptEditor({ name, source: source ?? "" });
    } catch (e) {
      console.error("[VibeLights] Get script source failed:", e);
    }
  }, []);

  const deleteScript = useCallback(async (name: string) => {
    try {
      await cmd.deleteScript(name);
      onLibraryChange();
      refresh();
    } catch (e) {
      console.error("[VibeLights] Delete script failed:", e);
    }
  }, [onLibraryChange, refresh]);

  return (
    <>
      <div
        className="border-border bg-surface flex shrink-0 flex-col border-l"
        style={{ width: PANEL_WIDTH }}
      >
        {/* Header */}
        <div className="border-border flex items-center justify-between border-b px-3 py-2">
          <span className="text-text text-xs font-semibold">Library</span>
          <button onClick={onClose} className="text-text-2 hover:text-text text-[11px]">
            &times;
          </button>
        </div>

        <div className="flex-1 overflow-y-auto">
          {/* Gradients */}
          <Section title="Gradients" count={gradients.length} onAdd={addGradient}>
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
                    <EditableName value={name} onRename={(n) => renameGradient(name, n)} />
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
                    onClick={() => deleteGradient(name)}
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
                      onChange={(stops) => updateGradient(name, stops)}
                    />
                  </div>
                )}
              </div>
            ))}
          </Section>

          {/* Curves */}
          <Section title="Curves" count={curves.length} onAdd={addCurve}>
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
                    <EditableName value={name} onRename={(n) => renameCurve(name, n)} />
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
                    onClick={() => deleteCurve(name)}
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
                      onChange={(pts) => updateCurve(name, pts)}
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
                  onClick={() => openScript(name)}
                >
                  {name}
                </button>
                <button
                  className="text-text-2 hover:text-error shrink-0 p-0.5"
                  onClick={() => deleteScript(name)}
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
            refresh();
          }}
          onCancel={() => setScriptEditor(null)}
        />
      )}

      {/* Curve Editor Dialog */}
      {curveDialog && (
        <CurveEditorDialog
          initialValue={curveDialog.points}
          onApply={(pts) => {
            updateCurve(curveDialog.name, pts);
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
            updateGradient(gradientDialog.name, stops);
            setGradientDialog(null);
          }}
          onCancel={() => setGradientDialog(null)}
        />
      )}
    </>
  );
}
