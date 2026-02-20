import type { PreviewSettings } from "../hooks/usePreviewSettings";

interface PreviewToolbarProps {
  settings: PreviewSettings;
  onUpdate: (partial: Partial<PreviewSettings>) => void;
  isPreviewingSelection: boolean;
  onResetView: () => void;
  onClose: () => void;
}

function Slider({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <label className="flex items-center gap-1.5">
      <span className="text-text-2 w-14 shrink-0 text-[10px]">{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="h-1 w-16 cursor-pointer accent-primary"
      />
      <span className="text-text-2 w-7 text-right font-mono text-[10px]">
        {value.toFixed(step < 0.1 ? 2 : 1)}
      </span>
    </label>
  );
}

export function PreviewToolbar({
  settings,
  onUpdate,
  isPreviewingSelection,
  onResetView,
  onClose,
}: PreviewToolbarProps) {
  return (
    <div className="border-border bg-surface flex items-center gap-3 border-b px-3 py-1">
      <span className="text-text-2 text-[11px] tracking-wider uppercase">
        Preview
      </span>

      {isPreviewingSelection && (
        <span className="text-primary bg-primary/10 border-primary/30 rounded border px-1.5 py-0.5 text-[9px]">
          Previewing selection
        </span>
      )}

      <div className="border-border mx-1 h-4 border-l" />

      <Slider
        label="Bulb Size"
        value={settings.bulbSize}
        min={0.2}
        max={5}
        step={0.1}
        onChange={(v) => onUpdate({ bulbSize: v })}
      />
      <Slider
        label="Opacity"
        value={settings.bulbOpacity}
        min={0}
        max={1}
        step={0.05}
        onChange={(v) => onUpdate({ bulbOpacity: v })}
      />

      <div className="border-border mx-1 h-4 border-l" />

      <Slider
        label="Glow Size"
        value={settings.glowSize}
        min={0}
        max={5}
        step={0.1}
        onChange={(v) => onUpdate({ glowSize: v })}
      />
      <Slider
        label="Glow Op."
        value={settings.glowOpacity}
        min={0}
        max={1}
        step={0.01}
        onChange={(v) => onUpdate({ glowOpacity: v })}
      />

      <div className="flex-1" />

      <button
        onClick={onResetView}
        className="text-text-2 hover:bg-surface-2 hover:text-text rounded px-2 py-0.5 text-[11px] transition-colors"
        title="Reset pan/zoom"
      >
        Reset View
      </button>
      <button
        onClick={onClose}
        className="text-text-2 hover:bg-surface-2 hover:text-text rounded px-2 py-0.5 text-[11px] transition-colors"
        title="Close preview window"
      >
        &#x2715;
      </button>
    </div>
  );
}
