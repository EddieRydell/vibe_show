import type { LayoutShape, Position2D } from "../types";

/**
 * Generate evenly-distributed pixel positions along a layout shape.
 * Mirrors the Rust LayoutShape::generate_positions logic.
 * Returns null for "Custom" shapes (positions are user-placed).
 */
export function generatePositions(
  shape: LayoutShape,
  pixelCount: number,
): Position2D[] | null {
  if (pixelCount === 0) return [];

  if (shape === "Custom") return null;

  if ("Line" in shape) {
    const { start, end } = shape.Line;
    return Array.from({ length: pixelCount }, (_, i) => {
      const t = pixelCount > 1 ? i / (pixelCount - 1) : 0.5;
      return {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
      };
    });
  }

  if ("Arc" in shape) {
    const { center, radius, start_angle, end_angle } = shape.Arc;
    return Array.from({ length: pixelCount }, (_, i) => {
      const t = pixelCount > 1 ? i / (pixelCount - 1) : 0.5;
      const angle = start_angle + (end_angle - start_angle) * t;
      return {
        x: center.x + radius * Math.cos(angle),
        y: center.y + radius * Math.sin(angle),
      };
    });
  }

  if ("Rectangle" in shape) {
    const { top_left, bottom_right } = shape.Rectangle;
    const w = Math.abs(bottom_right.x - top_left.x);
    const h = Math.abs(bottom_right.y - top_left.y);
    const perimeter = 2 * (w + h);
    if (perimeter === 0) {
      return Array.from({ length: pixelCount }, () => ({
        x: top_left.x,
        y: top_left.y,
      }));
    }
    return Array.from({ length: pixelCount }, (_, i) => {
      const t = pixelCount > 1 ? i / pixelCount : 0;
      const d = t * perimeter;
      if (d < w) {
        return { x: top_left.x + d, y: top_left.y };
      } else if (d < w + h) {
        return { x: bottom_right.x, y: top_left.y + (d - w) };
      } else if (d < 2 * w + h) {
        return { x: bottom_right.x - (d - w - h), y: bottom_right.y };
      } else {
        return { x: top_left.x, y: bottom_right.y - (d - 2 * w - h) };
      }
    });
  }

  if ("Grid" in shape) {
    const { top_left, bottom_right, columns } = shape.Grid;
    const cols = Math.max(1, columns);
    const rows = Math.ceil(pixelCount / cols);
    return Array.from({ length: pixelCount }, (_, i) => {
      const col = i % cols;
      const row = Math.floor(i / cols);
      const tx = cols > 1 ? col / (cols - 1) : 0.5;
      const ty = rows > 1 ? row / (rows - 1) : 0.5;
      return {
        x: top_left.x + (bottom_right.x - top_left.x) * tx,
        y: top_left.y + (bottom_right.y - top_left.y) * ty,
      };
    });
  }

  return null;
}
