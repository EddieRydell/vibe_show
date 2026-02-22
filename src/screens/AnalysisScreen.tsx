import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AnalysisFeatures, AudioAnalysis, PythonEnvStatus } from "../types";
import { cmd } from "../commands";
import { formatTimeTransport } from "../utils/formatTime";
import { ScreenShell } from "../components/ScreenShell";
import { PythonSetupWizard } from "../components/PythonSetupWizard";
import { AnalysisWizard } from "../components/AnalysisWizard";
import { AnalysisWorkspace } from "../components/AnalysisWorkspace";
import type { ManualBeat } from "../components/AnalysisWorkspace";
import { useAudio } from "../hooks/useAudio";

interface Props {
  profileSlug: string;
  filename: string;
  onBack: () => void;
}

export function AnalysisScreen({ filename, onBack }: Props) {
  const audio = useAudio();
  const [pythonStatus, setPythonStatus] = useState<PythonEnvStatus | null>(null);
  const [showPythonSetup, setShowPythonSetup] = useState(false);
  const [showAnalysisWizard, setShowAnalysisWizard] = useState(false);
  const [analysis, setAnalysis] = useState<AudioAnalysis | null>(null);

  // Playback state
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const currentTimeRef = useRef(0);
  const rafRef = useRef<number>(0);

  // Manual beats
  const [manualBeats, setManualBeats] = useState<ManualBeat[]>([]);
  const [selectedBeatId, setSelectedBeatId] = useState<string | null>(null);

  // ── Audio loading ─────────────────────────────────────────────────

  useEffect(() => {
    audio.loadAudio(filename);
  }, [filename, audio.loadAudio]);

  // ── Python check on mount + load cached analysis ──────────────────

  const checkPython = useCallback(async () => {
    const status = await invoke<PythonEnvStatus>("get_python_status");
    setPythonStatus(status);
    return status;
  }, []);

  const setupPython = useCallback(async () => {
    await invoke("setup_python_env");
    await checkPython();
  }, [checkPython]);

  useEffect(() => {
    (async () => {
      const status = await checkPython();
      if (!status.deps_installed) {
        setShowPythonSetup(true);
      }
      try {
        const cached = await cmd.getAnalysis();
        if (cached) setAnalysis(cached);
      } catch {
        // No cached analysis
      }
    })();
  }, [checkPython]);

  // ── rAF playback loop ─────────────────────────────────────────────

  useEffect(() => {
    if (!playing) return;

    function tick() {
      const t = audio.getCurrentTime();
      if (t != null) {
        currentTimeRef.current = t;
        setCurrentTime(t);
      }
      rafRef.current = requestAnimationFrame(tick);
    }
    rafRef.current = requestAnimationFrame(tick);

    return () => cancelAnimationFrame(rafRef.current);
  }, [playing, audio]);

  // ── Transport controls ────────────────────────────────────────────

  const handlePlay = useCallback(() => {
    audio.play();
    setPlaying(true);
  }, [audio]);

  const handlePause = useCallback(() => {
    audio.pause();
    setPlaying(false);
    // Snap to current audio time
    const t = audio.getCurrentTime();
    if (t != null) {
      currentTimeRef.current = t;
      setCurrentTime(t);
    }
  }, [audio]);

  const handleStop = useCallback(() => {
    audio.pause();
    audio.seek(0);
    setPlaying(false);
    currentTimeRef.current = 0;
    setCurrentTime(0);
  }, [audio]);

  const handleSeek = useCallback(
    (time: number) => {
      audio.seek(time);
      currentTimeRef.current = time;
      setCurrentTime(time);
    },
    [audio],
  );

  // Handle audio ended
  useEffect(() => {
    audio.onEnded.current = () => {
      setPlaying(false);
    };
  }, [audio.onEnded]);

  // ── Analysis wizard ───────────────────────────────────────────────

  const handleAnalyze = useCallback(
    async (features: AnalysisFeatures): Promise<AudioAnalysis> => {
      const result = await invoke<AudioAnalysis>("analyze_audio", {
        features,
      });
      setAnalysis(result);
      return result;
    },
    [],
  );

  // ── Beat operations ───────────────────────────────────────────────

  const handleAddBeat = useCallback((time: number) => {
    const beat: ManualBeat = { id: crypto.randomUUID(), time };
    setManualBeats((prev) => [...prev, beat].sort((a, b) => a.time - b.time));
    setSelectedBeatId(beat.id);
  }, []);

  const handleMoveBeat = useCallback((id: string, newTime: number) => {
    setManualBeats((prev) =>
      prev.map((b) => (b.id === id ? { ...b, time: newTime } : b)).sort((a, b) => a.time - b.time),
    );
  }, []);

  const handleDeleteBeat = useCallback(
    (id: string) => {
      setManualBeats((prev) => prev.filter((b) => b.id !== id));
      if (selectedBeatId === id) setSelectedBeatId(null);
    },
    [selectedBeatId],
  );

  // ── Keyboard shortcuts ────────────────────────────────────────────

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Don't handle if in an input or the analysis wizard is open
      if ((e.target as HTMLElement).closest("input, textarea, select")) return;
      if (showAnalysisWizard) return;

      if (e.key === " ") {
        e.preventDefault();
        if (playing) handlePause();
        else handlePlay();
      } else if (e.key === "b" || e.key === "B") {
        e.preventDefault();
        handleAddBeat(currentTimeRef.current);
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [playing, handlePlay, handlePause, handleAddBeat, showAnalysisWizard]);

  const duration = audio.waveform?.duration ?? 0;

  const toolbar = (
    <div className="border-border bg-surface flex select-none items-center gap-2 border-b px-3 py-1.5">
      {/* Transport controls */}
      <ToolBtn onClick={handleStop} title="Stop">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
          <rect x="2" y="2" width="12" height="12" rx="1" />
        </svg>
      </ToolBtn>
      {playing ? (
        <ToolBtn onClick={handlePause} active title="Pause (Space)">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
            <path d="M3 1h4v14H3zM9 1h4v14H9z" />
          </svg>
        </ToolBtn>
      ) : (
        <ToolBtn onClick={handlePlay} disabled={!audio.ready} title="Play (Space)">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
            <path d="M4 2l10 6-10 6V2z" />
          </svg>
        </ToolBtn>
      )}

      {/* Time display */}
      <div className="border-border bg-bg text-text min-w-32 rounded border px-2 py-0.5 text-center font-mono text-xs">
        {formatTimeTransport(currentTime)}
        <span className="text-text-2 mx-1">/</span>
        {formatTimeTransport(duration)}
      </div>

      {/* Divider */}
      <div className="border-border mx-1 h-5 border-l" />

      {/* Tap beat */}
      <ToolBtn onClick={() => handleAddBeat(currentTimeRef.current)} title="Tap beat (B)">
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        >
          <path d="M8 2v12M4 6v6M12 4v8" />
        </svg>
      </ToolBtn>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Analysis status */}
      {analysis && (
        <span className="text-text-2 text-[10px]">
          {analysis.beats ? `${analysis.beats.beats.length} AI beats` : "Analyzed"}
        </span>
      )}

      {/* Analyze button */}
      <ToolBtn
        onClick={() => setShowAnalysisWizard(true)}
        active={analysis != null}
        title="Analyze audio (AI)"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M2 12l3-4 2 2 3-5 4 7" />
          <path d="M2 14h12" />
        </svg>
      </ToolBtn>
    </div>
  );

  return (
    <ScreenShell title={filename} onBack={onBack} toolbar={toolbar}>
      {/* Workspace */}
      {audio.ready ? (
        <AnalysisWorkspace
          duration={duration}
          currentTime={currentTime}
          waveform={audio.waveform}
          analysis={analysis}
          manualBeats={manualBeats}
          selectedBeatId={selectedBeatId}
          onSeek={handleSeek}
          onAddBeat={handleAddBeat}
          onMoveBeat={handleMoveBeat}
          onSelectBeat={setSelectedBeatId}
          onDeleteBeat={handleDeleteBeat}
        />
      ) : (
        <div className="flex flex-1 items-center justify-center">
          <p className="text-text-2 text-sm">Loading audio...</p>
        </div>
      )}

      {/* Analysis wizard modal */}
      {showAnalysisWizard && (
        <AnalysisWizard onAnalyze={handleAnalyze} onClose={() => setShowAnalysisWizard(false)} />
      )}

      {/* Python setup wizard */}
      {showPythonSetup && (
        <PythonSetupWizard
          pythonStatus={pythonStatus}
          onSetup={setupPython}
          onCheckStatus={checkPython}
          onClose={() => setShowPythonSetup(false)}
        />
      )}
    </ScreenShell>
  );
}

function ToolBtn({
  children,
  onClick,
  active = false,
  disabled = false,
  title,
}: {
  children: React.ReactNode;
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={title}
      className={`rounded border px-3 py-1.5 text-xs font-semibold transition-colors duration-100 ${
        active
          ? "border-primary/30 bg-primary/10 text-primary hover:bg-primary/15"
          : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
      } disabled:cursor-not-allowed disabled:opacity-30`}
    >
      {children}
    </button>
  );
}
