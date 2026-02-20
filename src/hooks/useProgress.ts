import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ProgressEvent } from "../types";

/**
 * Listens for "progress" events from the Rust backend.
 * Returns a Map of active operations keyed by operation name.
 * Entries are removed when progress >= 1.0.
 */
export function useProgress(): Map<string, ProgressEvent> {
  const [ops, setOps] = useState<Map<string, ProgressEvent>>(new Map());

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<ProgressEvent>("progress", (event) => {
      const ev = event.payload;
      setOps((prev) => {
        const next = new Map(prev);
        if (ev.progress >= 1.0) {
          next.delete(ev.operation);
        } else {
          next.set(ev.operation, ev);
        }
        return next;
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  return ops;
}
