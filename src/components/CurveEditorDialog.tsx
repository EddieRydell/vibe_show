import { useCallback, useState } from "react";
import type { CurvePoint } from "../types";
import { CurveEditor } from "./controls/CurveEditor";
import { ModalBackdrop } from "./ModalBackdrop";

interface Props {
  initialValue: CurvePoint[];
  onApply: (value: CurvePoint[]) => void;
  onCancel: () => void;
}

export function CurveEditorDialog({ initialValue, onApply, onCancel }: Props) {
  const [value, setValue] = useState<CurvePoint[]>(initialValue);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        onApply(value);
      }
    },
    [onApply, value],
  );

  return (
    <ModalBackdrop onClose={onCancel}>
      <div
        className="bg-surface border-border flex w-[640px] flex-col rounded-lg border shadow-xl"
        onKeyDown={handleKeyDown}
      >
        {/* Header */}
        <div className="border-border flex items-center justify-between border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">Curve Editor</h3>
          <button onClick={onCancel} className="text-text-2 hover:text-text text-sm">
            &times;
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-4">
          <CurveEditor
            label=""
            value={value}
            onChange={setValue}
            width={560}
            height={280}
            expanded
          />
        </div>

        {/* Footer */}
        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          <button
            onClick={onCancel}
            className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-4 py-1.5 text-xs transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={() => onApply(value)}
            className="bg-primary hover:bg-primary/90 rounded px-4 py-1.5 text-xs font-medium text-white transition-colors"
          >
            Apply
          </button>
        </div>
      </div>
    </ModalBackdrop>
  );
}
