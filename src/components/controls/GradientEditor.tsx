import { useRef, useCallback, useEffect } from "react";
import type { Color, ColorStop } from "../../types";
import { ColorInput } from "./ColorInput";

interface GradientEditorProps {
  label: string;
  value: ColorStop[];
  minStops: number;
  maxStops: number;
  onChange: (value: ColorStop[]) => void;
}

function sortByPosition(stops: ColorStop[]): ColorStop[] {
  return [...stops].sort((a, b) => a.position - b.position);
}

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

const BAR_W = 220;
const BAR_H = 24;

export function GradientEditor({
  label,
  value,
  minStops,
  maxStops,
  onChange,
}: GradientEditorProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const canAdd = value.length < maxStops;
  const canRemove = value.length > minStops;

  const drawGradient = useCallback(() => {
    const ctx = canvasRef.current?.getContext("2d");
    if (!ctx) return;

    const sorted = sortByPosition(value);
    if (sorted.length === 0) return;

    // Draw checkerboard for transparency indication
    ctx.fillStyle = "#1a1a2e";
    ctx.fillRect(0, 0, BAR_W, BAR_H);

    // Draw gradient pixel by pixel for accurate linear RGB interpolation
    for (let px = 0; px < BAR_W; px++) {
      const pos = px / (BAR_W - 1);

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
          if (sorted[i].position >= pos) {
            idx = i;
            break;
          }
        }
        const a = sorted[idx - 1];
        const b = sorted[idx];
        const dp = b.position - a.position;
        const t = dp > 0 ? (pos - a.position) / dp : 0;
        color = lerpColor(a.color, b.color, t);
      }

      ctx.fillStyle = colorToCSS(color);
      ctx.fillRect(px, 0, 1, BAR_H);
    }

    // Draw stop markers
    ctx.strokeStyle = "#fff";
    ctx.lineWidth = 1;
    for (const stop of sorted) {
      const x = stop.position * (BAR_W - 1);
      ctx.beginPath();
      ctx.moveTo(x, BAR_H - 6);
      ctx.lineTo(x - 3, BAR_H);
      ctx.lineTo(x + 3, BAR_H);
      ctx.closePath();
      ctx.fillStyle = "#fff";
      ctx.fill();
    }

    // Border
    ctx.strokeStyle = "rgba(255,255,255,0.2)";
    ctx.lineWidth = 1;
    ctx.strokeRect(0, 0, BAR_W, BAR_H);
  }, [value]);

  useEffect(() => {
    drawGradient();
  }, [drawGradient]);

  const updatePosition = (index: number, raw: number) => {
    const pos = Math.min(1, Math.max(0, raw));
    const next = value.map((s, i) => (i === index ? { ...s, position: pos } : s));
    onChange(sortByPosition(next));
  };

  const updateColor = (index: number, color: Color) => {
    const next = value.map((s, i) => (i === index ? { ...s, color } : s));
    onChange(next);
  };

  const addStop = () => {
    const newStop: ColorStop = {
      position: 0.5,
      color: { r: 255, g: 255, b: 255, a: 255 },
    };
    onChange(sortByPosition([...value, newStop]));
  };

  const removeStop = (index: number) => {
    onChange(value.filter((_, i) => i !== index));
  };

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <label className="text-text-2 text-[11px]">{label}</label>
        {canAdd && (
          <button
            type="button"
            className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-1.5 py-0.5 text-[10px]"
            onClick={addStop}
          >
            + Stop
          </button>
        )}
      </div>

      {/* Visual gradient preview */}
      <canvas ref={canvasRef} width={BAR_W} height={BAR_H} className="rounded" />

      {/* Stop list */}
      <div className="flex max-h-32 flex-col gap-1 overflow-y-auto">
        {value.map((stop, i) => (
          <div key={i} className="flex items-center gap-1">
            <input
              type="range"
              className="accent-primary h-1 w-14 cursor-pointer"
              value={stop.position}
              min={0}
              max={1}
              step={0.01}
              onChange={(e) => {
                const v = parseFloat(e.target.value);
                if (!isNaN(v)) updatePosition(i, v);
              }}
            />
            <span className="text-text-2 w-7 text-center font-mono text-[9px]">
              {(stop.position * 100).toFixed(0)}%
            </span>
            <div className="flex-1">
              <ColorInput label="" value={stop.color} onChange={(c) => updateColor(i, c)} />
            </div>
            {canRemove && (
              <button
                type="button"
                className="text-text-2 hover:text-error text-[10px]"
                onClick={() => removeStop(i)}
              >
                x
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
