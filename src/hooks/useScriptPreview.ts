import { useCallback, useEffect, useRef, useState } from "react";
import { cmd } from "../commands";
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

  // Fetch full heatmap when script compiles successfully
  useEffect(() => {
    if (!scriptName || !compiled) {
      setHeatmap(null);
      return;
    }
    cmd.previewScript(scriptName, params, pixelCount, timeSamples)
      .then(setHeatmap)
      .catch(console.error);
  }, [scriptName, compiled, params, pixelCount, timeSamples]);

  // Derive strip from heatmap data (no IPC needed — instant)
  useEffect(() => {
    if (!heatmap) {
      setStrip(null);
      return;
    }
    const { width, height, pixels } = heatmap;
    const col = Math.round(currentTime * (width - 1));
    const frame: Array<[number, number, number, number]> = [];
    for (let row = 0; row < height; row++) {
      const idx = (row * width + col) * 4;
      frame.push([pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]]);
    }
    setStrip(frame);
  }, [heatmap, currentTime]);

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
