import { useCallback } from "react";
import { ExternalLink } from "lucide-react";
import type { IDockviewHeaderActionsProps } from "dockview-react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

export function HeaderActions({ activePanel }: IDockviewHeaderActionsProps) {
  const handlePopout = useCallback(() => {
    if (!activePanel) return;

    const panelId = activePanel.id;
    const title = activePanel.title ?? panelId;
    const label = `popout-${panelId}`;
    const seq = localStorage.getItem("vibelights-active-sequence") ?? "";

    void (async () => {
      // Focus existing popout for this panel if any
      const existing = await WebviewWindow.getByLabel(label);
      if (existing) {
        await existing.setFocus();
        return;
      }

      new WebviewWindow(label, {
        url: `/?view=panel&panelId=${encodeURIComponent(panelId)}&seq=${encodeURIComponent(seq)}`,
        title: `VibeLights — ${title}`,
        width: 400,
        height: 600,
        decorations: false,
        center: true,
      });
    })();
  }, [activePanel]);

  return (
    <div className="flex items-center pr-1">
      <button
        onClick={handlePopout}
        className="text-text-2 hover:text-text flex items-center justify-center rounded p-0.5 transition-colors"
        title="Pop out to window"
      >
        <ExternalLink size={10} />
      </button>
    </div>
  );
}
