import { useCallback, useState } from "react";
import { cmd } from "../commands";
import type { EffectKind, PlaybackInfo, Show } from "../types";
import { makeEffectKey } from "../utils/effectKey";

export interface EffectEditingState {
  addEffectState: { fixtureId: number; time: number; screenPos: { x: number; y: number } } | null;
  cancelAddEffect: () => void;
  handleAddEffect: (fixtureId: number, time: number, screenPos: { x: number; y: number }) => void;
  handleEffectTypeSelected: (kind: EffectKind) => Promise<void>;
  handleMoveEffect: (
    fromTrackIndex: number,
    effectIndex: number,
    targetFixtureId: number,
    newStart: number,
    newEnd: number,
  ) => Promise<void>;
  handleResizeEffect: (trackIndex: number, effectIndex: number, newStart: number, newEnd: number) => Promise<void>;
}

export function useEffectEditing(
  show: Show | null,
  playback: PlaybackInfo | null,
  setSelectedEffects: (s: Set<string>) => void,
  commitChange: () => void,
): EffectEditingState {
  const [addEffectState, setAddEffectState] = useState<{
    fixtureId: number;
    time: number;
    screenPos: { x: number; y: number };
  } | null>(null);

  const cancelAddEffect = useCallback(() => setAddEffectState(null), []);

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
        setSelectedEffects(new Set([`${toTrackIndex}-${newEffectIndex}`]));
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
    cancelAddEffect,
    handleAddEffect,
    handleEffectTypeSelected,
    handleMoveEffect,
    handleResizeEffect,
  };
}
