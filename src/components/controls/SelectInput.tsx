interface SelectInputProps {
  label: string;
  value: string;
  options: string[];
  onChange: (value: string) => void;
}

export function SelectInput({ label, value, options, onChange }: SelectInputProps) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-text-2 text-[11px]">{label}</label>
      <select
        className="bg-surface-2 border-border text-text w-full rounded border px-1.5 py-1 text-[11px]"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        {options.map((opt) => (
          <option key={opt} value={opt}>
            {opt}
          </option>
        ))}
      </select>
    </div>
  );
}
