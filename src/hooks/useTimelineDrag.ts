// Drag-related state and handlers extracted from Timeline.tsx.

import { useCallback, useEffect, useRef, useState } from "react";
import type { PlacedEffect, StackedRow } from "../utils/timelineLayout";
import type { InteractionMode } from "../types";
import type { WaveformData } from "./useAudio";
import { getEffectiveZoom } from "../utils/cssZoom";
import {
  DRAG_THRESHOLD,
  MIN_DURATION,
  RULER_HEIGHT,
  WAVEFORM_LANE_HEIGHT,
} from "../utils/timelineConstants";

// ── Drag state types ────────────────────────────────────────────────

export type DragState =
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

export interface DragPreview {
  key: string;
  start: number;
  end: number;
  targetFixtureId?: number;
}

export interface MarqueeRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface UseTimelineDragParams {
  duration: number;
  pxPerSecRef: React.RefObject<number>;
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  selectedEffects: Set<string>;
  onSelectionChange: (selection: Set<string>) => void;
  mode: InteractionMode;
  waveform: WaveformData | null | undefined;
  stackedRows: StackedRow[];
  rowOffsets: { fixtureId: number; top: number; bottom: number }[];
  pxPerSec: number;
  onMoveEffect?: ((
    fromTrackIndex: number,
    effectIndex: number,
    targetFixtureId: number,
    newStart: number,
    newEnd: number,
  ) => Promise<void>) | undefined;
  onResizeEffect?: ((
    trackIndex: number,
    effectIndex: number,
    newStart: number,
    newEnd: number,
  ) => Promise<void>) | undefined;
  onRefresh?: (() => void) | undefined;
  sequenceIndex: number | undefined;
}

export function useTimelineDrag({
  duration,
  pxPerSecRef,
  scrollContainerRef,
  selectedEffects,
  onSelectionChange,
  mode,
  waveform,
  stackedRows,
  rowOffsets,
  pxPerSec,
  onMoveEffect,
  onResizeEffect,
  onRefresh,
  sequenceIndex,
}: UseTimelineDragParams) {
  const [dragState, setDragState] = useState<DragState | null>(null);
  const [dragPreview, setDragPreview] = useState<DragPreview | null>(null);
  const [marqueeRect, setMarqueeRect] = useState<MarqueeRect | null>(null);
  const dragStateRef = useRef<DragState | null>(null);
  const justFinishedDragRef = useRef(false);

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
      if (y < 0) return rowOffsets[0]!.fixtureId;
      return rowOffsets[rowOffsets.length - 1]!.fixtureId;
    },
    [scrollContainerRef, rowOffsets, waveform],
  );

  /** Hit-test which effect keys fall inside a screen-space rectangle (relative to track area). */
  const getEffectsInRect = useCallback(
    (rectX: number, rectY: number, rectW: number, rectH: number) => {
      const container = scrollContainerRef.current;
      if (!container) return new Set<string>();
      const containerRect = container.getBoundingClientRect();
      const waveH = waveform ? WAVEFORM_LANE_HEIGHT : 0;

      const scrollLeft = container.scrollLeft;
      const scrollTopVal = container.scrollTop;
      const contentX = rectX - containerRect.left + scrollLeft;
      const contentY = rectY - containerRect.top - RULER_HEIGHT - waveH + scrollTopVal;

      const timeLeft = contentX / pxPerSec;
      const timeRight = (contentX + rectW) / pxPerSec;
      const yTop = contentY;
      const yBottom = contentY + rectH;

      const keys = new Set<string>();
      for (const row of stackedRows) {
        const rowInfo = rowOffsets.find((r) => r.fixtureId === row.fixtureId);
        if (!rowInfo) continue;
        if (rowInfo.bottom <= yTop || rowInfo.top >= yBottom) continue;
        for (const effect of row.effects) {
          const effEnd = effect.startSec + effect.durationSec;
          if (effEnd <= timeLeft || effect.startSec >= timeRight) continue;
          keys.add(effect.key);
        }
      }
      return keys;
    },
    [scrollContainerRef, pxPerSec, stackedRows, rowOffsets, waveform],
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
      if ((e.target as HTMLElement).dataset["resizeHandle"]) return;
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

  // ── Track area mousedown (marquee in Select mode, swipe in Swipe mode) ──

  const handleTrackAreaMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if ((e.target as HTMLElement).closest("[data-effect-key]")) return;

      if (mode === "select") {
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

  /** Start a swipe drag from an effect block mousedown. */
  const startSwipeDrag = useCallback(
    (e: React.MouseEvent, placedKey: string) => {
      e.stopPropagation();
      e.preventDefault();
      const state: DragState = {
        type: "swipe",
        altHeld: e.altKey,
        swipedKeys: new Set([placedKey]),
        baseSelection: new Set(selectedEffects),
      };
      setDragState(state);
      dragStateRef.current = state;
      // Immediately update selection
      const next = new Set(selectedEffects);
      if (e.altKey) next.delete(placedKey);
      else next.add(placedKey);
      onSelectionChange(next);
    },
    [selectedEffects, onSelectionChange],
  );

  // ── Global mousemove/mouseup for drag operations ──────────────

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
        const x = Math.min(e.clientX, ds.startClientX);
        const y = Math.min(e.clientY, ds.startClientY);
        const w = Math.abs(e.clientX - ds.startClientX);
        const h = Math.abs(e.clientY - ds.startClientY);
        setMarqueeRect({ x, y, width: w, height: h });

        const inside = getEffectsInRectRef.current(x, y, w, h);
        if (ds.shiftHeld) {
          const next = new Set(ds.baseSelection);
          for (const k of inside) next.add(k);
          onSelectionChangeRef.current(next);
        } else {
          onSelectionChangeRef.current(inside);
        }
      } else {
        // ds.type === "swipe"
        const el = document.elementFromPoint(e.clientX, e.clientY);
        if (el instanceof HTMLElement) {
          const effectEl = el.closest("[data-effect-key]");
          if (effectEl instanceof HTMLElement) {
            const key = effectEl.dataset["effectKey"];
            if (key && !ds.swipedKeys.has(key)) {
              ds.swipedKeys.add(key);
              const next = new Set(ds.baseSelection);
              if (ds.altHeld) {
                for (const k of ds.swipedKeys) next.delete(k);
              } else {
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
        const dist = Math.abs(e.clientX - ds.startClientX) + Math.abs(e.clientY - ds.startClientY);
        if (dist >= DRAG_THRESHOLD) {
          justFinishedDragRef.current = true;
        }
      } else {
        // ds.type === "swipe"
        if (ds.swipedKeys.size > 0) {
          justFinishedDragRef.current = true;
        }
      }

      setDragPreview(null);
    }

    function handleMouseUpSync(e: MouseEvent) {
      void handleMouseUp(e);
    }

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUpSync);
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUpSync);
    };
  }, [
    duration,
    sequenceIndex,
    onRefresh,
    onMoveEffect,
    onResizeEffect,
    getFixtureIdFromClientY,
    pxPerSecRef,
    scrollContainerRef,
  ]);

  return {
    dragState,
    setDragState,
    dragPreview,
    marqueeRect,
    dragStateRef,
    justFinishedDragRef,
    handleResizeMouseDown,
    handleMoveMouseDown,
    handleTrackAreaMouseDown,
    startSwipeDrag,
    getFixtureIdFromClientY,
    getEffectsInRect,
  };
}
