import type { ProgressEvent } from "../types";

interface ProgressOverlayProps {
  operations: Map<string, ProgressEvent>;
}

export function ProgressOverlay({ operations }: ProgressOverlayProps) {
  if (operations.size === 0) return null;

  // Show the first active operation
  const [, event] = [...operations.entries()][0];
  const indeterminate = event.progress < 0;
  const pct = indeterminate ? 0 : Math.round(event.progress * 100);

  return (
    <div className="fixed bottom-4 left-1/2 z-50 -translate-x-1/2">
      <div className="bg-surface border-border flex min-w-[280px] flex-col gap-1.5 rounded-lg border px-4 py-3 shadow-lg">
        <p className="text-text text-sm font-medium">{event.phase}</p>
        {event.detail && (
          <p className="text-text-2 text-xs">{event.detail}</p>
        )}
        <div className="bg-border/30 h-1.5 w-full overflow-hidden rounded-full">
          {indeterminate ? (
            <div className="bg-primary h-full w-1/3 animate-pulse rounded-full" />
          ) : (
            <div
              className="bg-primary h-full rounded-full transition-[width] duration-200"
              style={{ width: `${pct}%` }}
            />
          )}
        </div>
      </div>
    </div>
  );
}
