/** Tauri IPC event names for preview window communication. */
export const EVENT_SHOW_REFRESHED = "show-refreshed";
export const EVENT_SELECTION_CHANGED = "selection-changed";

/** Tauri window label for the detached preview. */
export const PREVIEW_WINDOW_LABEL = "preview";

/** URL parameter value that identifies the preview window. */
export const VIEW_PREVIEW = "preview";

/** localStorage key for persisted UI settings (theme, accent, scale). */
export const STORAGE_KEY_UI_SETTINGS = "ui-settings";

/** Supported audio file extensions for import. */
export const AUDIO_EXTENSIONS = ["mp3", "wav", "ogg", "flac", "m4a", "aac"];

// ── Curve Presets ──────────────────────────────────────────────────

import type { Color, ColorStop, CurvePoint } from "./types";

export interface CurvePreset {
  name: string;
  points: CurvePoint[];
}

export const CURVE_PRESETS: CurvePreset[] = [
  {
    name: "Linear",
    points: [
      { x: 0, y: 0 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Constant",
    points: [
      { x: 0, y: 1 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Triangle",
    points: [
      { x: 0, y: 0 },
      { x: 0.5, y: 1 },
      { x: 1, y: 0 },
    ],
  },
  {
    name: "Ease In",
    points: [
      { x: 0, y: 0 },
      { x: 0.4, y: 0.02 },
      { x: 0.7, y: 0.2 },
      { x: 0.9, y: 0.6 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Ease Out",
    points: [
      { x: 0, y: 0 },
      { x: 0.1, y: 0.4 },
      { x: 0.3, y: 0.8 },
      { x: 0.6, y: 0.98 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Ease In-Out",
    points: [
      { x: 0, y: 0 },
      { x: 0.2, y: 0.02 },
      { x: 0.4, y: 0.15 },
      { x: 0.5, y: 0.5 },
      { x: 0.6, y: 0.85 },
      { x: 0.8, y: 0.98 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Exponential",
    points: [
      { x: 0, y: 0 },
      { x: 0.25, y: 0.02 },
      { x: 0.5, y: 0.06 },
      { x: 0.75, y: 0.25 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Logarithmic",
    points: [
      { x: 0, y: 0 },
      { x: 0.1, y: 0.5 },
      { x: 0.3, y: 0.75 },
      { x: 0.6, y: 0.9 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Sine Wave",
    points: Array.from({ length: 17 }, (_, i) => ({
      x: i / 16,
      y: (Math.sin(i / 16 * Math.PI * 2) + 1) / 2,
    })),
  },
  {
    name: "Sawtooth",
    points: [
      { x: 0, y: 0 },
      { x: 0.49, y: 1 },
      { x: 0.5, y: 0 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Pulse",
    points: [
      { x: 0, y: 0 },
      { x: 0.24, y: 0 },
      { x: 0.25, y: 1 },
      { x: 0.75, y: 1 },
      { x: 0.76, y: 0 },
      { x: 1, y: 0 },
    ],
  },
  {
    name: "Step (2)",
    points: [
      { x: 0, y: 0 },
      { x: 0.49, y: 0 },
      { x: 0.5, y: 1 },
      { x: 1, y: 1 },
    ],
  },
  {
    name: "Step (4)",
    points: [
      { x: 0, y: 0 },
      { x: 0.24, y: 0 },
      { x: 0.25, y: 0.33 },
      { x: 0.49, y: 0.33 },
      { x: 0.5, y: 0.66 },
      { x: 0.74, y: 0.66 },
      { x: 0.75, y: 1 },
      { x: 1, y: 1 },
    ],
  },
];

// ── Gradient Presets ───────────────────────────────────────────────

export interface GradientPreset {
  name: string;
  stops: ColorStop[];
}

function c(r: number, g: number, b: number): Color {
  return { r, g, b, a: 255 };
}

export const GRADIENT_PRESETS: GradientPreset[] = [
  {
    name: "Rainbow",
    stops: [
      { position: 0, color: c(255, 0, 0) },
      { position: 0.17, color: c(255, 165, 0) },
      { position: 0.33, color: c(255, 255, 0) },
      { position: 0.5, color: c(0, 255, 0) },
      { position: 0.67, color: c(0, 0, 255) },
      { position: 0.83, color: c(75, 0, 130) },
      { position: 1, color: c(148, 0, 211) },
    ],
  },
  {
    name: "Fire",
    stops: [
      { position: 0, color: c(0, 0, 0) },
      { position: 0.3, color: c(180, 30, 0) },
      { position: 0.6, color: c(255, 100, 0) },
      { position: 0.85, color: c(255, 200, 50) },
      { position: 1, color: c(255, 255, 200) },
    ],
  },
  {
    name: "Ocean",
    stops: [
      { position: 0, color: c(0, 10, 40) },
      { position: 0.3, color: c(0, 60, 120) },
      { position: 0.6, color: c(0, 130, 180) },
      { position: 0.85, color: c(80, 200, 220) },
      { position: 1, color: c(180, 240, 255) },
    ],
  },
  {
    name: "Sunset",
    stops: [
      { position: 0, color: c(20, 0, 60) },
      { position: 0.25, color: c(120, 20, 80) },
      { position: 0.5, color: c(220, 60, 40) },
      { position: 0.75, color: c(255, 160, 30) },
      { position: 1, color: c(255, 230, 80) },
    ],
  },
  {
    name: "Forest",
    stops: [
      { position: 0, color: c(10, 30, 10) },
      { position: 0.3, color: c(20, 80, 20) },
      { position: 0.6, color: c(40, 140, 40) },
      { position: 0.85, color: c(100, 200, 60) },
      { position: 1, color: c(180, 240, 120) },
    ],
  },
  {
    name: "Ice",
    stops: [
      { position: 0, color: c(200, 230, 255) },
      { position: 0.3, color: c(140, 200, 255) },
      { position: 0.6, color: c(80, 160, 240) },
      { position: 0.85, color: c(40, 100, 200) },
      { position: 1, color: c(20, 60, 160) },
    ],
  },
  {
    name: "Warm",
    stops: [
      { position: 0, color: c(255, 80, 20) },
      { position: 0.5, color: c(255, 180, 40) },
      { position: 1, color: c(255, 240, 100) },
    ],
  },
  {
    name: "Cool",
    stops: [
      { position: 0, color: c(0, 60, 200) },
      { position: 0.5, color: c(0, 180, 220) },
      { position: 1, color: c(100, 240, 255) },
    ],
  },
  {
    name: "Neon",
    stops: [
      { position: 0, color: c(255, 0, 255) },
      { position: 0.33, color: c(0, 255, 255) },
      { position: 0.66, color: c(255, 255, 0) },
      { position: 1, color: c(255, 0, 255) },
    ],
  },
  {
    name: "Pastel",
    stops: [
      { position: 0, color: c(255, 180, 200) },
      { position: 0.33, color: c(180, 200, 255) },
      { position: 0.66, color: c(180, 255, 200) },
      { position: 1, color: c(255, 230, 180) },
    ],
  },
  {
    name: "Christmas",
    stops: [
      { position: 0, color: c(200, 0, 0) },
      { position: 0.33, color: c(0, 150, 0) },
      { position: 0.66, color: c(200, 0, 0) },
      { position: 1, color: c(0, 150, 0) },
    ],
  },
];
