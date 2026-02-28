/**
 * PopoutPanelHost — renders any registered panel in its own OS window.
 *
 * Wraps in the full EditorContextProvider so every panel gets the same
 * store, animation loop, and IPC access it has in the main dock.
 * To make a new component pop-out-able, just register it via registerPanel().
 */

import { useCallback, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppBar } from "../../components/AppBar";
import { ToastProvider } from "../../hooks/useToast";
import { AppShellContext } from "../../components/ScreenShell";
import { EditorContextProvider } from "../contexts/EditorContext";
import { getPanel } from "../registry";
import { registerEditorPanels } from "../panels/registerEditorPanels";

// Ensure panels are registered in this window context
registerEditorPanels();

export function PopoutPanelHost() {
  const params = new URLSearchParams(window.location.search);
  const panelId = params.get("panelId");
  const sequenceSlug =
    params.get("seq") ??
    localStorage.getItem("vibelights-active-sequence") ??
    "";

  const refreshRef = useRef<(() => void) | null>(null);
  const noop = useCallback(() => {}, []);

  const handleClose = useCallback(() => {
    void getCurrentWindow().close();
  }, []);

  const def = panelId ? getPanel(panelId) : undefined;

  if (!def || !sequenceSlug) {
    return (
      <div className="bg-bg text-text flex h-full flex-col">
        <AppBar onClose={handleClose} />
        <div className="flex flex-1 items-center justify-center">
          <span className="text-text-2 text-sm">
            {!sequenceSlug
              ? "No active sequence"
              : `Unknown panel: ${panelId ?? "(none)"}`}
          </span>
        </div>
      </div>
    );
  }

  const PanelComponent = def.component;

  return (
    <ToastProvider>
      <AppShellContext.Provider
        value={{
          chatOpen: false,
          toggleChat: noop,
          openSettings: noop,
          refreshRef,
        }}
      >
        <EditorContextProvider
          sequenceSlug={sequenceSlug}
          onBack={handleClose}
          onOpenScript={undefined}
        >
          <div className="bg-bg text-text flex h-full flex-col">
            <AppBar onClose={handleClose} />
            <div className="flex-1 overflow-hidden">
              <PanelComponent
                api={null as never}
                containerApi={null as never}
                params={{}}
              />
            </div>
          </div>
        </EditorContextProvider>
      </AppShellContext.Provider>
    </ToastProvider>
  );
}
