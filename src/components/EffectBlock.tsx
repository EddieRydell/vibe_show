import { useEffect, useRef, useState } from "react";
import { cmd } from "../commands";

interface EffectBlockProps {
  sequenceIndex: number;
  trackIndex: number;
  effectIndex: number;
  refreshKey?: number | undefined;
}

const THUMBNAIL_TIME_SAMPLES = 120;
const THUMBNAIL_PIXEL_ROWS = 16;

export function EffectBlock({
  sequenceIndex,
  trackIndex,
  effectIndex,
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
        if (entry!.isIntersecting) {
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

    cmd.renderEffectThumbnail(
      sequenceIndex,
      trackIndex,
      effectIndex,
      THUMBNAIL_TIME_SAMPLES,
      THUMBNAIL_PIXEL_ROWS,
    )
      .then((thumb) => {
        if (!thumb) return;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        canvas.width = thumb.width;
        canvas.height = thumb.height;

        const imageData = ctx.createImageData(thumb.width, thumb.height);
        for (let i = 0; i < thumb.pixels.length; i++) {
          imageData.data[i] = thumb.pixels[i]!;
        }
        ctx.putImageData(imageData, 0, 0);
        setLoaded(true);
      })
      .catch((e: unknown) => console.error("[VibeLights] Thumbnail render failed:", e));
  }, [isVisible, sequenceIndex, trackIndex, effectIndex, refreshKey]);

  return (
    <div ref={containerRef} className="relative size-full overflow-hidden">
      <canvas
        ref={canvasRef}
        className="absolute inset-0 size-full"
        style={{
          imageRendering: "pixelated",
          opacity: loaded ? 1 : 0,
        }}
      />
    </div>
  );
}
