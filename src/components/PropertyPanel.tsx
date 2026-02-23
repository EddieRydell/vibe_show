import { useCallback, useEffect, useRef, useState } from "react";
import { cmd } from "../commands";
import { Link, Maximize2 } from "lucide-react";
import type { Color, ColorMode, ColorStop, CurvePoint, EffectDetail, ParamKey, ParamSchema, ParamValue } from "../types";
import { CurveEditorDialog } from "./CurveEditorDialog";
import { GradientEditorDialog } from "./GradientEditorDialog";
import { paramKeyStr, effectKindLabel } from "../types";
import { parseEffectKey } from "../utils/effectKey";
import { formatTimeDuration } from "../utils/formatTime";
import { useShowVersion } from "../hooks/useShowVersion";
import {
  FloatSlider,
  IntSlider,
  BoolToggle,
  ColorInput,
  ColorListEditor,
  CurveEditor,
  GradientEditor,
  SelectInput,
} from "./controls";

interface PropertyPanelProps {
  selectedEffect: string | null;
  sequenceIndex: number;
  onParamChange: () => void;
}

const PANEL_WIDTH = 260;
const DEBOUNCE_MS = 50;

function getParamFloat(params: Record<string, ParamValue>, key: string, fallback: number): number {
  const v = params[key];
  if (v && "Float" in v) return v.Float;
  return fallback;
}

function getParamInt(params: Record<string, ParamValue>, key: string, fallback: number): number {
  const v = params[key];
  if (v && "Int" in v) return v.Int;
  return fallback;
}

function getParamBool(params: Record<string, ParamValue>, key: string, fallback: boolean): boolean {
  const v = params[key];
  if (v && "Bool" in v) return v.Bool;
  return fallback;
}

function getParamColor(params: Record<string, ParamValue>, key: string, fallback: Color): Color {
  const v = params[key];
  if (v && "Color" in v) return v.Color;
  return fallback;
}

function getParamColorList(
  params: Record<string, ParamValue>,
  key: string,
  fallback: Color[],
): Color[] {
  const v = params[key];
  if (v && "ColorList" in v) return v.ColorList;
  return fallback;
}

function getDefaultFloat(schema: ParamSchema): number {
  if ("Float" in schema.default) return schema.default.Float;
  return 0;
}

function getDefaultInt(schema: ParamSchema): number {
  if ("Int" in schema.default) return schema.default.Int;
  return 0;
}

function getDefaultBool(schema: ParamSchema): boolean {
  if ("Bool" in schema.default) return schema.default.Bool;
  return false;
}

function getDefaultColor(schema: ParamSchema): Color {
  if ("Color" in schema.default) return schema.default.Color;
  return { r: 255, g: 255, b: 255, a: 255 };
}

function getDefaultColorList(schema: ParamSchema): Color[] {
  if ("ColorList" in schema.default) return schema.default.ColorList;
  return [
    { r: 255, g: 0, b: 0, a: 255 },
    { r: 0, g: 0, b: 255, a: 255 },
  ];
}

function getParamCurve(
  params: Record<string, ParamValue>,
  key: string,
  fallback: CurvePoint[],
): CurvePoint[] {
  const v = params[key];
  if (v && "Curve" in v) return v.Curve.points;
  return fallback;
}

function getDefaultCurve(schema: ParamSchema): CurvePoint[] {
  if ("Curve" in schema.default) return schema.default.Curve.points;
  return [
    { x: 0, y: 0 },
    { x: 1, y: 1 },
  ];
}

function getParamGradient(
  params: Record<string, ParamValue>,
  key: string,
  fallback: ColorStop[],
): ColorStop[] {
  const v = params[key];
  if (v && "ColorGradient" in v) return v.ColorGradient.stops;
  return fallback;
}

function getDefaultGradient(schema: ParamSchema): ColorStop[] {
  if ("ColorGradient" in schema.default) return schema.default.ColorGradient.stops;
  return [
    { position: 0, color: { r: 255, g: 255, b: 255, a: 255 } },
    { position: 1, color: { r: 255, g: 255, b: 255, a: 255 } },
  ];
}

function getParamText(params: Record<string, ParamValue>, key: string, fallback: string): string {
  const v = params[key];
  if (v && "Text" in v) return v.Text;
  return fallback;
}

function getDefaultText(schema: ParamSchema): string {
  if ("Text" in schema.default) return schema.default.Text;
  return "";
}

function getParamColorMode(params: Record<string, ParamValue>, key: string, fallback: string): string {
  const v = params[key];
  if (v && "ColorMode" in v) return v.ColorMode;
  return fallback;
}

function getDefaultColorMode(schema: ParamSchema): string {
  if ("ColorMode" in schema.default) return schema.default.ColorMode;
  // Derive from schema options rather than hardcoding a variant name.
  const pt = schema.param_type;
  if (typeof pt === "object" && "ColorMode" in pt) {
    const opts = pt.ColorMode.options;
    if (opts.length > 0) return opts[0];
  }
  return "";
}

export function PropertyPanel({
  selectedEffect,
  sequenceIndex,
  onParamChange,
}: PropertyPanelProps) {
  const showVersion = useShowVersion();
  const [detail, setDetail] = useState<EffectDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const currentKeyRef = useRef<string | null>(null);
  const [gradientNames, setGradientNames] = useState<string[]>([]);
  const [curveNames, setCurveNames] = useState<string[]>([]);
  const [expandedEditor, setExpandedEditor] = useState<{
    type: "curve" | "gradient";
    key: string;
    curveValue?: CurvePoint[];
    gradientValue?: ColorStop[];
    minStops?: number;
    maxStops?: number;
  } | null>(null);

  // Fetch library names for link dropdowns
  useEffect(() => {
    cmd.listLibraryGradients()
      .then((items) => setGradientNames(items.map(([n]) => n)))
      .catch(() => {});
    cmd.listLibraryCurves()
      .then((items) => setCurveNames(items.map(([n]) => n)))
      .catch(() => {});
  }, [selectedEffect]);

  // Load effect detail when selection changes or after undo/redo (refreshKey)
  useEffect(() => {
    currentKeyRef.current = selectedEffect;
    if (!selectedEffect) {
      setDetail(null);
      return;
    }
    const parsed = parseEffectKey(selectedEffect);
    if (!parsed) {
      setDetail(null);
      return;
    }

    setLoading(true);
    cmd.getEffectDetail(sequenceIndex, parsed.trackIndex, parsed.effectIndex)
      .then((d) => {
        if (currentKeyRef.current === selectedEffect) {
          setDetail(d);
        }
      })
      .catch((e) => console.error("[VibeLights] Failed to get effect detail:", e))
      .finally(() => setLoading(false));
  }, [selectedEffect, sequenceIndex, showVersion]);

  // Clear debounce timer on unmount to prevent firing after cleanup
  useEffect(() => {
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, []);

  const updateParam = useCallback(
    (key: string, value: ParamValue) => {
      if (!selectedEffect) return;
      const parsed = parseEffectKey(selectedEffect);
      if (!parsed) return;

      // Optimistic local update
      setDetail((prev) => {
        if (!prev) return prev;
        return { ...prev, params: { ...prev.params, [key]: value } };
      });

      // Debounced IPC call
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        cmd.updateEffectParam(parsed.trackIndex, parsed.effectIndex, key as ParamKey, value)
          .then(() => onParamChange())
          .catch((e) => console.error("[VibeLights] Failed to update param:", e));
      }, DEBOUNCE_MS);
    },
    [selectedEffect, sequenceIndex, onParamChange],
  );

  if (!selectedEffect) {
    return (
      <div
        className="border-border bg-surface flex shrink-0 flex-col items-center justify-center border-l"
        style={{ width: PANEL_WIDTH }}
      >
        <span className="text-text-2 text-[11px]">Select an effect</span>
      </div>
    );
  }

  if (loading || !detail) {
    return (
      <div
        className="border-border bg-surface flex shrink-0 flex-col items-center justify-center border-l"
        style={{ width: PANEL_WIDTH }}
      >
        <span className="text-text-2 text-[11px]">Loading...</span>
      </div>
    );
  }

  return (
    <div
      className="border-border bg-surface flex shrink-0 flex-col border-l"
      style={{ width: PANEL_WIDTH }}
    >
      {/* Header */}
      <div className="border-border border-b px-3 py-2">
        <div className="text-text text-xs font-semibold">{effectKindLabel(detail.kind)}</div>
        <div className="text-text-2 mt-0.5 text-[10px]">
          {detail.track_name} &middot; {formatTimeDuration(detail.time_range.start)} -{" "}
          {formatTimeDuration(detail.time_range.end)}
        </div>
        {detail.blend_mode !== "Override" && (
          <div className="text-text-2 mt-0.5 text-[10px]">Blend: {detail.blend_mode}</div>
        )}
        {detail.opacity < 1.0 && (
          <div className="text-text-2 mt-0.5 text-[10px]">Opacity: {Math.round(detail.opacity * 100)}%</div>
        )}
      </div>

      {/* Parameter controls */}
      <div className="flex-1 overflow-y-auto px-3 py-2">
        <div className="flex flex-col gap-3">
          {detail.schema.map((schema) => (
            <ParamControl
              key={paramKeyStr(schema.key)}
              schema={schema}
              params={detail.params}
              onChange={(value) => updateParam(paramKeyStr(schema.key), value)}
              gradientNames={gradientNames}
              curveNames={curveNames}
              onExpandCurve={(k, v) => setExpandedEditor({ type: "curve", key: k, curveValue: v })}
              onExpandGradient={(k, v, min, max) => setExpandedEditor({ type: "gradient", key: k, gradientValue: v, minStops: min, maxStops: max })}
            />
          ))}
        </div>
      </div>

      {/* Expanded editor dialogs */}
      {expandedEditor?.type === "curve" && expandedEditor.curveValue && (
        <CurveEditorDialog
          initialValue={expandedEditor.curveValue}
          onApply={(v) => {
            updateParam(expandedEditor.key, { Curve: { points: v } });
            setExpandedEditor(null);
          }}
          onCancel={() => setExpandedEditor(null)}
        />
      )}
      {expandedEditor?.type === "gradient" && expandedEditor.gradientValue && (
        <GradientEditorDialog
          initialValue={expandedEditor.gradientValue}
          minStops={expandedEditor.minStops ?? 2}
          maxStops={expandedEditor.maxStops ?? 16}
          onApply={(v) => {
            updateParam(expandedEditor.key, { ColorGradient: { stops: v } });
            setExpandedEditor(null);
          }}
          onCancel={() => setExpandedEditor(null)}
        />
      )}
    </div>
  );
}

// ── LinkButton ────────────────────────────────────────────────────

function LinkButton({ items, onLink }: { items: string[]; onLink: (name: string) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  if (items.length === 0) return null;

  return (
    <div ref={ref} className="relative">
      <button
        className="text-text-2 hover:text-primary p-0.5"
        title="Link to library item"
        onClick={() => setOpen((o) => !o)}
      >
        <Link size={10} />
      </button>
      {open && (
        <div className="bg-surface border-border absolute right-0 top-full z-20 mt-1 min-w-[120px] rounded border py-0.5 shadow-lg">
          {items.map((name) => (
            <button
              key={name}
              className="text-text hover:bg-surface-2 block w-full px-3 py-1 text-left text-[11px]"
              onClick={() => { onLink(name); setOpen(false); }}
            >
              {name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

interface ParamControlProps {
  schema: ParamSchema;
  params: Record<string, ParamValue>;
  onChange: (value: ParamValue) => void;
  gradientNames: string[];
  curveNames: string[];
  onExpandCurve?: (key: string, value: CurvePoint[]) => void;
  onExpandGradient?: (key: string, value: ColorStop[], minStops: number, maxStops: number) => void;
}

function RefBadge({ label, name, onUnlink }: { label: string; name: string; onUnlink: () => void }) {
  return (
    <div>
      <label className="text-text-2 mb-0.5 block text-[10px] font-medium">{label}</label>
      <div className="bg-surface-2 border-border flex items-center gap-1.5 rounded border px-2 py-1">
        <span className="bg-primary/15 text-primary rounded px-1 py-0.5 text-[9px] font-semibold">
          {name}
        </span>
        <button
          className="text-text-2 hover:text-text ml-auto text-[10px]"
          onClick={onUnlink}
          title="Unlink from library"
        >
          unlink
        </button>
      </div>
    </div>
  );
}

function ParamControl({ schema, params, onChange, gradientNames, curveNames, onExpandCurve, onExpandGradient }: ParamControlProps) {
  const pt = schema.param_type;

  // Check for library references — display name badge instead of inline editor
  const keyStr = paramKeyStr(schema.key);
  const rawValue = params[keyStr];
  if (rawValue && typeof rawValue === "object") {
    if ("GradientRef" in rawValue) {
      return (
        <RefBadge
          label={schema.label}
          name={rawValue.GradientRef}
          onUnlink={() => {
            // Unlink: replace ref with a default inline gradient
            const fallback = getDefaultGradient(schema);
            onChange({ ColorGradient: { stops: fallback } });
          }}
        />
      );
    }
    if ("CurveRef" in rawValue) {
      return (
        <RefBadge
          label={schema.label}
          name={rawValue.CurveRef}
          onUnlink={() => {
            // Unlink: replace ref with a default inline curve
            const fallback = getDefaultCurve(schema);
            onChange({ Curve: { points: fallback } });
          }}
        />
      );
    }
  }

  if (typeof pt === "object" && "Float" in pt) {
    const value = getParamFloat(params, keyStr, getDefaultFloat(schema));
    return (
      <FloatSlider
        label={schema.label}
        value={value}
        min={pt.Float.min}
        max={pt.Float.max}
        step={pt.Float.step}
        onChange={(v) => onChange({ Float: v })}
      />
    );
  }

  if (typeof pt === "object" && "Int" in pt) {
    const value = getParamInt(params, keyStr, getDefaultInt(schema));
    return (
      <IntSlider
        label={schema.label}
        value={value}
        min={pt.Int.min}
        max={pt.Int.max}
        onChange={(v) => onChange({ Int: v })}
      />
    );
  }

  if (pt === "Bool") {
    const value = getParamBool(params, keyStr, getDefaultBool(schema));
    return (
      <BoolToggle label={schema.label} value={value} onChange={(v) => onChange({ Bool: v })} />
    );
  }

  if (pt === "Color") {
    const value = getParamColor(params, keyStr, getDefaultColor(schema));
    return (
      <ColorInput label={schema.label} value={value} onChange={(v) => onChange({ Color: v })} />
    );
  }

  if (typeof pt === "object" && "ColorList" in pt) {
    const value = getParamColorList(params, keyStr, getDefaultColorList(schema));
    return (
      <ColorListEditor
        label={schema.label}
        value={value}
        minColors={pt.ColorList.min_colors}
        maxColors={pt.ColorList.max_colors}
        onChange={(v) => onChange({ ColorList: v })}
      />
    );
  }

  if (pt === "Curve") {
    const value = getParamCurve(params, keyStr, getDefaultCurve(schema));
    return (
      <div>
        <div className="mb-0.5 flex items-center justify-between">
          <span className="text-text-2 text-[11px]">{schema.label}</span>
          <div className="flex items-center gap-1">
            <button
              className="text-text-2 hover:text-primary p-0.5"
              title="Expand editor"
              onClick={() => onExpandCurve?.(keyStr, value)}
            >
              <Maximize2 size={10} />
            </button>
            <LinkButton items={curveNames} onLink={(name) => onChange({ CurveRef: name })} />
          </div>
        </div>
        <CurveEditor
          label=""
          value={value}
          onChange={(v) => onChange({ Curve: { points: v } })}
        />
      </div>
    );
  }

  if (typeof pt === "object" && "ColorGradient" in pt) {
    const value = getParamGradient(params, keyStr, getDefaultGradient(schema));
    return (
      <div>
        <div className="mb-0.5 flex items-center justify-between">
          <span className="text-text-2 text-[11px]">{schema.label}</span>
          <div className="flex items-center gap-1">
            <button
              className="text-text-2 hover:text-primary p-0.5"
              title="Expand editor"
              onClick={() => onExpandGradient?.(keyStr, value, pt.ColorGradient.min_stops, pt.ColorGradient.max_stops)}
            >
              <Maximize2 size={10} />
            </button>
            <LinkButton items={gradientNames} onLink={(name) => onChange({ GradientRef: name })} />
          </div>
        </div>
        <GradientEditor
          label=""
          value={value}
          minStops={pt.ColorGradient.min_stops}
          maxStops={pt.ColorGradient.max_stops}
          onChange={(v) => onChange({ ColorGradient: { stops: v } })}
        />
      </div>
    );
  }

  if (typeof pt === "object" && "ColorMode" in pt) {
    const value = getParamColorMode(params, keyStr, getDefaultColorMode(schema));
    return (
      <SelectInput
        label={schema.label}
        value={value}
        options={pt.ColorMode.options}
        onChange={(v) => onChange({ ColorMode: v as ColorMode })}
      />
    );
  }

  if (typeof pt === "object" && "Text" in pt) {
    const value = getParamText(params, keyStr, getDefaultText(schema));
    return (
      <SelectInput
        label={schema.label}
        value={value}
        options={pt.Text.options}
        onChange={(v) => onChange({ Text: v })}
      />
    );
  }

  return null;
}
