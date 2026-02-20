import type { PlaybackInfo, UndoState } from "../types";

interface ToolbarProps {
  playback: PlaybackInfo | null;
  undoState: UndoState | null;
  onPlay: () => void;
  onPause: () => void;
  onStop: () => void;
  onSkipBack: () => void;
  onSkipForward: () => void;
  onUndo: () => void;
  onRedo: () => void;
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
  disabled = false,
  title,
}: {
  children: React.ReactNode;
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`rounded border px-3 py-1.5 text-xs font-semibold transition-colors duration-100 ${
        active
          ? "border-primary/30 bg-primary/10 text-primary hover:bg-primary/15"
          : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
      } disabled:cursor-not-allowed disabled:opacity-30`}
    >
      {children}
    </button>
  );
}

export function Toolbar({
  playback,
  undoState,
  onPlay,
  onPause,
  onStop,
  onSkipBack,
  onSkipForward,
  onUndo,
  onRedo,
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

      {/* Divider */}
      <div className="border-border mx-1 h-5 border-l" />

      {/* Undo / Redo */}
      <ToolBtn onClick={onUndo} disabled={!undoState?.can_undo} title={undoState?.undo_description ? `Undo: ${undoState.undo_description}` : "Undo (Ctrl+Z)"}>
        <UndoIcon />
      </ToolBtn>
      <ToolBtn onClick={onRedo} disabled={!undoState?.can_redo} title={undoState?.redo_description ? `Redo: ${undoState.redo_description}` : "Redo (Ctrl+Shift+Z)"}>
        <RedoIcon />
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

function UndoIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 7h7a3 3 0 0 1 0 6H9" />
      <path d="M6 4L3 7l3 3" />
    </svg>
  );
}

function RedoIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M13 7H6a3 3 0 0 0 0 6h1" />
      <path d="M10 4l3 3-3 3" />
    </svg>
  );
}
