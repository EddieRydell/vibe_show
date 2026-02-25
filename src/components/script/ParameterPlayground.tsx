import { useCallback } from "react";
import type { ParamValue, ScriptParams, ScriptParamInfo } from "../../types";
import {
  FloatSlider,
  IntSlider,
  BoolToggle,
  ColorInput,
  GradientEditor,
  CurveEditor,
  SelectInput,
} from "../controls";

interface Props {
  params: ScriptParamInfo[];
  values: ScriptParams;
  onChange: (values: ScriptParams) => void;
}

export function ParameterPlayground({ params, values, onChange }: Props) {
  const updateParam = useCallback(
    (name: string, value: ParamValue) => {
      onChange({ ...values, [name]: value });
    },
    [values, onChange],
  );

  const handleReset = useCallback(() => {
    const defaults: ScriptParams = {};
    for (const p of params) {
      if (p.default != null) {
        defaults[p.name] = p.default;
      }
    }
    onChange(defaults);
  }, [params, onChange]);

  if (params.length === 0) {
    return (
      <div className="text-text-2 px-3 py-2 text-[10px]">No parameters declared</div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <span className="text-text-2 text-[10px] font-medium">Parameters</span>
        <button
          onClick={handleReset}
          className="text-text-2 hover:text-text text-[10px] underline"
        >
          Reset
        </button>
      </div>
      {params.map((p) => (
        <ParamControl
          key={p.name}
          info={p}
          value={values[p.name] ?? p.default}
          onChange={(v) => updateParam(p.name, v)}
        />
      ))}
    </div>
  );
}

function ParamControl({
  info,
  value,
  onChange,
}: {
  info: ScriptParamInfo;
  value: ParamValue | null;
  onChange: (v: ParamValue) => void;
}) {
  const pt = info.param_type;

  if (typeof pt === "object" && "Float" in pt) {
    const v = value && "Float" in value ? value.Float : pt.Float.min;
    return (
      <FloatSlider
        label={info.name}
        value={v}
        min={pt.Float.min}
        max={pt.Float.max}
        step={pt.Float.step}
        onChange={(n) => onChange({ Float: n })}
      />
    );
  }

  if (typeof pt === "object" && "Int" in pt) {
    const v = value && "Int" in value ? value.Int : pt.Int.min;
    return (
      <IntSlider
        label={info.name}
        value={v}
        min={pt.Int.min}
        max={pt.Int.max}
        onChange={(n) => onChange({ Int: n })}
      />
    );
  }

  if (pt === "Bool") {
    const v = value && "Bool" in value ? value.Bool : false;
    return (
      <BoolToggle label={info.name} value={v} onChange={(b) => onChange({ Bool: b })} />
    );
  }

  if (pt === "Color") {
    const v =
      value && "Color" in value
        ? value.Color
        : { r: 255, g: 255, b: 255, a: 255 };
    return (
      <ColorInput label={info.name} value={v} onChange={(c) => onChange({ Color: c })} />
    );
  }

  if (typeof pt === "object" && "ColorGradient" in pt) {
    const v =
      value && "ColorGradient" in value
        ? value.ColorGradient.stops
        : [
            { position: 0, color: { r: 0, g: 0, b: 0, a: 255 } },
            { position: 1, color: { r: 255, g: 255, b: 255, a: 255 } },
          ];
    return (
      <GradientEditor
        label={info.name}
        value={v}
        minStops={pt.ColorGradient.min_stops}
        maxStops={pt.ColorGradient.max_stops}
        onChange={(stops) => onChange({ ColorGradient: { stops } })}
      />
    );
  }

  if (pt === "Curve") {
    const v =
      value && "Curve" in value
        ? value.Curve.points
        : [
            { x: 0, y: 0 },
            { x: 1, y: 1 },
          ];
    return (
      <CurveEditor
        label={info.name}
        value={v}
        onChange={(points) => onChange({ Curve: { points } })}
      />
    );
  }

  if (typeof pt === "object" && "Enum" in pt) {
    const v = value && "EnumVariant" in value ? value.EnumVariant : pt.Enum.options[0] ?? "";
    return (
      <SelectInput
        label={info.name}
        value={v}
        options={pt.Enum.options}
        onChange={(s) => onChange({ EnumVariant: s })}
      />
    );
  }

  return (
    <div className="text-text-2 text-[10px]">
      {info.name}: unsupported type
    </div>
  );
}
