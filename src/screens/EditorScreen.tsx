import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { MessageSquare, Settings, SlidersHorizontal } from "lucide-react";
import { Preview } from "../components/Preview";
import { Timeline } from "../components/Timeline";
import { Toolbar } from "../components/Toolbar";
import { PropertyPanel } from "../components/PropertyPanel";
import { EffectPicker } from "../components/EffectPicker";
import { SequenceSettingsDialog } from "../components/SequenceSettingsDialog";
import { ChatPanel } from "../components/ChatPanel";
import { AppBar } from "../components/AppBar";
import { useEngine } from "../hooks/useEngine";
import { useAudio } from "../hooks/useAudio";
import { useKeyboard } from "../hooks/useKeyboard";
import { useProgress } from "../hooks/useProgress";
import type { EffectKind, InteractionMode } from "../types";

interface Props {
  profileSlug: string;
  sequenceSlug: string;
  onBack: () => void;
  onOpenSettings: () => void;
}

export function EditorScreen({ sequenceSlug, onBack, onOpenSettings }: Props) {
  const progressOps = useProgress();
  const audio = useAudio();

  // Audio-master clock: returns audio time when audio is actively playing, null otherwise.
  // When null, useEngine falls back to its tick(dt) mode.
  const audioGetCurrentTime = useCallback((): number | null => {
    return audio.getCurrentTime();
  }, [audio]);

  const {
    show,
    frame,
    playback,
    undoState,
    error,
    play: enginePlay,
    pause: enginePause,
    seek: engineSeek,
    undo: engineUndo,
    redo: engineRedo,
    refreshAll,
  } = useEngine(audioGetCurrentTime);
  const [previewCollapsed, setPreviewCollapsed] = useState(false);
  const [previewDetached, setPreviewDetached] = useState(false);
  const previewWindowRef = useRef<WebviewWindow | null>(null);
  const [chatOpen, setChatOpen] = useState(false);
  const [selectedEffects, setSelectedEffects] = useState<Set<string>>(new Set());
  const [refreshKey, setRefreshKey] = useState(0);
  const [loading, setLoading] = useState(true);
  const [showSequenceSettings, setShowSequenceSettings] = useState(false);
  const [addEffectState, setAddEffectState] = useState<{
    fixtureId: number;
    time: number;
    screenPos: { x: number; y: number };
  } | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">("idle");
  const [interactionMode, setInteractionMode] = useState<InteractionMode>("select");
  const savedTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Load the sequence into the engine on mount
  useEffect(() => {
    invoke("open_sequence", { slug: sequenceSlug })
      .then(() => {
        refreshAll();
        setLoading(false);
      })
      .catch((e) => {
        console.error("[VibeLights] Failed to open sequence:", e);
        setLoading(false);
      });
  }, [sequenceSlug, refreshAll]);

  // Load audio when sequence changes
  const currentSequence = show?.sequences[playback?.sequence_index ?? 0];
  useEffect(() => {
    audio.loadAudio(currentSequence?.audio_file ?? null);
  }, [currentSequence?.audio_file, audio.loadAudio]);

  // Audio ended handler — stop at end
  useEffect(() => {
    audio.onEnded.current = () => {
      enginePause();
      audio.pause();
    };
  }, [audio, enginePause]);

  // ── Composed transport controls ────────────────────────────────────

  const play = useCallback(() => {
    enginePlay();
    if (audio.ready) audio.play();
  }, [enginePlay, audio]);

  const pause = useCallback(() => {
    enginePause();
    if (audio.ready) audio.pause();
  }, [enginePause, audio]);

  const seek = useCallback(
    (time: number) => {
      engineSeek(time);
      if (audio.ready) audio.seek(time);
    },
    [engineSeek, audio],
  );

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
      setDirty(true);
      refreshAll();
      setRefreshKey((k) => k + 1);
      if (previewDetached) emit("show-refreshed");
    } catch (e) {
      console.error("[VibeLights] Delete failed:", e);
    }
  }, [selectedEffects, playback, refreshAll, previewDetached]);

  const handleParamChange = useCallback(() => {
    setDirty(true);
    setRefreshKey((k) => k + 1);
  }, []);

  const handleSave = useCallback(async () => {
    if (saveState === "saving") return;
    setSaveState("saving");
    try {
      await invoke("save_current_sequence");
      setDirty(false);
      setSaveState("saved");
      clearTimeout(savedTimerRef.current);
      savedTimerRef.current = setTimeout(() => setSaveState("idle"), 1500);
    } catch (e) {
      console.error("[VibeLights] Save failed:", e);
      setSaveState("idle");
    }
  }, [saveState]);

  // Cleanup saved timer on unmount
  useEffect(() => () => clearTimeout(savedTimerRef.current), []);

  // ── Detachable preview ────────────────────────────────────────────

  const handleDetachPreview = useCallback(async () => {
    // If already detached, just focus the existing window
    if (previewWindowRef.current) {
      try {
        await previewWindowRef.current.setFocus();
      } catch {
        // Window may have been destroyed — fall through to create new one
        previewWindowRef.current = null;
      }
      if (previewWindowRef.current) return;
    }

    const previewWin = new WebviewWindow("preview", {
      url: "/?view=preview",
      title: "VibeLights Preview",
      width: 800,
      height: 600,
      decorations: false,
      center: true,
    });

    previewWindowRef.current = previewWin;
    setPreviewDetached(true);

    previewWin.onCloseRequested(() => {
      previewWindowRef.current = null;
      setPreviewDetached(false);
    });
  }, []);

  const handleFocusDetachedPreview = useCallback(async () => {
    if (previewWindowRef.current) {
      try {
        await previewWindowRef.current.setFocus();
      } catch {
        // Window gone
        previewWindowRef.current = null;
        setPreviewDetached(false);
      }
    }
  }, []);

  // Listen for reattach event from detached preview
  useEffect(() => {
    const unlisten = listen("preview-reattach", () => {
      previewWindowRef.current = null;
      setPreviewDetached(false);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Clean up detached preview on unmount
  useEffect(() => {
    return () => {
      if (previewWindowRef.current) {
        previewWindowRef.current.destroy().catch(() => {});
        previewWindowRef.current = null;
      }
    };
  }, []);

  const handleBack = useCallback(() => {
    if (dirty) {
      if (!confirm("You have unsaved changes. Leave without saving?")) return;
    }
    onBack();
  }, [dirty, onBack]);

  const handleRefresh = useCallback(() => {
    refreshAll();
    setRefreshKey((k) => k + 1);
    if (previewDetached) emit("show-refreshed");
  }, [refreshAll, previewDetached]);

  const handleSequenceSettingsSaved = useCallback(() => {
    setShowSequenceSettings(false);
    setDirty(true);
    refreshAll();
    setRefreshKey((k) => k + 1);
    if (previewDetached) emit("show-refreshed");
  }, [refreshAll, previewDetached]);

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

        setDirty(true);
        refreshAll();
        setRefreshKey((k) => k + 1);
        setSelectedEffects(new Set([`${trackIndex}-${effectIndex}`]));
        if (previewDetached) emit("show-refreshed");
      } catch (e) {
        console.error("[VibeLights] Add effect failed:", e);
      }
    },
    [addEffectState, show, playback, refreshAll, previewDetached],
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
        setDirty(true);
        refreshAll();
        setRefreshKey((k) => k + 1);
        if (previewDetached) emit("show-refreshed");
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

        setDirty(true);
        refreshAll();
        setRefreshKey((k) => k + 1);
        setSelectedEffects(new Set([`${toTrackIndex}-${newEffectIndex}`]));
        if (previewDetached) emit("show-refreshed");
      }
    },
    [show, playback, refreshAll, previewDetached],
  );

  const handleUndo = useCallback(async () => {
    await engineUndo();
    setDirty(true);
    setRefreshKey((k) => k + 1);
    if (previewDetached) emit("show-refreshed");
  }, [engineUndo, previewDetached]);

  const handleRedo = useCallback(async () => {
    await engineRedo();
    setDirty(true);
    setRefreshKey((k) => k + 1);
    if (previewDetached) emit("show-refreshed");
  }, [engineRedo, previewDetached]);

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
      onSave: handleSave,
      onUndo: handleUndo,
      onRedo: handleRedo,
      onSetModeSelect: () => setInteractionMode("select"),
      onSetModeEdit: () => setInteractionMode("edit"),
      onSetModeSwipe: () => setInteractionMode("swipe"),
    }),
    [handlePlayPause, handleStop, seek, playback?.duration, handleSelectAll, handleDeleteSelected, handleSave, handleUndo, handleRedo],
  );

  useKeyboard(keyboardActions);

  const singleSelected = selectedEffects.size === 1 ? [...selectedEffects][0] : null;

  if (loading) {
    const loadProgress = progressOps.get("open_sequence");
    const pct = loadProgress && loadProgress.progress >= 0 ? Math.round(loadProgress.progress * 100) : 0;
    const indeterminate = !loadProgress || loadProgress.progress < 0;
    return (
      <div className="bg-bg flex h-screen flex-col items-center justify-center gap-3">
        <div className="border-primary h-6 w-6 animate-spin rounded-full border-2 border-t-transparent" />
        <p className="text-text-2 text-sm">{loadProgress?.phase ?? "Loading sequence..."}</p>
        <div className="bg-border/30 h-1.5 w-48 overflow-hidden rounded-full">
          {indeterminate ? (
            <div className="bg-primary h-full w-1/3 animate-pulse rounded-full" />
          ) : (
            <div
              className="bg-primary h-full rounded-full transition-[width] duration-200"
              style={{ width: `${pct}%` }}
            />
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg text-text flex h-screen flex-col">
      {/* Title bar */}
      <AppBar />

      {/* Screen toolbar */}
      <div className="border-border bg-surface flex select-none items-center gap-1 border-b px-4 py-1.5">
        <button
          onClick={handleBack}
          className="text-text-2 hover:text-text mr-2 text-sm transition-colors"
        >
          &larr; Back
        </button>
        <span className="text-text-2 text-xs">{show?.name ?? "Untitled"}</span>
        <div className="flex-1" />
        <div className="flex items-center gap-1">
          <button
            onClick={() => setChatOpen((o) => !o)}
            className={`flex items-center gap-1 rounded border px-2 py-0.5 text-[11px] transition-colors ${
              chatOpen
                ? "border-primary/30 bg-primary/10 text-primary"
                : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
            }`}
          >
            <MessageSquare size={12} />
            Chat
          </button>
          <button
            onClick={() => setShowSequenceSettings(true)}
            className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text flex items-center gap-1 rounded border px-2 py-0.5 text-[11px] transition-colors"
          >
            <SlidersHorizontal size={12} />
            Sequence
          </button>
          <button
            onClick={handleSave}
            disabled={saveState === "saving" || (!dirty && saveState === "idle")}
            className={`rounded border px-2 py-0.5 text-[11px] transition-colors ${
              saveState === "saved"
                ? "border-green-500/30 bg-green-500/10 text-green-400"
                : dirty
                  ? "border-primary bg-primary text-white hover:bg-primary-hover"
                  : "border-border bg-surface text-text-2"
            } disabled:opacity-50`}
          >
            {saveState === "saving"
              ? "Saving..."
              : saveState === "saved"
                ? "Saved"
                : "Save"}
          </button>
          <button
            onClick={onOpenSettings}
            className="text-text-2 hover:text-text ml-1 p-1 transition-colors"
            title="Settings"
          >
            <Settings size={14} />
          </button>
        </div>
      </div>

      {/* Toolbar */}
      <Toolbar
        playback={playback}
        undoState={undoState}
        onPlay={play}
        onPause={pause}
        onStop={handleStop}
        onSkipBack={() => seek(0)}
        onSkipForward={() => seek(playback?.duration ?? 0)}
        onUndo={handleUndo}
        onRedo={handleRedo}
      />

      {/* Error banner */}
      {error && (
        <div className="bg-error/10 border-error/20 text-error border-b px-4 py-1.5 text-xs">
          Engine error: {error}
        </div>
      )}

      {/* Main area */}
      <div className="flex flex-1 overflow-hidden">
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
          waveform={audio.waveform}
          mode={interactionMode}
          onModeChange={setInteractionMode}
        />
        <PropertyPanel
          selectedEffect={singleSelected}
          sequenceIndex={playback?.sequence_index ?? 0}
          onParamChange={handleParamChange}
        />
        <ChatPanel
          open={chatOpen}
          onClose={() => setChatOpen(false)}
          onOpenSettings={onOpenSettings}
          onRefresh={handleRefresh}
        />
      </div>

      {/* Preview */}
      <Preview
        show={show}
        frame={frame}
        collapsed={previewCollapsed}
        onToggle={() => setPreviewCollapsed((c) => !c)}
        detached={previewDetached}
        onDetach={handleDetachPreview}
        onFocusDetached={handleFocusDetachedPreview}
      />

      {/* Effect Picker popover */}
      {addEffectState && (
        <EffectPicker
          position={addEffectState.screenPos}
          onSelect={handleEffectTypeSelected}
          onCancel={() => setAddEffectState(null)}
        />
      )}

      {/* Sequence Settings Dialog */}
      {showSequenceSettings && currentSequence && (
        <SequenceSettingsDialog
          sequence={currentSequence}
          sequenceIndex={playback?.sequence_index ?? 0}
          onSaved={handleSequenceSettingsSaved}
          onCancel={() => setShowSequenceSettings(false)}
        />
      )}
    </div>
  );
}
