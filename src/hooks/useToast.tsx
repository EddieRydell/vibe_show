import { createContext, useCallback, useContext, useRef, useState } from "react";
import type { ReactNode } from "react";
import { formatTauriError } from "../utils/formatError";

type ToastVariant = "error" | "success" | "warning" | "info";

interface Toast {
  id: number;
  variant: ToastVariant;
  message: string;
}

interface ToastContextValue {
  showError: (error: unknown) => void;
  showSuccess: (message: string) => void;
  showWarning: (message: string) => void;
  showInfo: (message: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const AUTO_DISMISS_MS = 5000;
const MAX_TOASTS = 5;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);

  const addToast = useCallback((variant: ToastVariant, message: string) => {
    const id = nextId.current++;
    setToasts((prev) => {
      const next = [...prev, { id, variant, message }];
      // Keep only the most recent toasts
      return next.length > MAX_TOASTS ? next.slice(-MAX_TOASTS) : next;
    });
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, AUTO_DISMISS_MS);
  }, []);

  const dismiss = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const showError = useCallback(
    (error: unknown) => addToast("error", formatTauriError(error)),
    [addToast],
  );
  const showSuccess = useCallback(
    (message: string) => addToast("success", message),
    [addToast],
  );
  const showWarning = useCallback(
    (message: string) => addToast("warning", message),
    [addToast],
  );
  const showInfo = useCallback(
    (message: string) => addToast("info", message),
    [addToast],
  );

  const value: ToastContextValue = { showError, showSuccess, showWarning, showInfo };

  return (
    <ToastContext.Provider value={value}>
      {children}
      {/* Toast container — bottom-right, above everything */}
      {toasts.length > 0 && (
        <div
          className="fixed bottom-4 right-4 z-9999 flex flex-col gap-2"
          aria-live="assertive"
          aria-atomic="false"
        >
          {toasts.map((toast) => (
            <ToastItem key={toast.id} toast={toast} onDismiss={dismiss} />
          ))}
        </div>
      )}
    </ToastContext.Provider>
  );
}

function ToastItem({ toast, onDismiss }: { toast: Toast; onDismiss: (id: number) => void }) {
  const variantClasses: Record<ToastVariant, string> = {
    error: "border-error/30 bg-error/10 text-error",
    success: "border-success/30 bg-success/10 text-success",
    warning: "border-warning/30 bg-warning/10 text-warning",
    info: "border-primary/30 bg-primary/10 text-primary",
  };

  return (
    <div
      className={`max-w-sm rounded-lg border px-4 py-2.5 text-xs shadow-lg backdrop-blur-sm ${variantClasses[toast.variant]}`}
      role="alert"
    >
      <div className="flex items-start gap-2">
        <span className="flex-1 wrap-break-word">{toast.message}</span>
        <button
          onClick={() => onDismiss(toast.id)}
          className="shrink-0 opacity-60 hover:opacity-100"
          aria-label="Dismiss"
        >
          &times;
        </button>
      </div>
    </div>
  );
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error("useToast must be used within a ToastProvider");
  }
  return ctx;
}
