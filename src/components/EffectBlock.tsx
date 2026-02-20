import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { BlendMode, EffectThumbnail } from "../types";

interface EffectBlockProps {
  sequenceIndex: number;
  trackIndex: number;
  effectIndex: number;
  effectKind: string;
  blendMode: BlendMode;
  compact?: boolean;
  refreshKey?: number;
}

const THUMBNAIL_TIME_SAMPLES = 120;
const THUMBNAIL_PIXEL_ROWS = 16;

const BLEND_LABELS: Record<BlendMode, string> = {
  Override: "OVR",
  Add: "ADD",
  Multiply: "MUL",
  Max: "MAX",
  Alpha: "ALP",
  Subtract: "SUB",
  Min: "MIN",
  Average: "AVG",
  Screen: "SCR",
  Mask: "MSK",
  IntensityOverlay: "INT",
};

export function EffectBlock({
  sequenceIndex,
  trackIndex,
  effectIndex,
  effectKind,
  blendMode,
  compact,
  refreshKey,
}: EffectBlockProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [loaded, setLoaded] = useState(false);
  const [isVisible, setIsVisible] = useState(false);

  // IntersectionObserver: only mark visible once, then disconnect
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setIsVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Only fire the IPC thumbnail call once visible
  useEffect(() => {
    if (!isVisible) return;
    const canvas = canvasRef.current;
    if (!canvas) return;

    invoke<EffectThumbnail | null>("render_effect_thumbnail", {
      sequenceIndex,
      trackIndex,
      effectIndex,
      timeSamples: THUMBNAIL_TIME_SAMPLES,
      pixelRows: THUMBNAIL_PIXEL_ROWS,
    })
      .then((thumb) => {
        if (!thumb) return;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        canvas.width = thumb.width;
        canvas.height = thumb.height;

        const imageData = ctx.createImageData(thumb.width, thumb.height);
        for (let i = 0; i < thumb.pixels.length; i++) {
          imageData.data[i] = thumb.pixels[i];
        }
        ctx.putImageData(imageData, 0, 0);
        setLoaded(true);
      })
      .catch((e) => console.error("[VibeLights] Thumbnail render failed:", e));
  }, [isVisible, sequenceIndex, trackIndex, effectIndex, refreshKey]);

  return (
    <div ref={containerRef} className="relative flex h-full w-full items-end overflow-hidden">
      <canvas
        ref={canvasRef}
        className="absolute inset-0 h-full w-full"
        style={{
          imageRendering: "pixelated",
          opacity: loaded ? 1 : 0,
        }}
      />
      {/* Labels overlay */}
      <div className="relative z-10 flex w-full items-center justify-between px-1 pb-0.5">
        <span
          className={`truncate font-semibold text-white drop-shadow-[0_1px_2px_rgba(0,0,0,0.9)] ${compact ? "text-[8px]" : "text-[9px]"}`}
        >
          {effectKind}
        </span>
        {!compact && blendMode !== "Override" && (
          <span className="shrink-0 rounded-sm bg-black/50 px-1 text-[8px] font-medium text-white/70">
            {BLEND_LABELS[blendMode]}
          </span>
        )}
      </div>
    </div>
  );
}
