import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { DetachedPreview } from "./screens/DetachedPreview";
import { applyUISettings } from "./hooks/useUISettings";
import "./index.css";

// Apply saved UI settings before first render to prevent flash
try {
  const raw = localStorage.getItem("ui-settings");
  if (raw) applyUISettings(JSON.parse(raw));
} catch {
  // Falls back to CSS defaults
}

const isPreviewWindow =
  new URLSearchParams(window.location.search).get("view") === "preview";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      {isPreviewWindow ? <DetachedPreview /> : <App />}
    </StrictMode>,
  );
}
