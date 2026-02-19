import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Preview } from "../components/Preview";
import { Timeline } from "../components/Timeline";
import { Toolbar } from "../components/Toolbar";
import { FixtureList } from "../components/FixtureList";
import { PropertyPanel } from "../components/PropertyPanel";
import { useEngine } from "../hooks/useEngine";
import { useKeyboard } from "../hooks/useKeyboard";

interface Props {
  profileSlug: string;
  showSlug: string;
  onBack: () => void;
}

export function EditorScreen({ showSlug, onBack }: Props) {
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

  const handleDeleteSelected = useCallback(() => {
    console.log("[VibeShow] Delete requested for:", [...selectedEffects]);
    setSelectedEffects(new Set());
  }, [selectedEffects]);

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
    </div>
  );
}
