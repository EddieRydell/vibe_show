import { useRef, useCallback, useEffect, useState } from "react";
import { cssMouseOffset, getEffectiveZoom } from "../../utils/cssZoom";
import type { Color, ColorStop } from "../../types";
import { GRADIENT_PRESETS } from "../../constants";
import { ColorPicker } from "./ColorPicker";

interface GradientEditorProps {
  label: string;
  value: ColorStop[];
  minStops: number;
  maxStops: number;
  onChange: (value: ColorStop[]) => void;
  height?: number;
  expanded?: boolean;
}

function sortByPosition(stops: ColorStop[]): ColorStop[] {
  return [...stops].sort((a, b) => a.position - b.position);
}

function colorToCSS(c: Color): string {
  return `rgb(${c.r},${c.g},${c.b})`;
}

function colorToHex(c: Color): string {
  const r = c.r.toString(16).padStart(2, "0");
  const g = c.g.toString(16).padStart(2, "0");
  const b = c.b.toString(16).padStart(2, "0");
  return `#${r}${g}${b}`;
}

function hexToColor(hex: string): Color {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return { r, g, b, a: 255 };
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

type TaggedStop = ColorStop & { _id: number };

export function GradientEditor({
  label,
  value,
  minStops,
  maxStops,
  onChange,
  height: barH = 24,
  expanded = false,
}: GradientEditorProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [barW, setBarW] = useState(220);
  /** Tagged copy of stops, only populated during a drag */
  const taggedRef = useRef<TaggedStop[]>([]);
  const dragIdRef = useRef<number | null>(null);
  const [draggingIdx, setDraggingIdx] = useState<number | null>(null);
  const [hoveredStop, setHoveredStop] = useState<number | null>(null);
  const [colorPickerStop, setColorPickerStop] = useState<number | null>(null);
  const [colorPickerPos, setColorPickerPos] = useState({ x: 0, y: 0 });
  /** Canvas-space cursor position for tooltip placement */
  const [cursorPos, setCursorPos] = useState<{ cx: number; cy: number } | null>(
    null,
  );
  const [hover, setHover] = useState<{ pos: number } | null>(null);

  const canAdd = value.length < maxStops;
  const canRemove = value.length > minStops;
  const totalH = barH + MARKER_H;

  // Measure container width
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const w = Math.floor(entry.contentRect.width);
        if (w > 0) setBarW(w);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

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

      ctx.beginPath();
      ctx.moveTo(x, barH);
      ctx.lineTo(x - 5, barH + MARKER_H);
      ctx.lineTo(x + 5, barH + MARKER_H);
      ctx.closePath();

      ctx.fillStyle = colorToCSS(sorted[i].color);
      ctx.fill();
    }

    // Hover tooltip following cursor (expanded mode)
    if (expanded && hover && cursorPos) {
      const stopLabel =
        hoveredStop !== null ? ` [stop ${hoveredStop + 1}]` : "";
      const text = `${Math.round(hover.pos * 100)}%${stopLabel}`;
      ctx.font = "10px monospace";
      const tw = ctx.measureText(text).width;
      const boxW = tw + 8;
      const boxH = 16;
      let tx = cursorPos.cx + 12;
      let ty = cursorPos.cy - 20;
      if (tx + boxW > barW) tx = cursorPos.cx - boxW - 4;
      if (ty < 0) ty = cursorPos.cy + 12;
      ctx.fillStyle = "rgba(0,0,0,0.7)";
      ctx.fillRect(tx, ty, boxW, boxH);
      ctx.fillStyle = "#e5e7eb";
      ctx.textAlign = "left";
      ctx.fillText(text, tx + 4, ty + 11);
    }
  }, [value, barW, barH, totalH, draggingIdx, hoveredStop, expanded, hover, cursorPos]);

  useEffect(() => {
    drawGradient();
  }, [drawGradient]);

  const getMousePos = (e: React.MouseEvent) => {
    const canvas = canvasRef.current;
    if (!canvas) return { cx: 0, cy: 0 };
    const { x, y } = cssMouseOffset(e, canvas);
    return { cx: x, cy: y };
  };

  // Hit-test: covers both the bar area and the marker triangles below
  const getStopAt = useCallback(
    (cx: number, cy: number): number | null => {
      if (cy < 0 || cy > totalH) return null;
      const sorted = sortByPosition(value);
      for (let i = 0; i < sorted.length; i++) {
        const mx = sorted[i].position * (barW - 1);
        if (Math.abs(cx - mx) < 8) return i;
      }
      return null;
    },
    [value, barW, totalH],
  );

  const onMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    const { cx, cy } = getMousePos(e);
    const sorted = sortByPosition(value);
    const hit = getStopAt(cx, cy);
    if (hit !== null) {
      // Tag all stops with stable IDs; track the hit stop's ID
      taggedRef.current = sorted.map((s, i) => ({ ...s, _id: i }));
      dragIdRef.current = hit;
      setDraggingIdx(hit);
    } else if (canAdd) {
      const pos = Math.max(0, Math.min(1, cx / (barW - 1)));
      const color = sampleGradientAt(value, pos);
      onChange(sortByPosition([...sorted, { position: pos, color }]));
    }
  };

  const onMouseMove = (e: React.MouseEvent) => {
    const { cx, cy } = getMousePos(e);
    if (expanded) {
      const pos = Math.max(0, Math.min(1, cx / (barW - 1)));
      setHover({ pos });
      setCursorPos({ cx, cy });
    }
    if (dragIdRef.current === null) {
      setHoveredStop(getStopAt(cx, cy));
      return;
    }
    const pos = Math.max(0, Math.min(1, cx / (barW - 1)));
    const id = dragIdRef.current;
    // Update the dragged stop in our tagged array
    taggedRef.current = taggedRef.current.map((s) =>
      s._id === id ? { ...s, position: pos } : s,
    );
    const sorted = [...taggedRef.current].sort(
      (a, b) => a.position - b.position,
    );
    setDraggingIdx(sorted.findIndex((s) => s._id === id));
    onChange(sorted.map(({ position, color }) => ({ position, color })));
  };

  const onMouseUp = () => {
    dragIdRef.current = null;
    taggedRef.current = [];
    setDraggingIdx(null);
  };

  const onMouseLeave = () => {
    dragIdRef.current = null;
    taggedRef.current = [];
    setDraggingIdx(null);
    setHoveredStop(null);
    setHover(null);
    setCursorPos(null);
  };

  const onDoubleClick = (e: React.MouseEvent) => {
    const { cx, cy } = getMousePos(e);
    const hit = getStopAt(cx, cy);
    if (hit !== null) {
      setColorPickerStop(hit);
      const canvas = canvasRef.current;
      const rect = canvas?.getBoundingClientRect();
      if (canvas && rect) {
        const zoom = getEffectiveZoom(canvas);
        const mx = sortByPosition(value)[hit].position * (barW - 1);
        setColorPickerPos({
          x: rect.left + mx * zoom - 90,
          y: rect.bottom + 8,
        });
      }
    }
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    if (!canRemove) return;
    const { cx, cy } = getMousePos(e);
    const hit = getStopAt(cx, cy);
    if (hit !== null) {
      if (colorPickerStop === hit) setColorPickerStop(null);
      const sorted = sortByPosition(value);
      onChange(sorted.filter((_, i) => i !== hit));
    }
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

  const updateStopColor = (index: number, color: Color) => {
    const sorted = sortByPosition(value);
    onChange(sorted.map((s, i) => (i === index ? { ...s, color } : s)));
  };

  const cursor =
    draggingIdx !== null
      ? "grabbing"
      : hoveredStop !== null
        ? "grab"
        : "crosshair";

  const sorted = sortByPosition(value);

  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <label className="text-text-2 text-[11px]">{label}</label>
      )}

      {/* Gradient bar + markers — responsive container */}
      <div ref={containerRef} className="w-full">
        <canvas
          ref={canvasRef}
          width={barW}
          height={totalH}
          className="w-full rounded"
          style={{ height: totalH, cursor }}
          onMouseDown={onMouseDown}
          onMouseMove={onMouseMove}
          onMouseUp={onMouseUp}
          onMouseLeave={onMouseLeave}
          onDoubleClick={onDoubleClick}
          onContextMenu={onContextMenu}
        />
      </div>

      {/* Color picker for double-click editing */}
      {colorPickerStop !== null && sorted[colorPickerStop] && (
        <ColorPicker
          color={sorted[colorPickerStop].color}
          onChange={(c) => updateStopColor(colorPickerStop, c)}
          onClose={() => setColorPickerStop(null)}
          initialPos={colorPickerPos}
        />
      )}

      {/* Expanded mode: stop table + action buttons + presets */}
      {expanded && (
        <div className="flex flex-col gap-2">
          {/* Stop table */}
          <div className="border-border mt-1 max-h-40 overflow-y-auto rounded border">
            <table className="w-full text-[10px]">
              <thead>
                <tr className="bg-surface-2 text-text-2">
                  <th className="px-2 py-0.5 text-left font-medium">#</th>
                  <th className="px-2 py-0.5 text-left font-medium">Pos</th>
                  <th className="px-2 py-0.5 text-left font-medium">Color</th>
                  <th className="px-2 py-0.5 text-right font-medium" />
                </tr>
              </thead>
              <tbody>
                {sorted.map((stop, i) => (
                  <tr
                    key={i}
                    className={`border-border border-t${hoveredStop === i ? " bg-surface-2" : ""}`}
                    onMouseEnter={() => setHoveredStop(i)}
                    onMouseLeave={() => setHoveredStop(null)}
                  >
                    <td className="text-text-2 px-2 py-0.5">{i + 1}</td>
                    <td className="px-1 py-0.5">
                      <input
                        type="number"
                        className="border-border bg-bg text-text w-14 rounded border px-1 py-0.5 text-[10px]"
                        value={Math.round(stop.position * 100)}
                        min={0}
                        max={100}
                        step={1}
                        onChange={(e) => {
                          const v = parseFloat(e.target.value);
                          if (!isNaN(v)) {
                            const clamped = Math.max(0, Math.min(1, v / 100));
                            const next = sorted.map((s, j) =>
                              j === i ? { ...s, position: clamped } : s,
                            );
                            onChange(sortByPosition(next));
                          }
                        }}
                      />
                    </td>
                    <td className="px-1 py-0.5">
                      <div className="flex items-center gap-1">
                        <div className="relative">
                          <div
                            className="border-border size-5 rounded border"
                            style={{ backgroundColor: colorToHex(stop.color) }}
                          />
                          <input
                            type="color"
                            className="absolute inset-0 cursor-pointer opacity-0"
                            value={colorToHex(stop.color)}
                            onChange={(e) =>
                              updateStopColor(i, hexToColor(e.target.value))
                            }
                          />
                        </div>
                        <span className="text-text-2 font-mono text-[9px]">
                          {colorToHex(stop.color)}
                        </span>
                      </div>
                    </td>
                    <td className="px-1 py-0.5 text-right">
                      {canRemove && (
                        <button
                          type="button"
                          className="text-text-2 hover:text-error text-[9px]"
                          onClick={() =>
                            onChange(sorted.filter((_, j) => j !== i))
                          }
                        >
                          &times;
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

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
                onClick={() => onChange(preset.stops)}
              >
                {preset.name}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
