// Effect action handlers extracted from EditorScreen.tsx.

import { useCallback, useState } from "react";
import { cmd } from "../commands";
import { makeEffectKey } from "../utils/effectKey";
import type { EffectKind, PlaybackInfo, Show } from "../types";

interface AddEffectState {
  fixtureId: number;
  time: number;
  screenPos: { x: number; y: number };
}

interface UseEffectActionsParams {
  show: Show | null;
  playback: PlaybackInfo | null;
  commitChange: (opts?: { skipRefreshAll?: boolean; skipDirty?: boolean }) => void;
  setSelectedEffects: (sel: Set<string>) => void;
}

export function useEffectActions({
  show,
  playback,
  commitChange,
  setSelectedEffects,
}: UseEffectActionsParams) {
  const [addEffectState, setAddEffectState] = useState<AddEffectState | null>(null);

  const handleAddEffect = useCallback(
    (fixtureId: number, time: number, screenPos: { x: number; y: number }) => {
      setAddEffectState({ fixtureId, time, screenPos });
    },
    [],
  );

  const handleEffectTypeSelected = useCallback(
    async (kind: EffectKind) => {
      if (!addEffectState || !show || !playback) return;
      const { fixtureId, time } = addEffectState;
      const sequenceIndex = playback.sequence_index;
      const sequence = show.sequences[sequenceIndex];
      if (!sequence) return;

      setAddEffectState(null);

      try {
        // Find existing track targeting this fixture, or create one
        let trackIndex = sequence.tracks.findIndex((t) => {
          const target = t.target;
          return (
            typeof target === "object" &&
            "Fixtures" in target &&
            target.Fixtures.length === 1 &&
            target.Fixtures[0] === fixtureId
          );
        });

        if (trackIndex === -1) {
          const fixture = show.fixtures.find((f) => f.id === fixtureId);
          const trackName = fixture ? fixture.name : `Fixture ${fixtureId}`;
          trackIndex = await cmd.addTrack(trackName, fixtureId);
        }

        const end = Math.min(time + 2.0, sequence.duration);
        const start = Math.max(0, end - 2.0);
        const effectIndex = await cmd.addEffect(trackIndex, kind, start, end);

        commitChange();
        setSelectedEffects(new Set([makeEffectKey(trackIndex, effectIndex)]));
      } catch (e) {
        console.error("[VibeLights] Add effect failed:", e);
      }
    },
    [addEffectState, show, playback, commitChange, setSelectedEffects],
  );

  const handleMoveEffect = useCallback(
    async (
      fromTrackIndex: number,
      effectIndex: number,
      targetFixtureId: number,
      newStart: number,
      newEnd: number,
    ) => {
      if (!show || !playback) return;
      const sequenceIndex = playback.sequence_index;
      const sequence = show.sequences[sequenceIndex];
      if (!sequence) return;

      const fromTrack = sequence.tracks[fromTrackIndex];
      if (!fromTrack) return;

      const fromTarget = fromTrack.target;
      const staysOnSameFixture =
        fromTarget === "All" ||
        (typeof fromTarget === "object" &&
          "Fixtures" in fromTarget &&
          fromTarget.Fixtures.includes(targetFixtureId));

      if (staysOnSameFixture) {
        await cmd.updateEffectTimeRange(fromTrackIndex, effectIndex, newStart, newEnd);
        commitChange();
      } else {
        let toTrackIndex = sequence.tracks.findIndex((t) => {
          const target = t.target;
          return (
            typeof target === "object" &&
            "Fixtures" in target &&
            target.Fixtures.length === 1 &&
            target.Fixtures[0] === targetFixtureId
          );
        });

        if (toTrackIndex === -1) {
          const fixture = show.fixtures.find((f) => f.id === targetFixtureId);
          const trackName = fixture ? fixture.name : `Fixture ${targetFixtureId}`;
          toTrackIndex = await cmd.addTrack(trackName, targetFixtureId);
        }

        const newEffectIndex = await cmd.moveEffectToTrack(fromTrackIndex, effectIndex, toTrackIndex);
        await cmd.updateEffectTimeRange(toTrackIndex, newEffectIndex, newStart, newEnd);

        commitChange();
        setSelectedEffects(new Set([makeEffectKey(toTrackIndex, newEffectIndex)]));
      }
    },
    [show, playback, commitChange, setSelectedEffects],
  );

  const handleResizeEffect = useCallback(
    async (trackIndex: number, effectIndex: number, newStart: number, newEnd: number) => {
      if (!playback) return;
      await cmd.updateEffectTimeRange(trackIndex, effectIndex, newStart, newEnd);
      commitChange();
    },
    [playback, commitChange],
  );

  return {
    addEffectState,
    setAddEffectState,
    handleAddEffect,
    handleEffectTypeSelected,
    handleMoveEffect,
    handleResizeEffect,
  };
}
