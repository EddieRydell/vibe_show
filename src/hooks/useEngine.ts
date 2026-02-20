import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Frame, PlaybackInfo, Show, TickResult, UndoState } from "../types";

export interface EngineState {
  show: Show | null;
  frame: Frame | null;
  playback: PlaybackInfo | null;
  undoState: UndoState | null;
  error: string | null;
  play: () => void;
  pause: () => void;
  seek: (time: number) => void;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  refreshAll: () => void;
}

export function useEngine(audioGetCurrentTime?: () => number | null): EngineState {
  const [show, setShow] = useState<Show | null>(null);
  const [frame, setFrame] = useState<Frame | null>(null);
  const [playback, setPlayback] = useState<PlaybackInfo | null>(null);
  const [undoState, setUndoState] = useState<UndoState | null>(null);
  const [error] = useState<string | null>(null);
  const animFrameRef = useRef<number>(0);
  const lastTimeRef = useRef<number>(0);
  const audioGetCurrentTimeRef = useRef(audioGetCurrentTime);
  audioGetCurrentTimeRef.current = audioGetCurrentTime;

  /** Refresh show + playback + frame + undo state from backend after a state change. */
  const refreshAll = useCallback(() => {
    invoke<Show>("get_show").then(setShow).catch(console.error);
    invoke<PlaybackInfo>("get_playback").then(setPlayback).catch(console.error);
    invoke<Frame>("get_frame", { time: 0.0 }).then(setFrame).catch(console.error);
    invoke<UndoState>("get_undo_state").then(setUndoState).catch(console.error);
  }, []);

  // Animation loop: tick the engine and receive frames.
  // Only schedules the next frame AFTER the current IPC completes to prevent
  // stacking concurrent invocations that cause jitter.
  useEffect(() => {
    let cancelled = false;

    const loop_ = (timestamp: number) => {
      if (cancelled) return;
      const audioTime = audioGetCurrentTimeRef.current?.();

      const scheduleNext = () => {
        if (!cancelled) {
          animFrameRef.current = requestAnimationFrame(loop_);
        }
      };

      if (audioTime != null) {
        // Audio-master mode: read time from audio element, get frame directly
        invoke<Frame>("get_frame", { time: audioTime })
          .then((f) => {
            setFrame(f);
            setPlayback((prev) =>
              prev ? { ...prev, current_time: audioTime, playing: true } : prev,
            );
          })
          .catch(() => {})
          .finally(scheduleNext);
      } else {
        // Existing tick mode
        const dt = lastTimeRef.current ? (timestamp - lastTimeRef.current) / 1000.0 : 0;
        lastTimeRef.current = timestamp;

        invoke<TickResult | null>("tick", { dt })
          .then((result) => {
            if (result) {
              setFrame(result.frame);
              setPlayback((prev) =>
                prev
                  ? { ...prev, current_time: result.current_time, playing: result.playing }
                  : prev,
              );
            }
          })
          .catch(() => {})
          .finally(scheduleNext);
      }
    };

    animFrameRef.current = requestAnimationFrame(loop_);
    return () => {
      cancelled = true;
      cancelAnimationFrame(animFrameRef.current);
    };
  }, []);

  const play = useCallback(() => {
    invoke("play")
      .then(() => invoke<PlaybackInfo>("get_playback").then(setPlayback))
      .catch((e) => console.error("[VibeLights] Play failed:", e));
  }, []);

  const pause = useCallback(() => {
    invoke("pause")
      .then(() => invoke<PlaybackInfo>("get_playback").then(setPlayback))
      .catch((e) => console.error("[VibeLights] Pause failed:", e));
  }, []);

  const seek = useCallback((time: number) => {
    invoke("seek", { time })
      .then(() => {
        invoke<Frame>("get_frame", { time }).then(setFrame);
        invoke<PlaybackInfo>("get_playback").then(setPlayback);
      })
      .catch((e) => console.error("[VibeLights] Seek failed:", e));
  }, []);

  const undo = useCallback(async () => {
    try {
      await invoke("undo");
      refreshAll();
    } catch (e) {
      console.error("[VibeLights] Undo failed:", e);
    }
  }, [refreshAll]);

  const redo = useCallback(async () => {
    try {
      await invoke("redo");
      refreshAll();
    } catch (e) {
      console.error("[VibeLights] Redo failed:", e);
    }
  }, [refreshAll]);

  return {
    show,
    frame,
    playback,
    undoState,
    error,
    play,
    pause,
    seek,
    undo,
    redo,
    refreshAll,
  };
}
