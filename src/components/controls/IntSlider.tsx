interface IntSliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}

export function IntSlider({ label, value, min, max, onChange }: IntSliderProps) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <label className="text-text-2 text-[11px]">{label}</label>
        <input
          type="number"
          className="bg-surface-2 border-border text-text w-16 rounded border px-1.5 py-0.5 text-right text-[11px]"
          value={value}
          min={min}
          max={max}
          step={1}
          onChange={(e) => {
            const v = parseInt(e.target.value, 10);
            if (!isNaN(v)) onChange(Math.min(max, Math.max(min, v)));
          }}
        />
      </div>
      <input
        type="range"
        className="accent-primary bg-surface-2 h-1.5 w-full cursor-pointer appearance-none rounded-full"
        value={value}
        min={min}
        max={max}
        step={1}
        onChange={(e) => onChange(parseInt(e.target.value, 10))}
      />
    </div>
  );
}
