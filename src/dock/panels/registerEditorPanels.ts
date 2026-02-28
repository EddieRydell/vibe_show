import { registerPanel } from "../registry";
import { PANEL } from "../panelIds";
import { PANEL_WIDTH } from "../../utils/layoutConstants";
import { DockableTimeline } from "./DockableTimeline";
import { DockablePropertyPanel } from "./DockablePropertyPanel";
import { DockableLibraryPanel } from "./DockableLibraryPanel";

let registered = false;

export function registerEditorPanels(): void {
  if (registered) return;
  registered = true;

  registerPanel({
    id: PANEL.TIMELINE,
    title: "Timeline",
    component: DockableTimeline,
    defaultPosition: "center",
    minimumWidth: 300,
    minimumHeight: 150,
  });

  registerPanel({
    id: PANEL.PROPERTY,
    title: "Properties",
    component: DockablePropertyPanel,
    defaultPosition: "right",
    preferredWidth: PANEL_WIDTH,
    minimumWidth: 200,
  });

  registerPanel({
    id: PANEL.LIBRARY,
    title: "Library",
    component: DockableLibraryPanel,
    defaultPosition: "right",
    preferredWidth: PANEL_WIDTH,
    minimumWidth: 200,
  });
}
