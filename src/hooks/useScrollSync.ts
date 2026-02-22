import { type RefObject, useEffect } from "react";

/**
 * Sync scroll position from a source element to a target element.
 *
 * Re-runs every render (no dependency array) so the listener is always
 * attached even if refs remount. The effect is cheap — just adding and
 * removing a single passive event listener.
 */
export function useScrollSync(
  sourceRef: RefObject<HTMLElement | null>,
  targetRef: RefObject<HTMLElement | null>,
  axis: "vertical" | "horizontal" | "both" = "vertical",
) {
  useEffect(() => {
    const source = sourceRef.current;
    const target = targetRef.current;
    if (!source || !target) return;

    const sync = () => {
      if (axis === "vertical" || axis === "both") target.scrollTop = source.scrollTop;
      if (axis === "horizontal" || axis === "both") target.scrollLeft = source.scrollLeft;
    };

    source.addEventListener("scroll", sync, { passive: true });
    return () => source.removeEventListener("scroll", sync);
  }); // No deps — re-runs every render to catch ref changes
}
