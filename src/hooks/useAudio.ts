import { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { cmd } from "../commands";
import { useToast } from "./useToast";

export interface WaveformData {
  peaks: Float32Array;
  duration: number;
}

export interface AudioState {
  ready: boolean;
  waveform: WaveformData | null;
  loadAudio: (filename: string | null) => void;
  play: () => void;
  pause: () => void;
  seek: (time: number) => void;
  getCurrentTime: () => number | null;
  onEnded: React.RefObject<(() => void) | null>;
}

const WAVEFORM_PEAKS = 2000;

function downsampleToPeaks(channelData: Float32Array, targetPeaks: number): Float32Array {
  const peaks = new Float32Array(targetPeaks);
  const samplesPerPeak = Math.floor(channelData.length / targetPeaks);
  if (samplesPerPeak === 0) {
    for (let i = 0; i < Math.min(channelData.length, targetPeaks); i++) {
      peaks[i] = Math.abs(channelData[i]!);
    }
    return peaks;
  }
  for (let i = 0; i < targetPeaks; i++) {
    let max = 0;
    const start = i * samplesPerPeak;
    const end = Math.min(start + samplesPerPeak, channelData.length);
    for (let j = start; j < end; j++) {
      const abs = Math.abs(channelData[j]!);
      if (abs > max) max = abs;
    }
    peaks[i] = max;
  }
  return peaks;
}

export function useAudio(): AudioState {
  const [ready, setReady] = useState(false);
  const [waveform, setWaveform] = useState<WaveformData | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const onEndedRef = useRef<(() => void) | null>(null);
  const { showError } = useToast();

  const cleanup = useCallback(() => {
    if (audioRef.current) {
      audioRef.current.pause();
      audioRef.current.removeAttribute("src");
      audioRef.current.load();
      audioRef.current = null;
    }
    setReady(false);
    setWaveform(null);
  }, []);

  useEffect(() => {
    return cleanup;
  }, [cleanup]);

  const loadAudio = useCallback(
    (filename: string | null) => {
      cleanup();

      if (!filename) return;

      cmd.resolveMediaPath(filename)
        .then((absolutePath) => {
          const url = convertFileSrc(absolutePath);

          // Create audio element for playback
          const audio = new Audio();
          audioRef.current = audio;

          audio.addEventListener("loadedmetadata", () => {
            setReady(true);
          });

          audio.addEventListener("ended", () => {
            onEndedRef.current?.();
          });

          audio.src = url;
          audio.load();

          // Decode audio for waveform in parallel
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
              const channel = decoded.getChannelData(0);
              const peaks = downsampleToPeaks(channel, WAVEFORM_PEAKS);
              setWaveform({ peaks, duration: decoded.duration });
            })
            .catch((err: unknown) => {
              console.warn("[VibeLights] Waveform extraction failed:", err);
            });
        })
        .catch((err: unknown) => {
          console.error("[VibeLights] Failed to resolve media path:", err);
        });
    },
    [cleanup],
  );

  const play = useCallback(() => {
    audioRef.current?.play().catch(showError);
  }, [showError]);

  const pause = useCallback(() => {
    audioRef.current?.pause();
  }, []);

  const seek = useCallback((time: number) => {
    if (audioRef.current) {
      audioRef.current.currentTime = time;
    }
  }, []);

  const getCurrentTime = useCallback((): number | null => {
    if (!audioRef.current || !ready || audioRef.current.paused) return null;
    return audioRef.current.currentTime;
  }, [ready]);

  return {
    ready,
    waveform,
    loadAudio,
    play,
    pause,
    seek,
    getCurrentTime,
    onEnded: onEndedRef,
  };
}
