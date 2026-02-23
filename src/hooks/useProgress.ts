import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ProgressEvent } from "../types";

export interface TrackedOperation {
  event: ProgressEvent;
  lastUpdate: number;
  stale: boolean;
}

const STALE_THRESHOLD_MS = 45_000;
const STALE_CHECK_INTERVAL_MS = 5_000;

/**
 * Listens for "progress" events from the Rust backend.
 * Returns a Map of active operations keyed by operation name.
 * Entries are removed when progress >= 1.0.
 * Each entry includes staleness tracking (no update for 45s).
 */
export function useProgress(): Map<string, TrackedOperation> {
  const [ops, setOps] = useState<Map<string, TrackedOperation>>(new Map());
  const opsRef = useRef(ops);
  opsRef.current = ops;

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<ProgressEvent>("progress", (event) => {
      const ev = event.payload;
      setOps((prev) => {
        const next = new Map(prev);
        if (ev.progress >= 1.0) {
          next.delete(ev.operation);
        } else {
          next.set(ev.operation, {
            event: ev,
            lastUpdate: Date.now(),
            stale: false,
          });
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

  // Periodically check for stale operations
  useEffect(() => {
    const interval = setInterval(() => {
      const now = Date.now();
      setOps((prev) => {
        let changed = false;
        const next = new Map(prev);
        for (const [key, tracked] of next) {
          const shouldBeStale = now - tracked.lastUpdate > STALE_THRESHOLD_MS;
          if (shouldBeStale !== tracked.stale) {
            next.set(key, { ...tracked, stale: shouldBeStale });
            changed = true;
          }
        }
        return changed ? next : prev;
      });
    }, STALE_CHECK_INTERVAL_MS);

    return () => clearInterval(interval);
  }, []);

  return ops;
}
