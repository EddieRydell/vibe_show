import type { PlaybackInfo } from "../types";

interface ToolbarProps {
  playback: PlaybackInfo | null;
  onPlay: () => void;
  onPause: () => void;
  onStop: () => void;
  onSkipBack: () => void;
  onSkipForward: () => void;
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  const ms = Math.floor((seconds % 1) * 100);
  return `${m}:${s.toString().padStart(2, "0")}.${ms.toString().padStart(2, "0")}`;
}

function ToolBtn({
  children,
  onClick,
  active = false,
}: {
  children: React.ReactNode;
  onClick: () => void;
  active?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded border px-3 py-1.5 text-xs font-semibold transition-colors duration-100 ${
        active
          ? "border-primary/30 bg-primary/10 text-primary hover:bg-primary/15"
          : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
      }`}
    >
      {children}
    </button>
  );
}

export function Toolbar({
  playback,
  onPlay,
  onPause,
  onStop,
  onSkipBack,
  onSkipForward,
}: ToolbarProps) {
  const playing = playback?.playing ?? false;
  const currentTime = playback?.current_time ?? 0;
  const duration = playback?.duration ?? 0;

  return (
    <div className="border-border bg-surface flex items-center gap-1.5 border-b px-3 py-1.5">
      {/* Transport */}
      <ToolBtn onClick={onSkipBack}>
        <SkipBackIcon />
      </ToolBtn>
      {playing ? (
        <ToolBtn onClick={onPause} active>
          <PauseIcon />
        </ToolBtn>
      ) : (
        <ToolBtn onClick={onPlay}>
          <PlayIcon />
        </ToolBtn>
      )}
      <ToolBtn onClick={onStop}>
        <StopIcon />
      </ToolBtn>
      <ToolBtn onClick={onSkipForward}>
        <SkipForwardIcon />
      </ToolBtn>

      {/* Time display */}
      <div className="border-border bg-bg text-text ml-3 min-w-36 rounded border px-3 py-1 text-center font-mono text-sm">
        {formatTime(currentTime)}
        <span className="text-text-2 mx-1">/</span>
        {formatTime(duration)}
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Status */}
      <span className="text-text-2 text-xs">
        {playback ? `${playback.duration.toFixed(0)}s @ 30fps` : ""}
      </span>
    </div>
  );
}

// Simple inline SVG icons - 16x16, consistent stroke style.
function PlayIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
      <path d="M4 2l10 6-10 6V2z" />
    </svg>
  );
}

function PauseIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
      <path d="M3 1h4v14H3zM9 1h4v14H9z" />
    </svg>
  );
}

function StopIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
      <rect x="2" y="2" width="12" height="12" rx="1" />
    </svg>
  );
}

function SkipBackIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
      <path d="M2 2h2v12H2zM6 8l8-6v12z" />
    </svg>
  );
}

function SkipForwardIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
      <path d="M12 2h2v12h-2zM2 2l8 6-8 6z" />
    </svg>
  );
}
