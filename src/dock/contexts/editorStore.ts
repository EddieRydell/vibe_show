/**
 * Zustand store for editor state.
 *
 * Owns engine state, audio element, and analysis IPC directly.
 * The RAF animation loop (startAnimationLoop) ticks the engine and updates playback time.
 * A mutable `callbacks` object provides access to nav functions whose identity may change.
 */

import { createStore } from "zustand/vanilla";
import { convertFileSrc } from "@tauri-apps/api/core";
import { emitTo } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { cmd } from "../../commands";
import { SELECTION_CHANGED, SHOW_REFRESHED } from "../../events";
import type { KeyboardActions } from "../../hooks/useKeyboard";
import { deduplicateEffectKeys, makeEffectKey } from "../../utils/effectKey";
import { WAVEFORM_PEAKS, downsampleToPeaks } from "../../utils/waveform";
import type { TrackedOperation } from "../../hooks/useProgress";
import type {
  AnalysisFeatures,
  AudioAnalysis,
  EffectKind,
  InteractionMode,
  PlaybackInfo,
  PythonEnvStatus,
  Show,
  UndoState,
  WaveformData,
} from "../../types";

// ── Mutable callbacks ────────────────────────────────────────────
// Updated by the Provider on every render; never triggers re-renders.

export interface EditorCallbacks {
  onBack: () => void;
  onOpenScript: ((name: string | null) => void) | undefined;
}

// ── State shape ───────────────────────────────────────────────────

export interface AddEffectState {
  fixtureId: number;
  time: number;
  screenPos: { x: number; y: number };
}

export interface EditorState {
  // Engine (owned by store, fetched via IPC)
  show: Show | null;
  playback: PlaybackInfo | null;
  undoState: UndoState | null;
  error: string | null;
  sequenceIndex: number;

  // Audio (owned by store)
  audioReady: boolean;
  waveform: WaveformData | null;

  // Analysis (owned by store)
  analysis: AudioAnalysis | null;
  pythonStatus: PythonEnvStatus | null;

  // Selection
  selectedEffects: Set<string>;
  singleSelected: string | null;
  interactionMode: InteractionMode;

  // Edit
  dirty: boolean;
  saveState: "idle" | "saving" | "saved";
  refreshKey: number;

  // Effect add
  addEffectState: AddEffectState | null;

  // UI
  loading: boolean;
  libraryOpen: boolean;
  previewOpen: boolean;

  // Dialogs
  showSequenceSettings: boolean;
  showLeaveConfirm: boolean;
  showPythonSetup: boolean;
  showAnalysisWizard: boolean;

  // Derived
  currentSequence: Show["sequences"][number] | undefined;

  // Progress (synced from hook)
  progressOps: Map<string, TrackedOperation>;

  // Nav
  onOpenScript: ((name: string | null) => void) | undefined;

  // ── Actions ────────────────────────────────────────────────────

  // Transport
  play: () => void;
  pause: () => void;
  seek: (time: number) => void;
  handleStop: () => void;
  handlePlayPause: () => void;
  handlePauseInPlace: () => void;

  // Edit actions
  commitChange: (opts?: { skipRefreshAll?: boolean; skipDirty?: boolean }) => void;
  handleSave: () => Promise<void>;
  handleUndo: () => Promise<void>;
  handleRedo: () => Promise<void>;
  handleDeleteSelected: () => Promise<void>;
  handleSelectAll: () => void;
  handleParamChange: () => void;
  handleRegionChange: (region: [number, number] | null) => void;
  handleToggleLoop: () => void;
  handleAnalyze: () => Promise<void>;

  // Selection
  setSelectedEffects: (sel: Set<string>) => void;
  setInteractionMode: (mode: InteractionMode) => void;

  // Effect actions
  setAddEffectState: (state: AddEffectState | null) => void;
  handleAddEffect: (fixtureId: number, time: number, screenPos: { x: number; y: number }) => void;
  handleEffectTypeSelected: (kind: EffectKind) => Promise<void>;
  handleMoveEffect: (fromTrackIndex: number, effectIndex: number, targetFixtureId: number, newStart: number, newEnd: number) => Promise<void>;
  handleResizeEffect: (trackIndex: number, effectIndex: number, newStart: number, newEnd: number) => Promise<void>;

  // Audio
  loadAudio: (filename: string | null) => void;

  // Preview
  handleTogglePreview: () => Promise<void>;

  // Library
  setLibraryOpen: (open: boolean | ((prev: boolean) => boolean)) => void;

  // Dialogs
  setShowSequenceSettings: (show: boolean) => void;
  setShowLeaveConfirm: (show: boolean) => void;
  setShowPythonSetup: (show: boolean) => void;
  setShowAnalysisWizard: (show: boolean) => void;

  // Navigation
  onBack: () => void;
  handleBack: () => void;

  // Analysis (direct IPC)
  checkPython: () => Promise<PythonEnvStatus>;
  setupPython: () => Promise<void>;
  runAnalysis: (features?: AnalysisFeatures) => Promise<AudioAnalysis | null>;
  refreshAnalysis: () => Promise<AudioAnalysis | null>;
  refreshAll: () => void;
  startAnimationLoop: () => () => void;
  openSequence: (slug: string) => void;
  keyboardActions: KeyboardActions;
}

// ── Store factory ─────────────────────────────────────────────────

export function createEditorStore(
  showError: (error: unknown) => void,
  initialCallbacks: EditorCallbacks,
  refreshRef: { current: (() => void) | null },
) {
  // Mutable callbacks — updated every render by the provider
  let callbacks: EditorCallbacks = initialCallbacks;

  // Preview window ref lives outside React — managed by store actions
  const previewWindowRef: { current: WebviewWindow | null } = { current: null };
  // Play-from mark for play/pause toggle
  const playFromMarkRef: { current: number | null } = { current: null };
  // Saved-state timer
  let savedTimer: ReturnType<typeof setTimeout> | undefined;
  // RAF loop state
  let animFrameId = 0;
  let lastTimestamp = 0;

  // ── Closure-scoped audio element ────────────────────────────────
  let audioEl: HTMLAudioElement | null = null;
  let audioReady = false;

  function audioGetCurrentTime(): number | null {
    if (!audioEl || !audioReady || audioEl.paused) return null;
    return audioEl.currentTime;
  }

  function audioPlay() {
    audioEl?.play().catch(showError);
  }

  function audioPause() {
    audioEl?.pause();
  }

  function audioSeek(time: number) {
    if (audioEl) {
      audioEl.currentTime = time;
    }
  }

  const store = createStore<EditorState>((set, get) => ({
    // ── Initial state ───────────────────────────────────────────
    show: null,
    playback: null,
    undoState: null,
    error: null,
    sequenceIndex: 0,
    audioReady: false,
    waveform: null,
    analysis: null,
    pythonStatus: null,
    selectedEffects: new Set<string>(),
    singleSelected: null,
    interactionMode: "select" as InteractionMode,
    dirty: false,
    saveState: "idle" as const,
    refreshKey: 0,
    addEffectState: null,
    loading: true,
    libraryOpen: false,
    previewOpen: false,
    showSequenceSettings: false,
    showLeaveConfirm: false,
    showPythonSetup: false,
    showAnalysisWizard: false,
    currentSequence: undefined,
    progressOps: new Map(),
    onOpenScript: initialCallbacks.onOpenScript,

    // ── Transport ───────────────────────────────────────────────

    play: () => {
      set({ error: null });
      cmd.play()
        .then(() => cmd.getPlayback())
        .then((pb) => set({ playback: pb }))
        .catch((e: unknown) => { set({ error: String(e) }); console.error("[VibeLights] Play failed:", e); });
      if (audioReady) audioPlay();
    },

    pause: () => {
      set({ error: null });
      cmd.pause()
        .then(() => cmd.getPlayback())
        .then((pb) => set({ playback: pb }))
        .catch((e: unknown) => { set({ error: String(e) }); console.error("[VibeLights] Pause failed:", e); });
      if (audioReady) audioPause();
    },

    seek: (time: number) => {
      playFromMarkRef.current = null;
      set({ error: null });
      cmd.seek(time)
        .then(() => cmd.getPlayback())
        .then((pb) => set({ playback: pb }))
        .catch((e: unknown) => { set({ error: String(e) }); console.error("[VibeLights] Seek failed:", e); });
      if (audioReady) audioSeek(time);
    },

    handleStop: () => {
      get().pause();
      get().seek(0);
    },

    handlePlayPause: () => {
      const pb = get().playback;
      if (pb?.playing) {
        get().pause();
        if (playFromMarkRef.current != null) {
          get().seek(playFromMarkRef.current);
        }
      } else {
        playFromMarkRef.current = pb?.current_time ?? 0;
        if (pb?.region) {
          const [regionStart, regionEnd] = pb.region;
          const ct = pb.current_time;
          if (ct < regionStart || ct >= regionEnd) {
            get().seek(regionStart);
            playFromMarkRef.current = regionStart;
          }
        }
        get().play();
      }
    },

    handlePauseInPlace: () => {
      const pb = get().playback;
      if (pb?.playing) {
        get().pause();
        playFromMarkRef.current = null;
      } else {
        playFromMarkRef.current = null;
        get().play();
      }
    },

    // ── Edit actions ────────────────────────────────────────────

    commitChange: (opts) => {
      if (!opts?.skipDirty) set({ dirty: true });
      if (!opts?.skipRefreshAll) get().refreshAll();
      set((s) => ({ refreshKey: s.refreshKey + 1 }));
      if (get().previewOpen) void emitTo("preview", SHOW_REFRESHED);
    },

    handleSave: async () => {
      if (get().saveState === "saving") return;
      set({ saveState: "saving" });
      try {
        await cmd.saveCurrentSequence();
        set({ dirty: false, saveState: "saved" });
        clearTimeout(savedTimer);
        savedTimer = setTimeout(() => set({ saveState: "idle" }), 1500);
      } catch (e) {
        console.error("[VibeLights] Save failed:", e);
        set({ saveState: "idle" });
      }
    },

    handleUndo: async () => {
      set({ error: null });
      try {
        await cmd.undo();
        get().refreshAll();
        get().commitChange({ skipRefreshAll: true });
      } catch (e) {
        set({ error: String(e) });
        console.error("[VibeLights] Undo failed:", e);
      }
    },

    handleRedo: async () => {
      set({ error: null });
      try {
        await cmd.redo();
        get().refreshAll();
        get().commitChange({ skipRefreshAll: true });
      } catch (e) {
        set({ error: String(e) });
        console.error("[VibeLights] Redo failed:", e);
      }
    },

    handleDeleteSelected: async () => {
      const { selectedEffects, playback } = get();
      if (selectedEffects.size === 0 || !playback) return;
      const targets = deduplicateEffectKeys(selectedEffects);
      try {
        await cmd.deleteEffects(targets);
        set({ selectedEffects: new Set(), singleSelected: null });
        get().commitChange();
      } catch (e) {
        console.error("[VibeLights] Delete failed:", e);
      }
    },

    handleSelectAll: () => {
      const { show, playback } = get();
      if (!show || !playback) return;
      const sequence = show.sequences[playback.sequence_index];
      if (!sequence) return;
      const allKeys = new Set<string>();
      for (let tIdx = 0; tIdx < sequence.tracks.length; tIdx++) {
        for (let eIdx = 0; eIdx < sequence.tracks[tIdx]!.effects.length; eIdx++) {
          allKeys.add(makeEffectKey(tIdx, eIdx));
        }
      }
      set({
        selectedEffects: allKeys,
        singleSelected: allKeys.size === 1 ? [...allKeys][0]! : null,
      });
    },

    handleParamChange: () => {
      get().commitChange({ skipRefreshAll: true });
    },

    handleRegionChange: (region) => {
      cmd.setRegion(region)
        .then(() => set((s) => ({ playback: s.playback ? { ...s.playback, region } : s.playback })))
        .catch(showError);
    },

    handleToggleLoop: () => {
      const pb = get().playback;
      const looping = !(pb?.looping ?? false);
      cmd.setLooping(looping)
        .then(() => set((s) => ({ playback: s.playback ? { ...s.playback, looping } : s.playback })))
        .catch(showError);
    },

    handleAnalyze: async () => {
      const status = await get().checkPython();
      if (!status.deps_installed) {
        set({ showPythonSetup: true });
        return;
      }
      set({ showAnalysisWizard: true });
    },

    // ── Selection ───────────────────────────────────────────────

    setSelectedEffects: (sel) => {
      set({
        selectedEffects: sel,
        singleSelected: sel.size === 1 ? [...sel][0]! : null,
      });
    },

    setInteractionMode: (mode) => set({ interactionMode: mode }),

    // ── Effect actions (absorbed from useEffectActions) ──────────

    setAddEffectState: (state) => set({ addEffectState: state }),

    handleAddEffect: (fixtureId, time, screenPos) => {
      set({ addEffectState: { fixtureId, time, screenPos } });
    },

    handleEffectTypeSelected: async (kind) => {
      const { addEffectState, show, playback } = get();
      if (!addEffectState || !show || !playback) return;
      const { fixtureId, time } = addEffectState;
      const sequenceIndex = playback.sequence_index;
      const sequence = show.sequences[sequenceIndex];
      if (!sequence) return;

      set({ addEffectState: null });

      try {
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

        get().commitChange();
        get().setSelectedEffects(new Set([makeEffectKey(trackIndex, effectIndex)]));
      } catch (e) {
        console.error("[VibeLights] Add effect failed:", e);
      }
    },

    handleMoveEffect: async (fromTrackIndex, effectIndex, targetFixtureId, newStart, newEnd) => {
      const { show, playback } = get();
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
        get().commitChange();
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

        get().commitChange();
        get().setSelectedEffects(new Set([makeEffectKey(toTrackIndex, newEffectIndex)]));
      }
    },

    handleResizeEffect: async (trackIndex, effectIndex, newStart, newEnd) => {
      if (!get().playback) return;
      await cmd.updateEffectTimeRange(trackIndex, effectIndex, newStart, newEnd);
      get().commitChange();
    },

    // ── Audio ────────────────────────────────────────────────────

    loadAudio: (filename: string | null) => {
      // Clean up existing
      if (audioEl) { audioEl.pause(); audioEl.removeAttribute("src"); audioEl.load(); }
      audioEl = null;
      audioReady = false;
      set({ audioReady: false, waveform: null });
      if (!filename) return;

      cmd.resolveMediaPath(filename).then((absolutePath) => {
        const url = convertFileSrc(absolutePath);
        const el = new Audio();
        audioEl = el;
        el.addEventListener("loadedmetadata", () => { audioReady = true; set({ audioReady: true }); });
        el.addEventListener("ended", () => { get().pause(); });
        el.src = url;
        el.load();

        // Waveform extraction in parallel
        void fetch(url)
          .then((res) => res.arrayBuffer())
          .then((buffer) => {
            const ctx = new AudioContext();
            return ctx.decodeAudioData(buffer).then((decoded) => {
              void ctx.close();
              return decoded;
            });
          })
          .then((decoded) => {
            const peaks = downsampleToPeaks(decoded.getChannelData(0), WAVEFORM_PEAKS);
            set({ waveform: { peaks, duration: decoded.duration } });
          })
          .catch((err: unknown) => console.warn("[VibeLights] Waveform extraction failed:", err));
      }).catch((err: unknown) => console.error("[VibeLights] Failed to resolve media path:", err));
    },

    // ── Preview ─────────────────────────────────────────────────

    handleTogglePreview: async () => {
      if (previewWindowRef.current) {
        try {
          await previewWindowRef.current.destroy();
        } catch {
          // Already destroyed
        }
        previewWindowRef.current = null;
        set({ previewOpen: false });
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
      set({ previewOpen: true });

      void previewWin.onCloseRequested(() => {
        previewWindowRef.current = null;
        set({ previewOpen: false });
      });
    },

    // ── Library ─────────────────────────────────────────────────

    setLibraryOpen: (open) => {
      if (typeof open === "function") {
        set((s) => ({ libraryOpen: open(s.libraryOpen) }));
      } else {
        set({ libraryOpen: open });
      }
    },

    // ── Dialogs ─────────────────────────────────────────────────

    setShowSequenceSettings: (v) => set({ showSequenceSettings: v }),
    setShowLeaveConfirm: (v) => set({ showLeaveConfirm: v }),
    setShowPythonSetup: (v) => set({ showPythonSetup: v }),
    setShowAnalysisWizard: (v) => set({ showAnalysisWizard: v }),

    // ── Navigation ──────────────────────────────────────────────

    onBack: () => callbacks.onBack(),

    handleBack: () => {
      if (get().dirty) {
        set({ showLeaveConfirm: true });
        return;
      }
      callbacks.onBack();
    },

    // ── Analysis (direct IPC) ───────────────────────────────────

    checkPython: async () => {
      const status = await cmd.getPythonStatus();
      set({ pythonStatus: status });
      return status;
    },

    setupPython: async () => {
      await cmd.setupPythonEnv();
      const status = await cmd.getPythonStatus();
      set({ pythonStatus: status });
    },

    refreshAnalysis: async () => {
      const cached = await cmd.getAnalysis();
      set({ analysis: cached });
      return cached;
    },

    runAnalysis: async (features?: AnalysisFeatures) => {
      const result = await cmd.analyzeAudio(features);
      set({ analysis: result });
      return result;
    },

    refreshAll: () => {
      cmd.getShow().then((s) => set({ show: s, currentSequence: s.sequences[get().sequenceIndex] })).catch(showError);
      cmd.getPlayback().then((pb) => set({ playback: pb })).catch(showError);
      cmd.getUndoState().then((us) => set({ undoState: us })).catch(showError);
    },

    openSequence: (slug: string) => {
      cmd.openSequence(slug)
        .then(() => {
          get().refreshAll();
          set({ loading: false });
        })
        .catch((e: unknown) => {
          console.error("[VibeLights] Failed to open sequence:", e);
          set({ loading: false });
        });
    },

    keyboardActions: {
      onPlayPause: () => { get().handlePlayPause(); },
      onPauseInPlace: () => { get().handlePauseInPlace(); },
      onStop: () => { get().handleStop(); },
      onSeekStart: () => { get().seek(0); },
      onSeekEnd: () => { get().seek(get().playback?.duration ?? 0); },
      onZoomIn: () => {},
      onZoomOut: () => {},
      onZoomFit: () => {},
      onSelectAll: () => { get().handleSelectAll(); },
      onDeleteSelected: () => { void get().handleDeleteSelected(); },
      onToggleLoop: () => { get().handleToggleLoop(); },
      onSave: () => { void get().handleSave(); },
      onUndo: () => { void get().handleUndo(); },
      onRedo: () => { void get().handleRedo(); },
      onSetModeSelect: () => { get().setInteractionMode("select"); },
      onSetModeEdit: () => { get().setInteractionMode("edit"); },
      onSetModeSwipe: () => { get().setInteractionMode("swipe"); },
    },

    startAnimationLoop: () => {
      let cancelled = false;

      const loop = (timestamp: number) => {
        if (cancelled) return;
        const audioTime = audioGetCurrentTime();

        const scheduleNext = () => {
          if (!cancelled) {
            animFrameId = requestAnimationFrame(loop);
          }
        };

        if (audioTime != null) {
          // Audio-master mode: read time from audio element
          const pb = get().playback;
          const region = pb?.region ?? null;
          if (region) {
            const [regionStart, regionEnd] = region;
            if (audioTime >= regionEnd) {
              const looping = pb?.looping ?? false;
              if (looping) {
                audioSeek(regionStart);
                scheduleNext();
                return;
              } else {
                if (audioReady) audioPause();
                audioSeek(regionEnd);
                cmd.pause().catch((e: unknown) => console.warn("[VibeLights] Pause at region end failed:", e));
                set((s) => ({
                  playback: s.playback
                    ? { ...s.playback, current_time: regionEnd, playing: false }
                    : s.playback,
                }));
                scheduleNext();
                return;
              }
            }
          }
          // Update playback time from audio
          set((s) => ({
            playback: s.playback
              ? { ...s.playback, current_time: audioTime, playing: true }
              : s.playback,
          }));
          scheduleNext();
        } else if (!get().playback?.playing) {
          // Not playing and no audio — skip tick, just schedule next
          lastTimestamp = timestamp;
          scheduleNext();
        } else {
          // Engine-clock mode: tick the engine
          const dt = lastTimestamp ? (timestamp - lastTimestamp) / 1000.0 : 0;
          lastTimestamp = timestamp;

          cmd.tick(dt)
            .then((result) => {
              if (result) {
                set((s) => ({
                  playback: s.playback
                    ? { ...s.playback, current_time: result.current_time, playing: result.playing }
                    : s.playback,
                }));
              }
            })
            .catch((e: unknown) => console.warn("[VibeLights] Tick failed:", e))
            .finally(scheduleNext);
          return; // scheduleNext called in finally
        }
      };

      animFrameId = requestAnimationFrame(loop);
      return () => {
        cancelled = true;
        cancelAnimationFrame(animFrameId);
      };
    },
  }));

  // ── Subscribers ──────────────────────────────────────────────────

  // Watch currentSequence.audio_file → auto-load audio + analysis
  let prevAudioFile: string | null | undefined;
  const unsubAudio = store.subscribe((state) => {
    const audioFile = state.currentSequence?.audio_file ?? null;
    if (audioFile !== prevAudioFile) {
      prevAudioFile = audioFile;
      state.loadAudio(audioFile);
      if (audioFile) void state.refreshAnalysis();
    }
  });

  // Emit selection-changed to preview window
  let prevSelectedEffects: Set<string> | undefined;
  let prevPreviewOpen: boolean | undefined;
  const unsubPreview = store.subscribe((state) => {
    if (state.selectedEffects === prevSelectedEffects && state.previewOpen === prevPreviewOpen) return;
    prevSelectedEffects = state.selectedEffects;
    prevPreviewOpen = state.previewOpen;
    if (!state.previewOpen) return;
    const effects = deduplicateEffectKeys(state.selectedEffects);
    emitTo("preview", SELECTION_CHANGED, { effects }).catch(() => {
      store.setState({ previewOpen: false });
    });
  });

  // Register refresh handler for ChatPanel
  refreshRef.current = () => store.getState().commitChange({ skipDirty: true });

  // Cleanup function for unmount
  const cleanup = () => {
    clearTimeout(savedTimer);
    cancelAnimationFrame(animFrameId);
    unsubAudio();
    unsubPreview();
    refreshRef.current = null;
    if (audioEl) { audioEl.pause(); audioEl.removeAttribute("src"); audioEl.load(); audioEl = null; }
    if (previewWindowRef.current) {
      previewWindowRef.current.destroy().catch(() => {});
      previewWindowRef.current = null;
    }
  };

  const setCallbacks = (cb: EditorCallbacks) => {
    callbacks = cb;
    store.setState({ onOpenScript: cb.onOpenScript });
  };

  return { store, cleanup, setCallbacks };
}

export type EditorStore = ReturnType<typeof createEditorStore>["store"];
