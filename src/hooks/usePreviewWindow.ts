import { useCallback, useEffect, useRef, useState } from "react";
import { emitTo } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { PREVIEW_WINDOW_LABEL, VIEW_PREVIEW } from "../constants";

export interface PreviewWindowState {
  previewOpen: boolean;
  handleTogglePreview: () => Promise<void>;
  notifyPreview: (event: string, payload?: unknown) => void;
}

export function usePreviewWindow(): PreviewWindowState {
  const [previewOpen, setPreviewOpen] = useState(false);
  const previewWindowRef = useRef<WebviewWindow | null>(null);

  const handleTogglePreview = useCallback(async () => {
    if (previewWindowRef.current) {
      try {
        await previewWindowRef.current.destroy();
      } catch {
        // Already destroyed
      }
      previewWindowRef.current = null;
      setPreviewOpen(false);
      return;
    }

    const previewWin = new WebviewWindow(PREVIEW_WINDOW_LABEL, {
      url: `/?view=${VIEW_PREVIEW}`,
      title: "VibeLights Preview",
      width: 800,
      height: 600,
      decorations: false,
      center: true,
    });

    previewWindowRef.current = previewWin;
    setPreviewOpen(true);

    previewWin.onCloseRequested(() => {
      previewWindowRef.current = null;
      setPreviewOpen(false);
    });
  }, []);

  const notifyPreview = useCallback(
    (event: string, payload?: unknown) => {
      if (previewOpen) emitTo(PREVIEW_WINDOW_LABEL, event, payload);
    },
    [previewOpen],
  );

  // Clean up preview window on unmount
  useEffect(() => {
    return () => {
      if (previewWindowRef.current) {
        previewWindowRef.current.destroy().catch(() => {});
        previewWindowRef.current = null;
      }
    };
  }, []);

  return { previewOpen, handleTogglePreview, notifyPreview };
}
