import { useCallback, useEffect, useState } from "react";
import { cmd } from "../../commands";
import type { EffectInfo } from "../../types";
import { ConfirmDialog } from "../../components/ConfirmDialog";

interface Props {
  setError: (e: string | null) => void;
  onOpenScript: (name: string | null) => void;
}

export function EffectsTab({ setError, onOpenScript }: Props) {
  const [effects, setEffects] = useState<EffectInfo[]>([]);
  const [scripts, setScripts] = useState<[string, string][]>([]);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const refreshEffects = useCallback(() => {
    cmd.listEffects().then(setEffects).catch(console.error);
  }, []);

  const refreshScripts = useCallback(() => {
    cmd.listGlobalScripts()
      .then(setScripts)
      .catch((e: unknown) => setError(String(e)));
  }, [setError]);

  useEffect(refreshEffects, [refreshEffects]);
  useEffect(refreshScripts, [refreshScripts]);

  const handleDeleteScript = useCallback(() => {
    if (!deleteTarget) return;
    cmd.deleteGlobalScript(deleteTarget)
      .then(refreshScripts)
      .catch((e: unknown) => setError(String(e)));
    setDeleteTarget(null);
  }, [deleteTarget, refreshScripts, setError]);

  return (
    <div className="p-6 space-y-8">
      <section>
        <h3 className="text-text mb-4 text-sm font-medium">Built-in Effects</h3>
        <div className="border-border divide-border divide-y rounded border">
          {effects.map((fx) => (
            <div
              key={fx.name}
              className="flex items-center justify-between px-4 py-2.5"
            >
              <span className="text-text text-sm font-medium">{fx.name}</span>
              {fx.schema.length > 0 && (
                <span className="text-text-2 text-xs">
                  {fx.schema.length} param{fx.schema.length !== 1 ? "s" : ""}
                </span>
              )}
            </div>
          ))}
        </div>
      </section>

      <section>
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-text text-sm font-medium">Custom Scripts</h3>
          <button
            onClick={() => onOpenScript(null)}
            className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
          >
            Effect Studio
          </button>
        </div>

        {scripts.length === 0 ? (
          <p className="text-text-2 text-center text-sm">
            No custom scripts yet. Create one to define custom effects.
          </p>
        ) : (
          <div className="border-border divide-border divide-y rounded border">
            {scripts.map(([name, source]) => (
              <div
                key={name}
                onClick={() => onOpenScript(name)}
                className="hover:bg-surface-2 group flex cursor-pointer items-center justify-between px-4 py-2.5 transition-colors"
              >
                <span className="text-text text-sm font-medium">{name}</span>
                <div className="flex items-center gap-3">
                  <span className="text-text-2 text-xs">{source.split("\n").length} lines</span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setDeleteTarget(name);
                    }}
                    className="text-text-2 hover:text-error text-[10px] opacity-0 transition-all group-hover:opacity-100"
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      {deleteTarget && (
        <ConfirmDialog
          title="Delete script"
          message={`Delete script "${deleteTarget}"? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={handleDeleteScript}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}
