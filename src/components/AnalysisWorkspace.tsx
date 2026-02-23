import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { WaveformData } from "../hooks/useAudio";
import type { AudioAnalysis } from "../types";

interface ManualBeat {
  id: string;
  time: number;
}

interface AnalysisWorkspaceProps {
  duration: number;
  currentTime: number;
  waveform: WaveformData | null;
  analysis: AudioAnalysis | null;
  manualBeats: ManualBeat[];
  selectedBeatId: string | null;
  onSeek: (time: number) => void;
  onAddBeat: (time: number) => void;
  onMoveBeat: (id: string, newTime: number) => void;
  onSelectBeat: (id: string | null) => void;
  onDeleteBeat: (id: string) => void;
}

const LABEL_WIDTH = 120;
const RULER_HEIGHT = 28;
const WAVEFORM_HEIGHT = 48;
const BEAT_LANE_HEIGHT = 36;
const MIN_PX_PER_SEC = 10;
const MAX_PX_PER_SEC = 500;
const DEFAULT_PX_PER_SEC = 40;
const ZOOM_FACTOR = 1.15;
const BEAT_HIT_WIDTH = 8;

function formatRulerTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return m > 0 ? `${m}:${s.toString().padStart(2, "0")}` : `${s}s`;
}

/** Map section labels to semi-transparent background colors. */
function sectionColor(label: string): string {
  const colors: Record<string, string> = {
    intro: "rgba(100, 149, 237, 0.12)",
    verse: "rgba(60, 179, 113, 0.12)",
    chorus: "rgba(255, 165, 0, 0.15)",
    bridge: "rgba(186, 85, 211, 0.12)",
    outro: "rgba(100, 149, 237, 0.12)",
    solo: "rgba(255, 99, 71, 0.12)",
    inst: "rgba(255, 215, 0, 0.12)",
  };
  const key = label.toLowerCase().replace(/[0-9]/g, "").trim();
  return colors[key] ?? "rgba(128, 128, 128, 0.08)";
}

export function AnalysisWorkspace({
  duration,
  currentTime,
  waveform,
  analysis,
  manualBeats,
  selectedBeatId,
  onSeek,
  onAddBeat,
  onMoveBeat,
  onSelectBeat,
  onDeleteBeat,
}: AnalysisWorkspaceProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const waveformCanvasRef = useRef<HTMLCanvasElement>(null);
  const [pxPerSec, setPxPerSec] = useState(DEFAULT_PX_PER_SEC);
  const pxPerSecRef = useRef(pxPerSec);
  pxPerSecRef.current = pxPerSec;

  // Drag state for beat movement
  const [dragState, setDragState] = useState<{
    id: string;
    startX: number;
    originalTime: number;
  } | null>(null);
  const dragStateRef = useRef(dragState);
  dragStateRef.current = dragState;
  const [dragPreviewTime, setDragPreviewTime] = useState<number | null>(null);

  const contentWidth = duration * pxPerSec;

  // ── Zoom ──────────────────────────────────────────────────────────

  const zoomIn = useCallback(() => {
    setPxPerSec((prev) => Math.min(prev * ZOOM_FACTOR, MAX_PX_PER_SEC));
  }, []);

  const zoomOut = useCallback(() => {
    setPxPerSec((prev) => Math.max(prev / ZOOM_FACTOR, MIN_PX_PER_SEC));
  }, []);

  const zoomFit = useCallback(() => {
    if (!scrollContainerRef.current || duration <= 0) return;
    const available = scrollContainerRef.current.clientWidth;
    setPxPerSec(Math.max(available / duration, MIN_PX_PER_SEC));
  }, [duration]);

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        if (e.deltaY < 0) zoomIn();
        else zoomOut();
      }
    },
    [zoomIn, zoomOut],
  );

  // ── Time from pixel ───────────────────────────────────────────────

  const timeFromClientX = useCallback(
    (clientX: number) => {
      const container = scrollContainerRef.current;
      if (!container) return 0;
      const rect = container.getBoundingClientRect();
      const scale = rect.width / container.offsetWidth;
      const x = (clientX - rect.left) / scale + container.scrollLeft;
      return Math.max(0, Math.min(x / pxPerSec, duration));
    },
    [pxPerSec, duration],
  );

  // ── Ruler ticks ───────────────────────────────────────────────────

  const idealTickSpacingPx = 80;
  const rawInterval = idealTickSpacingPx / pxPerSec;
  const niceIntervals = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60];
  const tickInterval = niceIntervals.find((iv) => iv >= rawInterval) ?? rawInterval;
  const ticks: number[] = [];
  for (let t = 0; t <= duration + tickInterval; t += tickInterval) {
    if (t <= duration) ticks.push(t);
  }

  const tickBackground = useMemo(() => {
    const tickPx = tickInterval * pxPerSec;
    if (tickPx < 2) return {};
    return {
      backgroundImage: `repeating-linear-gradient(to right, var(--border-tick) 0 1px, transparent 1px ${tickPx}px)`,
      backgroundSize: `${tickPx}px 100%`,
    };
  }, [tickInterval, pxPerSec]);

  // ── Waveform canvas drawing ───────────────────────────────────────

  const drawWaveform = useCallback(
    (canvas: HTMLCanvasElement | null) => {
      if (!canvas || !waveform) return;
      const width = Math.ceil(contentWidth);
      const height = WAVEFORM_HEIGHT;
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

      ctx.fillStyle =
        getComputedStyle(canvas).getPropertyValue("--primary").trim() || "#6366f1";
      ctx.globalAlpha = 0.7;

      const centerY = height / 2;
      const maxBarHeight = height / 2;

      // Draw as a continuous filled path for smooth appearance
      ctx.beginPath();
      ctx.moveTo(0, centerY);
      for (let px = 0; px < width; px++) {
        const timeSec = px / pxPerSec;
        const peakIndex = Math.floor((timeSec / audioDuration) * peaks.length);
        const amplitude = peakIndex >= 0 && peakIndex < peaks.length ? peaks[peakIndex] : 0;
        ctx.lineTo(px, centerY - amplitude * maxBarHeight);
      }
      ctx.lineTo(width, centerY);
      // Mirror bottom half
      for (let px = width - 1; px >= 0; px--) {
        const timeSec = px / pxPerSec;
        const peakIndex = Math.floor((timeSec / audioDuration) * peaks.length);
        const amplitude = peakIndex >= 0 && peakIndex < peaks.length ? peaks[peakIndex] : 0;
        ctx.lineTo(px, centerY + amplitude * maxBarHeight);
      }
      ctx.closePath();
      ctx.fill();
    },
    [waveform, contentWidth, pxPerSec],
  );

  useEffect(() => {
    drawWaveform(waveformCanvasRef.current);
  }, [drawWaveform]);

  // ── Click handlers ────────────────────────────────────────────────

  const handleRulerClick = useCallback(
    (e: React.MouseEvent) => {
      const time = timeFromClientX(e.clientX);
      onSeek(time);
    },
    [timeFromClientX, onSeek],
  );

  const handleWaveformClick = useCallback(
    (e: React.MouseEvent) => {
      const time = timeFromClientX(e.clientX);
      onSeek(time);
    },
    [timeFromClientX, onSeek],
  );

  const handleBeatLaneClick = useCallback(
    (e: React.MouseEvent) => {
      // Don't add beat if we just finished a drag
      if (dragStateRef.current) return;
      // Check if clicking on an existing beat marker
      const target = e.target as HTMLElement;
      if (target.closest("[data-beat-id]")) return;

      const time = timeFromClientX(e.clientX);
      onAddBeat(time);
    },
    [timeFromClientX, onAddBeat],
  );

  const handleBeatClick = useCallback(
    (e: React.MouseEvent, id: string) => {
      e.stopPropagation();
      onSelectBeat(id);
    },
    [onSelectBeat],
  );

  // ── Beat drag ─────────────────────────────────────────────────────

  const handleBeatMouseDown = useCallback(
    (e: React.MouseEvent, beat: ManualBeat) => {
      e.stopPropagation();
      e.preventDefault();
      onSelectBeat(beat.id);
      const state = {
        id: beat.id,
        startX: e.clientX,
        originalTime: beat.time,
      };
      setDragState(state);
      dragStateRef.current = state;
    },
    [onSelectBeat],
  );

  useEffect(() => {
    function handleMouseMove(e: MouseEvent) {
      const ds = dragStateRef.current;
      if (!ds) return;
      const pps = pxPerSecRef.current;
      const deltaPx = e.clientX - ds.startX;
      const deltaSec = deltaPx / pps;
      const newTime = Math.max(0, Math.min(ds.originalTime + deltaSec, duration));
      setDragPreviewTime(newTime);
    }

    function handleMouseUp(e: MouseEvent) {
      const ds = dragStateRef.current;
      if (!ds) return;
      dragStateRef.current = null;
      setDragState(null);

      const pps = pxPerSecRef.current;
      const deltaPx = e.clientX - ds.startX;
      const deltaSec = deltaPx / pps;
      const newTime = Math.max(0, Math.min(ds.originalTime + deltaSec, duration));

      // Only commit if actually dragged
      if (Math.abs(deltaPx) > 3) {
        onMoveBeat(ds.id, newTime);
      }
      setDragPreviewTime(null);
    }

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [duration, onMoveBeat]);

  // ── Keyboard shortcuts ────────────────────────────────────────────

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Delete" || e.key === "Backspace") {
        if (!selectedBeatId) return;
        if (e.target && (e.target as HTMLElement).closest("input, textarea")) return;
        e.preventDefault();
        onDeleteBeat(selectedBeatId);
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [selectedBeatId, onDeleteBeat]);

  // ── Auto-scroll to playhead during playback ───────────────────────

  const lastAutoScrollRef = useRef(0);
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    const now = Date.now();
    if (now - lastAutoScrollRef.current < 100) return;

    const playheadX = currentTime * pxPerSec;
    const viewLeft = container.scrollLeft;
    const viewRight = viewLeft + container.clientWidth;

    if (playheadX < viewLeft || playheadX > viewRight - 50) {
      container.scrollLeft = Math.max(0, playheadX - container.clientWidth * 0.25);
      lastAutoScrollRef.current = now;
    }
  }, [currentTime, pxPerSec]);

  // ── Playhead position ─────────────────────────────────────────────

  const playheadX = currentTime * pxPerSec;

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Zoom controls */}
      <div className="border-border bg-surface flex shrink-0 items-center gap-1 border-b px-3 py-1">
        <span className="text-text-2 text-[10px]">Zoom</span>
        <button
          onClick={zoomOut}
          className="border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text rounded border px-2 py-0.5 text-xs transition-colors"
        >
          -
        </button>
        <button
          onClick={zoomFit}
          className="border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text rounded border px-2 py-0.5 text-[10px] transition-colors"
        >
          Fit
        </button>
        <button
          onClick={zoomIn}
          className="border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text rounded border px-2 py-0.5 text-xs transition-colors"
        >
          +
        </button>
        <span className="text-text-2 ml-1 text-[10px]">{pxPerSec.toFixed(0)}px/s</span>

        {manualBeats.length > 0 && (
          <span className="text-primary ml-auto text-[10px]">
            {manualBeats.length} beat{manualBeats.length !== 1 ? "s" : ""} placed
          </span>
        )}
      </div>

      {/* Workspace area */}
      <div className="flex flex-1 overflow-hidden" onWheel={handleWheel}>
        {/* Lane labels (left sidebar) */}
        <div
          className="border-border bg-surface shrink-0 overflow-hidden border-r"
          style={{ width: LABEL_WIDTH }}
        >
          {/* Ruler spacer */}
          <div className="border-border border-b" style={{ height: RULER_HEIGHT }} />
          {/* Audio label */}
          <div
            className="border-border/40 flex items-center border-b px-3"
            style={{ height: WAVEFORM_HEIGHT }}
          >
            <span className="text-text-2 text-[10px] uppercase tracking-wider">Audio</span>
          </div>
          {/* Beats label */}
          <div
            className="border-border/40 flex items-center border-b px-3"
            style={{ height: BEAT_LANE_HEIGHT }}
          >
            <span className="text-text-2 text-[10px] uppercase tracking-wider">Beats</span>
          </div>
        </div>

        {/* Scrollable content */}
        <div ref={scrollContainerRef} className="flex-1 overflow-auto">
          <div style={{ width: contentWidth, minWidth: "100%" }}>
            {/* Ruler */}
            <div
              className="border-border bg-bg/95 sticky top-0 z-(--z-sticky) cursor-pointer border-b backdrop-blur-sm"
              style={{ height: RULER_HEIGHT }}
              onClick={handleRulerClick}
            >
              <div className="relative h-full" style={{ width: contentWidth }}>
                {ticks.map((t) => (
                  <div key={t} className="absolute inset-y-0" style={{ left: t * pxPerSec }}>
                    <div className="bg-border/60 absolute inset-y-0 left-0 w-px" />
                    <span className="text-text-2 absolute top-1.5 left-1 whitespace-nowrap text-[10px]">
                      {formatRulerTime(t)}
                    </span>
                  </div>
                ))}
                {/* Ruler playhead */}
                <div
                  className="bg-primary absolute inset-y-0 z-10 w-0.5"
                  style={{ left: playheadX }}
                />
              </div>
            </div>

            {/* Waveform lane */}
            <div
              className="border-border/40 bg-bg relative cursor-pointer border-b"
              style={{
                position: "sticky",
                top: RULER_HEIGHT,
                zIndex: 15,
                height: WAVEFORM_HEIGHT,
                ...tickBackground,
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                ["--border-tick" as any]: "color-mix(in srgb, var(--border) 15%, transparent)",
              }}
              onClick={handleWaveformClick}
            >
              <canvas
                ref={waveformCanvasRef}
                className="pointer-events-none absolute top-0 left-0"
              />
              {/* Section overlays */}
              {analysis?.structure?.sections.map((section, i) => (
                <div
                  key={`section-${i}`}
                  className="pointer-events-none absolute inset-y-0 z-3 border-l"
                  style={{
                    left: section.start * pxPerSec,
                    width: (section.end - section.start) * pxPerSec,
                    background: sectionColor(section.label),
                    borderColor: "color-mix(in srgb, var(--text-2) 30%, transparent)",
                  }}
                >
                  <span
                    className="absolute top-0 left-0.5 text-[8px] leading-none opacity-70"
                    style={{ color: "var(--text)" }}
                  >
                    {section.label}
                  </span>
                </div>
              ))}
              {/* Playhead */}
              <div
                className="bg-primary pointer-events-none absolute inset-y-0 z-10 w-0.5"
                style={{ left: playheadX }}
              />
            </div>

            {/* Beat lane */}
            <div
              className="bg-bg relative cursor-crosshair"
              style={{
                height: BEAT_LANE_HEIGHT,
                ...tickBackground,
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                ["--border-tick" as any]: "color-mix(in srgb, var(--border) 15%, transparent)",
              }}
              onClick={handleBeatLaneClick}
            >
              {/* AI beats (read-only) */}
              {analysis?.beats?.beats.map((beat, i) => {
                const isDownbeat = analysis.beats?.downbeats.includes(beat);
                return (
                  <div
                    key={`ai-beat-${i}`}
                    className="pointer-events-none absolute inset-y-0 z-2"
                    style={{
                      left: beat * pxPerSec,
                      width: isDownbeat ? 1.5 : 0.5,
                      background: isDownbeat
                        ? "color-mix(in srgb, var(--warning) 50%, transparent)"
                        : "color-mix(in srgb, var(--text-2) 20%, transparent)",
                    }}
                  />
                );
              })}

              {/* Manual beats (interactive) */}
              {manualBeats.map((beat) => {
                const isSelected = beat.id === selectedBeatId;
                const isDragging = dragState?.id === beat.id;
                const displayTime =
                  isDragging && dragPreviewTime != null ? dragPreviewTime : beat.time;

                return (
                  <div
                    key={beat.id}
                    data-beat-id={beat.id}
                    className="absolute inset-y-0 z-5"
                    style={{
                      left: displayTime * pxPerSec - BEAT_HIT_WIDTH / 2,
                      width: BEAT_HIT_WIDTH,
                      cursor: isDragging ? "grabbing" : "grab",
                    }}
                    onClick={(e) => handleBeatClick(e, beat.id)}
                    onMouseDown={(e) => handleBeatMouseDown(e, beat)}
                  >
                    {/* Visual line */}
                    <div
                      className="absolute inset-y-0"
                      style={{
                        left: BEAT_HIT_WIDTH / 2 - (isSelected ? 1 : 0.5),
                        width: isSelected ? 2 : 1,
                        background: "var(--primary)",
                        opacity: isSelected ? 1 : 0.8,
                        boxShadow: isSelected
                          ? "0 0 6px color-mix(in srgb, var(--primary) 40%, transparent)"
                          : "none",
                      }}
                    />
                    {/* Top diamond marker */}
                    <div
                      className="absolute"
                      style={{
                        left: BEAT_HIT_WIDTH / 2 - 3,
                        top: 0,
                        width: 6,
                        height: 6,
                        background: "var(--primary)",
                        transform: "rotate(45deg)",
                        opacity: isSelected ? 1 : 0.7,
                      }}
                    />
                  </div>
                );
              })}

              {/* Playhead */}
              <div
                className="bg-primary pointer-events-none absolute inset-y-0 z-10 w-0.5"
                style={{ left: playheadX }}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export type { ManualBeat };
