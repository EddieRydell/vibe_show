import { useCallback, useRef, useState } from "react";
import { getEffectiveZoom } from "../utils/cssZoom";

export interface CameraState {
  panX: number;
  panY: number;
  zoom: number;
}

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 20;

const DEFAULT_CAMERA: CameraState = { panX: 0, panY: 0, zoom: 1 };

export function usePreviewCamera() {
  const [camera, setCamera] = useState<CameraState>(DEFAULT_CAMERA);
  const draggingRef = useRef(false);
  const lastPosRef = useRef({ x: 0, y: 0 });

  const resetView = useCallback(() => {
    setCamera(DEFAULT_CAMERA);
  }, []);

  const onWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const zoom = getEffectiveZoom(el);
    const mouseX = (e.clientX - rect.left) / zoom;
    const mouseY = (e.clientY - rect.top) / zoom;

    setCamera((prev) => {
      const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
      const newZoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, prev.zoom * factor));
      const ratio = newZoom / prev.zoom;

      // Zoom toward cursor position
      const newPanX = mouseX - ratio * (mouseX - prev.panX);
      const newPanY = mouseY - ratio * (mouseY - prev.panY);

      return { panX: newPanX, panY: newPanY, zoom: newZoom };
    });
  }, []);

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    // Middle mouse button (button 1)
    if (e.button !== 1) return;
    e.preventDefault();
    draggingRef.current = true;
    lastPosRef.current = { x: e.clientX, y: e.clientY };
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    const el = e.currentTarget as HTMLElement;
    const zoom = getEffectiveZoom(el);
    const dx = (e.clientX - lastPosRef.current.x) / zoom;
    const dy = (e.clientY - lastPosRef.current.y) / zoom;
    lastPosRef.current = { x: e.clientX, y: e.clientY };

    setCamera((prev) => ({
      ...prev,
      panX: prev.panX + dx,
      panY: prev.panY + dy,
    }));
  }, []);

  const onPointerUp = useCallback((e: React.PointerEvent) => {
    if (e.button !== 1) return;
    draggingRef.current = false;
  }, []);

  const handlers = {
    onWheel,
    onPointerDown,
    onPointerMove,
    onPointerUp,
  };

  return { camera, resetView, handlers };
}
