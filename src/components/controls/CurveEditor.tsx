import { useRef, useCallback, useEffect, useState } from "react";
import type { CurvePoint } from "../../types";
import { CURVE_PRESETS } from "../../constants";

interface CurveEditorProps {
  label: string;
  value: CurvePoint[];
  onChange: (value: CurvePoint[]) => void;
  width?: number;
  height?: number;
  expanded?: boolean;
}

function sortByX(points: CurvePoint[]): CurvePoint[] {
  return [...points].sort((a, b) => a.x - b.x);
}

function round2(n: number): number {
  return Math.round(n * 100) / 100;
}

const PAD = 6;

export function CurveEditor({
  label,
  value,
  onChange,
  width: canvasW = 220,
  height: canvasH = 120,
  expanded = false,
}: CurveEditorProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [dragging, setDragging] = useState<number | null>(null);
  const [hover, setHover] = useState<{ x: number; y: number } | null>(null);

  const pointR = expanded ? 7 : 5;

  const toCanvasX = useCallback(
    (x: number) => PAD + x * (canvasW - 2 * PAD),
    [canvasW],
  );
  const toCanvasY = useCallback(
    (y: number) => canvasH - PAD - y * (canvasH - 2 * PAD),
    [canvasH],
  );
  const fromCanvasX = useCallback(
    (cx: number) => Math.max(0, Math.min(1, (cx - PAD) / (canvasW - 2 * PAD))),
    [canvasW],
  );
  const fromCanvasY = useCallback(
    (cy: number) =>
      Math.max(0, Math.min(1, (canvasH - PAD - cy) / (canvasH - 2 * PAD))),
    [canvasH],
  );

  const draw = useCallback(() => {
    const ctx = canvasRef.current?.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvasW, canvasH);

    // Background
    ctx.fillStyle = "rgba(0,0,0,0.3)";
    ctx.fillRect(0, 0, canvasW, canvasH);

    // Grid lines (0.25 gridlines)
    ctx.strokeStyle = "rgba(255,255,255,0.06)";
    ctx.lineWidth = 1;
    for (let i = 1; i < 4; i++) {
      const gx = toCanvasX(i / 4);
      ctx.beginPath();
      ctx.moveTo(gx, PAD);
      ctx.lineTo(gx, canvasH - PAD);
      ctx.stroke();
      const gy = toCanvasY(i / 4);
      ctx.beginPath();
      ctx.moveTo(PAD, gy);
      ctx.lineTo(canvasW - PAD, gy);
      ctx.stroke();
    }

    // Border
    ctx.strokeStyle = "rgba(255,255,255,0.15)";
    ctx.strokeRect(PAD, PAD, canvasW - 2 * PAD, canvasH - 2 * PAD);

    // Axis labels in expanded mode
    if (expanded) {
      ctx.fillStyle = "rgba(255,255,255,0.3)";
      ctx.font = "9px sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("0", toCanvasX(0), canvasH - 1);
      ctx.fillText("0.5", toCanvasX(0.5), canvasH - 1);
      ctx.fillText("1", toCanvasX(1), canvasH - 1);
      ctx.textAlign = "right";
      ctx.fillText("0", PAD - 2, toCanvasY(0) + 3);
      ctx.fillText(".5", PAD - 2, toCanvasY(0.5) + 3);
      ctx.fillText("1", PAD - 2, toCanvasY(1) + 3);
    }

    const sorted = sortByX(value);
    if (sorted.length >= 2) {
      // Filled area under curve
      ctx.beginPath();
      ctx.moveTo(toCanvasX(sorted[0].x), toCanvasY(0));
      for (const pt of sorted) {
        ctx.lineTo(toCanvasX(pt.x), toCanvasY(pt.y));
      }
      ctx.lineTo(toCanvasX(sorted[sorted.length - 1].x), toCanvasY(0));
      ctx.closePath();
      ctx.fillStyle = "rgba(96, 165, 250, 0.12)";
      ctx.fill();

      // Curve line
      ctx.beginPath();
      ctx.strokeStyle = "#60a5fa";
      ctx.lineWidth = 2;
      ctx.moveTo(toCanvasX(sorted[0].x), toCanvasY(sorted[0].y));
      for (let i = 1; i < sorted.length; i++) {
        ctx.lineTo(toCanvasX(sorted[i].x), toCanvasY(sorted[i].y));
      }
      ctx.stroke();
    }

    // Points
    for (let i = 0; i < sorted.length; i++) {
      const cx = toCanvasX(sorted[i].x);
      const cy = toCanvasY(sorted[i].y);
      ctx.beginPath();
      ctx.arc(cx, cy, pointR, 0, Math.PI * 2);
      ctx.fillStyle = dragging === i ? "#93c5fd" : "#3b82f6";
      ctx.fill();
      ctx.strokeStyle = "#fff";
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }

    // Hover coordinate readout in expanded mode
    if (expanded && hover) {
      const text = `x: ${round2(hover.x).toFixed(2)}, y: ${round2(hover.y).toFixed(2)}`;
      ctx.fillStyle = "rgba(0,0,0,0.7)";
      ctx.font = "10px monospace";
      const tw = ctx.measureText(text).width;
      ctx.fillRect(canvasW - tw - 10, 2, tw + 8, 16);
      ctx.fillStyle = "#e5e7eb";
      ctx.textAlign = "left";
      ctx.fillText(text, canvasW - tw - 6, 13);
    }
  }, [
    value,
    dragging,
    hover,
    canvasW,
    canvasH,
    toCanvasX,
    toCanvasY,
    expanded,
    pointR,
  ]);

  useEffect(() => {
    draw();
  }, [draw]);

  const getPointAt = useCallback(
    (cx: number, cy: number): number | null => {
      const sorted = sortByX(value);
      for (let i = 0; i < sorted.length; i++) {
        const px = toCanvasX(sorted[i].x);
        const py = toCanvasY(sorted[i].y);
        if (Math.hypot(cx - px, cy - py) < pointR + 4) return i;
      }
      return null;
    },
    [value, toCanvasX, toCanvasY, pointR],
  );

  const getMousePos = (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return { cx: 0, cy: 0 };
    return { cx: e.clientX - rect.left, cy: e.clientY - rect.top };
  };

  const onMouseDown = (e: React.MouseEvent) => {
    const { cx, cy } = getMousePos(e);
    const sorted = sortByX(value);
    const hit = getPointAt(cx, cy);
    if (hit !== null) {
      setDragging(hit);
    } else {
      const x = fromCanvasX(cx);
      const y = fromCanvasY(cy);
      const next = sortByX([...sorted, { x, y }]);
      onChange(next);
    }
  };

  const onMouseMove = (e: React.MouseEvent) => {
    const { cx, cy } = getMousePos(e);
    if (expanded) {
      setHover({ x: fromCanvasX(cx), y: fromCanvasY(cy) });
    }
    if (dragging === null) return;
    const x = fromCanvasX(cx);
    const y = fromCanvasY(cy);
    const sorted = sortByX(value);
    const next = sorted.map((p, i) => (i === dragging ? { x, y } : p));
    onChange(sortByX(next));
  };

  const onMouseUp = () => {
    setDragging(null);
  };

  const onMouseLeave = () => {
    setDragging(null);
    setHover(null);
  };

  const onDoubleClick = (e: React.MouseEvent) => {
    if (value.length <= 2) return;
    const { cx, cy } = getMousePos(e);
    const hit = getPointAt(cx, cy);
    if (hit !== null) {
      const sorted = sortByX(value);
      onChange(sorted.filter((_, i) => i !== hit));
    }
  };

  const updatePointField = (index: number, field: "x" | "y", val: number) => {
    const sorted = sortByX(value);
    const clamped = Math.max(0, Math.min(1, val));
    const next = sorted.map((p, i) =>
      i === index ? { ...p, [field]: clamped } : p,
    );
    onChange(sortByX(next));
  };

  const sorted = sortByX(value);

  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <div className="flex items-center justify-between">
          <label className="text-text-2 text-[11px]">{label}</label>
          <span className="text-text-2 text-[9px]">{value.length} pts</span>
        </div>
      )}
      <canvas
        ref={canvasRef}
        width={canvasW}
        height={canvasH}
        className="cursor-crosshair rounded"
        style={{ width: canvasW, height: canvasH }}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseLeave}
        onDoubleClick={onDoubleClick}
      />
      {/* Presets */}
      {expanded ? (
        <div className="flex flex-wrap gap-1">
          {CURVE_PRESETS.map((preset) => (
            <button
              key={preset.name}
              type="button"
              className="border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text rounded border px-1.5 py-0.5 text-[9px]"
              onClick={() => onChange(preset.points)}
            >
              {preset.name}
            </button>
          ))}
        </div>
      ) : (
        <div className="flex gap-1">
          {CURVE_PRESETS.slice(0, 3).map((preset) => (
            <button
              key={preset.name}
              type="button"
              className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-1.5 py-0.5 text-[9px]"
              onClick={() => onChange(preset.points)}
            >
              {preset.name}
            </button>
          ))}
        </div>
      )}
      {/* Point table in expanded mode */}
      {expanded && (
        <div className="border-border mt-1 max-h-40 overflow-y-auto rounded border">
          <table className="w-full text-[10px]">
            <thead>
              <tr className="bg-surface-2 text-text-2">
                <th className="px-2 py-0.5 text-left font-medium">#</th>
                <th className="px-2 py-0.5 text-left font-medium">X</th>
                <th className="px-2 py-0.5 text-left font-medium">Y</th>
                <th className="px-2 py-0.5 text-right font-medium" />
              </tr>
            </thead>
            <tbody>
              {sorted.map((pt, i) => (
                <tr key={i} className="border-border border-t">
                  <td className="text-text-2 px-2 py-0.5">{i + 1}</td>
                  <td className="px-1 py-0.5">
                    <input
                      type="number"
                      className="border-border bg-bg text-text w-14 rounded border px-1 py-0.5 text-[10px]"
                      value={round2(pt.x)}
                      min={0}
                      max={1}
                      step={0.01}
                      onChange={(e) => {
                        const v = parseFloat(e.target.value);
                        if (!isNaN(v)) updatePointField(i, "x", v);
                      }}
                    />
                  </td>
                  <td className="px-1 py-0.5">
                    <input
                      type="number"
                      className="border-border bg-bg text-text w-14 rounded border px-1 py-0.5 text-[10px]"
                      value={round2(pt.y)}
                      min={0}
                      max={1}
                      step={0.01}
                      onChange={(e) => {
                        const v = parseFloat(e.target.value);
                        if (!isNaN(v)) updatePointField(i, "y", v);
                      }}
                    />
                  </td>
                  <td className="px-1 py-0.5 text-right">
                    {value.length > 2 && (
                      <button
                        type="button"
                        className="text-text-2 hover:text-error text-[9px]"
                        onClick={() =>
                          onChange(sorted.filter((_, j) => j !== i))
                        }
                      >
                        x
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
