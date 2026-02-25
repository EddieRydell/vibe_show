import type { Color } from "../../types";
import { colorToHex, hexToColor } from "../../utils/colorUtils";

interface ColorInputProps {
  label: string;
  value: Color;
  onChange: (value: Color) => void;
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
