import { useCallback, useEffect, useState } from "react";

export interface PreviewSettings {
  bulbSize: number;
  bulbOpacity: number;
  glowSize: number;
  glowOpacity: number;
}

const STORAGE_KEY = "preview-settings";

const DEFAULTS: PreviewSettings = {
  bulbSize: 1,
  bulbOpacity: 1,
  glowSize: 1,
  glowOpacity: 0.12,
};

function load(): PreviewSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULTS;
    const parsed = JSON.parse(raw);
    return {
      bulbSize: parsed.bulbSize ?? DEFAULTS.bulbSize,
      bulbOpacity: parsed.bulbOpacity ?? DEFAULTS.bulbOpacity,
      glowSize: parsed.glowSize ?? DEFAULTS.glowSize,
      glowOpacity: parsed.glowOpacity ?? DEFAULTS.glowOpacity,
    };
  } catch {
    return DEFAULTS;
  }
}

export function usePreviewSettings() {
  const [settings, setSettings] = useState<PreviewSettings>(load);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  }, [settings]);

  const update = useCallback((partial: Partial<PreviewSettings>) => {
    setSettings((prev) => ({ ...prev, ...partial }));
  }, []);

  const reset = useCallback(() => {
    setSettings(DEFAULTS);
  }, []);

  return { settings, update, reset };
}
