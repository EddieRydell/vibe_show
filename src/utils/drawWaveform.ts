// Standalone waveform canvas drawing function extracted from Timeline.tsx.

import type { WaveformData } from "../hooks/useAudio";

/**
 * Draw a waveform on a canvas element.
 *
 * @param canvas  The target canvas element (or null to no-op).
 * @param height  The desired CSS-pixel height of the waveform.
 * @param alpha   Global alpha for the waveform bars.
 * @param waveform  The peak data and duration.
 * @param contentWidth  Total content width in CSS pixels.
 * @param pxPerSec  Horizontal zoom scale.
 */
export function drawWaveform(
  canvas: HTMLCanvasElement | null,
  height: number,
  alpha: number,
  waveform: WaveformData | null | undefined,
  contentWidth: number,
  pxPerSec: number,
): void {
  if (!canvas || !waveform) return;
  const width = Math.ceil(contentWidth);
  if (width <= 0 || height <= 0) return;

  const dpr = window.devicePixelRatio || 1;
  canvas.width = width * dpr;
  canvas.height = height * dpr;
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, width, height);

  const { peaks, duration: audioDuration } = waveform;
  if (audioDuration <= 0 || peaks.length === 0) return;

  ctx.fillStyle = getComputedStyle(canvas).getPropertyValue("--primary").trim() || "#6366f1";
  ctx.globalAlpha = alpha;

  const centerY = height / 2;
  const maxBarHeight = height / 2;

  for (let px = 0; px < width; px++) {
    const timeSec = px / pxPerSec;
    const peakIndex = Math.floor((timeSec / audioDuration) * peaks.length);
    if (peakIndex < 0 || peakIndex >= peaks.length) continue;
    const amplitude = peaks[peakIndex];
    const barH = amplitude * maxBarHeight;
    if (barH < 0.5) continue;
    ctx.fillRect(px, centerY - barH, 1, barH * 2);
  }
}
