import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Color, EffectDetail, ParamSchema, ParamValue } from "../types";
import { FloatSlider, IntSlider, BoolToggle, ColorInput, ColorListEditor } from "./controls";

interface PropertyPanelProps {
  selectedEffect: string | null;
  sequenceIndex: number;
  onParamChange: () => void;
}

const PANEL_WIDTH = 260;
const DEBOUNCE_MS = 50;

function parseEffectKey(key: string): { trackIndex: number; effectIndex: number } | null {
  const parts = key.split("-");
  if (parts.length !== 2) return null;
  const trackIndex = parseInt(parts[0], 10);
  const effectIndex = parseInt(parts[1], 10);
  if (isNaN(trackIndex) || isNaN(effectIndex)) return null;
  return { trackIndex, effectIndex };
}

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

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = (seconds % 60).toFixed(1);
  return m > 0 ? `${m}:${s.padStart(4, "0")}` : `${s}s`;
}

export function PropertyPanel({
  selectedEffect,
  sequenceIndex,
  onParamChange,
}: PropertyPanelProps) {
  const [detail, setDetail] = useState<EffectDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const currentKeyRef = useRef<string | null>(null);

  // Load effect detail when selection changes
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
    invoke<EffectDetail | null>("get_effect_detail", {
      sequenceIndex,
      trackIndex: parsed.trackIndex,
      effectIndex: parsed.effectIndex,
    })
      .then((d) => {
        if (currentKeyRef.current === selectedEffect) {
          setDetail(d);
        }
      })
      .catch((e) => console.error("[VibeShow] Failed to get effect detail:", e))
      .finally(() => setLoading(false));
  }, [selectedEffect, sequenceIndex]);

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
        invoke("update_effect_param", {
          sequenceIndex,
          trackIndex: parsed.trackIndex,
          effectIndex: parsed.effectIndex,
          key,
          value,
        })
          .then(() => onParamChange())
          .catch((e) => console.error("[VibeShow] Failed to update param:", e));
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
        <div className="text-text text-xs font-semibold">{detail.kind}</div>
        <div className="text-text-2 mt-0.5 text-[10px]">
          {detail.track_name} &middot; {formatTime(detail.time_range.start)} -{" "}
          {formatTime(detail.time_range.end)}
        </div>
        {detail.blend_mode !== "Override" && (
          <div className="text-text-2 mt-0.5 text-[10px]">Blend: {detail.blend_mode}</div>
        )}
      </div>

      {/* Parameter controls */}
      <div className="flex-1 overflow-y-auto px-3 py-2">
        <div className="flex flex-col gap-3">
          {detail.schema.map((schema) => (
            <ParamControl
              key={schema.key}
              schema={schema}
              params={detail.params}
              onChange={(value) => updateParam(schema.key, value)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

interface ParamControlProps {
  schema: ParamSchema;
  params: Record<string, ParamValue>;
  onChange: (value: ParamValue) => void;
}

function ParamControl({ schema, params, onChange }: ParamControlProps) {
  const pt = schema.param_type;

  if (typeof pt === "object" && "Float" in pt) {
    const value = getParamFloat(params, schema.key, getDefaultFloat(schema));
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
    const value = getParamInt(params, schema.key, getDefaultInt(schema));
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
    const value = getParamBool(params, schema.key, getDefaultBool(schema));
    return (
      <BoolToggle label={schema.label} value={value} onChange={(v) => onChange({ Bool: v })} />
    );
  }

  if (pt === "Color") {
    const value = getParamColor(params, schema.key, getDefaultColor(schema));
    return (
      <ColorInput label={schema.label} value={value} onChange={(v) => onChange({ Color: v })} />
    );
  }

  if (typeof pt === "object" && "ColorList" in pt) {
    const value = getParamColorList(params, schema.key, getDefaultColorList(schema));
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

  return null;
}
