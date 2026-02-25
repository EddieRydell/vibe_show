// Pure layout computation functions and types extracted from Timeline.tsx.

import type { BlendMode, Show, Track } from "../types";
import { effectKindLabel } from "../types";
import { makeEffectKey } from "./effectKey";
import { BASE_LANE_HEIGHT, MIN_ROW_HEIGHT, ROW_PADDING } from "./timelineConstants";

export interface PlacedEffect {
  key: string;
  trackIndex: number;
  effectIndex: number;
  startSec: number;
  durationSec: number;
  blendMode: BlendMode;
  kind: string;
  lane: number;
}

export interface StackedRow {
  fixtureId: number;
  effects: PlacedEffect[];
  laneCount: number;
  rowHeight: number;
}

/** Resolve a group to its fixture IDs with memoization. */
export function resolveGroupCached(
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

/** Build a Map from fixtureId -> Set<trackIndex>. O(tracks + groups) total. */
export function buildFixtureTrackMap(
  show: Show,
  sequence: { tracks: Track[] },
): Map<number, Set<number>> {
  const allFixtureIds = new Set(show.fixtures.map((f) => f.id));
  const groupCache = new Map<number, Set<number>>();
  const map = new Map<number, Set<number>>();

  for (let trackIdx = 0; trackIdx < sequence.tracks.length; trackIdx++) {
    const target = sequence.tracks[trackIdx]!.target;
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

/** Greedy lane assignment using pre-computed track indices. */
export function computeStackedLayoutFast(
  fixtureId: number,
  sequence: { tracks: Track[] },
  trackIndices: Set<number>,
): StackedRow {
  const effects: PlacedEffect[] = [];

  for (const trackIdx of trackIndices) {
    const track = sequence.tracks[trackIdx]!;
    for (let effectIdx = 0; effectIdx < track.effects.length; effectIdx++) {
      const effect = track.effects[effectIdx]!;
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
      if (effect.startSec >= laneEnds[i]!) {
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
