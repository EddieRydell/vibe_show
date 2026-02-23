import { createContext, useContext } from "react";

/**
 * A version counter that increments whenever the show state changes
 * (edits, undo, redo, chat-triggered mutations, etc.).
 *
 * Components that cache IPC data (e.g. PropertyPanel's effect detail,
 * LibraryPanel's gradient list) add `showVersion` to their useEffect
 * deps so they automatically re-fetch after any state change.
 *
 * Provided by EditorScreen via ShowVersionContext.Provider.
 */
export const ShowVersionContext = createContext<number>(0);

export function useShowVersion(): number {
  return useContext(ShowVersionContext);
}
