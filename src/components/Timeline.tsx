import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EffectBlock } from "./EffectBlock";
import type { AudioAnalysis, InteractionMode, PlaybackInfo, Show } from "../types";
import type { WaveformData } from "../hooks/useAudio";
import { getEffectiveZoom } from "../utils/cssZoom";
import { sectionColor } from "../utils/sectionColor";
import {
  LABEL_WIDTH,
  BASE_LANE_HEIGHT,
  LANE_GAP,
  ROW_PADDING,
  RULER_HEIGHT,
  WAVEFORM_LANE_HEIGHT,
  MIN_PX_PER_SEC,
  MAX_PX_PER_SEC,
  DEFAULT_PX_PER_SEC,
  ZOOM_FACTOR,
  DRAG_THRESHOLD,
} from "../utils/timelineConstants";
import { buildFixtureTrackMap, computeStackedLayoutFast } from "../utils/timelineLayout";
import { drawWaveform } from "../utils/drawWaveform";
import { useTimelineDrag } from "../hooks/useTimelineDrag";
import type { DragState } from "../hooks/useTimelineDrag";

interface TimelineProps {
  show: Show | null;
  playback: PlaybackInfo | null;
  onSeek: (time: number) => void;
  selectedEffects: Set<string>;
  onSelectionChange: (selection: Set<string>) => void;
  refreshKey?: number;
  onAddEffect?: (fixtureId: number, time: number, screenPos: { x: number; y: number }) => void;
  onRefresh?: () => void;
  onMoveEffect?: (
    fromTrackIndex: number,
    effectIndex: number,
    targetFixtureId: number,
    newStart: number,
    newEnd: number,
  ) => Promise<void>;
  onResizeEffect?: (
    trackIndex: number,
    effectIndex: number,
    newStart: number,
    newEnd: number,
  ) => Promise<void>;
  waveform?: WaveformData | null;
  mode?: InteractionMode;
  onModeChange?: (mode: InteractionMode) => void;
  region?: [number, number] | null;
  onRegionChange?: (region: [number, number] | null) => void;
  analysis?: AudioAnalysis | null;
}

function formatRulerTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return m > 0 ? `${m}:${s.toString().padStart(2, "0")}` : `${s}s`;
}

export function Timeline({
  show,
  playback,
  onSeek,
  selectedEffects,
  onSelectionChange,
  refreshKey,
  onAddEffect,
  onRefresh,
  onMoveEffect,
  onResizeEffect,
  waveform,
  mode = "select",
  onModeChange,
  region = null,
  onRegionChange,
  analysis = null,
}: TimelineProps) {
  const duration = playback?.duration ?? 0;
  const currentTime = playback?.current_time ?? 0;
  const sequence = show?.sequences[playback?.sequence_index ?? 0];
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [pxPerSec, setPxPerSec] = useState(DEFAULT_PX_PER_SEC);
  const [hoveredEffect, setHoveredEffect] = useState<string | null>(null);
  const pxPerSecRef = useRef(pxPerSec);
  pxPerSecRef.current = pxPerSec;
  const waveformLaneCanvasRef = useRef<HTMLCanvasElement>(null);
  const trackAreaRef = useRef<HTMLDivElement>(null);
  const labelContainerRef = useRef<HTMLDivElement>(null);

  // Virtualization state
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  const contentWidth = duration * pxPerSec;

  // Pre-compute fixture->tracks map: O(tracks + groups) instead of O(fixtures x tracks x groups)
  const fixtureTrackMap = useMemo(() => {
    if (!show || !sequence) return new Map<number, Set<number>>();
    return buildFixtureTrackMap(show, sequence);
  }, [show, sequence]);

  // Pre-compute stacked layouts for all fixtures
  const stackedRows = useMemo(() => {
    if (!show || !sequence) return [];
    return show.fixtures.map((fixture) => {
      const trackIndices = fixtureTrackMap.get(fixture.id) ?? new Set<number>();
      return computeStackedLayoutFast(fixture.id, sequence, trackIndices);
    });
  }, [show, sequence, fixtureTrackMap]);

  // Cumulative row offsets for fixture-from-clientY lookups
  const rowOffsets = useMemo(() => {
    const offsets: { fixtureId: number; top: number; bottom: number }[] = [];
    let y = 0;
    for (const row of stackedRows) {
      offsets.push({ fixtureId: row.fixtureId, top: y, bottom: y + row.rowHeight });
      y += row.rowHeight;
    }
    return offsets;
  }, [stackedRows]);

  const totalTrackHeight = useMemo(() => {
    return rowOffsets.length > 0 ? rowOffsets[rowOffsets.length - 1].bottom : 0;
  }, [rowOffsets]);

  // Compute visible row indices for virtualization (with overscan)
  const OVERSCAN = 5;
  const { startIdx, endIdx } = useMemo(() => {
    if (rowOffsets.length === 0 || viewportHeight === 0) {
      return { startIdx: 0, endIdx: 0 };
    }
    let start = 0;
    for (let i = 0; i < rowOffsets.length; i++) {
      if (rowOffsets[i].bottom > scrollTop) {
        start = i;
        break;
      }
    }
    let end = rowOffsets.length;
    for (let i = start; i < rowOffsets.length; i++) {
      if (rowOffsets[i].top >= scrollTop + viewportHeight) {
        end = i;
        break;
      }
    }
    return {
      startIdx: Math.max(0, start - OVERSCAN),
      endIdx: Math.min(rowOffsets.length, end + OVERSCAN),
    };
  }, [rowOffsets, scrollTop, viewportHeight]);

  const visibleRows = useMemo(() => {
    return stackedRows.slice(startIdx, endIdx);
  }, [stackedRows, startIdx, endIdx]);

  // Ruler tick calculation — needed by both the CSS pattern and the ruler rendering.
  const idealTickSpacingPx = 80;
  const rawInterval = idealTickSpacingPx / pxPerSec;
  const niceIntervals = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60];
  const tickInterval = niceIntervals.find((iv) => iv >= rawInterval) ?? rawInterval;
  const ticks: number[] = [];
  for (let t = 0; t <= duration + tickInterval; t += tickInterval) {
    if (t <= duration) ticks.push(t);
  }

  // CSS grid pattern for tick lines (replaces per-row tick divs)
  const tickBackground = useMemo(() => {
    const tickPx = tickInterval * pxPerSec;
    if (tickPx < 2) return {};
    return {
      backgroundImage: `repeating-linear-gradient(to right, var(--border-tick) 0 1px, transparent 1px ${tickPx}px)`,
      backgroundSize: `${tickPx}px 100%`,
    };
  }, [tickInterval, pxPerSec]);

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

  const timeFromClientX = useCallback(
    (clientX: number) => {
      const container = scrollContainerRef.current;
      if (!container) return 0;
      const rect = container.getBoundingClientRect();
      const zoom = getEffectiveZoom(container);
      const x = (clientX - rect.left) / zoom + container.scrollLeft;
      return Math.max(0, Math.min(x / pxPerSec, duration));
    },
    [pxPerSec, duration],
  );

  // ── Drag hook ──────────────────────────────────────────────────────

  const {
    dragState,
    dragPreview,
    marqueeRect,
    justFinishedDragRef,
    handleResizeMouseDown,
    handleMoveMouseDown,
    handleTrackAreaMouseDown,
    startSwipeDrag,
    getFixtureIdFromClientY,
  } = useTimelineDrag({
    duration,
    pxPerSecRef,
    scrollContainerRef,
    selectedEffects,
    onSelectionChange,
    mode,
    waveform: waveform ?? null,
    stackedRows,
    rowOffsets,
    pxPerSec,
    onMoveEffect,
    onResizeEffect,
    onRefresh,
    sequenceIndex: playback?.sequence_index,
  });

  const handleTrackAreaClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      // Only handle clicks on the background (not on effect blocks).
      if ((e.target as HTMLElement).closest("[data-effect-key]")) return;
      // Don't fire click if we just finished a marquee/swipe drag
      if (justFinishedDragRef.current) {
        justFinishedDragRef.current = false;
        return;
      }
      const time = timeFromClientX(e.clientX);
      onSeek(time);
      if (!e.shiftKey) {
        onSelectionChange(new Set());
      }
    },
    [timeFromClientX, onSeek, onSelectionChange, justFinishedDragRef],
  );

  const handleTrackAreaDoubleClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if ((e.target as HTMLElement).closest("[data-effect-key]")) return;
      if (!onAddEffect) return;
      const time = timeFromClientX(e.clientX);
      const fixtureId = getFixtureIdFromClientY(e.clientY);
      if (fixtureId == null) return;
      onAddEffect(fixtureId, time, { x: e.clientX, y: e.clientY });
    },
    [timeFromClientX, getFixtureIdFromClientY, onAddEffect],
  );

  const handleEffectClick = useCallback(
    (e: React.MouseEvent, key: string) => {
      e.stopPropagation();
      // In swipe mode, click toggles individual effect
      if (mode === "swipe") {
        const next = new Set(selectedEffects);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        onSelectionChange(next);
        return;
      }
      // Select and Edit modes: shift+click toggles, plain click replaces
      if (e.shiftKey) {
        const next = new Set(selectedEffects);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        onSelectionChange(next);
      } else {
        onSelectionChange(new Set([key]));
      }
    },
    [selectedEffects, onSelectionChange, mode],
  );

  // ── Ruler drag state (region selection) ──────────────────────────

  const rulerDragRef = useRef<{ startClientX: number; startTime: number } | null>(null);
  const [rulerDragPreview, setRulerDragPreview] = useState<[number, number] | null>(null);

  const handleRulerMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (e.button !== 0) return;
      e.preventDefault();
      const time = timeFromClientX(e.clientX);
      rulerDragRef.current = { startClientX: e.clientX, startTime: time };

      const handleRulerMouseMove = (me: MouseEvent) => {
        const drag = rulerDragRef.current;
        if (!drag) return;
        const dist = Math.abs(me.clientX - drag.startClientX);
        if (dist >= DRAG_THRESHOLD) {
          const endTime = timeFromClientX(me.clientX);
          const lo = Math.min(drag.startTime, endTime);
          const hi = Math.max(drag.startTime, endTime);
          setRulerDragPreview([lo, hi]);
        }
      };

      const handleRulerMouseUp = (me: MouseEvent) => {
        document.removeEventListener("mousemove", handleRulerMouseMove);
        document.removeEventListener("mouseup", handleRulerMouseUp);
        const drag = rulerDragRef.current;
        rulerDragRef.current = null;
        if (!drag) return;

        const dist = Math.abs(me.clientX - drag.startClientX);
        if (dist < DRAG_THRESHOLD) {
          // Click: clear region and seek
          setRulerDragPreview(null);
          onRegionChange?.(null);
          onSeek(timeFromClientX(me.clientX));
        } else {
          // Drag completed: commit region
          const endTime = timeFromClientX(me.clientX);
          const lo = Math.min(drag.startTime, endTime);
          const hi = Math.max(drag.startTime, endTime);
          setRulerDragPreview(null);
          onRegionChange?.([lo, hi]);
        }
      };

      document.addEventListener("mousemove", handleRulerMouseMove);
      document.addEventListener("mouseup", handleRulerMouseUp);
    },
    [timeFromClientX, onSeek, onRegionChange],
  );

  // The effective region to display (committed or being dragged)
  const displayRegion = rulerDragPreview ?? region;

  // Waveform canvas drawing
  const drawWaveformCb = useCallback(
    (canvas: HTMLCanvasElement | null, height: number, alpha: number) => {
      drawWaveform(canvas, height, alpha, waveform, contentWidth, pxPerSec);
    },
    [waveform, contentWidth, pxPerSec],
  );

  // Draw waveform lane
  useEffect(() => {
    drawWaveformCb(waveformLaneCanvasRef.current, WAVEFORM_LANE_HEIGHT, 0.7);
  }, [drawWaveformCb]);

  // Scroll/resize tracking for virtualization
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const update = () => {
      const waveH = waveform ? WAVEFORM_LANE_HEIGHT : 0;
      setScrollTop(container.scrollTop);
      setViewportHeight(container.clientHeight - RULER_HEIGHT - waveH);
    };
    update();

    container.addEventListener("scroll", update, { passive: true });
    const ro = new ResizeObserver(update);
    ro.observe(container);

    return () => {
      container.removeEventListener("scroll", update);
      ro.disconnect();
    };
  }, [waveform]);

  // Sync label panel scroll with main timeline scroll
  useEffect(() => {
    const container = scrollContainerRef.current;
    const labels = labelContainerRef.current;
    if (!container || !labels) return;

    const syncScroll = () => {
      labels.scrollTop = container.scrollTop;
    };

    container.addEventListener("scroll", syncScroll, { passive: true });
    return () => container.removeEventListener("scroll", syncScroll);
  }, [waveform]);

  const playheadX = currentTime * pxPerSec;

  return (
    <div className="bg-bg flex flex-1 flex-col overflow-hidden">
      {/* Mode toggle + Zoom controls bar */}
      <div className="border-border bg-surface flex shrink-0 items-center gap-1 border-b px-3 py-1">
        {/* Mode toggle buttons */}
        <button
          onClick={() => onModeChange?.("select")}
          className={`rounded border px-2 py-0.5 text-[10px] transition-colors ${
            mode === "select"
              ? "border-primary bg-primary text-white"
              : "border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text"
          }`}
          title="Select mode (V)"
          aria-label="Select mode"
          aria-pressed={mode === "select"}
        >
          <svg width="10" height="10" viewBox="0 0 16 16" fill="currentColor" className="mr-0.5 inline-block align-[-1px]">
            <path d="M2 1l10 6.5L7.5 9l-2 5.5L2 1z" />
          </svg>
          Select
        </button>
        <button
          onClick={() => onModeChange?.("edit")}
          className={`rounded border px-2 py-0.5 text-[10px] transition-colors ${
            mode === "edit"
              ? "border-primary bg-primary text-white"
              : "border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text"
          }`}
          title="Edit mode (M)"
          aria-label="Edit mode"
          aria-pressed={mode === "edit"}
        >
          <svg width="10" height="10" viewBox="0 0 16 16" fill="currentColor" className="mr-0.5 inline-block align-[-1px]">
            <path d="M8 2v4H4v1h4v4h1V7h4V6H9V2H8zM8 0l3 3H9V5H7V3H5L8 0zM8 16l-3-3h2v-2h2v2h2l-3 3zM0 8l3-3v2h2v2H3v2L0 8zM16 8l-3 3v-2h-2V7h2V5l3 3z" />
          </svg>
          Edit
        </button>
        <button
          onClick={() => onModeChange?.("swipe")}
          className={`rounded border px-2 py-0.5 text-[10px] transition-colors ${
            mode === "swipe"
              ? "border-primary bg-primary text-white"
              : "border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text"
          }`}
          title="Swipe mode (S)"
          aria-label="Swipe mode"
          aria-pressed={mode === "swipe"}
        >
          <svg width="10" height="10" viewBox="0 0 16 16" fill="currentColor" className="mr-0.5 inline-block align-[-1px]">
            <path d="M2 14L14 2M10 2h4v4" stroke="currentColor" strokeWidth="2" fill="none" />
          </svg>
          Swipe
        </button>

        <div className="bg-border mx-1 h-3 w-px" />

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

        {selectedEffects.size > 0 && (
          <span className="text-primary ml-auto text-[10px]">{selectedEffects.size} selected</span>
        )}
      </div>

      {/* Scrollable timeline area */}
      <div className="flex flex-1 overflow-hidden" onWheel={handleWheel}>
        {/* Fixed fixture labels */}
        <div
          className="border-border bg-surface shrink-0 overflow-hidden border-r"
          style={{ width: LABEL_WIDTH }}
        >
          {/* Spacer for ruler */}
          <div className="border-border border-b" style={{ height: RULER_HEIGHT }} />
          {/* Waveform label */}
          {waveform && (
            <div
              className="border-border/40 flex items-center border-b px-3"
              style={{ height: WAVEFORM_LANE_HEIGHT }}
            >
              <span className="text-text-2 text-[10px] tracking-wider uppercase">Audio</span>
            </div>
          )}
          {/* Virtualized label rows — synced with main scroll */}
          <div ref={labelContainerRef} className="overflow-y-auto scrollbar-none" style={{ height: `calc(100% - ${RULER_HEIGHT + (waveform ? WAVEFORM_LANE_HEIGHT : 0)}px)` }}>
            <div style={{ height: totalTrackHeight }}>
              {/* Top spacer */}
              {startIdx > 0 && (
                <div style={{ height: rowOffsets[startIdx]?.top ?? 0 }} />
              )}
              {visibleRows.map((row) => {
                const fixture = show?.fixtures.find((f) => f.id === row.fixtureId);
                return (
                  <div
                    key={row.fixtureId}
                    className="border-border/40 flex flex-col justify-center border-b px-3"
                    style={{ height: row.rowHeight }}
                  >
                    <span className="text-text-2 truncate text-xs font-medium">{fixture?.name}</span>
                    <span className="text-text-2 mt-0.5 text-[10px]">{fixture?.pixel_count}px</span>
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        {/* Scrollable content area */}
        <div ref={scrollContainerRef} className="flex-1 overflow-auto">
          <div style={{ width: contentWidth, minWidth: "100%" }}>
            {/* Ruler */}
            <div
              className="border-border bg-bg/95 sticky top-0 z-20 cursor-pointer border-b backdrop-blur-sm"
              style={{ height: RULER_HEIGHT }}
              onMouseDown={handleRulerMouseDown}
            >
              <div className="relative h-full" style={{ width: contentWidth }}>
                {ticks.map((t) => (
                  <div key={t} className="absolute inset-y-0" style={{ left: t * pxPerSec }}>
                    <div className="bg-border/60 absolute inset-y-0 left-0 w-px" />
                    <span className="text-text-2 absolute top-1.5 left-1 text-[10px] whitespace-nowrap">
                      {formatRulerTime(t)}
                    </span>
                  </div>
                ))}
                {/* Region highlight on ruler */}
                {displayRegion && (
                  <div
                    className="absolute inset-y-0 z-5"
                    style={{
                      left: displayRegion[0] * pxPerSec,
                      width: (displayRegion[1] - displayRegion[0]) * pxPerSec,
                      background: "color-mix(in srgb, var(--primary) 25%, transparent)",
                      borderLeft: "2px solid var(--primary)",
                      borderRight: "2px solid var(--primary)",
                    }}
                  />
                )}
                {/* Ruler playhead */}
                <div
                  className="bg-primary absolute inset-y-0 z-10 w-0.5"
                  style={{ left: playheadX, transform: "translateX(-50%)" }}
                />
              </div>
            </div>

            {/* Waveform lane — sticky below ruler */}
            {waveform && (
              <div
                className="border-border/40 bg-bg relative cursor-pointer border-b"
                style={{
                  position: "sticky",
                  top: RULER_HEIGHT,
                  zIndex: 15,
                  height: WAVEFORM_LANE_HEIGHT,
                  ...tickBackground,
                  // eslint-disable-next-line @typescript-eslint/no-explicit-any
                  ["--border-tick" as any]: "color-mix(in srgb, var(--border) 15%, transparent)",
                }}
                onMouseDown={handleRulerMouseDown}
              >
                <canvas
                  ref={waveformLaneCanvasRef}
                  className="pointer-events-none absolute top-0 left-0"
                />
                {/* Region highlight on waveform */}
                {displayRegion && (
                  <div
                    className="pointer-events-none absolute inset-y-0 z-5"
                    style={{
                      left: displayRegion[0] * pxPerSec,
                      width: (displayRegion[1] - displayRegion[0]) * pxPerSec,
                      background: "color-mix(in srgb, var(--primary) 15%, transparent)",
                    }}
                  />
                )}
                {/* Analysis: Section regions */}
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
                {/* Analysis: Beat markers */}
                {analysis?.beats?.beats.map((beat, i) => {
                  const isDownbeat = analysis.beats?.downbeats.includes(beat);
                  return (
                    <div
                      key={`beat-${i}`}
                      className="pointer-events-none absolute inset-y-0 z-4"
                      style={{
                        left: beat * pxPerSec,
                        width: isDownbeat ? 1.5 : 0.5,
                        background: isDownbeat
                          ? "color-mix(in srgb, var(--primary) 60%, transparent)"
                          : "color-mix(in srgb, var(--text-2) 25%, transparent)",
                      }}
                    />
                  );
                })}
                {/* Playhead */}
                <div
                  className="bg-primary pointer-events-none absolute inset-y-0 z-10 w-0.5"
                  style={{ left: playheadX, transform: "translateX(-50%)" }}
                />
              </div>
            )}

            {/* Fixture rows — virtualized with CSS tick pattern */}
            <div
              ref={trackAreaRef}
              className="relative"
              style={{
                height: totalTrackHeight,
                cursor:
                  mode === "edit" ? "move" : mode === "swipe" ? "crosshair" : "default",
                ...tickBackground,
                // CSS custom property for the tick pattern color
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                ["--border-tick" as any]: "color-mix(in srgb, var(--border) 15%, transparent)",
              }}
              onClick={handleTrackAreaClick}
              onDoubleClick={handleTrackAreaDoubleClick}
              onMouseDown={handleTrackAreaMouseDown}
            >
              {/* Top spacer for virtualization */}
              {startIdx > 0 && (
                <div style={{ height: rowOffsets[startIdx]?.top ?? 0 }} />
              )}

              {visibleRows.map((row) => {
                // Check if there's a move-drag ghost targeting this row
                const moveGhost =
                  dragPreview &&
                  dragState?.type === "move" &&
                  (dragState as Extract<DragState, { type: "move" }>).didDrag &&
                  dragPreview.targetFixtureId === row.fixtureId &&
                  dragPreview.targetFixtureId !==
                    (dragState as Extract<DragState, { type: "move" }>).originalFixtureId
                    ? dragPreview
                    : null;

                return (
                  <div
                    key={row.fixtureId}
                    className="border-border/30 relative border-b"
                    style={{ height: row.rowHeight }}
                    data-fixture-id={row.fixtureId}
                  >
                    {/* Effect blocks - Vixen-style stacked lanes */}
                    {row.effects.map((placed) => {
                      const isSelected = selectedEffects.has(placed.key);
                      const isHovered = hoveredEffect === placed.key;
                      const laneHeight = BASE_LANE_HEIGHT - LANE_GAP;
                      const top = ROW_PADDING + placed.lane * BASE_LANE_HEIGHT;

                      // Apply drag preview overrides
                      const preview = dragPreview?.key === placed.key ? dragPreview : null;
                      const isDragging =
                        dragState != null &&
                        "key" in dragState &&
                        dragState.key === placed.key;
                      const isMoveDragToOtherRow =
                        isDragging &&
                        dragState?.type === "move" &&
                        (dragState as Extract<DragState, { type: "move" }>).didDrag &&
                        preview?.targetFixtureId != null &&
                        preview.targetFixtureId !==
                          (dragState as Extract<DragState, { type: "move" }>).originalFixtureId;

                      const displayStart = preview ? preview.start : placed.startSec;
                      const displayDuration = preview
                        ? preview.end - preview.start
                        : placed.durationSec;

                      // Swipe drag: highlight effects being swiped
                      const isBeingSwiped =
                        dragState?.type === "swipe" &&
                        (dragState as Extract<DragState, { type: "swipe" }>).swipedKeys.has(
                          placed.key,
                        );

                      return (
                        <div
                          key={placed.key}
                          data-effect-key={placed.key}
                          className="absolute overflow-hidden transition-shadow duration-75"
                          style={{
                            left: displayStart * pxPerSec,
                            width: displayDuration * pxPerSec,
                            top,
                            height: laneHeight,
                            zIndex: isSelected ? 10 : isHovered ? 5 : 1,
                            borderRadius: 2,
                            cursor:
                              mode === "edit"
                                ? "move"
                                : mode === "swipe"
                                  ? "crosshair"
                                  : "pointer",
                            border: isBeingSwiped
                              ? "2px solid var(--primary)"
                              : isSelected
                                ? "2px solid var(--primary)"
                                : isHovered
                                  ? "1px solid var(--text-2)"
                                  : "1px solid var(--border)",
                            boxShadow: isBeingSwiped
                              ? "0 0 6px color-mix(in srgb, var(--primary) 40%, transparent)"
                              : isSelected
                                ? "0 0 4px color-mix(in srgb, var(--primary) 25%, transparent)"
                                : "none",
                            opacity: isMoveDragToOtherRow ? 0.3 : 1,
                          }}
                          onClick={(e) => {
                            if (!isDragging) handleEffectClick(e, placed.key);
                          }}
                          onMouseDown={(e) => {
                            if (mode === "edit") {
                              handleMoveMouseDown(e, placed, row.fixtureId);
                            } else if (mode === "swipe") {
                              startSwipeDrag(e, placed.key);
                            }
                          }}
                          onMouseEnter={() => !dragState && setHoveredEffect(placed.key)}
                          onMouseLeave={() => setHoveredEffect(null)}
                        >
                          {/* Left resize handle — Edit mode only */}
                          {mode === "edit" && isSelected && (
                            <div
                              data-resize-handle="left"
                              className="absolute top-0 left-0 z-20 h-full w-1.5 cursor-col-resize"
                              style={{ background: "var(--primary)", opacity: 0.4 }}
                              onMouseDown={(e) => handleResizeMouseDown(e, placed, "left")}
                            />
                          )}

                          <EffectBlock
                            sequenceIndex={playback?.sequence_index ?? 0}
                            trackIndex={placed.trackIndex}
                            effectIndex={placed.effectIndex}
                            refreshKey={refreshKey}
                          />

                          {/* Right resize handle — Edit mode only */}
                          {mode === "edit" && isSelected && (
                            <div
                              data-resize-handle="right"
                              className="absolute top-0 right-0 z-20 h-full w-1.5 cursor-col-resize"
                              style={{ background: "var(--primary)", opacity: 0.4 }}
                              onMouseDown={(e) => handleResizeMouseDown(e, placed, "right")}
                            />
                          )}
                        </div>
                      );
                    })}

                    {/* Ghost outline for move-drag into this row */}
                    {moveGhost && (
                      <div
                        className="pointer-events-none absolute"
                        style={{
                          left: moveGhost.start * pxPerSec,
                          width: (moveGhost.end - moveGhost.start) * pxPerSec,
                          top: ROW_PADDING,
                          height: BASE_LANE_HEIGHT - LANE_GAP,
                          borderRadius: 2,
                          border: "2px dashed var(--primary)",
                          opacity: 0.6,
                          zIndex: 20,
                        }}
                      />
                    )}
                  </div>
                );
              })}

              {/* Region overlay on track area */}
              {displayRegion && (
                <div
                  className="pointer-events-none absolute inset-y-0 z-2"
                  style={{
                    left: displayRegion[0] * pxPerSec,
                    width: (displayRegion[1] - displayRegion[0]) * pxPerSec,
                    background: "color-mix(in srgb, var(--primary) 8%, transparent)",
                  }}
                />
              )}

              {/* Single playhead line spanning all rows */}
              <div
                className="bg-primary pointer-events-none absolute inset-y-0 z-30 w-0.5"
                style={{ left: playheadX, transform: "translateX(-50%)" }}
              />
            </div>
          </div>
        </div>
      </div>

      {/* Marquee selection overlay (rendered in fixed position over the viewport) */}
      {marqueeRect && (
        <div
          className="pointer-events-none fixed z-50"
          style={{
            left: marqueeRect.x,
            top: marqueeRect.y,
            width: marqueeRect.width,
            height: marqueeRect.height,
            border: "1px solid var(--primary)",
            background: "color-mix(in srgb, var(--primary) 10%, transparent)",
            borderRadius: 2,
          }}
        />
      )}
    </div>
  );
}

export { type TimelineProps };
