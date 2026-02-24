interface BoolToggleProps {
  label: string;
  value: boolean;
  onChange: (value: boolean) => void;
}

export function BoolToggle({ label, value, onChange }: BoolToggleProps) {
  return (
    <div className="flex items-center justify-between">
      <label className="text-text-2 text-[11px]">{label}</label>
      <button
        type="button"
        role="switch"
        aria-checked={value}
        aria-label={label}
        className={`relative h-5 w-9 rounded-full transition-colors ${value ? "bg-primary" : "bg-surface-2"}`}
        onClick={() => onChange(!value)}
      >
        <span
          className={`absolute top-0.5 left-0.5 size-4  rounded-full bg-white transition-transform ${value ? "translate-x-4" : ""}`}
        />
      </button>
    </div>
  );
}
