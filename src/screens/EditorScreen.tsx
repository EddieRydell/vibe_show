import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Settings } from "lucide-react";
import { Preview } from "../components/Preview";
import { Timeline } from "../components/Timeline";
import { Toolbar } from "../components/Toolbar";
import { FixtureList } from "../components/FixtureList";
import { PropertyPanel } from "../components/PropertyPanel";
import { EffectPicker } from "../components/EffectPicker";
import { useEngine } from "../hooks/useEngine";
import { useKeyboard } from "../hooks/useKeyboard";
import type { EffectKind } from "../types";

interface Props {
  profileSlug: string;
  showSlug: string;
  onBack: () => void;
  onOpenSettings: () => void;
}

export function EditorScreen({ showSlug, onBack, onOpenSettings }: Props) {
  const {
    show,
    frame,
    playback,
    error,
    play,
    pause,
    seek,
    selectSequence,
    refreshAll,
  } = useEngine();
  const [previewCollapsed, setPreviewCollapsed] = useState(false);
  const [selectedEffects, setSelectedEffects] = useState<Set<string>>(new Set());
  const [refreshKey, setRefreshKey] = useState(0);
  const [loading, setLoading] = useState(true);
  const [addEffectState, setAddEffectState] = useState<{
    fixtureId: number;
    time: number;
    screenPos: { x: number; y: number };
  } | null>(null);

  // Load the show into the engine on mount
  useEffect(() => {
    invoke("open_show", { slug: showSlug })
      .then(() => {
        refreshAll();
        setLoading(false);
      })
      .catch((e) => {
        console.error("[VibeShow] Failed to open show:", e);
        setLoading(false);
      });
  }, [showSlug, refreshAll]);

  const handleStop = useCallback(() => {
    pause();
    seek(0);
  }, [pause, seek]);

  const handlePlayPause = useCallback(() => {
    if (playback?.playing) pause();
    else play();
  }, [playback?.playing, play, pause]);

  const handleSelectAll = useCallback(() => {
    if (!show || !playback) return;
    const sequence = show.sequences[playback.sequence_index];
    if (!sequence) return;
    const allKeys = new Set<string>();
    for (let tIdx = 0; tIdx < sequence.tracks.length; tIdx++) {
      for (let eIdx = 0; eIdx < sequence.tracks[tIdx].effects.length; eIdx++) {
        allKeys.add(`${tIdx}-${eIdx}`);
      }
    }
    setSelectedEffects(allKeys);
  }, [show, playback]);

  const handleDeleteSelected = useCallback(async () => {
    if (selectedEffects.size === 0 || !playback) return;
    const targets = [...selectedEffects].map((key) => {
      const [trackIndex, effectIndex] = key.split("-").map(Number);
      return [trackIndex, effectIndex] as [number, number];
    });
    try {
      await invoke("delete_effects", {
        sequenceIndex: playback.sequence_index,
        targets,
      });
      setSelectedEffects(new Set());
      refreshAll();
      setRefreshKey((k) => k + 1);
    } catch (e) {
      console.error("[VibeShow] Delete failed:", e);
    }
  }, [selectedEffects, playback, refreshAll]);

  const handleParamChange = useCallback(() => {
    setRefreshKey((k) => k + 1);
  }, []);

  const handleSave = useCallback(() => {
    invoke("save_current_show").catch((e) =>
      console.error("[VibeShow] Save failed:", e),
    );
  }, []);

  const handleSequenceChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      selectSequence(Number(e.target.value));
      setSelectedEffects(new Set());
    },
    [selectSequence],
  );

  const handleRefresh = useCallback(() => {
    refreshAll();
    setRefreshKey((k) => k + 1);
  }, [refreshAll]);

  // ── Add effect flow ──────────────────────────────────────────────

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
          trackIndex = await invoke<number>("add_track", {
            sequenceIndex,
            name: trackName,
            target: { Fixtures: [fixtureId] },
            blendMode: "Override",
          });
        }

        const end = Math.min(time + 2.0, sequence.duration);
        const start = Math.max(0, end - 2.0);
        const effectIndex = await invoke<number>("add_effect", {
          sequenceIndex,
          trackIndex,
          kind,
          start,
          end,
        });

        refreshAll();
        setRefreshKey((k) => k + 1);
        setSelectedEffects(new Set([`${trackIndex}-${effectIndex}`]));
      } catch (e) {
        console.error("[VibeShow] Add effect failed:", e);
      }
    },
    [addEffectState, show, playback, refreshAll],
  );

  // ── Move effect flow ─────────────────────────────────────────────

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

      // Check if the effect is staying on a track that targets this fixture
      const fromTarget = fromTrack.target;
      const staysOnSameFixture =
        fromTarget === "All" ||
        (typeof fromTarget === "object" &&
          "Fixtures" in fromTarget &&
          fromTarget.Fixtures.includes(targetFixtureId));

      if (staysOnSameFixture) {
        // Just update time
        await invoke("update_effect_time_range", {
          sequenceIndex,
          trackIndex: fromTrackIndex,
          effectIndex,
          start: newStart,
          end: newEnd,
        });
        refreshAll();
        setRefreshKey((k) => k + 1);
      } else {
        // Moving to a different fixture: find or create target track
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
          toTrackIndex = await invoke<number>("add_track", {
            sequenceIndex,
            name: trackName,
            target: { Fixtures: [targetFixtureId] },
            blendMode: fromTrack.blend_mode,
          });
        }

        const newEffectIndex = await invoke<number>("move_effect_to_track", {
          sequenceIndex,
          fromTrack: fromTrackIndex,
          effectIndex,
          toTrack: toTrackIndex,
        });

        await invoke("update_effect_time_range", {
          sequenceIndex,
          trackIndex: toTrackIndex,
          effectIndex: newEffectIndex,
          start: newStart,
          end: newEnd,
        });

        refreshAll();
        setRefreshKey((k) => k + 1);
        setSelectedEffects(new Set([`${toTrackIndex}-${newEffectIndex}`]));
      }
    },
    [show, playback, refreshAll],
  );

  const keyboardActions = useMemo(
    () => ({
      onPlayPause: handlePlayPause,
      onStop: handleStop,
      onSeekStart: () => seek(0),
      onSeekEnd: () => seek(playback?.duration ?? 0),
      onZoomIn: () => {},
      onZoomOut: () => {},
      onZoomFit: () => {},
      onSelectAll: handleSelectAll,
      onDeleteSelected: handleDeleteSelected,
    }),
    [handlePlayPause, handleStop, seek, playback?.duration, handleSelectAll, handleDeleteSelected],
  );

  useKeyboard(keyboardActions);

  const singleSelected = selectedEffects.size === 1 ? [...selectedEffects][0] : null;

  if (loading) {
    return (
      <div className="bg-bg flex h-screen items-center justify-center">
        <p className="text-text-2 text-sm">Loading show...</p>
      </div>
    );
  }

  return (
    <div className="bg-bg text-text flex h-screen flex-col">
      {/* Header */}
      <div className="border-border bg-bg flex items-center gap-1 border-b px-4 py-1.5">
        <button
          onClick={onBack}
          className="text-text-2 hover:text-text mr-2 text-sm transition-colors"
        >
          &larr; Back
        </button>
        <span className="text-text text-[15px] font-bold">VibeShow</span>
        <span className="text-text-2 ml-2 text-xs">{show?.name ?? "Untitled"}</span>
        <div className="ml-auto flex items-center gap-1">
          <button
            onClick={handleSave}
            className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-2 py-0.5 text-[11px] transition-colors"
          >
            Save
          </button>
          <button
            onClick={onOpenSettings}
            className="text-text-2 hover:text-text p-1 transition-colors"
            title="Settings"
          >
            <Settings size={14} />
          </button>
        </div>
      </div>

      {/* Toolbar */}
      <Toolbar
        playback={playback}
        onPlay={play}
        onPause={pause}
        onStop={handleStop}
        onSkipBack={() => seek(0)}
        onSkipForward={() => seek(playback?.duration ?? 0)}
      />

      {/* Sequence selector */}
      {show && show.sequences.length > 1 && (
        <div className="border-border bg-surface flex items-center gap-2 border-b px-4 py-1">
          <span className="text-text-2 text-[10px] tracking-wider uppercase">Sequence</span>
          <select
            value={playback?.sequence_index ?? 0}
            onChange={handleSequenceChange}
            className="border-border bg-surface-2 text-text rounded border px-2 py-0.5 text-xs"
          >
            {show.sequences.map((seq, i) => (
              <option key={i} value={i}>
                {seq.name}
              </option>
            ))}
          </select>
        </div>
      )}

      {/* Error banner */}
      {error && (
        <div className="bg-error/10 border-error/20 text-error border-b px-4 py-1.5 text-xs">
          Engine error: {error}
        </div>
      )}

      {/* Main area */}
      <div className="flex flex-1 overflow-hidden">
        <FixtureList show={show} />
        <Timeline
          show={show}
          playback={playback}
          onSeek={seek}
          selectedEffects={selectedEffects}
          onSelectionChange={setSelectedEffects}
          refreshKey={refreshKey}
          onAddEffect={handleAddEffect}
          onRefresh={handleRefresh}
          onMoveEffect={handleMoveEffect}
        />
        <PropertyPanel
          selectedEffect={singleSelected}
          sequenceIndex={playback?.sequence_index ?? 0}
          onParamChange={handleParamChange}
        />
      </div>

      {/* Preview */}
      <Preview
        show={show}
        frame={frame}
        collapsed={previewCollapsed}
        onToggle={() => setPreviewCollapsed((c) => !c)}
      />

      {/* Effect Picker popover */}
      {addEffectState && (
        <EffectPicker
          position={addEffectState.screenPos}
          onSelect={handleEffectTypeSelected}
          onCancel={() => setAddEffectState(null)}
        />
      )}
    </div>
  );
}
