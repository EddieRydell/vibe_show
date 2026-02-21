import { useEffect, useRef, useState } from "react";
import { Settings } from "lucide-react";
import type { PreviewSettings } from "../hooks/usePreviewSettings";

interface PreviewToolbarProps {
  settings: PreviewSettings;
  onUpdate: (partial: Partial<PreviewSettings>) => void;
  isPreviewingSelection: boolean;
  onResetView: () => void;
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
        className="h-1 w-20 cursor-pointer accent-primary"
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
}: PreviewToolbarProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close dropdown on outside click
  useEffect(() => {
    if (!settingsOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setSettingsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [settingsOpen]);

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

      <div className="flex-1" />

      {/* Settings gear */}
      <div className="relative" ref={menuRef}>
        <button
          onClick={() => setSettingsOpen((o) => !o)}
          className={`rounded p-1 transition-colors ${
            settingsOpen
              ? "bg-surface-2 text-text"
              : "text-text-2 hover:bg-surface-2 hover:text-text"
          }`}
          title="Preview settings"
        >
          <Settings size={14} />
        </button>

        {settingsOpen && (
          <div className="border-border bg-surface absolute right-0 top-full z-50 mt-1 flex w-56 flex-col gap-2.5 rounded-lg border p-3 shadow-lg">
            <span className="text-text-2 text-[10px] tracking-wider uppercase">
              Display
            </span>
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

            <div className="border-border border-t" />

            <span className="text-text-2 text-[10px] tracking-wider uppercase">
              Glow
            </span>
            <Slider
              label="Size"
              value={settings.glowSize}
              min={0}
              max={5}
              step={0.1}
              onChange={(v) => onUpdate({ glowSize: v })}
            />
            <Slider
              label="Opacity"
              value={settings.glowOpacity}
              min={0}
              max={1}
              step={0.01}
              onChange={(v) => onUpdate({ glowOpacity: v })}
            />

            <div className="border-border border-t" />

            <button
              onClick={onResetView}
              className="text-text-2 hover:bg-surface-2 hover:text-text rounded px-2 py-1 text-left text-[11px] transition-colors"
            >
              Reset View
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
