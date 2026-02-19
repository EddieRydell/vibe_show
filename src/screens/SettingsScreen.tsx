import { useUISettings, type ThemeMode } from "../hooks/useUISettings";

interface Props {
  onBack: () => void;
}

const ACCENT_PRESETS = [
  { label: "Violet", value: "#7C5CFF" },
  { label: "Blue", value: "#3B82F6" },
  { label: "Teal", value: "#14B8A6" },
  { label: "Green", value: "#22C55E" },
  { label: "Orange", value: "#F97316" },
  { label: "Rose", value: "#F43F5E" },
];

const SCALE_OPTIONS = [85, 90, 95, 100, 105, 110, 115, 120];

export function SettingsScreen({ onBack }: Props) {
  const { settings, update, reset, defaults } = useUISettings();

  return (
    <div className="bg-bg text-text flex h-screen flex-col">
      {/* Header */}
      <div className="border-border flex items-center border-b px-6 py-3">
        <button
          onClick={onBack}
          className="text-text-2 hover:text-text mr-3 text-sm transition-colors"
        >
          &larr; Back
        </button>
        <h2 className="text-text text-lg font-bold">Settings</h2>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-lg space-y-8 p-6">
          {/* ── Theme ── */}
          <Section title="Theme">
            <div className="flex gap-2">
              {(["light", "dark", "system"] as ThemeMode[]).map((mode) => (
                <button
                  key={mode}
                  onClick={() => update({ theme: mode })}
                  className={`rounded-lg border px-4 py-2 text-sm font-medium capitalize transition-colors ${
                    settings.theme === mode
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
                  }`}
                >
                  {mode}
                </button>
              ))}
            </div>
            <p className="text-text-2 mt-2 text-xs">
              {settings.theme === "system"
                ? "Follows your operating system preference."
                : `Always use ${settings.theme} mode.`}
            </p>
          </Section>

          {/* ── Accent Color ── */}
          <Section title="Accent Color">
            <div className="flex flex-wrap gap-2">
              {ACCENT_PRESETS.map((preset) => (
                <button
                  key={preset.value}
                  onClick={() => update({ accentColor: preset.value })}
                  className={`flex items-center gap-2 rounded-lg border px-3 py-1.5 text-xs transition-colors ${
                    settings.accentColor.toLowerCase() === preset.value.toLowerCase()
                      ? "border-primary bg-primary/10 text-text"
                      : "border-border bg-surface text-text-2 hover:bg-surface-2"
                  }`}
                >
                  <span
                    className="h-3 w-3 rounded-full"
                    style={{ backgroundColor: preset.value }}
                  />
                  {preset.label}
                </button>
              ))}
            </div>
            <div className="mt-3 flex items-center gap-3">
              <label className="text-text-2 text-xs">Custom</label>
              <div className="relative">
                <div
                  className="border-border h-8 w-8 rounded border"
                  style={{ backgroundColor: settings.accentColor }}
                />
                <input
                  type="color"
                  className="absolute inset-0 cursor-pointer opacity-0"
                  value={settings.accentColor}
                  onChange={(e) => update({ accentColor: e.target.value })}
                />
              </div>
              <input
                type="text"
                className="border-border bg-surface text-text w-24 rounded border px-2 py-1 font-mono text-xs uppercase outline-none focus:border-primary"
                value={settings.accentColor}
                onChange={(e) => {
                  const v = e.target.value;
                  if (/^#[0-9a-fA-F]{6}$/.test(v)) {
                    update({ accentColor: v });
                  }
                }}
              />
            </div>
          </Section>

          {/* ── UI Scale ── */}
          <Section title="UI Scale">
            <div className="flex flex-wrap gap-1.5">
              {SCALE_OPTIONS.map((scale) => (
                <button
                  key={scale}
                  onClick={() => update({ uiScale: scale })}
                  className={`rounded border px-3 py-1.5 text-xs font-medium transition-colors ${
                    settings.uiScale === scale
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
                  }`}
                >
                  {scale}%
                </button>
              ))}
            </div>
            <p className="text-text-2 mt-2 text-xs">
              Scales the entire interface. Default is 100%.
            </p>
          </Section>

          {/* ── Reset ── */}
          <div className="border-border border-t pt-6">
            <button
              onClick={reset}
              className="text-text-2 hover:text-text text-xs underline transition-colors"
            >
              Reset all settings to defaults
            </button>
            {settings.accentColor !== defaults.accentColor && (
              <p className="text-text-2 mt-1 text-[10px]">
                Default accent: {defaults.accentColor}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h3 className="text-text mb-3 text-sm font-medium">{title}</h3>
      {children}
    </div>
  );
}
