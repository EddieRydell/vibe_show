import { useCallback, useEffect, useRef } from "react";

interface ConfirmDialogProps {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  message,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  destructive = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
      if (e.key === "Enter") onConfirm();
    },
    [onCancel, onConfirm],
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={handleKeyDown}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div className="bg-surface border-border w-[360px] rounded-lg border shadow-xl">
        <div className="border-border border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">{title}</h3>
        </div>
        <div className="px-5 py-4">
          <p className="text-text-2 text-sm">{message}</p>
        </div>
        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          <button
            ref={cancelRef}
            onClick={onCancel}
            className="border-border bg-surface-2 text-text-2 hover:bg-bg hover:text-text rounded border px-3 py-1.5 text-xs transition-colors"
          >
            {cancelLabel}
          </button>
          <button
            onClick={onConfirm}
            className={`rounded border px-3 py-1.5 text-xs font-medium transition-colors ${
              destructive
                ? "border-error bg-error text-white hover:bg-error/80"
                : "border-primary bg-primary hover:bg-primary-hover text-white"
            }`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
