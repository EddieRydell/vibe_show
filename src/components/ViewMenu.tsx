import { useState, useEffect, useRef } from "react";
import { Check, PanelLeft } from "lucide-react";
import { getAllPanels } from "../dock/registry";
import { useEditorStore } from "../dock/contexts/EditorContext";

interface ViewMenuProps {
  onTogglePanel: (panelId: string) => void;
}

export function ViewMenu({ onTogglePanel }: ViewMenuProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const panelVisibility = useEditorStore((s) => s.panelVisibility);
  const panels = getAllPanels();

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen((o) => !o)}
        className={`flex items-center gap-1 rounded border px-2 py-0.5 text-[11px] transition-colors ${
          open
            ? "border-primary/30 bg-primary/10 text-primary"
            : "border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
        }`}
      >
        <PanelLeft size={12} />
        View
      </button>
      {open && (
        <div className="bg-surface border-border absolute left-0 top-full z-50 mt-1 min-w-[140px] rounded border py-0.5 shadow-lg">
          {panels.map((panel) => (
            <button
              key={panel.id}
              className="text-text hover:bg-surface-2 flex w-full items-center gap-2 px-3 py-1.5 text-left text-[11px]"
              onClick={() => onTogglePanel(panel.id)}
            >
              <span className="flex w-3 items-center justify-center">
                {panelVisibility[panel.id] && (
                  <Check size={10} className="text-primary" />
                )}
              </span>
              {panel.title}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
