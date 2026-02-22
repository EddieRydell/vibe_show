import { useCallback, useEffect, useRef, useState } from "react";
import { cmd } from "../commands";

export interface SaveStateResult {
  dirty: boolean;
  saveState: "idle" | "saving" | "saved";
  markDirty: () => void;
  clearDirty: () => void;
  handleSave: () => Promise<void>;
}

export function useSaveState(): SaveStateResult {
  const [dirty, setDirty] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">("idle");
  const savedTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const markDirty = useCallback(() => setDirty(true), []);
  const clearDirty = useCallback(() => setDirty(false), []);

  const handleSave = useCallback(async () => {
    if (saveState === "saving") return;
    setSaveState("saving");
    try {
      await cmd.saveCurrentSequence();
      setDirty(false);
      setSaveState("saved");
      clearTimeout(savedTimerRef.current);
      savedTimerRef.current = setTimeout(() => setSaveState("idle"), 1500);
    } catch (e) {
      console.error("[VibeLights] Save failed:", e);
      setSaveState("idle");
    }
  }, [saveState]);

  // Cleanup timer on unmount
  useEffect(() => () => clearTimeout(savedTimerRef.current), []);

  return { dirty, saveState, markDirty, clearDirty, handleSave };
}
