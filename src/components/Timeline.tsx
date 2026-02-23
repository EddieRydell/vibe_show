import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EffectBlock } from "./EffectBlock";
import type { AudioAnalysis, BlendMode, InteractionMode, PlaybackInfo, Show, Track } from "../types";
import { effectKindLabel } from "../types";
import type { WaveformData } from "../hooks/useAudio";
import { makeEffectKey } from "../utils/effectKey";
import { getEffectiveZoom } from "../utils/cssZoom";

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

const LABEL_WIDTH = 160;
const BASE_LANE_HEIGHT = 24;
const LANE_GAP = 2;
const ROW_PADDING = 2;
const MIN_ROW_HEIGHT = 48;
const RULER_HEIGHT = 28;
const WAVEFORM_LANE_HEIGHT = 48;
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

/** Map song section labels to semi-transparent background colors. */
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

/** Resolve a group to its fixture IDs with memoization. */
function resolveGroupCached(
  groupId: number,
  show: Show,
  cache: Map<number, Set<number>>,
  visited?: Set<number>,
): Set<number> {
  const cached = cache.get(groupId);
  if (cached) return cached;

  const seen = visited ?? new Set<number>();
  if (seen.has(groupId)) return new Set();
  seen.add(groupId);

  const group = show.groups.find((g) => g.id === groupId);
  if (!group) {
    cache.set(groupId, new Set());
    return new Set();
  }

  const ids = new Set<number>();
  for (const m of group.members) {
    if ("Fixture" in m) ids.add((m as { Fixture: number }).Fixture);
    else if ("Group" in m) {
      for (const fid of resolveGroupCached((m as { Group: number }).Group, show, cache, seen)) {
        ids.add(fid);
      }
    }
  }
  cache.set(groupId, ids);
  return ids;
}

/** Build a Map from fixtureId → Set<trackIndex>. O(tracks + groups) total. */
function buildFixtureTrackMap(
  show: Show,
  sequence: { tracks: Track[] },
): Map<number, Set<number>> {
  const allFixtureIds = new Set(show.fixtures.map((f) => f.id));
  const groupCache = new Map<number, Set<number>>();
  const map = new Map<number, Set<number>>();

  for (let trackIdx = 0; trackIdx < sequence.tracks.length; trackIdx++) {
    const target = sequence.tracks[trackIdx].target;
    let fixtureIds: Iterable<number>;

    if (target === "All") {
      fixtureIds = allFixtureIds;
    } else if ("Fixtures" in target) {
      fixtureIds = target.Fixtures;
    } else if ("Group" in target) {
      fixtureIds = resolveGroupCached(target.Group, show, groupCache);
    } else {
      continue;
    }

    for (const fid of fixtureIds) {
      let set = map.get(fid);
      if (!set) {
        set = new Set();
        map.set(fid, set);
      }
      set.add(trackIdx);
    }
  }

  return map;
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

/** Greedy lane assignment using pre-computed track indices. */
function computeStackedLayoutFast(
  fixtureId: number,
  sequence: { tracks: Track[] },
  trackIndices: Set<number>,
): StackedRow {
  const effects: PlacedEffect[] = [];

  for (const trackIdx of trackIndices) {
    const track = sequence.tracks[trackIdx];
    for (let effectIdx = 0; effectIdx < track.effects.length; effectIdx++) {
      const effect = track.effects[effectIdx];
      effects.push({
        key: makeEffectKey(trackIdx, effectIdx),
        trackIndex: trackIdx,
        effectIndex: effectIdx,
        startSec: effect.time_range.start,
        durationSec: effect.time_range.end - effect.time_range.start,
        blendMode: effect.blend_mode,
        kind: effectKindLabel(effect.kind),
        lane: 0,
      });
    }
  }

  // Sort by start time for greedy assignment
  effects.sort((a, b) => a.startSec - b.startSec);

  // laneEnds[i] = the end time of the last effect placed in lane i
  const laneEnds: number[] = [];

  for (const effect of effects) {
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
    }
  | {
      type: "marquee";
      startClientX: number;
      startClientY: number;
      shiftHeld: boolean;
      /** Selection snapshot at drag start (for shift+marquee additive) */
      baseSelection: Set<string>;
    }
  | {
      type: "swipe";
      altHeld: boolean;
      /** Keys swiped during this drag */
      swipedKeys: Set<string>;
      /** Selection snapshot at drag start */
      baseSelection: Set<string>;
    };

interface DragPreview {
  key: string;
  start: number;
  end: number;
  targetFixtureId?: number;
}

interface MarqueeRect {
  x: number;
  y: number;
  width: number;
  height: number;
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
  const [dragState, setDragState] = useState<DragState | null>(null);
  const [dragPreview, setDragPreview] = useState<DragPreview | null>(null);
  const [marqueeRect, setMarqueeRect] = useState<MarqueeRect | null>(null);
  const dragStateRef = useRef<DragState | null>(null);
  const justFinishedDragRef = useRef(false);
  const pxPerSecRef = useRef(pxPerSec);
  pxPerSecRef.current = pxPerSec;
  const waveformLaneCanvasRef = useRef<HTMLCanvasElement>(null);
  const trackAreaRef = useRef<HTMLDivElement>(null);
  const labelContainerRef = useRef<HTMLDivElement>(null);

  // Virtualization state
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  const contentWidth = duration * pxPerSec;

  // Pre-compute fixture→tracks map: O(tracks + groups) instead of O(fixtures x tracks x groups)
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

  /** Get fixture ID from a clientY position relative to the track area. */
  const getFixtureIdFromClientY = useCallback(
    (clientY: number) => {
      const container = scrollContainerRef.current;
      if (!container || rowOffsets.length === 0) return null;
      const rect = container.getBoundingClientRect();
      const zoom = getEffectiveZoom(container);
      const waveH = waveform ? WAVEFORM_LANE_HEIGHT : 0;
      const y = (clientY - rect.top) / zoom - RULER_HEIGHT - waveH + container.scrollTop;
      for (const row of rowOffsets) {
        if (y >= row.top && y < row.bottom) return row.fixtureId;
      }
      // Clamp: return first or last row
      if (y < 0) return rowOffsets[0].fixtureId;
      return rowOffsets[rowOffsets.length - 1].fixtureId;
    },
    [rowOffsets, waveform],
  );

  /** Hit-test which effect keys fall inside a screen-space rectangle (relative to track area). */
  const getEffectsInRect = useCallback(
    (rectX: number, rectY: number, rectW: number, rectH: number) => {
      // Convert screen rect to time/row space
      const container = scrollContainerRef.current;
      if (!container) return new Set<string>();
      const containerRect = container.getBoundingClientRect();
      const waveH = waveform ? WAVEFORM_LANE_HEIGHT : 0;

      // rectX/rectY are client coords; convert to content space
      const scrollLeft = container.scrollLeft;
      const scrollTop = container.scrollTop;
      const contentX = rectX - containerRect.left + scrollLeft;
      const contentY = rectY - containerRect.top - RULER_HEIGHT - waveH + scrollTop;

      const timeLeft = contentX / pxPerSec;
      const timeRight = (contentX + rectW) / pxPerSec;
      const yTop = contentY;
      const yBottom = contentY + rectH;

      const keys = new Set<string>();
      for (const row of stackedRows) {
        const rowInfo = rowOffsets.find((r) => r.fixtureId === row.fixtureId);
        if (!rowInfo) continue;
        // Check vertical overlap between marquee and this row
        if (rowInfo.bottom <= yTop || rowInfo.top >= yBottom) continue;
        for (const effect of row.effects) {
          const effEnd = effect.startSec + effect.durationSec;
          // Check time overlap
          if (effEnd <= timeLeft || effect.startSec >= timeRight) continue;
          keys.add(effect.key);
        }
      }
      return keys;
    },
    [pxPerSec, stackedRows, rowOffsets, waveform],
  );

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

  // ── Track area mousedown (marquee in Select mode, swipe in Swipe mode) ──

  const handleTrackAreaMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      // Don't interfere with effect blocks
      if ((e.target as HTMLElement).closest("[data-effect-key]")) return;

      if (mode === "select") {
        // Start marquee selection
        e.preventDefault();
        const state: DragState = {
          type: "marquee",
          startClientX: e.clientX,
          startClientY: e.clientY,
          shiftHeld: e.shiftKey,
          baseSelection: e.shiftKey ? new Set(selectedEffects) : new Set(),
        };
        setDragState(state);
        dragStateRef.current = state;
      } else if (mode === "swipe") {
        // Start swipe selection
        e.preventDefault();
        const state: DragState = {
          type: "swipe",
          altHeld: e.altKey,
          swipedKeys: new Set(),
          baseSelection: new Set(selectedEffects),
        };
        setDragState(state);
        dragStateRef.current = state;
      }
    },
    [mode, selectedEffects],
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

  // Stable refs for callbacks used in the global effect
  const selectedEffectsRef = useRef(selectedEffects);
  selectedEffectsRef.current = selectedEffects;
  const onSelectionChangeRef = useRef(onSelectionChange);
  onSelectionChangeRef.current = onSelectionChange;
  const getEffectsInRectRef = useRef(getEffectsInRect);
  getEffectsInRectRef.current = getEffectsInRect;
  useEffect(() => {
    function handleMouseMove(e: MouseEvent) {
      const ds = dragStateRef.current;
      if (!ds) return;

      if (ds.type === "resize") {
        const pps = pxPerSecRef.current;
        const container = scrollContainerRef.current;
        const zoom = container ? getEffectiveZoom(container) : 1;
        const deltaPx = (e.clientX - ds.startClientX) / zoom;
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
        setDragPreview({ key: ds.key, start: newStart, end: newEnd });
      } else if (ds.type === "move") {
        const pps = pxPerSecRef.current;
        const container = scrollContainerRef.current;
        const zoom = container ? getEffectiveZoom(container) : 1;
        const deltaPx = (e.clientX - ds.startClientX) / zoom;
        const deltaSec = deltaPx / pps;
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
      } else if (ds.type === "marquee") {
        // Update marquee rectangle (use min/max to handle any drag direction)
        const x = Math.min(e.clientX, ds.startClientX);
        const y = Math.min(e.clientY, ds.startClientY);
        const w = Math.abs(e.clientX - ds.startClientX);
        const h = Math.abs(e.clientY - ds.startClientY);
        setMarqueeRect({ x, y, width: w, height: h });

        // Live-update selection based on effects inside marquee
        const inside = getEffectsInRectRef.current(x, y, w, h);
        if (ds.shiftHeld) {
          // Additive: base selection + marquee contents
          const next = new Set(ds.baseSelection);
          for (const k of inside) next.add(k);
          onSelectionChangeRef.current(next);
        } else {
          onSelectionChangeRef.current(inside);
        }
      } else if (ds.type === "swipe") {
        // Check element under cursor for effect keys
        const el = document.elementFromPoint(e.clientX, e.clientY);
        if (el) {
          const effectEl = (el as HTMLElement).closest("[data-effect-key]");
          if (effectEl) {
            const key = (effectEl as HTMLElement).dataset.effectKey;
            if (key && !ds.swipedKeys.has(key)) {
              ds.swipedKeys.add(key);
              // Update selection
              const next = new Set(ds.baseSelection);
              if (ds.altHeld) {
                // Alt: remove swiped keys
                for (const k of ds.swipedKeys) next.delete(k);
              } else {
                // Normal: add swiped keys
                for (const k of ds.swipedKeys) next.add(k);
              }
              onSelectionChangeRef.current(next);
            }
          }
        }
      }
    }

    async function handleMouseUp(e: MouseEvent) {
      const ds = dragStateRef.current;
      if (!ds) return;

      dragStateRef.current = null;
      setDragState(null);

      if (ds.type === "resize") {
        const pps = pxPerSecRef.current;
        const container = scrollContainerRef.current;
        const zoom = container ? getEffectiveZoom(container) : 1;
        const deltaPx = (e.clientX - ds.startClientX) / zoom;
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

        if (onResizeEffect) {
          try {
            await onResizeEffect(ds.trackIndex, ds.effectIndex, newStart, newEnd);
          } catch (err) {
            console.error("[VibeLights] Resize failed:", err);
          }
        }
      } else if (ds.type === "move") {
        if (!ds.didDrag) {
          // It was a click, not a drag — handle selection
          const sel = selectedEffectsRef.current;
          if (e.shiftKey) {
            const next = new Set(sel);
            if (next.has(ds.key)) next.delete(ds.key);
            else next.add(ds.key);
            onSelectionChangeRef.current(next);
          } else {
            onSelectionChangeRef.current(new Set([ds.key]));
          }
        } else if (onMoveEffect) {
          const pps = pxPerSecRef.current;
          const container = scrollContainerRef.current;
          const zoom = container ? getEffectiveZoom(container) : 1;
          const deltaSec = (e.clientX - ds.startClientX) / zoom / pps;
          const dur = ds.originalEnd - ds.originalStart;
          let newStart = ds.originalStart + deltaSec;
          newStart = Math.max(0, Math.min(newStart, duration - dur));
          const newEnd = newStart + dur;
          const targetFixtureId = getFixtureIdFromClientY(e.clientY) ?? ds.originalFixtureId;

          try {
            await onMoveEffect(ds.trackIndex, ds.effectIndex, targetFixtureId, newStart, newEnd);
          } catch (err) {
            console.error("[VibeLights] Move failed:", err);
          }
        }
      } else if (ds.type === "marquee") {
        setMarqueeRect(null);
        // Only suppress the click event if the mouse actually moved (real drag).
        // Zero-distance clicks fall through to handleTrackAreaClick → seek + clear selection.
        const dist = Math.abs(e.clientX - ds.startClientX) + Math.abs(e.clientY - ds.startClientY);
        if (dist >= DRAG_THRESHOLD) {
          justFinishedDragRef.current = true;
        }
      } else if (ds.type === "swipe") {
        // Only suppress click if effects were actually swiped
        if (ds.swipedKeys.size > 0) {
          justFinishedDragRef.current = true;
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
  ]);

  // Waveform canvas drawing
  const drawWaveform = useCallback(
    (canvas: HTMLCanvasElement | null, height: number, alpha: number) => {
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
    },
    [waveform, contentWidth, pxPerSec],
  );

  // Draw waveform lane
  useEffect(() => {
    drawWaveform(waveformLaneCanvasRef.current, WAVEFORM_LANE_HEIGHT, 0.7);
  }, [drawWaveform]);

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
      // Both the ruler and waveform are sticky in the main scroll container,
      // so they always occupy RULER_HEIGHT + WAVEFORM_LANE_HEIGHT at the top.
      // The label panel has matching fixed-height spacers for the ruler and
      // waveform labels, so the label scroll area and the track content area
      // start at the same vertical offset. Syncing scrollTop directly keeps
      // labels perfectly aligned with their corresponding track rows.
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
                              // In swipe mode, mousedown on effect starts swipe
                              e.stopPropagation();
                              e.preventDefault();
                              const state: DragState = {
                                type: "swipe",
                                altHeld: e.altKey,
                                swipedKeys: new Set([placed.key]),
                                baseSelection: new Set(selectedEffects),
                              };
                              setDragState(state);
                              dragStateRef.current = state;
                              // Immediately update selection
                              const next = new Set(selectedEffects);
                              if (e.altKey) next.delete(placed.key);
                              else next.add(placed.key);
                              onSelectionChange(next);
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
