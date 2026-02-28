import type { IDockviewPanelProps } from "dockview-react";

export interface PanelDefinition {
  id: string;
  title: string;
  component: React.FunctionComponent<IDockviewPanelProps>;
  defaultPosition?: "left" | "right" | "bottom" | "center";
  preferredWidth?: number;
  preferredHeight?: number;
  minimumWidth?: number;
  minimumHeight?: number;
}

const panels = new Map<string, PanelDefinition>();

export function registerPanel(def: PanelDefinition): void {
  panels.set(def.id, def);
}

export function getPanel(id: string): PanelDefinition | undefined {
  return panels.get(id);
}

