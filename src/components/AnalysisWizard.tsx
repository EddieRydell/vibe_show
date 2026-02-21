import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import type { AnalysisFeatures, AudioAnalysis, ProgressEvent } from "../types";

interface AnalysisWizardProps {
  onAnalyze: (features: AnalysisFeatures) => Promise<AudioAnalysis>;
  onClose: () => void;
}

const FEATURE_LABELS: { key: keyof AnalysisFeatures; label: string; description: string }[] = [
  { key: "beats", label: "Beats & Tempo", description: "Beat positions, downbeats, BPM" },
  { key: "structure", label: "Song Structure", description: "Verse, chorus, bridge sections" },
  { key: "stems", label: "Source Separation", description: "Separate vocals, drums, bass, other" },
  { key: "lyrics", label: "Lyrics", description: "Word-level transcription with timestamps" },
  { key: "mood", label: "Mood & Energy", description: "Valence, arousal, danceability" },
  { key: "harmony", label: "Key & Chords", description: "Musical key and chord progression" },
  { key: "low_level", label: "Audio Features", description: "RMS energy, spectral centroid, chromagram" },
  { key: "pitch", label: "Pitch Detection", description: "Polyphonic note detection (MIDI)" },
  { key: "drums", label: "Drum Onsets", description: "Drum hit times and strengths" },
  { key: "vocal_presence", label: "Vocal Presence", description: "Regions where vocals are active" },
];

export function AnalysisWizard({ onAnalyze, onClose }: AnalysisWizardProps) {
  const [phase, setPhase] = useState<"select" | "running" | "done" | "error">("select");
  const [features, setFeatures] = useState<AnalysisFeatures>({
    beats: true,
    structure: true,
    stems: true,
    lyrics: true,
    mood: true,
    harmony: true,
    low_level: true,
    pitch: true,
    drums: true,
    vocal_presence: true,
  });
  const [progress, setProgress] = useState(0);
  const [progressMessage, setProgressMessage] = useState("");
  const [result, setResult] = useState<AudioAnalysis | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    const unlisten = listen<ProgressEvent>("progress", (event) => {
      if (event.payload.operation === "analysis") {
        setProgress(event.payload.progress);
        setProgressMessage(event.payload.phase);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const toggleFeature = useCallback((key: keyof AnalysisFeatures) => {
    setFeatures((prev) => ({ ...prev, [key]: !prev[key] }));
  }, []);

  const selectAll = useCallback(() => {
    setFeatures({
      beats: true,
      structure: true,
      stems: true,
      lyrics: true,
      mood: true,
      harmony: true,
      low_level: true,
      pitch: true,
      drums: true,
      vocal_presence: true,
    });
  }, []);

  const selectNone = useCallback(() => {
    setFeatures({
      beats: false,
      structure: false,
      stems: false,
      lyrics: false,
      mood: false,
      harmony: false,
      low_level: false,
      pitch: false,
      drums: false,
      vocal_presence: false,
    });
  }, []);

  const handleRun = useCallback(async () => {
    setPhase("running");
    setProgress(0);
    setProgressMessage("Starting analysis...");
    try {
      const analysisResult = await onAnalyze(features);
      setResult(analysisResult);
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }, [features, onAnalyze]);

  const anySelected = Object.values(features).some(Boolean);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape" && phase !== "running") onClose();
    },
    [onClose, phase],
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={handleKeyDown}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && phase !== "running") onClose();
      }}
    >
      <div className="bg-surface border-border w-[520px] rounded-lg border shadow-xl">
        <div className="border-border border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">Analyze Audio</h3>
        </div>

        <div className="px-5 py-4">
          {phase === "select" && (
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <p className="text-text-2 text-sm">Select features to analyze:</p>
                <div className="flex gap-2">
                  <button
                    onClick={selectAll}
                    className="text-primary text-xs hover:underline"
                  >
                    All
                  </button>
                  <button
                    onClick={selectNone}
                    className="text-primary text-xs hover:underline"
                  >
                    None
                  </button>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-2">
                {FEATURE_LABELS.map(({ key, label, description }) => (
                  <label
                    key={key}
                    className="border-border hover:bg-surface-2 flex cursor-pointer items-start gap-2 rounded border p-2"
                  >
                    <input
                      type="checkbox"
                      checked={features[key]}
                      onChange={() => toggleFeature(key)}
                      className="mt-0.5"
                    />
                    <div>
                      <div className="text-text text-xs font-medium">
                        {label}
                      </div>
                      <div className="text-text-2 text-[10px]">
                        {description}
                      </div>
                    </div>
                  </label>
                ))}
              </div>
              <p className="text-text-2 text-[10px]">
                Full analysis takes ~5-8 min on CPU, ~1 min with GPU.
                Source separation (stems) is the slowest step.
              </p>
            </div>
          )}

          {phase === "running" && (
            <div className="space-y-3">
              <p className="text-text-2 text-sm">{progressMessage}</p>
              <div className="bg-bg border-border h-2 overflow-hidden rounded-full border">
                <div
                  className="bg-primary h-full transition-all duration-500"
                  style={{ width: `${Math.round(progress * 100)}%` }}
                />
              </div>
              <p className="text-text-2 text-xs">
                {Math.round(progress * 100)}% — This may take several minutes
              </p>
            </div>
          )}

          {phase === "done" && (
            <div className="space-y-2">
              <p className="text-sm text-green-500">Analysis complete</p>
              <div className="text-text-2 space-y-1 text-xs">
                {result?.beats && (
                  <p>
                    Beats: {result.beats.beats.length} beats at{" "}
                    {result.beats.tempo.toFixed(1)} BPM
                  </p>
                )}
                {result?.structure && (
                  <p>
                    Structure: {result.structure.sections.length} sections
                  </p>
                )}
                {result?.stems && <p>Stems: 4 tracks separated</p>}
                {result?.lyrics && (
                  <p>Lyrics: {result.lyrics.words.length} words transcribed</p>
                )}
                {result?.harmony && <p>Key: {result.harmony.key}</p>}
              </div>
            </div>
          )}

          {phase === "error" && (
            <div className="space-y-2">
              <p className="text-sm text-red-500">Analysis failed</p>
              <p className="text-text-2 break-all text-xs">{error}</p>
            </div>
          )}
        </div>

        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          {phase === "select" && (
            <>
              <button
                onClick={onClose}
                className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-3 py-1.5 text-xs transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleRun}
                disabled={!anySelected}
                className="border-primary bg-primary hover:bg-primary-hover rounded border px-3 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-40"
              >
                Analyze
              </button>
            </>
          )}
          {(phase === "done" || phase === "error") && (
            <button
              onClick={onClose}
              className="border-primary bg-primary hover:bg-primary-hover rounded border px-3 py-1.5 text-xs font-medium text-white transition-colors"
            >
              Close
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
