import type { DockviewApi } from "dockview-react";

const STORAGE_KEY = "vibelights-layouts";
const LAYOUT_VERSION = 1;

interface StoredLayouts {
  version: number;
  layouts: Record<string, unknown>;
}

function getStore(): StoredLayouts {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { version: LAYOUT_VERSION, layouts: {} };
    const parsed = JSON.parse(raw) as StoredLayouts;
    if (parsed.version !== LAYOUT_VERSION) return { version: LAYOUT_VERSION, layouts: {} };
    return parsed;
  } catch {
    return { version: LAYOUT_VERSION, layouts: {} };
  }
}

function setStore(store: StoredLayouts): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
  } catch {
    // Storage full or unavailable — best effort
  }
}

export function saveLayout(screenKey: string, api: DockviewApi): void {
  const store = getStore();
  store.layouts[screenKey] = api.toJSON();
  setStore(store);
}

export function loadLayout(screenKey: string): Record<string, unknown> | null {
  const val = getStore().layouts[screenKey];
  if (val && typeof val === "object") return val as Record<string, unknown>;
  return null;
}

export function clearLayout(screenKey: string): void {
  const store = getStore();
  store.layouts = Object.fromEntries(
    Object.entries(store.layouts).filter(([k]) => k !== screenKey),
  );
  setStore(store);
}

/**
 * Creates a debounced auto-save handler.
 * Returns a cleanup function to clear pending timers.
 */
export function createAutoSave(
  screenKey: string,
  api: DockviewApi,
  delayMs = 500,
): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const disposable = api.onDidLayoutChange(() => {
    clearTimeout(timer);
    timer = setTimeout(() => saveLayout(screenKey, api), delayMs);
  });

  return () => {
    clearTimeout(timer);
    disposable.dispose();
  };
}
