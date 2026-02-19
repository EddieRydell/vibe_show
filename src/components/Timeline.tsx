import { useCallback, useMemo, useRef, useState } from "react";
import { EffectBlock } from "./EffectBlock";
import type { BlendMode, PlaybackInfo, Show, Track } from "../types";

interface TimelineProps {
  show: Show | null;
  playback: PlaybackInfo | null;
  onSeek: (time: number) => void;
  selectedEffects: Set<string>;
  onSelectionChange: (selection: Set<string>) => void;
  refreshKey?: number;
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
    else if ("Group" in m) ids.push(...resolveGroupFixtures((m as { Group: number }).Group, show, seen));
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

export function Timeline({
  show,
  playback,
  onSeek,
  selectedEffects,
  onSelectionChange,
  refreshKey,
}: TimelineProps) {
  const duration = playback?.duration ?? 0;
  const currentTime = playback?.current_time ?? 0;
  const sequence = show?.sequences[playback?.sequence_index ?? 0];
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [pxPerSec, setPxPerSec] = useState(DEFAULT_PX_PER_SEC);
  const [hoveredEffect, setHoveredEffect] = useState<string | null>(null);

  const contentWidth = duration * pxPerSec;

  // Pre-compute stacked layouts for all fixtures
  const stackedRows = useMemo(() => {
    if (!show || !sequence) return [];
    return show.fixtures.map((fixture) => computeStackedLayout(fixture.id, show, sequence));
  }, [show, sequence]);

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
          <span className="text-primary ml-auto text-[10px]">
            {selectedEffects.size} selected
          </span>
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
                <span className="text-text-2 truncate text-xs font-medium">
                  {fixture.name}
                </span>
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
            <div onClick={handleTrackAreaClick}>
              {stackedRows.map((row) => (
                <div
                  key={row.fixtureId}
                  className="border-border/30 relative border-b"
                  style={{ height: row.rowHeight }}
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

                    return (
                      <div
                        key={placed.key}
                        data-effect-key={placed.key}
                        className="absolute cursor-pointer overflow-hidden transition-[box-shadow] duration-75"
                        style={{
                          left: placed.startSec * pxPerSec,
                          width: placed.durationSec * pxPerSec,
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
                            ? "0 0 4px rgba(124, 92, 255, 0.25)"
                            : "none",
                        }}
                        onClick={(e) => handleEffectClick(e, placed.key)}
                        onMouseEnter={() => setHoveredEffect(placed.key)}
                        onMouseLeave={() => setHoveredEffect(null)}
                      >
                        <EffectBlock
                          sequenceIndex={playback?.sequence_index ?? 0}
                          trackIndex={placed.trackIndex}
                          effectIndex={placed.effectIndex}
                          effectKind={placed.kind}
                          blendMode={placed.blendMode}
                          compact={compact}
                          refreshKey={refreshKey}
                        />
                      </div>
                    );
                  })}

                  {/* Playhead */}
                  <div
                    className="bg-primary pointer-events-none absolute inset-y-0 z-30 w-0.5"
                    style={{ left: playheadX }}
                  />
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export { type TimelineProps };
