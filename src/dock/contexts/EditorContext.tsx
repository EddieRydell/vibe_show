/**
 * EditorContext — zustand-backed editor state with selector-based subscriptions.
 *
 * Components subscribe to individual slices via `useEditorStore(s => s.foo)`.
 * The Provider creates the store, starts the RAF loop, and syncs external hook state.
 */

import { createContext, useContext, useEffect, useRef, type ReactNode } from "react";
import { useStore } from "zustand";
import { useKeyboard } from "../../hooks/useKeyboard";
import { useProgress } from "../../hooks/useProgress";
import { useToast } from "../../hooks/useToast";
import { useAppShell } from "../../components/ScreenShell";
import { createEditorStore, type EditorState, type EditorStore } from "./editorStore";

// ── Store context (just the store reference, not the state) ───────

const EditorStoreContext = createContext<EditorStore | null>(null);

export function useEditorStore<T>(selector: (s: EditorState) => T): T {
  const store = useContext(EditorStoreContext);
  if (!store) throw new Error("useEditorStore must be used within EditorContextProvider");
  return useStore(store, selector);
}

// ── Provider ─────────────────────────────────────────────────────

interface EditorContextProviderProps {
  sequenceSlug: string;
  onBack: () => void;
  onOpenScript: ((name: string | null) => void) | undefined;
  children: ReactNode;
}

export function EditorContextProvider({
  sequenceSlug,
  onBack,
  onOpenScript,
  children,
}: EditorContextProviderProps) {
  const { refreshRef } = useAppShell();
  const progressOps = useProgress();
  const { showError } = useToast();

  // Create store once
  const storeRef = useRef<ReturnType<typeof createEditorStore> | null>(null);
  if (!storeRef.current) {
    storeRef.current = createEditorStore(showError, { onBack, onOpenScript }, refreshRef);
  }
  const { store, cleanup, setCallbacks } = storeRef.current;

  // Update mutable callbacks every render
  setCallbacks({ onBack, onOpenScript });

  // Cleanup on unmount
  useEffect(() => cleanup, [cleanup]);

  // Start RAF animation loop
  useEffect(() => store.getState().startAnimationLoop(), [store]);

  // Sync progress ops from hook into store
  useEffect(() => { store.setState({ progressOps }); }, [progressOps, store]);

  // Open sequence on mount; persist slug for popout windows
  useEffect(() => {
    localStorage.setItem("vibelights-active-sequence", sequenceSlug);
    store.getState().openSequence(sequenceSlug);
  }, [sequenceSlug, store]);

  // Keyboard shortcuts
  useKeyboard(store.getState().keyboardActions);

  return (
    <EditorStoreContext.Provider value={store}>
      {children}
    </EditorStoreContext.Provider>
  );
}
