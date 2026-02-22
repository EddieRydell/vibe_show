import { useCallback, useEffect } from "react";
import { Portal } from "./Portal";

interface ModalBackdropProps {
  children: React.ReactNode;
  onClose?: () => void;
  className?: string;
}

/**
 * Shared modal backdrop rendered via Portal.
 *
 * - Fixed overlay with semi-transparent background
 * - Centers children by default (flex items-center justify-center)
 * - Clicking the backdrop calls `onClose`
 * - Escape key calls `onClose`
 * - Pass `className` to override positioning (e.g. "top-8" for wizards)
 */
export function ModalBackdrop({ children, onClose, className }: ModalBackdropProps) {
  // Escape key handler
  useEffect(() => {
    if (!onClose) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const handleBackdropMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget && onClose) onClose();
    },
    [onClose],
  );

  return (
    <Portal>
      <div
        className={`fixed inset-0 z-[var(--z-modal-backdrop)] flex items-center justify-center bg-black/50 ${className ?? ""}`}
        onMouseDown={handleBackdropMouseDown}
      >
        {children}
      </div>
    </Portal>
  );
}
