import { useRef, useCallback, useEffect, useState } from "react";
import type { CurvePoint } from "../../types";

interface CurveEditorProps {
  label: string;
  value: CurvePoint[];
  onChange: (value: CurvePoint[]) => void;
}

function sortByX(points: CurvePoint[]): CurvePoint[] {
  return [...points].sort((a, b) => a.x - b.x);
}

const CANVAS_W = 220;
const CANVAS_H = 120;
const PAD = 6;
const POINT_R = 5;

function toCanvasX(x: number) {
  return PAD + x * (CANVAS_W - 2 * PAD);
}
function toCanvasY(y: number) {
  return CANVAS_H - PAD - y * (CANVAS_H - 2 * PAD);
}
function fromCanvasX(cx: number) {
  return Math.max(0, Math.min(1, (cx - PAD) / (CANVAS_W - 2 * PAD)));
}
function fromCanvasY(cy: number) {
  return Math.max(0, Math.min(1, (CANVAS_H - PAD - cy) / (CANVAS_H - 2 * PAD)));
}

export function CurveEditor({ label, value, onChange }: CurveEditorProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [dragging, setDragging] = useState<number | null>(null);

  const draw = useCallback(() => {
    const ctx = canvasRef.current?.getContext("2d");
    if (!ctx) return;
    const w = CANVAS_W;
    const h = CANVAS_H;
    ctx.clearRect(0, 0, w, h);

    // Background
    ctx.fillStyle = "rgba(0,0,0,0.3)";
    ctx.fillRect(0, 0, w, h);

    // Grid lines
    ctx.strokeStyle = "rgba(255,255,255,0.08)";
    ctx.lineWidth = 1;
    for (let i = 1; i < 4; i++) {
      const gx = toCanvasX(i / 4);
      ctx.beginPath();
      ctx.moveTo(gx, PAD);
      ctx.lineTo(gx, h - PAD);
      ctx.stroke();
      const gy = toCanvasY(i / 4);
      ctx.beginPath();
      ctx.moveTo(PAD, gy);
      ctx.lineTo(w - PAD, gy);
      ctx.stroke();
    }

    // Border
    ctx.strokeStyle = "rgba(255,255,255,0.15)";
    ctx.strokeRect(PAD, PAD, w - 2 * PAD, h - 2 * PAD);

    // Curve line
    const sorted = sortByX(value);
    if (sorted.length >= 2) {
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
      ctx.arc(cx, cy, POINT_R, 0, Math.PI * 2);
      ctx.fillStyle = dragging === i ? "#93c5fd" : "#3b82f6";
      ctx.fill();
      ctx.strokeStyle = "#fff";
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
  }, [value, dragging]);

  useEffect(() => {
    draw();
  }, [draw]);

  const getPointAt = useCallback(
    (cx: number, cy: number): number | null => {
      const sorted = sortByX(value);
      for (let i = 0; i < sorted.length; i++) {
        const px = toCanvasX(sorted[i].x);
        const py = toCanvasY(sorted[i].y);
        if (Math.hypot(cx - px, cy - py) < POINT_R + 4) return i;
      }
      return null;
    },
    [value],
  );

  const getMousePos = (e: React.MouseEvent) => {
    const rect = canvasRef.current!.getBoundingClientRect();
    return { cx: e.clientX - rect.left, cy: e.clientY - rect.top };
  };

  const onMouseDown = (e: React.MouseEvent) => {
    const { cx, cy } = getMousePos(e);
    const sorted = sortByX(value);
    const hit = getPointAt(cx, cy);
    if (hit !== null) {
      setDragging(hit);
    } else {
      // Add a new point
      const x = fromCanvasX(cx);
      const y = fromCanvasY(cy);
      const next = sortByX([...sorted, { x, y }]);
      onChange(next);
    }
  };

  const onMouseMove = (e: React.MouseEvent) => {
    if (dragging === null) return;
    const { cx, cy } = getMousePos(e);
    const x = fromCanvasX(cx);
    const y = fromCanvasY(cy);
    const sorted = sortByX(value);
    const next = sorted.map((p, i) => (i === dragging ? { x, y } : p));
    onChange(sortByX(next));
  };

  const onMouseUp = () => {
    setDragging(null);
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

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <label className="text-text-2 text-[11px]">{label}</label>
        <span className="text-text-2 text-[9px]">{value.length} pts</span>
      </div>
      <canvas
        ref={canvasRef}
        width={CANVAS_W}
        height={CANVAS_H}
        className="cursor-crosshair rounded"
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseUp}
        onDoubleClick={onDoubleClick}
      />
      <div className="flex gap-1">
        <button
          type="button"
          className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-1.5 py-0.5 text-[9px]"
          onClick={() =>
            onChange([
              { x: 0, y: 0 },
              { x: 1, y: 1 },
            ])
          }
        >
          Linear
        </button>
        <button
          type="button"
          className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-1.5 py-0.5 text-[9px]"
          onClick={() =>
            onChange([
              { x: 0, y: 0 },
              { x: 0.5, y: 1 },
              { x: 1, y: 0 },
            ])
          }
        >
          Triangle
        </button>
        <button
          type="button"
          className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-1.5 py-0.5 text-[9px]"
          onClick={() =>
            onChange([
              { x: 0, y: 1 },
              { x: 1, y: 1 },
            ])
          }
        >
          Constant
        </button>
      </div>
    </div>
  );
}
