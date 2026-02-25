import { useEffect, useRef, type DependencyList } from "react";
import { listen } from "@tauri-apps/api/event";

/**
 * Subscribe to a Tauri event with automatic cleanup.
 * Uses the cancelled-flag pattern to prevent stale listener accumulation.
 */
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- T constrains listen<T> in the body
export function useTauriListener<T>(
  event: string,
  handler: (payload: T) => void,
  deps: DependencyList = [],
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<T>(event, (e) => handlerRef.current(e.payload)).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [event, ...deps]);
}
