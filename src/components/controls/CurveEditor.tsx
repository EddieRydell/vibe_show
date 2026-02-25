import { useRef, useCallback, useEffect, useState } from "react";
import { cssMouseOffset } from "../../utils/cssZoom";
import { FlipHorizontal2, FlipVertical2, RotateCcw } from "lucide-react";
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

type TaggedPoint = CurvePoint & { _id: number };

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
  /** Tagged copy of points, only populated during a drag */
  const taggedRef = useRef<TaggedPoint[]>([]);
  const dragIdRef = useRef<number | null>(null);
  const [draggingIdx, setDraggingIdx] = useState<number | null>(null);
  const [hover, setHover] = useState<{ x: number; y: number } | null>(null);
  const [hoveredPoint, setHoveredPoint] = useState<number | null>(null);
  /** Canvas-space cursor position for tooltip placement */
  const [cursorPos, setCursorPos] = useState<{ cx: number; cy: number } | null>(
    null,
  );

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

    // Dashed center reference lines at 0.5
    ctx.save();
    ctx.strokeStyle = "rgba(255,255,255,0.14)";
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 3]);
    const cx5 = toCanvasX(0.5);
    ctx.beginPath();
    ctx.moveTo(cx5, PAD);
    ctx.lineTo(cx5, canvasH - PAD);
    ctx.stroke();
    const cy5 = toCanvasY(0.5);
    ctx.beginPath();
    ctx.moveTo(PAD, cy5);
    ctx.lineTo(canvasW - PAD, cy5);
    ctx.stroke();
    ctx.restore();

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
      // Vertical gradient fill under curve
      const grad = ctx.createLinearGradient(0, toCanvasY(1), 0, toCanvasY(0));
      grad.addColorStop(0, "rgba(96, 165, 250, 0.22)");
      grad.addColorStop(1, "rgba(96, 165, 250, 0.03)");

      ctx.beginPath();
      ctx.moveTo(toCanvasX(sorted[0]!.x), toCanvasY(0));
      for (const pt of sorted) {
        ctx.lineTo(toCanvasX(pt.x), toCanvasY(pt.y));
      }
      ctx.lineTo(toCanvasX(sorted[sorted.length - 1]!.x), toCanvasY(0));
      ctx.closePath();
      ctx.fillStyle = grad;
      ctx.fill();

      // Curve line
      ctx.beginPath();
      ctx.strokeStyle = "#60a5fa";
      ctx.lineWidth = 1;
      ctx.moveTo(toCanvasX(sorted[0]!.x), toCanvasY(sorted[0]!.y));
      for (let i = 1; i < sorted.length; i++) {
        ctx.lineTo(toCanvasX(sorted[i]!.x), toCanvasY(sorted[i]!.y));
      }
      ctx.stroke();
    }

    // Points (no border)
    for (let i = 0; i < sorted.length; i++) {
      const px = toCanvasX(sorted[i]!.x);
      const py = toCanvasY(sorted[i]!.y);
      const isDragging = draggingIdx === i;
      const isHovered = hoveredPoint === i;
      const r = isDragging || isHovered ? pointR + 2 : pointR;
      const fill = isDragging
        ? "#93c5fd"
        : isHovered
          ? "#60a5fa"
          : "#3b82f6";
      ctx.beginPath();
      ctx.arc(px, py, r, 0, Math.PI * 2);
      ctx.fillStyle = fill;
      ctx.fill();
    }

    // Hover coordinate tooltip following cursor
    if (expanded && hover && cursorPos) {
      const pointLabel =
        hoveredPoint !== null ? ` [pt ${hoveredPoint + 1}]` : "";
      const text = `x: ${round2(hover.x).toFixed(2)}, y: ${round2(hover.y).toFixed(2)}${pointLabel}`;
      ctx.font = "10px monospace";
      const tw = ctx.measureText(text).width;
      const boxW = tw + 8;
      const boxH = 16;
      // Position near cursor, clamped inside canvas
      let tx = cursorPos.cx + 12;
      let ty = cursorPos.cy - 20;
      if (tx + boxW > canvasW) tx = cursorPos.cx - boxW - 4;
      if (ty < 0) ty = cursorPos.cy + 12;
      ctx.fillStyle = "rgba(0,0,0,0.7)";
      ctx.fillRect(tx, ty, boxW, boxH);
      ctx.fillStyle = "#e5e7eb";
      ctx.textAlign = "left";
      ctx.fillText(text, tx + 4, ty + 11);
    }
  }, [
    value,
    draggingIdx,
    hover,
    hoveredPoint,
    cursorPos,
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
        const px = toCanvasX(sorted[i]!.x);
        const py = toCanvasY(sorted[i]!.y);
        if (Math.hypot(cx - px, cy - py) < pointR + 4) return i;
      }
      return null;
    },
    [value, toCanvasX, toCanvasY, pointR],
  );

  const getMousePos = (e: React.MouseEvent) => {
    const canvas = canvasRef.current;
    if (!canvas) return { cx: 0, cy: 0 };
    const { x, y } = cssMouseOffset(e, canvas);
    return { cx: x, cy: y };
  };

  const onMouseDown = (e: React.MouseEvent) => {
    const { cx, cy } = getMousePos(e);
    const sorted = sortByX(value);
    const hit = getPointAt(cx, cy);
    if (hit !== null) {
      // Tag all points with stable IDs; track the hit point's ID
      taggedRef.current = sorted.map((p, i) => ({ ...p, _id: i }));
      dragIdRef.current = hit;
      setDraggingIdx(hit);
    } else {
      const x = fromCanvasX(cx);
      const y = fromCanvasY(cy);
      onChange(sortByX([...sorted, { x, y }]));
    }
  };

  const onMouseMove = (e: React.MouseEvent) => {
    const { cx, cy } = getMousePos(e);
    if (expanded) {
      setHover({ x: fromCanvasX(cx), y: fromCanvasY(cy) });
      setCursorPos({ cx, cy });
    }
    if (dragIdRef.current === null) {
      setHoveredPoint(getPointAt(cx, cy));
      return;
    }
    const x = fromCanvasX(cx);
    const y = fromCanvasY(cy);
    const id = dragIdRef.current;
    // Update the dragged point in our tagged array
    taggedRef.current = taggedRef.current.map((p) =>
      p._id === id ? { ...p, x, y } : p,
    );
    const sorted = [...taggedRef.current].sort((a, b) => a.x - b.x);
    setDraggingIdx(sorted.findIndex((p) => p._id === id));
    onChange(sorted.map(({ x, y }) => ({ x, y })));
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
    setHover(null);
    setHoveredPoint(null);
    setCursorPos(null);
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    if (value.length <= 2) return;
    const { cx, cy } = getMousePos(e);
    const hit = getPointAt(cx, cy);
    if (hit !== null) {
      const sorted = sortByX(value);
      onChange(sorted.filter((_, i) => i !== hit));
    }
  };

  const flipH = () => {
    onChange(sortByX(value.map((p) => ({ x: round2(1 - p.x), y: p.y }))));
  };

  const flipV = () => {
    onChange(sortByX(value.map((p) => ({ x: p.x, y: round2(1 - p.y) }))));
  };

  const resetLinear = () => {
    onChange([
      { x: 0, y: 0 },
      { x: 1, y: 1 },
    ]);
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

  const cursor =
    draggingIdx !== null
      ? "grabbing"
      : hoveredPoint !== null
        ? "grab"
        : "crosshair";

  const toolbarBtnClass =
    "border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text rounded border p-1";

  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <label className="text-text-2 text-[11px]">{label}</label>
      )}
      {/* Toolbar — only in expanded mode */}
      {expanded && (
        <div className="flex items-center gap-1">
          <button
            type="button"
            className={toolbarBtnClass}
            onClick={flipH}
            title="Flip Horizontal"
          >
            <FlipHorizontal2 size={12} />
          </button>
          <button
            type="button"
            className={toolbarBtnClass}
            onClick={flipV}
            title="Flip Vertical"
          >
            <FlipVertical2 size={12} />
          </button>
          <button
            type="button"
            className={toolbarBtnClass}
            onClick={resetLinear}
            title="Reset to Linear"
          >
            <RotateCcw size={12} />
          </button>
          <span className="text-text-2 ml-auto text-[9px]">
            {value.length} pts
          </span>
        </div>
      )}
      <canvas
        ref={canvasRef}
        width={canvasW}
        height={canvasH}
        className="rounded"
        style={{ width: canvasW, height: canvasH, cursor }}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseLeave}
        onContextMenu={onContextMenu}
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
        <div className="flex items-center gap-1">
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
          <span className="text-text-2 ml-auto text-[9px]">
            {value.length} pts
          </span>
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
                <tr
                  key={i}
                  className={`border-border border-t${hoveredPoint === i ? " bg-surface-2" : ""}`}
                  onMouseEnter={() => setHoveredPoint(i)}
                  onMouseLeave={() => setHoveredPoint(null)}
                >
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
                        &times;
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
