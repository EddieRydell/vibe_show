import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { DetachedPreview } from "./screens/DetachedPreview";
import { PopoutPanelHost } from "./dock/popout/PopoutPanelHost";
import { applyUISettings, type UISettings } from "./hooks/useUISettings";
import "./index.css";

// Apply saved UI settings before first render to prevent flash
try {
  const raw = localStorage.getItem("ui-settings");
  if (raw) applyUISettings(JSON.parse(raw) as UISettings);
} catch {
  // Falls back to CSS defaults
}

const view = new URLSearchParams(window.location.search).get("view");

function RootComponent() {
  if (view === "preview") return <DetachedPreview />;
  if (view === "panel") return <PopoutPanelHost />;
  return <App />;
}

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <RootComponent />
    </StrictMode>,
  );
}
