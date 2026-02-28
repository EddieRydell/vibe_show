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

// --- Fix A: Module-level thumbnail cache (survives unmount/remount) ---

interface CachedThumbnail {
  width: number;
  height: number;
  pixels: number[];
}

const MAX_CACHE_SIZE = 500;
const thumbnailCache = new Map<string, CachedThumbnail>();

function cacheKey(seq: number, track: number, effect: number, refresh?: number) {
  return `${seq}-${track}-${effect}-${refresh ?? 0}`;
}

function cacheSet(key: string, value: CachedThumbnail) {
  // LRU eviction: delete oldest entries when at capacity
  if (thumbnailCache.size >= MAX_CACHE_SIZE) {
    const first = thumbnailCache.keys().next().value;
    if (first !== undefined) thumbnailCache.delete(first);
  }
  thumbnailCache.set(key, value);
}

// --- Fix C: IPC concurrency limiter ---

let inFlight = 0;
const MAX_CONCURRENT = 6;
const waiting: (() => void)[] = [];

async function acquireSlot(): Promise<void> {
  if (inFlight < MAX_CONCURRENT) {
    inFlight++;
    return;
  }
  return new Promise((resolve) => waiting.push(resolve));
}

function releaseSlot() {
  inFlight--;
  const next = waiting.shift();
  if (next) {
    inFlight++;
    next();
  }
}

// --- Draw cached thumbnail to canvas ---

function drawThumbnail(
  canvas: HTMLCanvasElement,
  thumb: CachedThumbnail,
) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  canvas.width = thumb.width;
  canvas.height = thumb.height;
  const imageData = ctx.createImageData(thumb.width, thumb.height);
  for (let i = 0; i < thumb.pixels.length; i++) {
    imageData.data[i] = thumb.pixels[i]!;
  }
  ctx.putImageData(imageData, 0, 0);
}

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

  // Render thumbnail: check cache first, then IPC with concurrency limit
  useEffect(() => {
    if (!isVisible) return;
    const canvas = canvasRef.current;
    if (!canvas) return;

    const key = cacheKey(sequenceIndex, trackIndex, effectIndex, refreshKey);

    // Fix A: cache hit — draw immediately, no IPC
    const cached = thumbnailCache.get(key);
    if (cached) {
      drawThumbnail(canvas, cached);
      setLoaded(true);
      return;
    }

    // Cache miss — fetch via IPC with concurrency limit
    const ctrl = { cancelled: false };

    void (async () => {
      await acquireSlot();
      try {
        if (ctrl.cancelled) return;
        const thumb = await cmd.renderEffectThumbnail(
          sequenceIndex,
          trackIndex,
          effectIndex,
          THUMBNAIL_TIME_SAMPLES,
          THUMBNAIL_PIXEL_ROWS,
        );
        // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- checked across await boundary
        if (ctrl.cancelled || !thumb) return;

        // Store in cache
        cacheSet(key, {
          width: thumb.width,
          height: thumb.height,
          pixels: [...thumb.pixels],
        });

        drawThumbnail(canvas, thumb);
        setLoaded(true);
      } catch (e: unknown) {
        console.error("[VibeLights] Thumbnail render failed:", e);
      } finally {
        releaseSlot();
      }
    })();

    return () => {
      ctrl.cancelled = true;
    };
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
