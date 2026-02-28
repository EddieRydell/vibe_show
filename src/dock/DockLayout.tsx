import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";
import {
  DockviewReact,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewPanelProps,
} from "dockview-react";
import { getPanel } from "./registry";
import { HeaderActions } from "./HeaderActions";
import "dockview-react/dist/styles/dockview.css";

export interface DockLayoutHandle {
  api: DockviewApi | null;
}

interface DockLayoutProps {
  onReady?: (event: DockviewReadyEvent) => void;
  onDidLayoutChange?: () => void;
  className?: string;
}

/** Resolves panel components from the registry by their component id. */
const components: Record<string, React.FunctionComponent<IDockviewPanelProps>> = new Proxy(
  {} as Record<string, React.FunctionComponent<IDockviewPanelProps>>,
  {
    get(_target, prop: string) {
      const def = getPanel(prop);
      return def?.component;
    },
  },
);

export const DockLayout = forwardRef<DockLayoutHandle, DockLayoutProps>(
  function DockLayout({ onReady, onDidLayoutChange, className }, ref) {
    const apiRef = useRef<DockviewApi | null>(null);

    useImperativeHandle(ref, () => ({ api: apiRef.current }), []);

    const handleReady = useCallback(
      (event: DockviewReadyEvent) => {
        apiRef.current = event.api;
        if (onDidLayoutChange) {
          event.api.onDidLayoutChange(onDidLayoutChange);
        }
        onReady?.(event);
      },
      [onReady, onDidLayoutChange],
    );

    return (
      <DockviewReact
        className={className ?? ""}
        components={components}
        onReady={handleReady}
        rightHeaderActionsComponent={HeaderActions}
      />
    );
  },
);
