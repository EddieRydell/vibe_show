import { useCallback, useEffect, useRef, useState } from "react";

const STORAGE_KEY = "preview-panel-height";
const MIN_HEIGHT = 100;
const MAX_VIEWPORT_FRACTION = 0.8;
const DEFAULT_HEIGHT = 192;

export function useResizablePanel() {
  const [height, setHeight] = useState(() => {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = parseFloat(stored);
      if (Number.isFinite(parsed)) return clamp(parsed);
    }
    return DEFAULT_HEIGHT;
  });

  const draggingRef = useRef(false);

  function clamp(h: number) {
    const maxH = window.innerHeight * MAX_VIEWPORT_FRACTION;
    return Math.max(MIN_HEIGHT, Math.min(h, maxH));
  }

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    draggingRef.current = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    const newHeight = clamp(window.innerHeight - e.clientY);
    setHeight(newHeight);
  }, []);

  const onPointerUp = useCallback(() => {
    draggingRef.current = false;
  }, []);

  // Persist height changes to localStorage
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, String(height));
  }, [height]);

  return { height, onPointerDown, onPointerMove, onPointerUp };
}
