import { useCallback, useEffect, useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { MediaFile, Sequence } from "../types";

interface Props {
  sequence: Sequence;
  sequenceIndex: number;
  onSaved: () => void;
  onCancel: () => void;
}

const FRAME_RATE_PRESETS = [20, 30, 40, 60];

export function SequenceSettingsDialog({ sequence, sequenceIndex, onSaved, onCancel }: Props) {
  const [name, setName] = useState(sequence.name);
  const [audioFile, setAudioFile] = useState<string | null>(sequence.audio_file);
  const [duration, setDuration] = useState(String(sequence.duration));
  const [frameRate, setFrameRate] = useState(String(sequence.frame_rate));
  const [mediaFiles, setMediaFiles] = useState<MediaFile[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    invoke<MediaFile[]>("list_media").then(setMediaFiles).catch(console.error);
  }, []);

  const handleMatchAudio = useCallback(() => {
    if (!audioFile) return;
    // Use a temporary Audio element to detect duration
    invoke<string>("resolve_media_path", { filename: audioFile })
      .then((path) => {
        const url = convertFileSrc(path);
        const tmp = new Audio(url);
        tmp.addEventListener("loadedmetadata", () => {
          if (tmp.duration && isFinite(tmp.duration)) {
            setDuration(tmp.duration.toFixed(2));
          }
        });
      })
      .catch(console.error);
  }, [audioFile]);

  const handleSave = useCallback(async () => {
    const dur = parseFloat(duration);
    const fr = parseFloat(frameRate);
    if (!name.trim() || isNaN(dur) || dur <= 0 || isNaN(fr) || fr <= 0) return;

    setSaving(true);
    try {
      await invoke("update_sequence_settings", {
        sequenceIndex,
        name: name.trim(),
        audioFile: audioFile !== sequence.audio_file ? audioFile : undefined,
        duration: dur !== sequence.duration ? dur : undefined,
        frameRate: fr !== sequence.frame_rate ? fr : undefined,
      });
      onSaved();
    } catch (e) {
      console.error("[VibeShow] Save sequence settings failed:", e);
    } finally {
      setSaving(false);
    }
  }, [name, audioFile, duration, frameRate, sequenceIndex, sequence, onSaved]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) handleSave();
    },
    [onCancel, handleSave],
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={handleKeyDown}
    >
      <div className="bg-surface border-border w-[440px] rounded-lg border shadow-xl">
        <div className="border-border border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">Sequence Settings</h3>
        </div>

        <div className="space-y-3 px-5 py-4">
          {/* Name */}
          <label className="block">
            <span className="text-text-2 mb-1 block text-xs">Name</span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
              className="border-border bg-surface-2 text-text focus:border-primary w-full rounded border px-3 py-1.5 text-sm outline-none"
            />
          </label>

          {/* Audio File */}
          <label className="block">
            <span className="text-text-2 mb-1 block text-xs">Audio File</span>
            <select
              value={audioFile ?? ""}
              onChange={(e) => setAudioFile(e.target.value || null)}
              className="border-border bg-surface-2 text-text focus:border-primary w-full rounded border px-2 py-1.5 text-sm outline-none"
            >
              <option value="">None</option>
              {mediaFiles.map((mf) => (
                <option key={mf.filename} value={mf.filename}>
                  {mf.filename}
                </option>
              ))}
            </select>
          </label>

          {/* Duration */}
          <div className="flex items-end gap-2">
            <label className="flex-1">
              <span className="text-text-2 mb-1 block text-xs">Duration (seconds)</span>
              <input
                type="number"
                value={duration}
                onChange={(e) => setDuration(e.target.value)}
                min={0.1}
                step={0.1}
                className="border-border bg-surface-2 text-text focus:border-primary w-full rounded border px-3 py-1.5 text-sm outline-none"
              />
            </label>
            {audioFile && (
              <button
                onClick={handleMatchAudio}
                className="border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text rounded border px-3 py-1.5 text-xs transition-colors"
              >
                Match Audio
              </button>
            )}
          </div>

          {/* Frame Rate */}
          <label className="block">
            <span className="text-text-2 mb-1 block text-xs">Frame Rate (fps)</span>
            <div className="flex items-center gap-2">
              <input
                type="number"
                value={frameRate}
                onChange={(e) => setFrameRate(e.target.value)}
                min={1}
                step={1}
                className="border-border bg-surface-2 text-text focus:border-primary w-24 rounded border px-3 py-1.5 text-sm outline-none"
              />
              <div className="flex gap-1">
                {FRAME_RATE_PRESETS.map((preset) => (
                  <button
                    key={preset}
                    onClick={() => setFrameRate(String(preset))}
                    className={`rounded border px-2 py-1 text-[11px] transition-colors ${
                      String(preset) === frameRate
                        ? "border-primary/30 bg-primary/10 text-primary"
                        : "border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text"
                    }`}
                  >
                    {preset}
                  </button>
                ))}
              </div>
            </div>
          </label>
        </div>

        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          <button
            onClick={onCancel}
            className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-4 py-1.5 text-xs transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving}
            className="bg-primary hover:bg-primary/90 rounded px-4 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-50"
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
