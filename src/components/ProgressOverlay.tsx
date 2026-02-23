import { cmd } from "../commands";
import type { TrackedOperation } from "../hooks/useProgress";

interface ProgressOverlayProps {
  operations: Map<string, TrackedOperation>;
}

export function ProgressOverlay({ operations }: ProgressOverlayProps) {
  if (operations.size === 0) return null;

  return (
    <div className="fixed bottom-4 left-1/2 z-50 -translate-x-1/2 flex flex-col gap-2">
      {[...operations.entries()].map(([opName, tracked]) => (
        <ProgressItem key={opName} opName={opName} tracked={tracked} />
      ))}
    </div>
  );
}

function ProgressItem({ opName, tracked }: { opName: string; tracked: TrackedOperation }) {
  const { event, stale } = tracked;
  const indeterminate = event.progress < 0;
  const pct = indeterminate ? 0 : Math.round(event.progress * 100);

  const handleCancel = () => {
    cmd.cancelOperation(opName);
  };

  return (
    <div className="bg-surface border-border flex min-w-[280px] flex-col gap-1.5 rounded-lg border px-4 py-3 shadow-lg">
      <div className="flex items-center justify-between">
        <p className="text-text text-sm font-medium">{event.phase}</p>
        <button
          onClick={handleCancel}
          className="text-text-2 hover:text-text ml-2 text-xs leading-none"
          title="Cancel operation"
        >
          ✕
        </button>
      </div>
      {event.detail && (
        <p className="text-text-2 text-xs">{event.detail}</p>
      )}
      {stale && (
        <div className="flex items-center justify-between">
          <p className="text-warning text-xs">No progress update — operation may be stuck</p>
          <button
            onClick={handleCancel}
            className="text-warning hover:text-error ml-2 text-xs underline"
          >
            Force cancel
          </button>
        </div>
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
  );
}
