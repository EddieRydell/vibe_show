import type { Color } from "../types";

/** Convert a Color to a hex string like "#ff00aa". */
export function colorToHex(c: Color): string {
  return `#${c.r.toString(16).padStart(2, "0")}${c.g.toString(16).padStart(2, "0")}${c.b.toString(16).padStart(2, "0")}`;
}

/** Parse a "#RRGGBB" hex string into a Color. Assumes valid input. */
export function hexToColor(hex: string): Color {
  return {
    r: parseInt(hex.slice(1, 3), 16),
    g: parseInt(hex.slice(3, 5), 16),
    b: parseInt(hex.slice(5, 7), 16),
    a: 255,
  };
}

/** Parse a hex string into a Color, returning null if invalid. Accepts with or without "#". */
export function tryParseHex(hex: string): Color | null {
  const match = hex.match(/^#?([0-9a-f]{6})$/i);
  if (!match) return null;
  const h = match[1]!;
  return {
    r: parseInt(h.slice(0, 2), 16),
    g: parseInt(h.slice(2, 4), 16),
    b: parseInt(h.slice(4, 6), 16),
    a: 255,
  };
}

/** Convert a Color to a CSS rgb() string for canvas/style use. */
export function colorToCSS(c: Color): string {
  return `rgb(${c.r},${c.g},${c.b})`;
}

/** Linearly interpolate between two colors. */
export function lerpColor(a: Color, b: Color, t: number): Color {
  return {
    r: Math.round(a.r + (b.r - a.r) * t),
    g: Math.round(a.g + (b.g - a.g) * t),
    b: Math.round(a.b + (b.b - a.b) * t),
    a: 255,
  };
}

/** Convert RGB (0–255 each) to HSV. Returns [h (0–360), s (0–1), v (0–1)]. */
export function rgbToHsv(r: number, g: number, b: number): [number, number, number] {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  let h = 0;
  if (d > 0) {
    if (max === r) h = 60 * (((g - b) / d + 6) % 6);
    else if (max === g) h = 60 * ((b - r) / d + 2);
    else h = 60 * ((r - g) / d + 4);
  }
  const s = max === 0 ? 0 : d / max;
  return [h, s, max];
}

/** Convert HSV to a Color. h: 0–360, s: 0–1, v: 0–1. */
export function hsvToRgb(h: number, s: number, v: number): Color {
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  let r = 0,
    g = 0,
    b = 0;
  if (h < 60) {
    r = c;
    g = x;
  } else if (h < 120) {
    r = x;
    g = c;
  } else if (h < 180) {
    g = c;
    b = x;
  } else if (h < 240) {
    g = x;
    b = c;
  } else if (h < 300) {
    r = x;
    b = c;
  } else {
    r = c;
    b = x;
  }
  return {
    r: Math.round((r + m) * 255),
    g: Math.round((g + m) * 255),
    b: Math.round((b + m) * 255),
    a: 255,
  };
}
