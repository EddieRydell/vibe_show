import { useEffect, useRef } from "react";
import type { ScriptPreviewData } from "../../types";

interface Props {
  heatmap: ScriptPreviewData | null;
  strip: Array<[number, number, number, number]> | null;
  currentTime: number;
  playing: boolean;
  pixelCount: number;
  duration: number;
  onScrub: (t: number) => void;
  onTogglePlay: () => void;
  onDurationChange: (d: number) => void;
}

const DURATION_PRESETS = [1, 2, 5, 10, 30];

export function ScriptPreview({
  heatmap,
  strip,
  currentTime,
  playing,
  pixelCount,
  duration,
  onScrub,
  onTogglePlay,
  onDurationChange,
}: Props) {
  const stripCanvasRef = useRef<HTMLCanvasElement>(null);
  const heatmapCanvasRef = useRef<HTMLCanvasElement>(null);

  // Draw pixel strip
  useEffect(() => {
    const canvas = stripCanvasRef.current;
    if (!canvas || !strip) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);

    const cellW = w / strip.length;
    for (let i = 0; i < strip.length; i++) {
      const [r, g, b] = strip[i]!;
      ctx.fillStyle = `rgb(${r},${g},${b})`;
      ctx.fillRect(Math.floor(i * cellW), 0, Math.ceil(cellW) + 1, h);
    }
  }, [strip]);

  // Draw spacetime heatmap
  useEffect(() => {
    const canvas = heatmapCanvasRef.current;
    if (!canvas || !heatmap) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const { width, height, pixels } = heatmap;
    canvas.width = width;
    canvas.height = height;

    const imageData = ctx.createImageData(width, height);
    // heatmap is stored as rows (pixels) × cols (time samples)
    for (let row = 0; row < height; row++) {
      for (let col = 0; col < width; col++) {
        const srcIdx = (row * width + col) * 4;
        const dstIdx = (row * width + col) * 4;
        imageData.data[dstIdx] = pixels[srcIdx]!;
        imageData.data[dstIdx + 1] = pixels[srcIdx + 1]!;
        imageData.data[dstIdx + 2] = pixels[srcIdx + 2]!;
        imageData.data[dstIdx + 3] = pixels[srcIdx + 3]!;
      }
    }
    ctx.putImageData(imageData, 0, 0);
  }, [heatmap]);

  // Draw playhead overlay on heatmap
  useEffect(() => {
    const canvas = heatmapCanvasRef.current;
    if (!canvas || !heatmap) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Redraw the heatmap first
    const { width, height, pixels } = heatmap;
    const imageData = ctx.createImageData(width, height);
    for (let i = 0; i < pixels.length; i++) {
      imageData.data[i] = pixels[i]!;
    }
    ctx.putImageData(imageData, 0, 0);

    // Draw playhead line
    const x = currentTime * (width - 1);
    ctx.strokeStyle = "rgba(255, 255, 255, 0.8)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
  }, [heatmap, currentTime]);

  return (
    <div className="flex flex-col gap-2">
      {/* Pixel strip */}
      <div>
        <div className="text-text-2 mb-1 text-[10px] font-medium">Pixel Strip</div>
        <canvas
          ref={stripCanvasRef}
          width={320}
          height={40}
          className="border-border w-full rounded border"
          style={{ imageRendering: "pixelated" }}
        />
      </div>

      {/* Spacetime heatmap */}
      <div>
        <div className="text-text-2 mb-1 text-[10px] font-medium">
          Spacetime ({pixelCount}px)
        </div>
        <canvas
          ref={heatmapCanvasRef}
          width={100}
          height={50}
          className="border-border w-full rounded border"
          style={{ imageRendering: "pixelated", height: 120 }}
        />
      </div>

      {/* Time slider */}
      <div className="flex items-center gap-2">
        <button
          onClick={onTogglePlay}
          aria-label={playing ? "Pause preview" : "Play preview"}
          className="border-border bg-surface text-text-2 hover:text-text rounded border px-2 py-0.5 text-[10px]"
        >
          {playing ? "Pause" : "Play"}
        </button>
        <input
          type="range"
          aria-label="Preview time"
          min={0}
          max={1}
          step={0.001}
          value={currentTime}
          onChange={(e) => onScrub(Number(e.target.value))}
          className="flex-1"
        />
        <span className="text-text-2 w-10 text-right font-mono text-[10px]">
          {(currentTime * duration).toFixed(1)}s
        </span>
      </div>

      {/* Duration presets */}
      <div className="flex items-center gap-1.5">
        <span className="text-text-2 text-[10px]">Duration:</span>
        {DURATION_PRESETS.map((d) => (
          <button
            key={d}
            onClick={() => onDurationChange(d)}
            className={`rounded px-1.5 py-0.5 text-[10px] transition-colors ${
              duration === d
                ? "bg-primary/15 text-primary border-primary/30 border"
                : "border-border bg-surface text-text-2 hover:text-text border"
            }`}
          >
            {d}s
          </button>
        ))}
      </div>
    </div>
  );
}
