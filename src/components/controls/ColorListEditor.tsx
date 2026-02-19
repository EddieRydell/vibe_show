import type { Color } from "../../types";
import { ColorInput } from "./ColorInput";

interface ColorListEditorProps {
  label: string;
  value: Color[];
  minColors: number;
  maxColors: number;
  onChange: (value: Color[]) => void;
}

export function ColorListEditor({
  label,
  value,
  minColors,
  maxColors,
  onChange,
}: ColorListEditorProps) {
  const canAdd = value.length < maxColors;
  const canRemove = value.length > minColors;

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <label className="text-text-2 text-[11px]">{label}</label>
        {canAdd && (
          <button
            type="button"
            className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-1.5 py-0.5 text-[10px]"
            onClick={() => onChange([...value, { r: 255, g: 255, b: 255, a: 255 }])}
          >
            + Add
          </button>
        )}
      </div>
      {value.map((color, i) => (
        <div key={i} className="flex items-center gap-1">
          <div className="flex-1">
            <ColorInput
              label={`#${i + 1}`}
              value={color}
              onChange={(c) => {
                const next = [...value];
                next[i] = c;
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
  );
}
