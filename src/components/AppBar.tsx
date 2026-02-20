import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Wordmark } from "./Wordmark";

export function AppBar({ onClose }: { onClose?: () => void } = {}) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    appWindow.isMaximized().then(setMaximized);
    const unlisten = appWindow.onResized(() => {
      appWindow.isMaximized().then(setMaximized);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleMinimize = useCallback(() => {
    getCurrentWindow().minimize();
  }, []);

  const handleToggleMaximize = useCallback(() => {
    getCurrentWindow().toggleMaximize();
  }, []);

  const handleClose = useCallback(() => {
    if (onClose) onClose();
    else getCurrentWindow().close();
  }, [onClose]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    // Don't drag if clicking on an interactive element
    if ((e.target as HTMLElement).closest("button, input, a, select, textarea")) return;
    if (e.button !== 0) return;
    e.preventDefault();
    getCurrentWindow().startDragging();
  }, []);

  const handleDoubleClick = useCallback((e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button, input, a, select, textarea")) return;
    getCurrentWindow().toggleMaximize();
  }, []);

  return (
    <div
      className="border-border bg-bg flex select-none items-center border-b px-3 py-1"
      onMouseDown={handleMouseDown}
      onDoubleClick={handleDoubleClick}
    >
      <Wordmark size={13} className="text-text" />
      <div className="flex-1" />

      {/* Window controls */}
      <div className="flex items-center">
        <button
          onClick={handleMinimize}
          className="text-text-2 hover:bg-surface-2 hover:text-text flex h-6 w-8 items-center justify-center rounded-sm transition-colors"
          title="Minimize"
        >
          <svg width="10" height="1" viewBox="0 0 10 1">
            <rect width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        <button
          onClick={handleToggleMaximize}
          className="text-text-2 hover:bg-surface-2 hover:text-text flex h-6 w-8 items-center justify-center rounded-sm transition-colors"
          title={maximized ? "Restore" : "Maximize"}
        >
          {maximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path
                d="M2 0h8v8h-2v2H0V2h2V0zm1 1v1h5v5h1V1H3zM1 3v6h6V3H1z"
                fill="currentColor"
              />
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect
                x="0.5"
                y="0.5"
                width="9"
                height="9"
                fill="none"
                stroke="currentColor"
                strokeWidth="1"
              />
            </svg>
          )}
        </button>
        <button
          onClick={handleClose}
          className="text-text-2 hover:bg-error flex h-6 w-8 items-center justify-center rounded-sm transition-colors hover:text-white"
          title="Close"
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path
              d="M1 1l8 8M9 1l-8 8"
              stroke="currentColor"
              strokeWidth="1.2"
            />
          </svg>
        </button>
      </div>
    </div>
  );
}
