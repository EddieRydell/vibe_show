import type { DockviewApi } from "dockview-react";
import { PANEL } from "../panelIds";
import { getPanel } from "../registry";
import { PANEL_WIDTH } from "../../utils/layoutConstants";

/**
 * Applies the default editor layout programmatically.
 * Timeline fills the center, PropertyPanel on the right.
 */
export function applyDefaultEditorLayout(api: DockviewApi): void {
  const timelineDef = getPanel(PANEL.TIMELINE);
  const propertyDef = getPanel(PANEL.PROPERTY);

  // Add timeline panel first (fills center by default)
  const timelinePanel = api.addPanel({
    id: PANEL.TIMELINE,
    component: PANEL.TIMELINE,
    title: timelineDef?.title ?? "Timeline",
  });

  // Add property panel to the right
  api.addPanel({
    id: PANEL.PROPERTY,
    component: PANEL.PROPERTY,
    title: propertyDef?.title ?? "Properties",
    position: {
      referencePanel: timelinePanel,
      direction: "right",
    },
    initialWidth: PANEL_WIDTH,
  });
}
