import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EffectBlock } from "./EffectBlock";
import type { BlendMode, PlaybackInfo, Show, Track } from "../types";
import type { WaveformData } from "../hooks/useAudio";

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
  waveform?: WaveformData | null;
}

const LABEL_WIDTH = 160;
const BASE_LANE_HEIGHT = 24;
const LANE_GAP = 2;
const ROW_PADDING = 2;
const MIN_ROW_HEIGHT = 48;
const RULER_HEIGHT = 28;
const MIN_PX_PER_SEC = 10;
const MAX_PX_PER_SEC = 500;
const DEFAULT_PX_PER_SEC = 40;
const ZOOM_FACTOR = 1.15;
const DRAG_THRESHOLD = 3;
const MIN_DURATION = 0.1;

function formatRulerTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return m > 0 ? `${m}:${s.toString().padStart(2, "0")}` : `${s}s`;
}

/** Recursively resolve all fixture IDs from a group's members. */
function resolveGroupFixtures(groupId: number, show: Show, visited?: Set<number>): number[] {
  const seen = visited ?? new Set<number>();
  if (seen.has(groupId)) return [];
  seen.add(groupId);
  const group = show.groups.find((g) => g.id === groupId);
  if (!group) return [];
  const ids: number[] = [];
  for (const m of group.members) {
    if ("Fixture" in m) ids.push((m as { Fixture: number }).Fixture);
    else if ("Group" in m)
      ids.push(...resolveGroupFixtures((m as { Group: number }).Group, show, seen));
  }
  return ids;
}

function trackTargetsFixture(track: Track, fixtureId: number, show: Show): boolean {
  const target = track.target;
  if (target === "All") return true;
  if ("Fixtures" in target) return target.Fixtures.includes(fixtureId);
  if ("Group" in target) {
    return resolveGroupFixtures(target.Group, show).includes(fixtureId);
  }
  return false;
}

interface PlacedEffect {
  key: string;
  trackIndex: number;
  effectIndex: number;
  startSec: number;
  durationSec: number;
  blendMode: BlendMode;
  kind: string;
  lane: number;
}

interface StackedRow {
  fixtureId: number;
  effects: PlacedEffect[];
  laneCount: number;
  rowHeight: number;
}

function getFixtureEffects(
  fixtureId: number,
  show: Show,
  sequence: { tracks: Track[] },
): PlacedEffect[] {
  const effects: PlacedEffect[] = [];

  for (let trackIdx = 0; trackIdx < sequence.tracks.length; trackIdx++) {
    const track = sequence.tracks[trackIdx];
    if (!trackTargetsFixture(track, fixtureId, show)) continue;

    for (let effectIdx = 0; effectIdx < track.effects.length; effectIdx++) {
      const effect = track.effects[effectIdx];
      effects.push({
        key: `${trackIdx}-${effectIdx}`,
        trackIndex: trackIdx,
        effectIndex: effectIdx,
        startSec: effect.time_range.start,
        durationSec: effect.time_range.end - effect.time_range.start,
        blendMode: track.blend_mode,
        kind: effect.kind,
        lane: 0,
      });
    }
  }

  return effects;
}

/** Greedy lane assignment: sorted by start time, assign each to the first lane where it fits. */
function computeStackedLayout(
  fixtureId: number,
  show: Show,
  sequence: { tracks: Track[] },
): StackedRow {
  const effects = getFixtureEffects(fixtureId, show, sequence);

  // Sort by start time for greedy assignment
  const sorted = [...effects].sort((a, b) => a.startSec - b.startSec);

  // laneEnds[i] = the end time of the last effect placed in lane i
  const laneEnds: number[] = [];

  for (const effect of sorted) {
    const endSec = effect.startSec + effect.durationSec;
    let assignedLane = -1;
    for (let i = 0; i < laneEnds.length; i++) {
      if (effect.startSec >= laneEnds[i]) {
        assignedLane = i;
        laneEnds[i] = endSec;
        break;
      }
    }
    if (assignedLane === -1) {
      assignedLane = laneEnds.length;
      laneEnds.push(endSec);
    }
    effect.lane = assignedLane;
  }

  const laneCount = Math.max(laneEnds.length, 1);
  const rowHeight = Math.max(laneCount * BASE_LANE_HEIGHT + ROW_PADDING * 2, MIN_ROW_HEIGHT);

  return { fixtureId, effects, laneCount, rowHeight };
}

// ── Drag state types ────────────────────────────────────────────────

type DragState =
  | {
      type: "resize";
      key: string;
      trackIndex: number;
      effectIndex: number;
      edge: "left" | "right";
      originalStart: number;
      originalEnd: number;
      startClientX: number;
    }
  | {
      type: "move";
      key: string;
      trackIndex: number;
      effectIndex: number;
      originalStart: number;
      originalEnd: number;
      originalFixtureId: number;
      startClientX: number;
      startClientY: number;
      didDrag: boolean;
    };

interface DragPreview {
  key: string;
  start: number;
  end: number;
  targetFixtureId?: number;
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
  waveform,
}: TimelineProps) {
  const duration = playback?.duration ?? 0;
  const currentTime = playback?.current_time ?? 0;
  const sequence = show?.sequences[playback?.sequence_index ?? 0];
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [pxPerSec, setPxPerSec] = useState(DEFAULT_PX_PER_SEC);
  const [hoveredEffect, setHoveredEffect] = useState<string | null>(null);
  const [dragState, setDragState] = useState<DragState | null>(null);
  const [dragPreview, setDragPreview] = useState<DragPreview | null>(null);
  const dragStateRef = useRef<DragState | null>(null);
  const pxPerSecRef = useRef(pxPerSec);
  pxPerSecRef.current = pxPerSec;
  const waveformCanvasRef = useRef<HTMLCanvasElement>(null);

  const contentWidth = duration * pxPerSec;

  // Pre-compute stacked layouts for all fixtures
  const stackedRows = useMemo(() => {
    if (!show || !sequence) return [];
    return show.fixtures.map((fixture) => computeStackedLayout(fixture.id, show, sequence));
  }, [show, sequence]);

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
      const x = clientX - rect.left + container.scrollLeft;
      return Math.max(0, Math.min(x / pxPerSec, duration));
    },
    [pxPerSec, duration],
  );

  /** Get fixture ID from a clientY position relative to the track area. */
  const getFixtureIdFromClientY = useCallback(
    (clientY: number) => {
      const container = scrollContainerRef.current;
      if (!container || rowOffsets.length === 0) return null;
      const rect = container.getBoundingClientRect();
      const y = clientY - rect.top - RULER_HEIGHT + container.scrollTop;
      for (const row of rowOffsets) {
        if (y >= row.top && y < row.bottom) return row.fixtureId;
      }
      // Clamp: return first or last row
      if (y < 0) return rowOffsets[0].fixtureId;
      return rowOffsets[rowOffsets.length - 1].fixtureId;
    },
    [rowOffsets],
  );

  const handleTrackAreaClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      // Only handle clicks on the background (not on effect blocks).
      if ((e.target as HTMLElement).closest("[data-effect-key]")) return;
      const time = timeFromClientX(e.clientX);
      onSeek(time);
      if (!e.shiftKey) {
        onSelectionChange(new Set());
      }
    },
    [timeFromClientX, onSeek, onSelectionChange],
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
      if (e.shiftKey) {
        const next = new Set(selectedEffects);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        onSelectionChange(next);
      } else {
        onSelectionChange(new Set([key]));
      }
    },
    [selectedEffects, onSelectionChange],
  );

  const handleRulerClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const time = timeFromClientX(e.clientX);
      onSeek(time);
    },
    [timeFromClientX, onSeek],
  );

  // ── Resize handle mousedown ─────────────────────────────────────

  const handleResizeMouseDown = useCallback(
    (e: React.MouseEvent, placed: PlacedEffect, edge: "left" | "right") => {
      e.stopPropagation();
      e.preventDefault();
      const state: DragState = {
        type: "resize",
        key: placed.key,
        trackIndex: placed.trackIndex,
        effectIndex: placed.effectIndex,
        edge,
        originalStart: placed.startSec,
        originalEnd: placed.startSec + placed.durationSec,
        startClientX: e.clientX,
      };
      setDragState(state);
      dragStateRef.current = state;
    },
    [],
  );

  // ── Move mousedown (on effect body) ────────────────────────────

  const handleMoveMouseDown = useCallback(
    (e: React.MouseEvent, placed: PlacedEffect, fixtureId: number) => {
      // Don't start move if clicking on a resize handle
      if ((e.target as HTMLElement).dataset.resizeHandle) return;
      e.stopPropagation();
      e.preventDefault();
      const state: DragState = {
        type: "move",
        key: placed.key,
        trackIndex: placed.trackIndex,
        effectIndex: placed.effectIndex,
        originalStart: placed.startSec,
        originalEnd: placed.startSec + placed.durationSec,
        originalFixtureId: fixtureId,
        startClientX: e.clientX,
        startClientY: e.clientY,
        didDrag: false,
      };
      setDragState(state);
      dragStateRef.current = state;
    },
    [],
  );

  // ── Global mousemove/mouseup for drag operations ──────────────

  useEffect(() => {
    function handleMouseMove(e: MouseEvent) {
      const ds = dragStateRef.current;
      if (!ds) return;

      const pps = pxPerSecRef.current;
      const deltaPx = e.clientX - ds.startClientX;
      const deltaSec = deltaPx / pps;

      if (ds.type === "resize") {
        let newStart = ds.originalStart;
        let newEnd = ds.originalEnd;
        if (ds.edge === "left") {
          newStart = Math.max(0, ds.originalStart + deltaSec);
          if (newEnd - newStart < MIN_DURATION) newStart = newEnd - MIN_DURATION;
        } else {
          newEnd = Math.min(duration, ds.originalEnd + deltaSec);
          if (newEnd - newStart < MIN_DURATION) newEnd = newStart + MIN_DURATION;
        }
        newStart = Math.max(0, newStart);
        newEnd = Math.min(duration, newEnd);
        setDragPreview({ key: ds.key, start: newStart, end: newEnd });
      } else if (ds.type === "move") {
        const totalDelta =
          Math.abs(e.clientX - ds.startClientX) + Math.abs(e.clientY - ds.startClientY);
        if (!ds.didDrag && totalDelta < DRAG_THRESHOLD) return;
        ds.didDrag = true;

        const dur = ds.originalEnd - ds.originalStart;
        let newStart = ds.originalStart + deltaSec;
        newStart = Math.max(0, Math.min(newStart, duration - dur));
        const newEnd = newStart + dur;
        const targetFixtureId = getFixtureIdFromClientY(e.clientY) ?? ds.originalFixtureId;
        setDragPreview({ key: ds.key, start: newStart, end: newEnd, targetFixtureId });
      }
    }

    async function handleMouseUp(e: MouseEvent) {
      const ds = dragStateRef.current;
      if (!ds) return;

      dragStateRef.current = null;
      setDragState(null);

      if (ds.type === "resize") {
        const pps = pxPerSecRef.current;
        const deltaPx = e.clientX - ds.startClientX;
        const deltaSec = deltaPx / pps;
        let newStart = ds.originalStart;
        let newEnd = ds.originalEnd;
        if (ds.edge === "left") {
          newStart = Math.max(0, ds.originalStart + deltaSec);
          if (newEnd - newStart < MIN_DURATION) newStart = newEnd - MIN_DURATION;
        } else {
          newEnd = Math.min(duration, ds.originalEnd + deltaSec);
          if (newEnd - newStart < MIN_DURATION) newEnd = newStart + MIN_DURATION;
        }
        newStart = Math.max(0, newStart);
        newEnd = Math.min(duration, newEnd);

        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("update_effect_time_range", {
            sequenceIndex: playback?.sequence_index ?? 0,
            trackIndex: ds.trackIndex,
            effectIndex: ds.effectIndex,
            start: newStart,
            end: newEnd,
          });
          onRefresh?.();
        } catch (err) {
          console.error("[VibeShow] Resize failed:", err);
        }
      } else if (ds.type === "move") {
        if (!ds.didDrag) {
          // It was a click, not a drag — handle selection
          if (e.shiftKey) {
            const next = new Set(selectedEffects);
            if (next.has(ds.key)) next.delete(ds.key);
            else next.add(ds.key);
            onSelectionChange(next);
          } else {
            onSelectionChange(new Set([ds.key]));
          }
        } else if (onMoveEffect) {
          const pps = pxPerSecRef.current;
          const deltaSec = (e.clientX - ds.startClientX) / pps;
          const dur = ds.originalEnd - ds.originalStart;
          let newStart = ds.originalStart + deltaSec;
          newStart = Math.max(0, Math.min(newStart, duration - dur));
          const newEnd = newStart + dur;
          const targetFixtureId = getFixtureIdFromClientY(e.clientY) ?? ds.originalFixtureId;

          try {
            await onMoveEffect(ds.trackIndex, ds.effectIndex, targetFixtureId, newStart, newEnd);
          } catch (err) {
            console.error("[VibeShow] Move failed:", err);
          }
        }
      }

      setDragPreview(null);
    }

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [
    duration,
    playback?.sequence_index,
    onRefresh,
    onMoveEffect,
    getFixtureIdFromClientY,
    selectedEffects,
    onSelectionChange,
  ]);

  // Waveform canvas drawing
  const totalRowHeight = useMemo(
    () => stackedRows.reduce((sum, r) => sum + r.rowHeight, 0),
    [stackedRows],
  );

  useEffect(() => {
    const canvas = waveformCanvasRef.current;
    if (!canvas || !waveform) return;

    const width = Math.ceil(contentWidth);
    const height = totalRowHeight;
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

    ctx.fillStyle = "var(--primary)";
    ctx.globalAlpha = 0.12;

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
  }, [waveform, contentWidth, totalRowHeight, pxPerSec]);

  // Ruler tick calculation.
  const idealTickSpacingPx = 80;
  const rawInterval = idealTickSpacingPx / pxPerSec;
  const niceIntervals = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60];
  const tickInterval = niceIntervals.find((iv) => iv >= rawInterval) ?? rawInterval;
  const ticks: number[] = [];
  for (let t = 0; t <= duration + tickInterval; t += tickInterval) {
    if (t <= duration) ticks.push(t);
  }

  const playheadX = currentTime * pxPerSec;

  return (
    <div className="bg-bg flex flex-1 flex-col overflow-hidden">
      {/* Zoom controls bar */}
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

        {selectedEffects.size > 0 && (
          <span className="text-primary ml-auto text-[10px]">{selectedEffects.size} selected</span>
        )}
      </div>

      {/* Scrollable timeline area */}
      <div className="flex flex-1 overflow-hidden" onWheel={handleWheel}>
        {/* Fixed fixture labels */}
        <div
          className="border-border bg-surface shrink-0 overflow-y-auto border-r"
          style={{ width: LABEL_WIDTH }}
        >
          {/* Spacer for ruler */}
          <div className="border-border border-b" style={{ height: RULER_HEIGHT }} />
          {show?.fixtures.map((fixture, i) => {
            const row = stackedRows[i];
            return (
              <div
                key={fixture.id}
                className="border-border/40 flex flex-col justify-center border-b px-3"
                style={{ height: row?.rowHeight ?? MIN_ROW_HEIGHT }}
              >
                <span className="text-text-2 truncate text-xs font-medium">{fixture.name}</span>
                <span className="text-text-2 mt-0.5 text-[10px]">{fixture.pixel_count}px</span>
              </div>
            );
          })}
        </div>

        {/* Scrollable content area */}
        <div ref={scrollContainerRef} className="flex-1 overflow-auto">
          <div style={{ width: contentWidth, minWidth: "100%" }}>
            {/* Ruler */}
            <div
              className="border-border bg-bg/95 sticky top-0 z-20 cursor-pointer border-b backdrop-blur-sm"
              style={{ height: RULER_HEIGHT }}
              onClick={handleRulerClick}
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
                {/* Ruler playhead */}
                <div
                  className="bg-primary absolute inset-y-0 z-10 w-0.5"
                  style={{ left: playheadX }}
                />
              </div>
            </div>

            {/* Fixture rows */}
            <div
              className="relative"
              onClick={handleTrackAreaClick}
              onDoubleClick={handleTrackAreaDoubleClick}
            >
              {/* Waveform canvas backdrop */}
              {waveform && (
                <canvas
                  ref={waveformCanvasRef}
                  className="pointer-events-none absolute top-0 left-0 z-0"
                />
              )}
              {stackedRows.map((row) => {
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
                    {/* Grid lines (echo ruler ticks) */}
                    {ticks.map((t) => (
                      <div
                        key={t}
                        className="bg-border/15 absolute inset-y-0 w-px"
                        style={{ left: t * pxPerSec }}
                      />
                    ))}

                    {/* Effect blocks - Vixen-style stacked lanes */}
                    {row.effects.map((placed) => {
                      const isSelected = selectedEffects.has(placed.key);
                      const isHovered = hoveredEffect === placed.key;
                      const compact = row.laneCount > 2;
                      const laneHeight = BASE_LANE_HEIGHT - LANE_GAP;
                      const top = ROW_PADDING + placed.lane * BASE_LANE_HEIGHT;

                      // Apply drag preview overrides
                      const preview = dragPreview?.key === placed.key ? dragPreview : null;
                      const isDragging = dragState?.key === placed.key;
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

                      return (
                        <div
                          key={placed.key}
                          data-effect-key={placed.key}
                          className="absolute cursor-pointer overflow-hidden transition-[box-shadow] duration-75"
                          style={{
                            left: displayStart * pxPerSec,
                            width: displayDuration * pxPerSec,
                            top,
                            height: laneHeight,
                            zIndex: isSelected ? 10 : isHovered ? 5 : 1,
                            borderRadius: 2,
                            border: isSelected
                              ? "2px solid var(--primary)"
                              : isHovered
                                ? "1px solid var(--text-2)"
                                : "1px solid var(--border)",
                            boxShadow: isSelected
                              ? "0 0 4px color-mix(in srgb, var(--primary) 25%, transparent)"
                              : "none",
                            opacity: isMoveDragToOtherRow ? 0.3 : 1,
                          }}
                          onClick={(e) => {
                            if (!isDragging) handleEffectClick(e, placed.key);
                          }}
                          onMouseDown={(e) => handleMoveMouseDown(e, placed, row.fixtureId)}
                          onMouseEnter={() => !dragState && setHoveredEffect(placed.key)}
                          onMouseLeave={() => setHoveredEffect(null)}
                        >
                          {/* Left resize handle */}
                          {isSelected && (
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
                            effectKind={placed.kind}
                            blendMode={placed.blendMode}
                            compact={compact}
                            refreshKey={refreshKey}
                          />

                          {/* Right resize handle */}
                          {isSelected && (
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

                    {/* Playhead */}
                    <div
                      className="bg-primary pointer-events-none absolute inset-y-0 z-30 w-0.5"
                      style={{ left: playheadX }}
                    />
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export { type TimelineProps };
