import { useRef, useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import type { Color } from "../../types";
import { colorToHex, tryParseHex, rgbToHsv, hsvToRgb } from "../../utils/colorUtils";
import { getEffectiveZoom } from "../../utils/cssZoom";

type ColorFormat = "hex" | "rgb" | "hsv";

const SV_SIZE = 180;
const HUE_H = 14;
const PANEL_W = SV_SIZE + 24;

const inputClass =
  "border-border bg-bg text-text w-12 rounded border px-1 py-0.5 text-center font-mono text-[11px]";
const labelClass = "text-text-2 text-[10px]";

interface ColorPickerProps {
  color: Color;
  onChange: (color: Color) => void;
  onClose: () => void;
  initialPos: { x: number; y: number };
}

export function ColorPicker({
  color,
  onChange,
  onClose,
  initialPos,
}: ColorPickerProps) {
  const [hsv, _setHsv] = useState<[number, number, number]>(() =>
    rgbToHsv(color.r, color.g, color.b),
  );
  const hsvRef = useRef(hsv);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  const [format, setFormat] = useState<ColorFormat>("hex");
  const [hexInput, setHexInput] = useState(colorToHex(color));
  const inputFocused = useRef(false);

  const setHsv = useCallback((next: [number, number, number]) => {
    hsvRef.current = next;
    _setHsv(next);
    const c = hsvToRgb(next[0], next[1], next[2]);
    onChangeRef.current(c);
    if (!inputFocused.current) setHexInput(colorToHex(c));
  }, []);

  const svRef = useRef<HTMLCanvasElement>(null);
  const hueRef = useRef<HTMLCanvasElement>(null);

  const [draggingSV, setDraggingSV] = useState(false);
  const [draggingHue, setDraggingHue] = useState(false);

  const [pos, setPos] = useState(() => ({
    x: Math.max(4, Math.min(initialPos.x, window.innerWidth - PANEL_W - 4)),
    y: Math.max(4, Math.min(initialPos.y, window.innerHeight - 300)),
  }));
  const dragOffset = useRef<{ x: number; y: number } | null>(null);

  const [h, s, v] = hsv;
  const currentColor = hsvToRgb(h, s, v);

  // Draw SV canvas
  useEffect(() => {
    const ctx = svRef.current?.getContext("2d");
    if (!ctx) return;

    ctx.fillStyle = `hsl(${h}, 100%, 50%)`;
    ctx.fillRect(0, 0, SV_SIZE, SV_SIZE);

    const white = ctx.createLinearGradient(0, 0, SV_SIZE, 0);
    white.addColorStop(0, "rgba(255,255,255,1)");
    white.addColorStop(1, "rgba(255,255,255,0)");
    ctx.fillStyle = white;
    ctx.fillRect(0, 0, SV_SIZE, SV_SIZE);

    const black = ctx.createLinearGradient(0, 0, 0, SV_SIZE);
    black.addColorStop(0, "rgba(0,0,0,0)");
    black.addColorStop(1, "rgba(0,0,0,1)");
    ctx.fillStyle = black;
    ctx.fillRect(0, 0, SV_SIZE, SV_SIZE);

    // Indicator circle
    const ix = s * SV_SIZE;
    const iy = (1 - v) * SV_SIZE;
    ctx.beginPath();
    ctx.arc(ix, iy, 6, 0, Math.PI * 2);
    ctx.strokeStyle = "#fff";
    ctx.lineWidth = 2;
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(ix, iy, 5, 0, Math.PI * 2);
    ctx.strokeStyle = "#000";
    ctx.lineWidth = 1;
    ctx.stroke();
  }, [h, s, v]);

  // Draw hue bar
  useEffect(() => {
    const ctx = hueRef.current?.getContext("2d");
    if (!ctx) return;

    const grad = ctx.createLinearGradient(0, 0, SV_SIZE, 0);
    for (let i = 0; i <= 6; i++) {
      grad.addColorStop(i / 6, `hsl(${i * 60}, 100%, 50%)`);
    }
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, SV_SIZE, HUE_H);

    // Indicator
    const ix = (h / 360) * SV_SIZE;
    ctx.fillStyle = "rgba(0,0,0,0.3)";
    ctx.fillRect(ix - 3, 0, 6, HUE_H);
    ctx.strokeStyle = "#fff";
    ctx.lineWidth = 2;
    ctx.strokeRect(ix - 2, 1, 4, HUE_H - 2);
  }, [h]);

  // SV drag
  useEffect(() => {
    if (!draggingSV) return;
    const onMove = (e: MouseEvent) => {
      const el = svRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const zoom = getEffectiveZoom(el);
      const ns = Math.max(0, Math.min(1, (e.clientX - rect.left) / zoom / SV_SIZE));
      const nv = Math.max(
        0,
        Math.min(1, 1 - (e.clientY - rect.top) / zoom / SV_SIZE),
      );
      setHsv([hsvRef.current[0], ns, nv]);
    };
    const onUp = () => setDraggingSV(false);
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    return () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
  }, [draggingSV, setHsv]);

  // Hue drag
  useEffect(() => {
    if (!draggingHue) return;
    const onMove = (e: MouseEvent) => {
      const el = hueRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const zoom = getEffectiveZoom(el);
      const nh = Math.max(
        0,
        Math.min(360, ((e.clientX - rect.left) / zoom / SV_SIZE) * 360),
      );
      setHsv([nh, hsvRef.current[1], hsvRef.current[2]]);
    };
    const onUp = () => setDraggingHue(false);
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    return () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
  }, [draggingHue, setHsv]);

  // Panel drag
  const onHeaderMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    dragOffset.current = { x: e.clientX - pos.x, y: e.clientY - pos.y };
    const onMove = (ev: MouseEvent) => {
      if (dragOffset.current) {
        setPos({
          x: ev.clientX - dragOffset.current.x,
          y: ev.clientY - dragOffset.current.y,
        });
      }
    };
    const onUp = () => {
      dragOffset.current = null;
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  };

  // Escape key
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  return createPortal(
    <div
      className="border-border bg-surface z-9999 flex flex-col rounded-lg border shadow-xl"
      style={{ position: "fixed", top: pos.y, left: pos.x, width: PANEL_W }}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
    >
      {/* Draggable header */}
      <div
        className="bg-surface-2 flex cursor-move select-none items-center justify-between rounded-t-lg px-3 py-1.5"
        onMouseDown={onHeaderMouseDown}
      >
        <span className="text-text-2 text-[10px] font-medium">Color</span>
        <button
          type="button"
          className="text-text-2 hover:text-text text-sm leading-none"
          onClick={onClose}
        >
          &times;
        </button>
      </div>

      <div className="flex flex-col gap-2 p-3">
        {/* SV square */}
        <canvas
          ref={svRef}
          width={SV_SIZE}
          height={SV_SIZE}
          className="cursor-crosshair rounded"
          style={{ width: SV_SIZE, height: SV_SIZE }}
          onMouseDown={(e) => {
            setDraggingSV(true);
            const el = svRef.current;
            if (!el) return;
            const rect = el.getBoundingClientRect();
            const zoom = getEffectiveZoom(el);
            const ns = Math.max(
              0,
              Math.min(1, (e.clientX - rect.left) / zoom / SV_SIZE),
            );
            const nv = Math.max(
              0,
              Math.min(1, 1 - (e.clientY - rect.top) / zoom / SV_SIZE),
            );
            setHsv([hsvRef.current[0], ns, nv]);
          }}
        />

        {/* Hue bar */}
        <canvas
          ref={hueRef}
          width={SV_SIZE}
          height={HUE_H}
          className="cursor-crosshair rounded"
          style={{ width: SV_SIZE, height: HUE_H }}
          onMouseDown={(e) => {
            setDraggingHue(true);
            const el = hueRef.current;
            if (!el) return;
            const rect = el.getBoundingClientRect();
            const zoom = getEffectiveZoom(el);
            const nh = Math.max(
              0,
              Math.min(360, ((e.clientX - rect.left) / zoom / SV_SIZE) * 360),
            );
            setHsv([nh, hsvRef.current[1], hsvRef.current[2]]);
          }}
        />

        {/* Color preview + format selector */}
        <div className="flex items-center gap-2">
          <div
            className="border-border size-6 shrink-0 rounded border"
            style={{ backgroundColor: colorToHex(currentColor) }}
          />
          <select
            className="border-border bg-bg text-text rounded border px-1 py-0.5 text-[10px]"
            value={format}
            onChange={(e) => setFormat(e.target.value as ColorFormat)}
          >
            <option value="hex">HEX</option>
            <option value="rgb">RGB</option>
            <option value="hsv">HSV</option>
          </select>
        </div>

        {/* Format-specific inputs */}
        {format === "hex" && (
          <div className="flex items-center gap-1">
            <input
              className="border-border bg-bg text-text flex-1 rounded border px-1.5 py-0.5 font-mono text-[11px]"
              value={hexInput}
              onFocus={() => {
                inputFocused.current = true;
              }}
              onBlur={() => {
                inputFocused.current = false;
                const c = tryParseHex(hexInput);
                if (c) {
                  setHsv(rgbToHsv(c.r, c.g, c.b));
                } else {
                  setHexInput(colorToHex(currentColor));
                }
              }}
              onChange={(e) => {
                setHexInput(e.target.value);
                const c = tryParseHex(e.target.value);
                if (c) setHsv(rgbToHsv(c.r, c.g, c.b));
              }}
            />
          </div>
        )}

        {format === "rgb" && (
          <div className="flex items-center gap-1.5">
            <label className={labelClass}>R</label>
            <input
              type="number"
              className={inputClass}
              value={currentColor.r}
              min={0}
              max={255}
              onChange={(e) => {
                const r = Math.max(0, Math.min(255, parseInt(e.target.value) || 0));
                setHsv(rgbToHsv(r, currentColor.g, currentColor.b));
              }}
            />
            <label className={labelClass}>G</label>
            <input
              type="number"
              className={inputClass}
              value={currentColor.g}
              min={0}
              max={255}
              onChange={(e) => {
                const g = Math.max(0, Math.min(255, parseInt(e.target.value) || 0));
                setHsv(rgbToHsv(currentColor.r, g, currentColor.b));
              }}
            />
            <label className={labelClass}>B</label>
            <input
              type="number"
              className={inputClass}
              value={currentColor.b}
              min={0}
              max={255}
              onChange={(e) => {
                const b = Math.max(0, Math.min(255, parseInt(e.target.value) || 0));
                setHsv(rgbToHsv(currentColor.r, currentColor.g, b));
              }}
            />
          </div>
        )}

        {format === "hsv" && (
          <div className="flex items-center gap-1.5">
            <label className={labelClass}>H</label>
            <input
              type="number"
              className={inputClass}
              value={Math.round(h)}
              min={0}
              max={360}
              onChange={(e) => {
                const nh = Math.max(0, Math.min(360, parseInt(e.target.value) || 0));
                setHsv([nh, s, v]);
              }}
            />
            <label className={labelClass}>S</label>
            <input
              type="number"
              className={inputClass}
              value={Math.round(s * 100)}
              min={0}
              max={100}
              onChange={(e) => {
                const ns = Math.max(0, Math.min(100, parseInt(e.target.value) || 0)) / 100;
                setHsv([h, ns, v]);
              }}
            />
            <label className={labelClass}>V</label>
            <input
              type="number"
              className={inputClass}
              value={Math.round(v * 100)}
              min={0}
              max={100}
              onChange={(e) => {
                const nv = Math.max(0, Math.min(100, parseInt(e.target.value) || 0)) / 100;
                setHsv([h, s, nv]);
              }}
            />
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
