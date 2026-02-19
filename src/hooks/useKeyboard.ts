import { useEffect } from "react";

interface KeyboardActions {
  onPlayPause: () => void;
  onStop: () => void;
  onSeekStart: () => void;
  onSeekEnd: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomFit: () => void;
  onSelectAll: () => void;
  onDeleteSelected: () => void;
}

export function useKeyboard(actions: KeyboardActions) {
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Don't capture if user is typing in an input.
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
      }

      switch (e.code) {
        case "Space":
          e.preventDefault();
          actions.onPlayPause();
          break;
        case "Home":
          e.preventDefault();
          actions.onSeekStart();
          break;
        case "End":
          e.preventDefault();
          actions.onSeekEnd();
          break;
        case "Equal": // + key
        case "NumpadAdd":
          e.preventDefault();
          actions.onZoomIn();
          break;
        case "Minus":
        case "NumpadSubtract":
          e.preventDefault();
          actions.onZoomOut();
          break;
        case "Digit0":
          if (e.ctrlKey || e.metaKey) {
            e.preventDefault();
            actions.onZoomFit();
          }
          break;
        case "KeyA":
          if (e.ctrlKey || e.metaKey) {
            e.preventDefault();
            actions.onSelectAll();
          }
          break;
        case "Delete":
        case "Backspace":
          e.preventDefault();
          actions.onDeleteSelected();
          break;
        case "Escape":
          // Deselect all is handled by the timeline click-on-empty.
          break;
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [actions]);
}
