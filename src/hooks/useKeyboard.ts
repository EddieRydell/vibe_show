import { useEffect } from "react";

interface KeyboardActions {
  onPlayPause: () => void;
  onPauseInPlace?: () => void;
  onStop: () => void;
  onSeekStart: () => void;
  onSeekEnd: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomFit: () => void;
  onSelectAll: () => void;
  onDeleteSelected: () => void;
  onToggleLoop?: () => void;
  onSave?: () => void;
  onUndo?: () => void;
  onRedo?: () => void;
  onSetModeSelect?: () => void;
  onSetModeEdit?: () => void;
  onSetModeSwipe?: () => void;
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
          if (e.shiftKey) {
            actions.onPauseInPlace?.();
          } else {
            actions.onPlayPause();
          }
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
        case "KeyZ":
          if (e.ctrlKey || e.metaKey) {
            e.preventDefault();
            if (e.shiftKey) {
              actions.onRedo?.();
            } else {
              actions.onUndo?.();
            }
          }
          break;
        case "KeyY":
          if (e.ctrlKey || e.metaKey) {
            e.preventDefault();
            actions.onRedo?.();
          }
          break;
        case "KeyS":
          if (e.ctrlKey || e.metaKey) {
            e.preventDefault();
            actions.onSave?.();
          } else if (!e.altKey) {
            actions.onSetModeSwipe?.();
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
        case "KeyV":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            actions.onSetModeSelect?.();
          }
          break;
        case "KeyM":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            actions.onSetModeEdit?.();
          }
          break;
        case "KeyL":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            actions.onToggleLoop?.();
          }
          break;
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [actions]);
}
