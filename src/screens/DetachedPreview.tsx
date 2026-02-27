import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { cmd } from "../commands";
import { SHOW_REFRESHED, SELECTION_CHANGED } from "../events";
import { useTauriListener } from "../hooks/useTauriListener";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Frame, Show } from "../types";
import { usePreviewRenderer } from "../hooks/usePreviewRenderer";
import { usePreviewSettings } from "../hooks/usePreviewSettings";
import { usePreviewCamera } from "../hooks/usePreviewCamera";
import { PreviewToolbar } from "../components/PreviewToolbar";
import { AppBar } from "../components/AppBar";

// ── Selection preview loop (adapted from usePreviewLoop) ────────────

interface SelectionInfo {
  effects: [number, number][];
  start: number;
  end: number;
}

function parseSelection(
  selected: [number, number][],
  show: Show | null,
  sequenceIndex: number,
): SelectionInfo | null {
  if (!show || selected.length === 0) return null;
  const sequence = show.sequences[sequenceIndex];
  if (!sequence) return null;

  let min = Infinity;
  let max = -Infinity;
  const valid: [number, number][] = [];

  for (const [trackIdx, effectIdx] of selected) {
    const track = sequence.tracks[trackIdx];
    if (!track) continue;
    const effect = track.effects[effectIdx];
    if (!effect) continue;
    valid.push([trackIdx, effectIdx]);
    if (effect.time_range.start < min) min = effect.time_range.start;
    if (effect.time_range.end > max) max = effect.time_range.end;
  }

  if (valid.length === 0 || !isFinite(min) || !isFinite(max) || min >= max) return null;
  return { effects: valid, start: min, end: max };
}

export function DetachedPreview() {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [show, setShow] = useState<Show | null>(null);
  const [frame, setFrame] = useState<Frame | null>(null);
  const [selectedEffects, setSelectedEffects] = useState<[number, number][]>([]);
  const [mainPlaying, setMainPlaying] = useState(false);
  const [selectionFrame, setSelectionFrame] = useState<Frame | null>(null);
  const [error, setError] = useState<string | null>(null);

  const { settings, update: updateSettings, reset: resetSettings } = usePreviewSettings();
  const { camera, resetView, handlers: cameraHandlers } = usePreviewCamera();

  // Fetch show data
  const fetchShow = useCallback(() => {
    cmd.getShow()
      .then((s) => { setShow(s); setError(null); })
      .catch((e: unknown) => setError(String(e)));
  }, []);

  useEffect(() => {
    fetchShow();
  }, [fetchShow]);

  // Listen for show-refreshed events from the main window
  useTauriListener(SHOW_REFRESHED, () => {
    fetchShow();
  }, [fetchShow]);

  // Listen for selection-changed events from the main window
  useTauriListener<{ effects: [number, number][] }>(SELECTION_CHANGED, (payload) => {
    setSelectedEffects(payload.effects);
  });

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

  // Main playback frame loop — calls tick() to drive time from this window
  useEffect(() => {
    let cancelled = false;
    let rafId = 0;

    const loop = () => {
      if (cancelled) return;
      cmd.tick(0)
        .then((result) => {
          if (cancelled) return;
          setError(null);
          if (result) {
            setMainPlaying(result.playing);
            setFrame(result.frame);
            return;
          }
          // Paused — read current state for display
          return cmd.getPlayback().then((pb) => {
            if (cancelled) return;
            setMainPlaying(pb.playing);
            return cmd.getFrame(pb.current_time);
          }).then((f) => {
            if (f && !cancelled) setFrame(f);
          });
        })
        .catch((e: unknown) => setError(String(e)))
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

  // Selection preview loop: runs when paused and selection is non-empty
  const selectionInfo = useMemo(
    () => parseSelection(selectedEffects, show, 0),
    [selectedEffects, show],
  );

  const isPreviewingSelection = selectionInfo != null && !mainPlaying;
  const selectionEffectsRef = useRef(selectionInfo?.effects ?? []);
  selectionEffectsRef.current = selectionInfo?.effects ?? [];

  useEffect(() => {
    if (!selectionInfo || mainPlaying) {
      setSelectionFrame(null);
      return;
    }

    const { start, end } = selectionInfo;
    const duration = end - start;
    let cancelled = false;
    let currentTime = start;
    let lastTimestamp: number | null = null;
    let rafId = 0;

    const loop = (timestamp: number) => {
      if (cancelled) return;

      const dt = lastTimestamp != null ? (timestamp - lastTimestamp) / 1000.0 : 0;
      lastTimestamp = timestamp;

      currentTime += dt;
      if (currentTime >= end) {
        currentTime = start + ((currentTime - start) % duration);
      }

      const t = currentTime;

      cmd.getFrameFiltered(t, selectionEffectsRef.current)
        .then((f) => {
          if (!cancelled) { setSelectionFrame(f); setError(null); }
        })
        .catch((e: unknown) => setError(String(e)))
        .finally(() => {
          if (!cancelled) rafId = requestAnimationFrame(loop);
        });
    };

    rafId = requestAnimationFrame(loop);
    return () => {
      cancelled = true;
      cancelAnimationFrame(rafId);
      setSelectionFrame(null);
    };
  }, [selectionInfo, mainPlaying]);

  const displayFrame = isPreviewingSelection ? selectionFrame : frame;

  const handleClose = useCallback(() => {
    void getCurrentWindow().close();
  }, []);

  const handleResetAll = useCallback(() => {
    resetView();
    resetSettings();
  }, [resetView, resetSettings]);

  usePreviewRenderer(canvasRef, show, displayFrame, size.width, size.height, settings, camera);

  return (
    <div className="bg-bg text-text flex h-full flex-col">
      <AppBar onClose={handleClose} />

      {error && (
        <div className="bg-red-600/90 px-3 py-1.5 text-center text-sm text-white">
          {error}
        </div>
      )}

      <PreviewToolbar
        settings={settings}
        onUpdate={updateSettings}
        isPreviewingSelection={isPreviewingSelection}
        onResetView={handleResetAll}
      />

      <div
        ref={containerRef}
        className="flex-1 overflow-hidden"
        {...cameraHandlers}
      >
        <canvas ref={canvasRef} className="block size-full " />
      </div>
    </div>
  );
}
