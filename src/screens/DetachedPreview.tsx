import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Frame, PlaybackInfo, Show } from "../types";
import { usePreviewRenderer } from "../hooks/usePreviewRenderer";
import { AppBar } from "../components/AppBar";

export function DetachedPreview() {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [show, setShow] = useState<Show | null>(null);
  const [frame, setFrame] = useState<Frame | null>(null);
  const [loading, setLoading] = useState(true);

  // Fetch show data on mount
  const fetchShow = useCallback(() => {
    invoke<Show>("get_show")
      .then((s) => {
        setShow(s);
        setLoading(false);
      })
      .catch((e) => {
        console.error("[VibeLights] DetachedPreview get_show failed:", e);
        setLoading(false);
      });
  }, []);

  useEffect(() => {
    fetchShow();
  }, [fetchShow]);

  // Listen for show-refreshed events from the main window
  useEffect(() => {
    const unlisten = listen("show-refreshed", () => {
      fetchShow();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [fetchShow]);

  // Observe container size
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        setSize({
          width: Math.floor(width),
          height: Math.floor(height),
        });
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  // Animation loop: poll playback state and fetch frames
  useEffect(() => {
    let cancelled = false;
    let rafId = 0;

    const loop = () => {
      if (cancelled) return;
      invoke<PlaybackInfo>("get_playback")
        .then((pb) => {
          if (cancelled) return;
          return invoke<Frame>("get_frame", { time: pb.current_time });
        })
        .then((f) => {
          if (f && !cancelled) setFrame(f);
        })
        .catch(() => {})
        .finally(() => {
          if (!cancelled) rafId = requestAnimationFrame(loop);
        });
    };

    rafId = requestAnimationFrame(loop);
    return () => {
      cancelled = true;
      cancelAnimationFrame(rafId);
    };
  }, []);

  const handleReattach = useCallback(() => {
    emit("preview-reattach");
    getCurrentWindow().close();
  }, []);

  usePreviewRenderer(canvasRef, show, frame, size.width, size.height);

  return (
    <div className="bg-bg text-text flex h-screen flex-col">
      <AppBar />

      {/* Reattach toolbar */}
      <div className="border-border bg-surface flex items-center border-b px-3 py-1">
        <span className="text-text-2 flex-1 text-[11px] tracking-wider uppercase">
          Preview
        </span>
        <button
          onClick={handleReattach}
          className="text-text-2 hover:bg-surface-2 hover:text-text rounded px-2 py-0.5 text-[11px] transition-colors"
          title="Reattach to main window"
        >
          &#x2199; Reattach
        </button>
      </div>

      {loading ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3">
          <div className="border-primary h-6 w-6 animate-spin rounded-full border-2 border-t-transparent" />
          <p className="text-text-2 text-sm">Loading preview...</p>
        </div>
      ) : (
        <div
          ref={containerRef}
          className="flex-1 overflow-hidden"
        >
          <canvas ref={canvasRef} className="block h-full w-full" />
        </div>
      )}
    </div>
  );
}
