import { memo } from "react";
import { Timeline } from "../../components/Timeline";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { useEditorStore } from "../contexts/EditorContext";

export const DockableTimeline = memo(function DockableTimeline() {
  const show = useEditorStore((s) => s.show);
  const playback = useEditorStore((s) => s.playback);
  const selectedEffects = useEditorStore((s) => s.selectedEffects);
  const refreshKey = useEditorStore((s) => s.refreshKey);
  const waveform = useEditorStore((s) => s.waveform);
  const mode = useEditorStore((s) => s.interactionMode);
  const analysis = useEditorStore((s) => s.analysis);

  // Actions — stable references, created once in the store
  const seek = useEditorStore((s) => s.seek);
  const setSelectedEffects = useEditorStore((s) => s.setSelectedEffects);
  const commitChange = useEditorStore((s) => s.commitChange);
  const handleAddEffect = useEditorStore((s) => s.handleAddEffect);
  const handleMoveEffect = useEditorStore((s) => s.handleMoveEffect);
  const handleResizeEffect = useEditorStore((s) => s.handleResizeEffect);
  const setInteractionMode = useEditorStore((s) => s.setInteractionMode);
  const handleRegionChange = useEditorStore((s) => s.handleRegionChange);

  return (
    <ErrorBoundary>
      <Timeline
        show={show}
        playback={playback}
        onSeek={seek}
        selectedEffects={selectedEffects}
        onSelectionChange={setSelectedEffects}
        refreshKey={refreshKey}
        onAddEffect={handleAddEffect}
        onRefresh={() => commitChange({ skipDirty: true })}
        onMoveEffect={handleMoveEffect}
        onResizeEffect={handleResizeEffect}
        waveform={waveform}
        mode={mode}
        onModeChange={setInteractionMode}
        region={playback?.region ?? null}
        onRegionChange={handleRegionChange}
        analysis={analysis}
      />
    </ErrorBoundary>
  );
}, () => true);
