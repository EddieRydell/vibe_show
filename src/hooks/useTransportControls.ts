import { useCallback, useRef } from "react";
import type { PlaybackInfo } from "../types";

interface AudioControls {
  ready: boolean;
  play: () => void;
  pause: () => void;
  seek: (time: number) => void;
}

export interface TransportControlsState {
  play: () => void;
  pause: () => void;
  seek: (time: number) => void;
  handleStop: () => void;
  handlePlayPause: () => void;
  handlePauseInPlace: () => void;
}

export function useTransportControls(
  playback: PlaybackInfo | null,
  enginePlay: () => void,
  enginePause: () => void,
  engineSeek: (time: number) => void,
  audio: AudioControls,
): TransportControlsState {
  const playFromMarkRef = useRef<number | null>(null);

  const play = useCallback(() => {
    enginePlay();
    if (audio.ready) audio.play();
  }, [enginePlay, audio]);

  const pause = useCallback(() => {
    enginePause();
    if (audio.ready) audio.pause();
  }, [enginePause, audio]);

  const seek = useCallback(
    (time: number) => {
      playFromMarkRef.current = null;
      engineSeek(time);
      if (audio.ready) audio.seek(time);
    },
    [engineSeek, audio],
  );

  const handleStop = useCallback(() => {
    pause();
    seek(0);
  }, [pause, seek]);

  const handlePlayPause = useCallback(() => {
    if (playback?.playing) {
      pause();
      if (playFromMarkRef.current != null) {
        engineSeek(playFromMarkRef.current);
        if (audio.ready) audio.seek(playFromMarkRef.current);
      }
    } else {
      playFromMarkRef.current = playback?.current_time ?? 0;
      if (playback?.region) {
        const [regionStart, regionEnd] = playback.region;
        const ct = playback.current_time ?? 0;
        if (ct < regionStart || ct >= regionEnd) {
          engineSeek(regionStart);
          if (audio.ready) audio.seek(regionStart);
          playFromMarkRef.current = regionStart;
        }
      }
      play();
    }
  }, [playback?.playing, playback?.current_time, playback?.region, play, pause, engineSeek, audio]);

  const handlePauseInPlace = useCallback(() => {
    if (playback?.playing) {
      pause();
      playFromMarkRef.current = null;
    } else {
      playFromMarkRef.current = null;
      play();
    }
  }, [playback?.playing, play, pause]);

  return { play, pause, seek, handleStop, handlePlayPause, handlePauseInPlace };
}
