import { useCallback, useEffect, useRef } from "react";
import type { Frame, Show } from "../types";
import { BULB_SHAPE_RADIUS } from "../types";

const BG_COLOR = "#0E0E0E";
const OUTLINE_COLOR = "#1D1D1D";
const BASE_RADIUS = 6;
const GLOW_SCALE = 4; // 1/4 resolution for glow layer

// ── Geometry ────────────────────────────────────────────────────────

interface Geometry {
  /** Screen X positions for every pixel (flat across all fixtures). */
  xs: Float32Array;
  /** Screen Y positions for every pixel. */
  ys: Float32Array;
  /** Per-pixel display radius. */
  radii: Float32Array;
  /** Map from fixture_id to [startIndex, count] in the flat arrays. */
  fixtureSlices: Map<number, [number, number]>;
  /** Total pixel count. */
  totalPixels: number;
  /** Canvas logical width/height used to compute this geometry. */
  width: number;
  height: number;
}

function computeGeometry(
  show: Show,
  width: number,
  height: number,
): Geometry {
  // Count total pixels and build fixture order
  let totalPixels = 0;
  const fixtureOrder: { fixtureId: number; positions: { x: number; y: number }[]; radiusMul: number }[] = [];

  for (const fl of show.layout.fixtures) {
    const def = show.fixtures.find((f) => f.id === fl.fixture_id);
    const bulbShape = def?.bulb_shape ?? "LED";
    const radiusMul = def?.display_radius_override ?? BULB_SHAPE_RADIUS[bulbShape] ?? 1.0;
    fixtureOrder.push({
      fixtureId: fl.fixture_id,
      positions: fl.pixel_positions,
      radiusMul,
    });
    totalPixels += fl.pixel_positions.length;
  }

  // Find bounding box of all normalized positions
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  for (const fo of fixtureOrder) {
    for (const p of fo.positions) {
      if (p.x < minX) minX = p.x;
      if (p.x > maxX) maxX = p.x;
      if (p.y < minY) minY = p.y;
      if (p.y > maxY) maxY = p.y;
    }
  }

  // Letterbox/pillarbox to maintain aspect ratio
  const layoutW = maxX - minX || 1;
  const layoutH = maxY - minY || 1;
  const layoutAspect = layoutW / layoutH;

  const padding = 24;
  const availW = width - padding * 2;
  const availH = height - padding * 2;
  const canvasAspect = availW / availH;

  let scale: number, offsetX: number, offsetY: number;
  if (canvasAspect > layoutAspect) {
    // Canvas is wider → pillarbox
    scale = availH / layoutH;
    offsetX = padding + (availW - layoutW * scale) / 2;
    offsetY = padding;
  } else {
    // Canvas is taller → letterbox
    scale = availW / layoutW;
    offsetX = padding;
    offsetY = padding + (availH - layoutH * scale) / 2;
  }

  const xs = new Float32Array(totalPixels);
  const ys = new Float32Array(totalPixels);
  const radii = new Float32Array(totalPixels);
  const fixtureSlices = new Map<number, [number, number]>();

  let idx = 0;
  for (const fo of fixtureOrder) {
    const start = idx;
    const r = BASE_RADIUS * fo.radiusMul;
    for (const p of fo.positions) {
      xs[idx] = offsetX + (p.x - minX) * scale;
      ys[idx] = offsetY + (p.y - minY) * scale;
      radii[idx] = r;
      idx++;
    }
    fixtureSlices.set(fo.fixtureId, [start, fo.positions.length]);
  }

  return { xs, ys, radii, fixtureSlices, totalPixels, width, height };
}

// ── Outline Layer ───────────────────────────────────────────────────

function renderOutlineLayer(
  canvas: OffscreenCanvas,
  geo: Geometry,
  dpr: number,
) {
  canvas.width = geo.width * dpr;
  canvas.height = geo.height * dpr;
  const ctx = canvas.getContext("2d")!;
  ctx.scale(dpr, dpr);

  ctx.fillStyle = BG_COLOR;
  ctx.fillRect(0, 0, geo.width, geo.height);

  // Single batched path for all outline circles
  ctx.beginPath();
  for (let i = 0; i < geo.totalPixels; i++) {
    ctx.moveTo(geo.xs[i] + geo.radii[i] + 1, geo.ys[i]);
    ctx.arc(geo.xs[i], geo.ys[i], geo.radii[i] + 1, 0, Math.PI * 2);
  }
  ctx.fillStyle = OUTLINE_COLOR;
  ctx.fill();
}

// ── Frame Rendering ─────────────────────────────────────────────────

/** Decode base64-encoded RGBA pixel data into a pre-allocated buffer. */
function decodeBase64Into(b64: string, target: Uint8Array, offset: number) {
  const bin = atob(b64);
  const len = bin.length;
  for (let i = 0; i < len; i++) {
    target[offset + i] = bin.charCodeAt(i);
  }
  return len;
}

function renderFrame(
  mainCanvas: HTMLCanvasElement,
  outlineCanvas: OffscreenCanvas,
  geo: Geometry,
  frame: Frame | null,
  colorBuf: Uint8Array,
  dpr: number,
) {
  const ctx = mainCanvas.getContext("2d")!;
  const w = geo.width;
  const h = geo.height;

  // 1. Stamp outlines (one drawImage)
  ctx.clearRect(0, 0, mainCanvas.width, mainCanvas.height);
  ctx.drawImage(outlineCanvas, 0, 0, w * dpr, h * dpr, 0, 0, mainCanvas.width, mainCanvas.height);

  // 2. Decode frame data into flat color buffer
  colorBuf.fill(0);
  if (frame) {
    for (const [fidStr, b64] of Object.entries(frame.fixtures)) {
      const fid = Number(fidStr);
      const slice = geo.fixtureSlices.get(fid);
      if (!slice) continue;
      const [startIdx] = slice;
      decodeBase64Into(b64, colorBuf, startIdx * 4);
    }
  }

  // 3. Group non-black pixels by hex color for batched drawing
  const colorGroups = new Map<number, number[]>();
  for (let i = 0; i < geo.totalPixels; i++) {
    const off = i * 4;
    const r = colorBuf[off], g = colorBuf[off + 1], b = colorBuf[off + 2];
    if (r === 0 && g === 0 && b === 0) continue;
    const key = (r << 16) | (g << 8) | b;
    let group = colorGroups.get(key);
    if (!group) {
      group = [];
      colorGroups.set(key, group);
    }
    group.push(i);
  }

  // 4. Glow layer at 1/4 resolution
  const glowW = Math.ceil(w / GLOW_SCALE);
  const glowH = Math.ceil(h / GLOW_SCALE);
  const glowCanvas = new OffscreenCanvas(glowW, glowH);
  const glowCtx = glowCanvas.getContext("2d")!;
  glowCtx.clearRect(0, 0, glowW, glowH);

  const glowRadius = Math.max(3, Math.round(BASE_RADIUS * 2.5 / GLOW_SCALE));
  const glowData = glowCtx.getImageData(0, 0, glowW, glowH);
  const gd = glowData.data;

  for (const [colorKey, indices] of colorGroups) {
    const cr = (colorKey >> 16) & 0xFF;
    const cg = (colorKey >> 8) & 0xFF;
    const cb = colorKey & 0xFF;

    for (const i of indices) {
      const cx = Math.round(geo.xs[i] / GLOW_SCALE);
      const cy = Math.round(geo.ys[i] / GLOW_SCALE);

      // Stamp radial falloff
      for (let dy = -glowRadius; dy < glowRadius; dy++) {
        const py = cy + dy;
        if (py < 0 || py >= glowH) continue;
        for (let dx = -glowRadius; dx < glowRadius; dx++) {
          const px = cx + dx;
          if (px < 0 || px >= glowW) continue;
          const dist = Math.sqrt(dx * dx + dy * dy) / glowRadius;
          if (dist >= 1) continue;
          const alpha = (1 - dist * dist) * 0.6;
          const off = (py * glowW + px) * 4;
          // Additive blend (clamped to 255)
          gd[off] = Math.min(255, gd[off] + cr * alpha);
          gd[off + 1] = Math.min(255, gd[off + 1] + cg * alpha);
          gd[off + 2] = Math.min(255, gd[off + 2] + cb * alpha);
          gd[off + 3] = Math.min(255, gd[off + 3] + 255 * alpha);
        }
      }
    }
  }

  glowCtx.putImageData(glowData, 0, 0);

  // 5. Composite glow (scaled up with smoothing, semi-transparent)
  ctx.save();
  ctx.globalAlpha = 0.15;
  ctx.imageSmoothingEnabled = true;
  ctx.drawImage(glowCanvas, 0, 0, glowW, glowH, 0, 0, mainCanvas.width, mainCanvas.height);
  ctx.restore();

  // 6. Batched color arcs (one fill per color group)
  ctx.save();
  ctx.scale(dpr, dpr);
  for (const [colorKey, indices] of colorGroups) {
    const cr = (colorKey >> 16) & 0xFF;
    const cg = (colorKey >> 8) & 0xFF;
    const cb = colorKey & 0xFF;

    ctx.beginPath();
    for (const i of indices) {
      ctx.moveTo(geo.xs[i] + geo.radii[i], geo.ys[i]);
      ctx.arc(geo.xs[i], geo.ys[i], geo.radii[i], 0, Math.PI * 2);
    }
    ctx.fillStyle = `rgb(${cr},${cg},${cb})`;
    ctx.fill();
  }
  ctx.restore();
}

// ── Hook ────────────────────────────────────────────────────────────

export function usePreviewRenderer(
  canvasRef: React.RefObject<HTMLCanvasElement | null>,
  show: Show | null,
  frame: Frame | null,
  width: number,
  height: number,
) {
  const geoRef = useRef<Geometry | null>(null);
  const outlineRef = useRef<OffscreenCanvas | null>(null);
  const colorBufRef = useRef<Uint8Array | null>(null);

  // Recompute geometry when show or canvas size changes
  const updateGeometry = useCallback(() => {
    if (!show || width <= 0 || height <= 0) {
      geoRef.current = null;
      return;
    }

    const dpr = window.devicePixelRatio || 1;
    const geo = computeGeometry(show, width, height);
    geoRef.current = geo;

    // Pre-allocate color buffer
    colorBufRef.current = new Uint8Array(geo.totalPixels * 4);

    // Render outline layer (only on geometry change)
    const outline = new OffscreenCanvas(width * dpr, height * dpr);
    renderOutlineLayer(outline, geo, dpr);
    outlineRef.current = outline;
  }, [show, width, height]);

  useEffect(() => {
    updateGeometry();
  }, [updateGeometry]);

  // Render frame
  useEffect(() => {
    const canvas = canvasRef.current;
    const geo = geoRef.current;
    const outline = outlineRef.current;
    const colorBuf = colorBufRef.current;
    if (!canvas || !geo || !outline || !colorBuf) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = geo.width * dpr;
    canvas.height = geo.height * dpr;

    renderFrame(canvas, outline, geo, frame, colorBuf, dpr);
  }, [canvasRef, frame, width, height, show]);
}
