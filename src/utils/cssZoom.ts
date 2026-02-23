/**
 * Utilities for correct mouse coordinate handling under CSS zoom.
 *
 * When CSS `zoom` is applied to an ancestor (e.g. `document.documentElement`),
 * `getBoundingClientRect()` returns visual/viewport-pixel values, but layout
 * properties like `scrollLeft`, `offsetWidth`, and CSS dimensions remain in
 * CSS pixels. `clientX - rect.left` therefore gives a viewport-pixel offset,
 * not a CSS-pixel offset.  The error scales linearly with distance from the
 * element's origin — zero at the left edge, growing toward the right.
 *
 * These helpers convert viewport mouse coordinates into CSS-pixel coordinates
 * so they work correctly with canvas drawing, scroll positions, and layout
 * values regardless of the current zoom level.
 */

/**
 * Compute the effective CSS zoom for an element.
 * Returns the ratio between viewport (visual) pixels and CSS pixels.
 * When no CSS zoom is applied, this returns 1.
 */
export function getEffectiveZoom(el: HTMLElement): number {
  if (!el.offsetWidth) return 1;
  return el.getBoundingClientRect().width / el.offsetWidth;
}

/**
 * Convert a mouse event's viewport coordinates to CSS-pixel coordinates
 * relative to the given element, accounting for CSS zoom.
 */
export function cssMouseOffset(
  e: { clientX: number; clientY: number },
  el: HTMLElement,
): { x: number; y: number } {
  const rect = el.getBoundingClientRect();
  const zoom = el.offsetWidth ? rect.width / el.offsetWidth : 1;
  return {
    x: (e.clientX - rect.left) / zoom,
    y: (e.clientY - rect.top) / zoom,
  };
}
