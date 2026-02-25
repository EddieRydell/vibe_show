import { useCallback, useEffect, useState } from "react";

export type ThemeMode = "light" | "dark" | "system";

export interface UISettings {
  theme: ThemeMode;
  accentColor: string;
  uiScale: number;
}

const STORAGE_KEY = "ui-settings";
const DEFAULT_ACCENT = "#3B82F6";

const DEFAULTS: UISettings = {
  theme: "system",
  accentColor: DEFAULT_ACCENT,
  uiScale: 100,
};

function load(): UISettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULTS;
    const parsed = JSON.parse(raw) as Partial<UISettings>;
    return {
      theme: parsed.theme ?? DEFAULTS.theme,
      accentColor: parsed.accentColor ?? DEFAULTS.accentColor,
      uiScale: parsed.uiScale ?? DEFAULTS.uiScale,
    };
  } catch {
    return DEFAULTS;
  }
}

function save(settings: UISettings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}

/** Compute a hover variant: darken in light mode, lighten in dark mode. */
function computeHoverColor(hex: string, isDark: boolean): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);

  const factor = isDark ? 1.2 : 0.85;
  const clamp = (v: number) => Math.min(255, Math.max(0, Math.round(v * factor)));

  return `#${clamp(r).toString(16).padStart(2, "0")}${clamp(g).toString(16).padStart(2, "0")}${clamp(b).toString(16).padStart(2, "0")}`;
}

function resolveIsDark(theme: ThemeMode): boolean {
  if (theme === "dark") return true;
  if (theme === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/** Apply all UI settings to the document. */
export function applyUISettings(settings: UISettings) {
  const root = document.documentElement;
  const isDark = resolveIsDark(settings.theme);

  // Theme class
  if (isDark) {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }

  // Sync localStorage theme key for the index.html blocking script
  if (settings.theme === "system") {
    localStorage.removeItem("theme");
  } else {
    localStorage.setItem("theme", settings.theme);
  }

  // Accent color
  root.style.setProperty("--primary", settings.accentColor);
  root.style.setProperty("--primary-hover", computeHoverColor(settings.accentColor, isDark));

  // UI scale
  root.style.zoom = `${settings.uiScale}%`;
}

/** Hook for reading and updating UI settings. */
export function useUISettings() {
  const [settings, setSettings] = useState<UISettings>(load);

  // Apply on mount and whenever settings change
  useEffect(() => {
    applyUISettings(settings);
    save(settings);
  }, [settings]);

  // Listen for system theme changes when in "system" mode
  useEffect(() => {
    if (settings.theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyUISettings(settings);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [settings]);

  const update = useCallback((partial: Partial<UISettings>) => {
    setSettings((prev) => ({ ...prev, ...partial }));
  }, []);

  const reset = useCallback(() => {
    setSettings(DEFAULTS);
  }, []);

  return { settings, update, reset, defaults: DEFAULTS };
}
