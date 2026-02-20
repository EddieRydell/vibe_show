import { useEffect, useRef, useState } from "react";
import type { Frame, Show } from "../types";
import { usePreviewRenderer } from "../hooks/usePreviewRenderer";
import { useResizablePanel } from "../hooks/useResizablePanel";

interface PreviewProps {
  show: Show | null;
  frame: Frame | null;
  collapsed: boolean;
  onToggle: () => void;
  detached: boolean;
  onDetach: () => void;
  onFocusDetached: () => void;
}

export function Preview({ show, frame, collapsed, onToggle, detached, onDetach, onFocusDetached }: PreviewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const { height, onPointerDown, onPointerMove, onPointerUp } =
    useResizablePanel();

  // Observe container size (pattern from LayoutCanvas.tsx)
  useEffect(() => {
    const container = containerRef.current;
    if (!container || collapsed) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        setSize({
          width: Math.floor(width),
          height: Math.floor(height),
        });
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [collapsed]);

  usePreviewRenderer(canvasRef, show, frame, size.width, size.height);

  return (
    <div className="border-border bg-bg flex flex-col border-t">
      {/* Resize handle */}
      {!collapsed && (
        <div
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          className="bg-border hover:bg-primary cursor-ns-resize transition-colors"
          style={{ height: 4 }}
        />
      )}
      <div className="border-border bg-surface flex items-center border-b">
        <button
          onClick={onToggle}
          className="text-text-2 hover:bg-surface-2 hover:text-text flex flex-1 items-center gap-1.5 px-3 py-1 text-left text-[11px] tracking-wider uppercase"
        >
          <span className="text-[8px]">{collapsed ? "\u25B6" : "\u25BC"}</span>
          Preview
          {detached && (
            <span className="text-text-2 ml-1 text-[9px] normal-case tracking-normal">(detached)</span>
          )}
        </button>
        {detached ? (
          <button
            onClick={onFocusDetached}
            className="text-text-2 hover:bg-surface-2 hover:text-text px-2 py-1 text-[11px]"
            title="Focus detached preview"
          >
            Focus
          </button>
        ) : (
          <button
            onClick={onDetach}
            className="text-text-2 hover:bg-surface-2 hover:text-text px-2 py-1 text-[11px]"
            title="Open preview in separate window"
          >
            &#x2197;
          </button>
        )}
      </div>

      {!collapsed && !detached && (
        <div
          ref={containerRef}
          className="overflow-hidden"
          style={{ height }}
        >
          <canvas
            ref={canvasRef}
            className="block h-full w-full"
          />
        </div>
      )}

      {!collapsed && detached && (
        <div
          className="text-text-2 flex items-center justify-center gap-2 text-xs"
          style={{ height }}
        >
          Preview is in a separate window
        </div>
      )}
    </div>
  );
}
