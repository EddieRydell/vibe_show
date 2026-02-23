import type { Color } from "../../types";

interface ColorInputProps {
  label: string;
  value: Color;
  onChange: (value: Color) => void;
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

export function ColorInput({ label, value, onChange }: ColorInputProps) {
  const hex = colorToHex(value);

  return (
    <div className="flex items-center justify-between">
      <label className="text-text-2 text-[11px]">{label}</label>
      <div className="flex items-center gap-1.5">
        <span className="text-text-2 font-mono text-[10px]">{hex}</span>
        <div className="relative">
          <div
            className="border-border size-6  rounded border"
            style={{ backgroundColor: hex }}
          />
          <input
            type="color"
            className="absolute inset-0 cursor-pointer opacity-0"
            value={hex}
            onChange={(e) => onChange(hexToColor(e.target.value))}
          />
        </div>
      </div>
    </div>
  );
}
