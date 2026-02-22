import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { EffectParams, ScriptPreviewData } from "../types";

interface UseScriptPreviewOptions {
  scriptName: string | null;
  compiled: boolean;
  params: EffectParams;
  pixelCount: number;
  timeSamples?: number;
}

export function useScriptPreview({
  scriptName,
  compiled,
  params,
  pixelCount,
  timeSamples = 100,
}: UseScriptPreviewOptions) {
  const [heatmap, setHeatmap] = useState<ScriptPreviewData | null>(null);
  const [strip, setStrip] = useState<Array<[number, number, number, number]> | null>(null);
  const [currentTime, setCurrentTime] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [duration, setDuration] = useState(2);

  const rafRef = useRef<number>(0);
  const startRef = useRef<number>(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Fetch full heatmap when script compiles successfully
  useEffect(() => {
    if (!scriptName || !compiled) {
      setHeatmap(null);
      return;
    }
    invoke<ScriptPreviewData | null>("preview_script", {
      name: scriptName,
      params,
      pixelCount,
      timeSamples,
    })
      .then(setHeatmap)
      .catch(console.error);
  }, [scriptName, compiled, params, pixelCount, timeSamples]);

  // Fetch single frame for strip (debounced)
  const fetchStrip = useCallback(
    (t: number) => {
      if (!scriptName || !compiled) return;
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        invoke<Array<[number, number, number, number]> | null>("preview_script_frame", {
          name: scriptName,
          params,
          pixelCount,
          t,
        })
          .then(setStrip)
          .catch(console.error);
      }, 16);
    },
    [scriptName, compiled, params, pixelCount],
  );

  // Fetch strip when time changes
  useEffect(() => {
    fetchStrip(currentTime);
  }, [currentTime, fetchStrip]);

  // Auto-play animation loop
  useEffect(() => {
    if (!playing) return;
    startRef.current = performance.now() - currentTime * duration * 1000;

    function tick() {
      const elapsed = (performance.now() - startRef.current) / 1000;
      const t = (elapsed / duration) % 1;
      setCurrentTime(t);
      rafRef.current = requestAnimationFrame(tick);
    }
    rafRef.current = requestAnimationFrame(tick);

    return () => cancelAnimationFrame(rafRef.current);
  }, [playing, duration]);

  const togglePlay = useCallback(() => setPlaying((p) => !p), []);

  const scrub = useCallback(
    (t: number) => {
      setCurrentTime(t);
      if (playing) {
        startRef.current = performance.now() - t * duration * 1000;
      }
    },
    [playing, duration],
  );

  return {
    heatmap,
    strip,
    currentTime,
    playing,
    duration,
    setDuration,
    togglePlay,
    scrub,
    setPlaying,
  };
}
