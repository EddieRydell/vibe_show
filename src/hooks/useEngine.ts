import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Frame, PlaybackInfo, Show, TickResult } from "../types";

export interface EngineState {
  show: Show | null;
  frame: Frame | null;
  playback: PlaybackInfo | null;
  error: string | null;
  play: () => void;
  pause: () => void;
  seek: (time: number) => void;
  selectSequence: (index: number) => void;
  refreshAll: () => void;
}

export function useEngine(): EngineState {
  const [show, setShow] = useState<Show | null>(null);
  const [frame, setFrame] = useState<Frame | null>(null);
  const [playback, setPlayback] = useState<PlaybackInfo | null>(null);
  const [error] = useState<string | null>(null);
  const animFrameRef = useRef<number>(0);
  const lastTimeRef = useRef<number>(0);

  /** Refresh show + playback + frame from backend after a state change. */
  const refreshAll = useCallback(() => {
    invoke<Show>("get_show").then(setShow).catch(console.error);
    invoke<PlaybackInfo>("get_playback").then(setPlayback).catch(console.error);
    invoke<Frame>("get_frame", { time: 0.0 }).then(setFrame).catch(console.error);
  }, []);

  // Animation loop: tick the engine and receive frames.
  useEffect(() => {
    const loop_ = (timestamp: number) => {
      const dt = lastTimeRef.current ? (timestamp - lastTimeRef.current) / 1000.0 : 0;
      lastTimeRef.current = timestamp;

      invoke<TickResult | null>("tick", { dt })
        .then((result) => {
          if (result) {
            setFrame(result.frame);
            setPlayback((prev) =>
              prev ? { ...prev, current_time: result.current_time, playing: true } : prev,
            );
          }
        })
        .catch(() => {});

      animFrameRef.current = requestAnimationFrame(loop_);
    };

    animFrameRef.current = requestAnimationFrame(loop_);
    return () => cancelAnimationFrame(animFrameRef.current);
  }, []);

  const play = useCallback(() => {
    invoke("play")
      .then(() => invoke<PlaybackInfo>("get_playback").then(setPlayback))
      .catch((e) => console.error("[VibeShow] Play failed:", e));
  }, []);

  const pause = useCallback(() => {
    invoke("pause")
      .then(() => invoke<PlaybackInfo>("get_playback").then(setPlayback))
      .catch((e) => console.error("[VibeShow] Pause failed:", e));
  }, []);

  const seek = useCallback((time: number) => {
    invoke("seek", { time })
      .then(() => {
        invoke<Frame>("get_frame", { time }).then(setFrame);
        invoke<PlaybackInfo>("get_playback").then(setPlayback);
      })
      .catch((e) => console.error("[VibeShow] Seek failed:", e));
  }, []);

  const selectSequence = useCallback((index: number) => {
    invoke<PlaybackInfo | null>("select_sequence", { index })
      .then((p) => {
        if (p) {
          setPlayback(p);
          invoke<Frame>("get_frame", { time: 0.0 }).then(setFrame).catch(console.error);
        }
      })
      .catch((e) => console.error("[VibeShow] Select sequence failed:", e));
  }, []);

  return {
    show,
    frame,
    playback,
    error,
    play,
    pause,
    seek,
    selectSequence,
    refreshAll,
  };
}
