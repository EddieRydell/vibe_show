import { memo, useCallback, useEffect, useRef } from "react";
import type { DockviewApi, DockviewReadyEvent, SerializedDockview } from "dockview-react";
import { Code, Layers, RotateCcw, SlidersHorizontal } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { Toolbar } from "../components/Toolbar";
import { EffectPicker } from "../components/EffectPicker";
import { SequenceSettingsDialog } from "../components/SequenceSettingsDialog";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { PythonSetupWizard } from "../components/PythonSetupWizard";
import { AnalysisWizard } from "../components/AnalysisWizard";
import { ScreenShell } from "../components/ScreenShell";
import { ViewMenu } from "../components/ViewMenu";
import { DockLayout, type DockLayoutHandle } from "../dock/DockLayout";
import { PANEL } from "../dock/panelIds";
import { getPanel, getAllPanels } from "../dock/registry";
import { registerEditorPanels } from "../dock/panels/registerEditorPanels";
import { applyDefaultEditorLayout } from "../dock/layouts/editorLayout";
import { loadLayout, clearLayout, createAutoSave } from "../dock/persistence";
import {
  EditorContextProvider,
  useEditorStore,
} from "../dock/contexts/EditorContext";

// Ensure panels are registered before first render
registerEditorPanels();

interface Props {
  setupSlug: string;
  sequenceSlug: string;
  onBack: () => void;
  onOpenScript?: (name: string | null) => void;
}

export function EditorScreen({ sequenceSlug, onBack, onOpenScript }: Props) {
  return (
    <EditorContextProvider
      sequenceSlug={sequenceSlug}
      onBack={onBack}
      onOpenScript={onOpenScript}
    >
      <EditorLayout />
    </EditorContextProvider>
  );
}

// ── Memoized DockLayout wrapper ──────────────────────────────────
// Prevents DockviewReact from re-rendering when EditorLayout re-renders.
// DockLayout's props (onReady, className) are stable, so memo is effective.
const MemoizedDockLayout = memo(DockLayout);

// ── Toolbar wrapper ──────────────────────────────────────────────
// Subscribes only to playback + the few fields it needs.
function EditorToolbar() {
  const playback = useEditorStore((s) => s.playback);
  const undoState = useEditorStore((s) => s.undoState);
  const previewOpen = useEditorStore((s) => s.previewOpen);
  const analysis = useEditorStore((s) => s.analysis);
  const play = useEditorStore((s) => s.play);
  const pause = useEditorStore((s) => s.pause);
  const handleStop = useEditorStore((s) => s.handleStop);
  const seek = useEditorStore((s) => s.seek);
  const handleUndo = useEditorStore((s) => s.handleUndo);
  const handleRedo = useEditorStore((s) => s.handleRedo);
  const handleTogglePreview = useEditorStore((s) => s.handleTogglePreview);
  const handleToggleLoop = useEditorStore((s) => s.handleToggleLoop);
  const handleAnalyze = useEditorStore((s) => s.handleAnalyze);

  return (
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
      onUndo={() => { void handleUndo(); }}
      onRedo={() => { void handleRedo(); }}
      onTogglePreview={() => { void handleTogglePreview(); }}
      onToggleLoop={handleToggleLoop}
      onAnalyze={() => { void handleAnalyze(); }}
    />
  );
}

function EditorLayout() {
  // Individual selectors — only re-render when specific state changes
  const show = useEditorStore((s) => s.show);
  const error = useEditorStore((s) => s.error);
  const loading = useEditorStore((s) => s.loading);
  const dirty = useEditorStore((s) => s.dirty);
  const saveState = useEditorStore((s) => s.saveState);
  const panelVisibility = useEditorStore((s) => s.panelVisibility);
  const addEffectState = useEditorStore((s) => s.addEffectState);
  const showSequenceSettings = useEditorStore((s) => s.showSequenceSettings);
  const showLeaveConfirm = useEditorStore((s) => s.showLeaveConfirm);
  const showPythonSetup = useEditorStore((s) => s.showPythonSetup);
  const showAnalysisWizard = useEditorStore((s) => s.showAnalysisWizard);
  const currentSequence = useEditorStore((s) => s.currentSequence);
  const sequenceIndex = useEditorStore((s) => s.sequenceIndex);
  const pythonStatus = useEditorStore((s) => s.pythonStatus);
  const onOpenScript = useEditorStore((s) => s.onOpenScript);
  const progressOps = useEditorStore((s) => s.progressOps);

  // Actions (stable references)
  const commitChange = useEditorStore((s) => s.commitChange);
  const handleSave = useEditorStore((s) => s.handleSave);
  const handleBack = useEditorStore((s) => s.handleBack);
  const onBack = useEditorStore((s) => s.onBack);
  const setPanelVisible = useEditorStore((s) => s.setPanelVisible);
  const setShowSequenceSettings = useEditorStore((s) => s.setShowSequenceSettings);
  const setShowLeaveConfirm = useEditorStore((s) => s.setShowLeaveConfirm);
  const setShowPythonSetup = useEditorStore((s) => s.setShowPythonSetup);
  const setShowAnalysisWizard = useEditorStore((s) => s.setShowAnalysisWizard);
  const setAddEffectState = useEditorStore((s) => s.setAddEffectState);
  const handleEffectTypeSelected = useEditorStore((s) => s.handleEffectTypeSelected);
  const setupPython = useEditorStore((s) => s.setupPython);
  const checkPython = useEditorStore((s) => s.checkPython);
  const runAnalysis = useEditorStore((s) => s.runAnalysis);

  const dockRef = useRef<DockLayoutHandle>(null);
  const dockApiRef = useRef<DockviewApi | null>(null);

  const autoSaveCleanupRef = useRef<(() => void) | null>(null);

  const handleDockReady = useCallback((event: DockviewReadyEvent) => {
    dockApiRef.current = event.api;

    // Try restoring saved layout; fall back to default
    const saved = loadLayout("editor");
    if (saved) {
      try {
        event.api.fromJSON(saved as unknown as SerializedDockview);
      } catch {
        // Corrupted layout — fall back to default
        event.api.clear();
        applyDefaultEditorLayout(event.api);
      }
    } else {
      applyDefaultEditorLayout(event.api);
    }

    // Sync panelVisibility with dockview's actual panel state
    for (const panel of getAllPanels()) {
      setPanelVisible(panel.id, event.api.panels.some((p) => p.id === panel.id));
    }
    event.api.onDidAddPanel((e) => setPanelVisible(e.id, true));
    event.api.onDidRemovePanel((e) => setPanelVisible(e.id, false));

    // Set up auto-save
    autoSaveCleanupRef.current = createAutoSave("editor", event.api);
  }, [setPanelVisible]);

  // Cleanup auto-save on unmount
  useEffect(() => {
    return () => autoSaveCleanupRef.current?.();
  }, []);

  const handleSequenceSettingsSaved = useCallback(() => {
    setShowSequenceSettings(false);
    commitChange();
  }, [setShowSequenceSettings, commitChange]);

  const handleResetLayout = useCallback(() => {
    const api = dockApiRef.current;
    if (!api) return;
    clearLayout("editor");
    api.clear();
    applyDefaultEditorLayout(api);
  }, []);

  const handleTogglePanel = useCallback((panelId: string) => {
    const api = dockApiRef.current;
    if (!api) return;

    const existing = api.panels.find((p) => p.id === panelId);
    if (existing) {
      existing.api.close();
    } else {
      const def = getPanel(panelId);
      if (!def) return;
      if (def.defaultPosition === "right") {
        // Place below an existing right-side panel (not "within" — that tabs/stacks them)
        const ref = api.panels.find(
          (p) => p.id !== panelId && (p.id === PANEL.PROPERTY || p.id === PANEL.LIBRARY),
        );
        api.addPanel({
          id: panelId,
          component: panelId,
          title: def.title,
          position: ref
            ? { referencePanel: ref, direction: "below" }
            : { direction: "right" },
        });
      } else {
        api.addPanel({
          id: panelId,
          component: panelId,
          title: def.title,
          position: { direction: "below" },
        });
      }
    }
  }, []);

  // Re-add panel when its popout window is closed
  useEffect(() => {
    const promise = listen<{ panelId: string }>("popout-closed", (event) => {
      const api = dockApiRef.current;
      if (!api) return;
      const { panelId } = event.payload;
      const alreadyExists = api.panels.some((p) => p.id === panelId);
      if (!alreadyExists) handleTogglePanel(panelId);
    });
    return () => { void promise.then((unlisten) => unlisten()); };
  }, [handleTogglePanel]);

  if (loading) {
    const loadTracked = progressOps.get("open_sequence");
    const loadProgress = loadTracked?.event;
    const pct = loadProgress && loadProgress.progress >= 0 ? Math.round(loadProgress.progress * 100) : 0;
    const indeterminate = !loadProgress || loadProgress.progress < 0;
    return (
      <div className="bg-bg flex h-full flex-col items-center justify-center gap-3">
        <div className="border-primary size-6 animate-spin rounded-full border-2 border-t-transparent" />
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
      <div className="flex items-center gap-1">
        <ViewMenu onTogglePanel={handleTogglePanel} />
        <button
          onClick={handleResetLayout}
          className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text flex items-center gap-1 rounded border px-2 py-0.5 text-[11px] transition-colors"
          title="Reset panel layout"
        >
          <RotateCcw size={12} />
          Reset Layout
        </button>
      </div>
      <div className="flex-1" />
      <div className="flex items-center gap-1">
        <button
          onClick={() => handleTogglePanel(PANEL.LIBRARY)}
          className={`flex items-center gap-1 rounded border px-2 py-0.5 text-[11px] transition-colors ${
            panelVisibility[PANEL.LIBRARY]
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
          onClick={() => { void handleSave(); }}
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
            title="Effect Studio"
          >
            <Code size={14} />
          </button>
        )}
      </div>
    </div>
  );

  return (
    <ScreenShell
      title={show?.name ?? "Untitled"}
      onBack={handleBack}
      toolbar={screenToolbar}
    >
      {/* Transport Toolbar — subscribes to playback independently */}
      <EditorToolbar />

      {/* Error banner */}
      {error && (
        <div className="bg-error/10 border-error/20 text-error border-b px-4 py-1.5 text-xs">
          Engine error: {error}
        </div>
      )}

      {/* Dockview main area — memoized to prevent re-rendering on parent updates */}
      <MemoizedDockLayout
        ref={dockRef}
        onReady={handleDockReady}
        className="flex-1"
      />

      {/* Dialogs — portaled overlays, outside dockview */}
      {addEffectState && (
        <EffectPicker
          position={addEffectState.screenPos}
          onSelect={(kind) => { void handleEffectTypeSelected(kind); }}
          onCancel={() => setAddEffectState(null)}
        />
      )}

      {showSequenceSettings && currentSequence && (
        <SequenceSettingsDialog
          sequence={currentSequence}
          sequenceIndex={sequenceIndex}
          onSaved={handleSequenceSettingsSaved}
          onCancel={() => setShowSequenceSettings(false)}
        />
      )}

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

      {showPythonSetup && (
        <PythonSetupWizard
          pythonStatus={pythonStatus}
          onSetup={setupPython}
          onCheckStatus={checkPython}
          onClose={() => setShowPythonSetup(false)}
        />
      )}

      {showAnalysisWizard && (
        <AnalysisWizard
          onAnalyze={runAnalysis as (features: Parameters<typeof runAnalysis>[0]) => Promise<NonNullable<Awaited<ReturnType<typeof runAnalysis>>>>}
          onClose={() => setShowAnalysisWizard(false)}
        />
      )}
    </ScreenShell>
  );
}
