import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cmd } from "../commands";
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
  setRegion: (region: [number, number] | null) => void;
  setLooping: (looping: boolean) => void;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  refreshAll: () => void;
}

export function useEngine(
  audioGetCurrentTime?: () => number | null,
  audioSeek?: (time: number) => void,
  audioPause?: () => void,
): EngineState {
  const [show, setShow] = useState<Show | null>(null);
  const [frame, setFrame] = useState<Frame | null>(null);
  const [playback, setPlayback] = useState<PlaybackInfo | null>(null);
  const [undoState, setUndoState] = useState<UndoState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const animFrameRef = useRef<number>(0);
  const lastTimeRef = useRef<number>(0);
  const playbackRef = useRef<PlaybackInfo | null>(null);
  const playingRef = useRef(false);
  const audioGetCurrentTimeRef = useRef(audioGetCurrentTime);
  audioGetCurrentTimeRef.current = audioGetCurrentTime;
  const audioSeekRef = useRef(audioSeek);
  audioSeekRef.current = audioSeek;
  const audioPauseRef = useRef(audioPause);
  audioPauseRef.current = audioPause;

  /** Refresh show + playback + frame + undo state from backend after a state change. */
  const refreshAll = useCallback(() => {
    cmd.getShow().then(setShow).catch(console.error);
    cmd
      .getPlayback()
      .then((pb) => {
        setPlayback(pb);
        playbackRef.current = pb;
        playingRef.current = pb.playing;
      })
      .catch(console.error);
    const time = playbackRef.current?.current_time ?? 0.0;
    invoke<Frame>("get_frame", { time }).then(setFrame).catch(console.error);
    cmd.getUndoState().then(setUndoState).catch(console.error);
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
        // Enforce region boundaries on audio playback
        const region = playbackRef.current?.region ?? null;
        if (region) {
          const [, regionEnd] = region;
          if (audioTime >= regionEnd) {
            const looping = playbackRef.current?.looping ?? false;
            if (looping) {
              audioSeekRef.current?.(region[0]);
              scheduleNext();
              return;
            } else {
              audioPauseRef.current?.();
              audioSeekRef.current?.(regionEnd);
              cmd.pause().catch(() => {});
              setPlayback((prev) => {
                const updated = prev ? { ...prev, current_time: regionEnd, playing: false } : prev;
                playbackRef.current = updated;
                playingRef.current = false;
                return updated;
              });
              scheduleNext();
              return;
            }
          }
        }
        invoke<Frame>("get_frame", { time: audioTime })
          .then((f) => {
            setFrame(f);
            setPlayback((prev) => {
              const updated = prev ? { ...prev, current_time: audioTime, playing: true } : prev;
              playbackRef.current = updated;
              playingRef.current = true;
              return updated;
            });
          })
          .catch(() => {})
          .finally(scheduleNext);
      } else if (!playingRef.current) {
        // Not playing and no audio — skip tick, just schedule next
        lastTimeRef.current = timestamp;
        scheduleNext();
      } else {
        // Existing tick mode
        const dt = lastTimeRef.current ? (timestamp - lastTimeRef.current) / 1000.0 : 0;
        lastTimeRef.current = timestamp;

        invoke<TickResult | null>("tick", { dt })
          .then((result) => {
            if (result) {
              setFrame(result.frame);
              setPlayback((prev) => {
                const updated = prev
                  ? { ...prev, current_time: result.current_time, playing: result.playing }
                  : prev;
                playbackRef.current = updated;
                playingRef.current = result.playing;
                return updated;
              });
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
    setError(null);
    cmd
      .play()
      .then(() =>
        cmd.getPlayback().then((pb) => {
          setPlayback(pb);
          playbackRef.current = pb;
          playingRef.current = pb.playing;
        }),
      )
      .catch((e) => {
        const msg = String(e);
        setError(msg);
        console.error("[VibeLights] Play failed:", e);
      });
  }, []);

  const pause = useCallback(() => {
    setError(null);
    cmd
      .pause()
      .then(() =>
        cmd.getPlayback().then((pb) => {
          setPlayback(pb);
          playbackRef.current = pb;
          playingRef.current = pb.playing;
        }),
      )
      .catch((e) => {
        const msg = String(e);
        setError(msg);
        console.error("[VibeLights] Pause failed:", e);
      });
  }, []);

  const seek = useCallback((time: number) => {
    setError(null);
    cmd
      .seek(time)
      .then(() => {
        invoke<Frame>("get_frame", { time }).then(setFrame);
        cmd.getPlayback().then((pb) => {
          setPlayback(pb);
          playbackRef.current = pb;
          playingRef.current = pb.playing;
        });
      })
      .catch((e) => {
        const msg = String(e);
        setError(msg);
        console.error("[VibeLights] Seek failed:", e);
      });
  }, []);

  const setRegion = useCallback((region: [number, number] | null) => {
    cmd.setRegion(region).catch(console.error);
    setPlayback((prev) => (prev ? { ...prev, region } : prev));
  }, []);

  const setLooping = useCallback((looping: boolean) => {
    cmd.setLooping(looping).catch(console.error);
    setPlayback((prev) => (prev ? { ...prev, looping } : prev));
  }, []);

  const undo = useCallback(async () => {
    setError(null);
    try {
      await cmd.undo();
      refreshAll();
    } catch (e) {
      const msg = String(e);
      setError(msg);
      console.error("[VibeLights] Undo failed:", e);
    }
  }, [refreshAll]);

  const redo = useCallback(async () => {
    setError(null);
    try {
      await cmd.redo();
      refreshAll();
    } catch (e) {
      const msg = String(e);
      setError(msg);
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
    setRegion,
    setLooping,
    undo,
    redo,
    refreshAll,
  };
}
