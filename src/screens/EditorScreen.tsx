import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emitTo } from "@tauri-apps/api/event";
import { cmd } from "../commands";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Code, Layers, SlidersHorizontal } from "lucide-react";
import { Timeline } from "../components/Timeline";
import { Toolbar } from "../components/Toolbar";
import { PropertyPanel } from "../components/PropertyPanel";
import { EffectPicker } from "../components/EffectPicker";
import { SequenceSettingsDialog } from "../components/SequenceSettingsDialog";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { LibraryPanel } from "../components/LibraryPanel";
import { PythonSetupWizard } from "../components/PythonSetupWizard";
import { AnalysisWizard } from "../components/AnalysisWizard";
import { ScreenShell, useAppShell } from "../components/ScreenShell";
import { useEngine } from "../hooks/useEngine";
import { useAudio } from "../hooks/useAudio";
import { useAnalysis } from "../hooks/useAnalysis";
import { useKeyboard } from "../hooks/useKeyboard";
import { useProgress } from "../hooks/useProgress";
import { ShowVersionContext } from "../hooks/useShowVersion";
import { deduplicateEffectKeys, makeEffectKey } from "../utils/effectKey";
import type { EffectKind, InteractionMode } from "../types";

interface Props {
  profileSlug: string;
  sequenceSlug: string;
  onBack: () => void;
  onOpenScript?: (name: string | null) => void;
}

export function EditorScreen({ sequenceSlug, onBack, onOpenScript }: Props) {
  const { refreshRef } = useAppShell();
  const progressOps = useProgress();
  const audio = useAudio();
  const {
    analysis,
    pythonStatus,
    checkPython,
    setupPython,
    runAnalysis,
    refreshAnalysis,
  } = useAnalysis();
  const [showPythonSetup, setShowPythonSetup] = useState(false);
  const [showAnalysisWizard, setShowAnalysisWizard] = useState(false);

  // Audio-master clock: returns audio time when audio is actively playing, null otherwise.
  // When null, useEngine falls back to its tick(dt) mode.
  const audioGetCurrentTime = useCallback((): number | null => {
    return audio.getCurrentTime();
  }, [audio]);

  const audioSeekCb = useCallback((t: number) => { if (audio.ready) audio.seek(t); }, [audio]);
  const audioPauseCb = useCallback(() => { if (audio.ready) audio.pause(); }, [audio]);

  const {
    show,
    playback,
    undoState,
    error,
    play: enginePlay,
    pause: enginePause,
    seek: engineSeek,
    setRegion: engineSetRegion,
    setLooping: engineSetLooping,
    undo: engineUndo,
    redo: engineRedo,
    refreshAll,
  } = useEngine(audioGetCurrentTime, audioSeekCb, audioPauseCb);
  const [previewOpen, setPreviewOpen] = useState(false);
  const previewWindowRef = useRef<WebviewWindow | null>(null);
  const [libraryOpen, setLibraryOpen] = useState(false);
  const [selectedEffects, setSelectedEffects] = useState<Set<string>>(new Set());
  const [refreshKey, setRefreshKey] = useState(0);
  const [loading, setLoading] = useState(true);
  const [showSequenceSettings, setShowSequenceSettings] = useState(false);
  const [showLeaveConfirm, setShowLeaveConfirm] = useState(false);
  const [addEffectState, setAddEffectState] = useState<{
    fixtureId: number;
    time: number;
    screenPos: { x: number; y: number };
  } | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">("idle");
  const [interactionMode, setInteractionMode] = useState<InteractionMode>("select");
  const savedTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const playFromMarkRef = useRef<number | null>(null);

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

  // ── Emit selection-changed to preview window ────────────────────────

  useEffect(() => {
    if (!previewOpen) return;
    const effects = deduplicateEffectKeys(selectedEffects);
    emitTo("preview", "selection-changed", { effects }).catch(() => {});
  }, [selectedEffects, previewOpen]);

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
      playFromMarkRef.current = null;
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
    if (playback?.playing) {
      // Pause and return to the play-from mark
      pause();
      if (playFromMarkRef.current != null) {
        engineSeek(playFromMarkRef.current);
        if (audio.ready) audio.seek(playFromMarkRef.current);
      }
    } else {
      // Remember current position as the mark, then play
      playFromMarkRef.current = playback?.current_time ?? 0;
      // If region is set and current time is outside, seek to region start
      if (playback?.region) {
        const [regionStart, regionEnd] = playback.region;
        const ct = playback.current_time ?? 0;
        if (ct < regionStart || ct >= regionEnd) {
          engineSeek(regionStart);
          if (audio.ready) audio.seek(regionStart);
          playFromMarkRef.current = regionStart;
        }
      }
      play();
    }
  }, [playback?.playing, playback?.current_time, playback?.region, play, pause, engineSeek, audio]);

  const handlePauseInPlace = useCallback(() => {
    if (playback?.playing) {
      // Pause where you are — don't return to mark
      pause();
      playFromMarkRef.current = null;
    } else {
      // Resume from current position — clear mark so next Space starts fresh
      playFromMarkRef.current = null;
      play();
    }
  }, [playback?.playing, play, pause]);

  const handleSelectAll = useCallback(() => {
    if (!show || !playback) return;
    const sequence = show.sequences[playback.sequence_index];
    if (!sequence) return;
    const allKeys = new Set<string>();
    for (let tIdx = 0; tIdx < sequence.tracks.length; tIdx++) {
      for (let eIdx = 0; eIdx < sequence.tracks[tIdx].effects.length; eIdx++) {
        allKeys.add(makeEffectKey(tIdx, eIdx));
      }
    }
    setSelectedEffects(allKeys);
  }, [show, playback]);

  /** Mark the show dirty, refresh state from backend, bump thumbnail keys, and notify preview. */
  const commitChange = useCallback(
    (opts?: { skipRefreshAll?: boolean; skipDirty?: boolean }) => {
      if (!opts?.skipDirty) setDirty(true);
      if (!opts?.skipRefreshAll) refreshAll();
      setRefreshKey((k) => k + 1);
      if (previewOpen) emitTo("preview", "show-refreshed");
    },
    [refreshAll, previewOpen],
  );

  // Register refresh handler for ChatPanel via context
  const handleRefresh = useCallback(() => {
    commitChange({ skipDirty: true });
  }, [commitChange]);

  useEffect(() => {
    refreshRef.current = handleRefresh;
    return () => { refreshRef.current = null; };
  }, [handleRefresh, refreshRef]);

  const handleDeleteSelected = useCallback(async () => {
    if (selectedEffects.size === 0 || !playback) return;
    const targets = deduplicateEffectKeys(selectedEffects);
    try {
      await cmd.deleteEffects(targets);
      setSelectedEffects(new Set());
      commitChange();
    } catch (e) {
      console.error("[VibeLights] Delete failed:", e);
    }
  }, [selectedEffects, playback, commitChange]);

  const handleParamChange = useCallback(() => {
    commitChange({ skipRefreshAll: true });
  }, [commitChange]);

  const handleSave = useCallback(async () => {
    if (saveState === "saving") return;
    setSaveState("saving");
    try {
      await cmd.saveCurrentSequence();
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

  // ── Preview window toggle ──────────────────────────────────────────

  const handleTogglePreview = useCallback(async () => {
    if (previewWindowRef.current) {
      try {
        await previewWindowRef.current.destroy();
      } catch {
        // Already destroyed
      }
      previewWindowRef.current = null;
      setPreviewOpen(false);
      return;
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
    setPreviewOpen(true);

    previewWin.onCloseRequested(() => {
      previewWindowRef.current = null;
      setPreviewOpen(false);
    });
  }, []);

  // Clean up preview window on unmount
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
      setShowLeaveConfirm(true);
      return;
    }
    onBack();
  }, [dirty, onBack]);

  const handleSequenceSettingsSaved = useCallback(() => {
    setShowSequenceSettings(false);
    commitChange();
  }, [commitChange]);

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
    [addEffectState, show, playback, commitChange],
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
        await cmd.updateEffectTimeRange(fromTrackIndex, effectIndex, newStart, newEnd);
        commitChange();
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
          toTrackIndex = await cmd.addTrack(trackName, targetFixtureId);
        }

        const newEffectIndex = await cmd.moveEffectToTrack(fromTrackIndex, effectIndex, toTrackIndex);

        await cmd.updateEffectTimeRange(toTrackIndex, newEffectIndex, newStart, newEnd);

        commitChange();
        setSelectedEffects(new Set([makeEffectKey(toTrackIndex, newEffectIndex)]));
      }
    },
    [show, playback, commitChange],
  );

  const handleUndo = useCallback(async () => {
    await engineUndo();
    commitChange({ skipRefreshAll: true });
  }, [engineUndo, commitChange]);

  const handleRedo = useCallback(async () => {
    await engineRedo();
    commitChange({ skipRefreshAll: true });
  }, [engineRedo, commitChange]);

  const handleResizeEffect = useCallback(
    async (trackIndex: number, effectIndex: number, newStart: number, newEnd: number) => {
      if (!playback) return;
      await cmd.updateEffectTimeRange(trackIndex, effectIndex, newStart, newEnd);
      commitChange();
    },
    [playback, commitChange],
  );

  const handleRegionChange = useCallback(
    (region: [number, number] | null) => {
      engineSetRegion(region);
    },
    [engineSetRegion],
  );

  const handleToggleLoop = useCallback(() => {
    engineSetLooping(!(playback?.looping ?? false));
  }, [engineSetLooping, playback?.looping]);

  const handleAnalyze = useCallback(async () => {
    // Check if Python is ready; if not, show setup wizard
    const status = await checkPython();
    if (!status.deps_installed) {
      setShowPythonSetup(true);
      return;
    }
    // If analysis already exists, show it; otherwise open analysis wizard
    if (analysis) {
      // Could toggle overlays here; for now, re-open wizard
      setShowAnalysisWizard(true);
    } else {
      setShowAnalysisWizard(true);
    }
  }, [checkPython, analysis]);

  // Load cached analysis when sequence is opened
  useEffect(() => {
    if (!loading && currentSequence?.audio_file) {
      refreshAnalysis();
    }
  }, [loading, currentSequence?.audio_file, refreshAnalysis]);

  const keyboardActions = useMemo(
    () => ({
      onPlayPause: handlePlayPause,
      onPauseInPlace: handlePauseInPlace,
      onStop: handleStop,
      onSeekStart: () => seek(0),
      onSeekEnd: () => seek(playback?.duration ?? 0),
      onZoomIn: () => {},
      onZoomOut: () => {},
      onZoomFit: () => {},
      onSelectAll: handleSelectAll,
      onDeleteSelected: handleDeleteSelected,
      onToggleLoop: handleToggleLoop,
      onSave: handleSave,
      onUndo: handleUndo,
      onRedo: handleRedo,
      onSetModeSelect: () => setInteractionMode("select"),
      onSetModeEdit: () => setInteractionMode("edit"),
      onSetModeSwipe: () => setInteractionMode("swipe"),
    }),
    [handlePlayPause, handlePauseInPlace, handleStop, seek, playback?.duration, handleSelectAll, handleDeleteSelected, handleToggleLoop, handleSave, handleUndo, handleRedo],
  );

  useKeyboard(keyboardActions);

  const singleSelected = selectedEffects.size === 1 ? [...selectedEffects][0] : null;

  if (loading) {
    const loadTracked = progressOps.get("open_sequence");
    const loadProgress = loadTracked?.event;
    const pct = loadProgress && loadProgress.progress >= 0 ? Math.round(loadProgress.progress * 100) : 0;
    const indeterminate = !loadProgress || loadProgress.progress < 0;
    return (
      <div className="bg-bg flex h-full flex-col items-center justify-center gap-3">
        <div className="border-primary size-6  animate-spin rounded-full border-2 border-t-transparent" />
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

  const screenToolbar = (
    <div className="border-border bg-surface flex select-none items-center gap-1 border-b px-4 py-1.5">
      <div className="flex-1" />
      <div className="flex items-center gap-1">
        <button
          onClick={() => setLibraryOpen((o) => !o)}
          className={`flex items-center gap-1 rounded border px-2 py-0.5 text-[11px] transition-colors ${
            libraryOpen
              ? "border-primary/30 bg-primary/10 text-primary"
              : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
          }`}
        >
          <Layers size={12} />
          Library
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
        {onOpenScript && (
          <button
            onClick={() => onOpenScript(null)}
            className="text-text-2 hover:text-text ml-1 p-1 transition-colors"
            title="Script Studio"
          >
            <Code size={14} />
          </button>
        )}
      </div>
    </div>
  );

  return (
    <ShowVersionContext.Provider value={refreshKey}>
    <ScreenShell
      title={show?.name ?? "Untitled"}
      onBack={handleBack}
      toolbar={screenToolbar}
    >
      {/* Toolbar */}
      <Toolbar
        playback={playback}
        undoState={undoState}
        previewOpen={previewOpen}
        looping={playback?.looping ?? false}
        hasAnalysis={analysis != null}
        onPlay={play}
        onPause={pause}
        onStop={handleStop}
        onSkipBack={() => seek(0)}
        onSkipForward={() => seek(playback?.duration ?? 0)}
        onUndo={handleUndo}
        onRedo={handleRedo}
        onTogglePreview={handleTogglePreview}
        onToggleLoop={handleToggleLoop}
        onAnalyze={handleAnalyze}
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
          onResizeEffect={handleResizeEffect}
          waveform={audio.waveform}
          mode={interactionMode}
          onModeChange={setInteractionMode}
          region={playback?.region ?? null}
          onRegionChange={handleRegionChange}
          analysis={analysis}
        />
        {libraryOpen && (
          <LibraryPanel
            onClose={() => setLibraryOpen(false)}
            onLibraryChange={() => commitChange()}
          />
        )}
        <PropertyPanel
          selectedEffect={singleSelected}
          sequenceIndex={playback?.sequence_index ?? 0}
          onParamChange={handleParamChange}
        />
      </div>

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

      {/* Unsaved changes confirmation */}
      {showLeaveConfirm && (
        <ConfirmDialog
          title="Unsaved changes"
          message="You have unsaved changes. Leave without saving?"
          confirmLabel="Leave"
          destructive
          onConfirm={() => { setShowLeaveConfirm(false); onBack(); }}
          onCancel={() => setShowLeaveConfirm(false)}
        />
      )}

      {/* Python Setup Wizard */}
      {showPythonSetup && (
        <PythonSetupWizard
          pythonStatus={pythonStatus}
          onSetup={setupPython}
          onCheckStatus={checkPython}
          onClose={() => setShowPythonSetup(false)}
        />
      )}

      {/* Analysis Wizard */}
      {showAnalysisWizard && (
        <AnalysisWizard
          onAnalyze={runAnalysis}
          onClose={() => setShowAnalysisWizard(false)}
        />
      )}
    </ScreenShell>
    </ShowVersionContext.Provider>
  );
}
