import { useRef, useCallback, useEffect, useState } from "react";
import type { Color, ColorStop } from "../../types";
import { ColorInput } from "./ColorInput";
import { GRADIENT_PRESETS } from "../../constants";

interface GradientEditorProps {
  label: string;
  value: ColorStop[];
  minStops: number;
  maxStops: number;
  onChange: (value: ColorStop[]) => void;
  width?: number;
  height?: number;
  expanded?: boolean;
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

function sampleGradientAt(stops: ColorStop[], pos: number): Color {
  const sorted = sortByPosition(stops);
  if (sorted.length === 0) return { r: 255, g: 255, b: 255, a: 255 };
  if (sorted.length === 1) return sorted[0].color;
  if (pos <= sorted[0].position) return sorted[0].color;
  if (pos >= sorted[sorted.length - 1].position)
    return sorted[sorted.length - 1].color;
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
  return lerpColor(a.color, b.color, t);
}

const MARKER_H = 10;

export function GradientEditor({
  label,
  value,
  minStops,
  maxStops,
  onChange,
  width: barW = 220,
  height: barH = 24,
  expanded = false,
}: GradientEditorProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [selectedStop, setSelectedStop] = useState<number | null>(null);
  const [draggingStop, setDraggingStop] = useState<number | null>(null);
  const canAdd = value.length < maxStops;
  const canRemove = value.length > minStops;

  const totalH = barH + MARKER_H;

  const drawGradient = useCallback(() => {
    const ctx = canvasRef.current?.getContext("2d");
    if (!ctx) return;

    const sorted = sortByPosition(value);
    if (sorted.length === 0) return;

    ctx.clearRect(0, 0, barW, totalH);

    // Background
    ctx.fillStyle = "#1a1a2e";
    ctx.fillRect(0, 0, barW, barH);

    // Draw gradient pixel by pixel
    for (let px = 0; px < barW; px++) {
      const pos = px / (barW - 1);
      const color = sampleGradientAt(sorted, pos);
      ctx.fillStyle = colorToCSS(color);
      ctx.fillRect(px, 0, 1, barH);
    }

    // Border
    ctx.strokeStyle = "rgba(255,255,255,0.2)";
    ctx.lineWidth = 1;
    ctx.strokeRect(0, 0, barW, barH);

    // Draw stop markers (triangles below bar)
    for (let i = 0; i < sorted.length; i++) {
      const x = sorted[i].position * (barW - 1);
      const isSelected = selectedStop === i;
      const isDragging = draggingStop === i;

      ctx.beginPath();
      ctx.moveTo(x, barH);
      ctx.lineTo(x - 5, barH + MARKER_H);
      ctx.lineTo(x + 5, barH + MARKER_H);
      ctx.closePath();

      if (isSelected || isDragging) {
        ctx.fillStyle = "#60a5fa";
        ctx.strokeStyle = "#fff";
      } else {
        ctx.fillStyle = colorToCSS(sorted[i].color);
        ctx.strokeStyle = "#fff";
      }
      ctx.fill();
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
  }, [value, barW, barH, totalH, selectedStop, draggingStop]);

  useEffect(() => {
    drawGradient();
  }, [drawGradient]);

  const getMousePos = (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return { cx: 0, cy: 0 };
    return { cx: e.clientX - rect.left, cy: e.clientY - rect.top };
  };

  const hitTestMarker = useCallback(
    (cx: number, cy: number): number | null => {
      if (cy < barH - 2) return null;
      const sorted = sortByPosition(value);
      for (let i = 0; i < sorted.length; i++) {
        const mx = sorted[i].position * (barW - 1);
        if (Math.abs(cx - mx) < 8 && cy >= barH - 2) return i;
      }
      return null;
    },
    [value, barW, barH],
  );

  const onMouseDown = (e: React.MouseEvent) => {
    const { cx, cy } = getMousePos(e);
    const hit = hitTestMarker(cx, cy);
    if (hit !== null) {
      setSelectedStop(hit);
      setDraggingStop(hit);
    } else if (cy < barH && canAdd) {
      // Click on gradient bar = add a new stop
      const pos = Math.max(0, Math.min(1, cx / (barW - 1)));
      const color = sampleGradientAt(value, pos);
      const newStops = sortByPosition([...value, { position: pos, color }]);
      onChange(newStops);
      const newIdx = newStops.findIndex((s) => s.position === pos);
      setSelectedStop(newIdx >= 0 ? newIdx : null);
    }
  };

  const onMouseMove = (e: React.MouseEvent) => {
    if (draggingStop === null) return;
    const { cx } = getMousePos(e);
    const pos = Math.max(0, Math.min(1, cx / (barW - 1)));
    const sorted = sortByPosition(value);
    const next = sorted.map((s, i) =>
      i === draggingStop ? { ...s, position: pos } : s,
    );
    const resorted = sortByPosition(next);
    onChange(resorted);
    const newIdx = resorted.findIndex(
      (s) => Math.abs(s.position - pos) < 0.001,
    );
    if (newIdx >= 0) {
      setDraggingStop(newIdx);
      setSelectedStop(newIdx);
    }
  };

  const onMouseUp = () => {
    setDraggingStop(null);
  };

  const onDoubleClick = (e: React.MouseEvent) => {
    if (!canRemove) return;
    const { cx, cy } = getMousePos(e);
    const hit = hitTestMarker(cx, cy);
    if (hit !== null) {
      const sorted = sortByPosition(value);
      onChange(sorted.filter((_, i) => i !== hit));
      setSelectedStop(null);
    }
  };

  const updateSelectedColor = (color: Color) => {
    if (selectedStop === null) return;
    const sorted = sortByPosition(value);
    const next = sorted.map((s, i) =>
      i === selectedStop ? { ...s, color } : s,
    );
    onChange(next);
  };

  const updateSelectedPosition = (pos: number) => {
    if (selectedStop === null) return;
    const clamped = Math.max(0, Math.min(1, pos));
    const sorted = sortByPosition(value);
    const next = sorted.map((s, i) =>
      i === selectedStop ? { ...s, position: clamped } : s,
    );
    const resorted = sortByPosition(next);
    onChange(resorted);
    const newIdx = resorted.findIndex(
      (s) => Math.abs(s.position - clamped) < 0.001,
    );
    if (newIdx >= 0) setSelectedStop(newIdx);
  };

  const evenSpacing = () => {
    const sorted = sortByPosition(value);
    const count = sorted.length;
    if (count < 2) return;
    const next = sorted.map((s, i) => ({
      ...s,
      position: i / (count - 1),
    }));
    onChange(next);
  };

  const sorted = sortByPosition(value);
  const selectedStopData =
    selectedStop !== null ? sorted[selectedStop] : null;

  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <div className="flex items-center justify-between">
          <label className="text-text-2 text-[11px]">{label}</label>
          {canAdd && !expanded && (
            <button
              type="button"
              className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-1.5 py-0.5 text-[10px]"
              onClick={() => {
                const newStop: ColorStop = {
                  position: 0.5,
                  color: { r: 255, g: 255, b: 255, a: 255 },
                };
                onChange(sortByPosition([...value, newStop]));
              }}
            >
              + Stop
            </button>
          )}
        </div>
      )}

      {/* Gradient bar + markers */}
      <canvas
        ref={canvasRef}
        width={barW}
        height={totalH}
        className="cursor-pointer rounded"
        style={{ width: barW, height: totalH }}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseUp}
        onDoubleClick={onDoubleClick}
      />

      {/* Expanded mode controls */}
      {expanded ? (
        <div className="flex flex-col gap-2">
          {/* Selected stop detail */}
          {selectedStopData && (
            <div className="border-border bg-surface-2 flex items-center gap-2 rounded border p-2">
              <span className="text-text-2 text-[10px]">
                Stop {(selectedStop ?? 0) + 1}:
              </span>
              <input
                type="number"
                className="border-border bg-bg text-text w-16 rounded border px-1 py-0.5 text-[10px]"
                value={Math.round(selectedStopData.position * 100)}
                min={0}
                max={100}
                step={1}
                onChange={(e) => {
                  const v = parseFloat(e.target.value);
                  if (!isNaN(v)) updateSelectedPosition(v / 100);
                }}
              />
              <span className="text-text-2 text-[10px]">%</span>
              <div className="flex-1">
                <ColorInput
                  label=""
                  value={selectedStopData.color}
                  onChange={updateSelectedColor}
                />
              </div>
              {canRemove && (
                <button
                  type="button"
                  className="text-text-2 hover:text-error text-[10px]"
                  onClick={() => {
                    onChange(sorted.filter((_, i) => i !== selectedStop));
                    setSelectedStop(null);
                  }}
                >
                  x
                </button>
              )}
            </div>
          )}

          {/* Action buttons */}
          <div className="flex gap-1">
            {canAdd && (
              <button
                type="button"
                className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-2 py-0.5 text-[10px]"
                onClick={() => {
                  const newStop: ColorStop = {
                    position: 0.5,
                    color: sampleGradientAt(value, 0.5),
                  };
                  onChange(sortByPosition([...value, newStop]));
                }}
              >
                + Add Stop
              </button>
            )}
            <button
              type="button"
              className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-2 py-0.5 text-[10px]"
              onClick={evenSpacing}
            >
              Even Spacing
            </button>
          </div>

          {/* Presets */}
          <div className="flex flex-wrap gap-1">
            {GRADIENT_PRESETS.map((preset) => (
              <button
                key={preset.name}
                type="button"
                className="border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text rounded border px-1.5 py-0.5 text-[9px]"
                onClick={() => {
                  onChange(preset.stops);
                  setSelectedStop(null);
                }}
              >
                {preset.name}
              </button>
            ))}
          </div>
        </div>
      ) : (
        /* Compact stop list */
        <div className="flex max-h-32 flex-col gap-1 overflow-y-auto">
          {sorted.map((stop, i) => (
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
                  if (!isNaN(v)) {
                    const pos = Math.min(1, Math.max(0, v));
                    const next = value.map((s, j) =>
                      j === i ? { ...s, position: pos } : s,
                    );
                    onChange(sortByPosition(next));
                  }
                }}
              />
              <span className="text-text-2 w-7 text-center font-mono text-[9px]">
                {(stop.position * 100).toFixed(0)}%
              </span>
              <div className="flex-1">
                <ColorInput
                  label=""
                  value={stop.color}
                  onChange={(c) => {
                    const next = value.map((s, j) =>
                      j === i ? { ...s, color: c } : s,
                    );
                    onChange(next);
                  }}
                />
              </div>
              {canRemove && (
                <button
                  type="button"
                  className="text-text-2 hover:text-error text-[10px]"
                  onClick={() => onChange(value.filter((_, j) => j !== i))}
                >
                  x
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
