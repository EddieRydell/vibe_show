import { useEffect, useRef, useState, useCallback } from "react";
import { Portal } from "./Portal";

interface DropdownProps {
  anchorRef: React.RefObject<HTMLElement | null>;
  onClose: () => void;
  children: React.ReactNode;
  align?: "left" | "right";
}

/**
 * Dropdown menu rendered via Portal at the z-dropdown layer.
 *
 * - Positions itself below the anchor element using getBoundingClientRect
 * - Flips above if not enough space below
 * - Aligns left or right edge to anchor
 * - Handles click-outside to close
 * - Handles Escape key to close
 */
export function Dropdown({ anchorRef, onClose, children, align = "left" }: DropdownProps) {
  const dropdownRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{
    top: number;
    left: number;
  } | null>(null);

  // Calculate position based on anchor element
  const updatePosition = useCallback(() => {
    const anchor = anchorRef.current;
    const dropdown = dropdownRef.current;
    if (!anchor || !dropdown) return;

    const anchorRect = anchor.getBoundingClientRect();
    const dropdownRect = dropdown.getBoundingClientRect();
    const viewportHeight = window.innerHeight;
    const viewportWidth = window.innerWidth;

    // Vertical: prefer below, flip above if not enough space
    const spaceBelow = viewportHeight - anchorRect.bottom;
    const spaceAbove = anchorRect.top;
    const top =
      spaceBelow >= dropdownRect.height || spaceBelow >= spaceAbove
        ? anchorRect.bottom + 4
        : anchorRect.top - dropdownRect.height - 4;

    // Horizontal: align to anchor edge, constrain to viewport
    let left: number;
    if (align === "right") {
      left = anchorRect.right - dropdownRect.width;
    } else {
      left = anchorRect.left;
    }
    // Clamp to viewport
    if (left + dropdownRect.width > viewportWidth) {
      left = viewportWidth - dropdownRect.width - 4;
    }
    if (left < 4) {
      left = 4;
    }

    setPosition({ top, left });
  }, [anchorRef, align]);

  // Initial positioning (use ResizeObserver to handle content changes)
  useEffect(() => {
    // Run positioning after first render so dropdown has dimensions
    requestAnimationFrame(updatePosition);
  }, [updatePosition]);

  // Click outside to close
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(e.target as Node) &&
        anchorRef.current &&
        !anchorRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose, anchorRef]);

  // Escape key to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <Portal>
      <div
        ref={dropdownRef}
        className="fixed z-[var(--z-dropdown)]"
        style={
          position
            ? { top: position.top, left: position.left }
            : { visibility: "hidden", top: 0, left: 0 }
        }
      >
        {children}
      </div>
    </Portal>
  );
}
