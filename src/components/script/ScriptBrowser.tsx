import { useCallback, useEffect, useState } from "react";
import { Plus, Trash2, Copy } from "lucide-react";
import { cmd } from "../../commands";
import { ConfirmDialog } from "../ConfirmDialog";

interface Props {
  currentScript: string | null;
  onSelectScript: (name: string) => void;
  onNewScript: () => void;
  refreshKey: number;
}

export function ScriptBrowser({
  currentScript,
  onSelectScript,
  onNewScript,
  refreshKey,
}: Props) {
  const [scripts, setScripts] = useState<string[]>([]);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const refresh = useCallback(() => {
    cmd.listProfileScripts()
      .then((pairs) => setScripts(pairs.map(([name]) => name)))
      .catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  const handleDelete = useCallback(
    async (name: string) => {
      await cmd.deleteProfileScript(name);
      setDeleteTarget(null);
      refresh();
      if (currentScript === name) {
        // Select another script or clear
        const remaining = scripts.filter((s) => s !== name);
        if (remaining.length > 0) {
          onSelectScript(remaining[0]);
        }
      }
    },
    [currentScript, scripts, onSelectScript, refresh],
  );

  const handleDuplicate = useCallback(
    async (name: string) => {
      const allScripts = await cmd.listProfileScripts();
      const source = allScripts.find(([n]) => n === name)?.[1];
      if (!source) return;
      const existingNames = allScripts.map(([n]) => n);
      let newName = `${name}_copy`;
      let counter = 2;
      while (existingNames.includes(newName)) {
        newName = `${name}_copy${counter}`;
        counter++;
      }
      await cmd.compileProfileScript(newName, source);
      refresh();
      onSelectScript(newName);
    },
    [onSelectScript, refresh],
  );

  return (
    <div className="border-border flex h-full w-[200px] flex-shrink-0 flex-col border-r">
      <div className="border-border flex items-center justify-between border-b px-3 py-2">
        <span className="text-text-2 text-[10px] font-medium uppercase tracking-wide">
          Scripts
        </span>
        <button
          onClick={onNewScript}
          className="text-text-2 hover:text-text"
          title="New Script"
        >
          <Plus size={14} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {scripts.length === 0 && (
          <div className="text-text-2 px-3 py-4 text-center text-[10px]">
            No scripts yet.
            <br />
            Click + to create one.
          </div>
        )}
        {scripts.map((name) => (
          <div
            key={name}
            className={`group flex cursor-pointer items-center gap-1 px-3 py-1.5 text-[11px] ${
              name === currentScript
                ? "bg-primary/10 text-primary"
                : "text-text hover:bg-surface-2"
            }`}
            onClick={() => onSelectScript(name)}
          >
            <span className="min-w-0 flex-1 truncate">{name}</span>
            <div className="flex gap-0.5 opacity-0 group-hover:opacity-100">
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleDuplicate(name);
                }}
                className="text-text-2 hover:text-text p-0.5"
                title="Duplicate"
              >
                <Copy size={10} />
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setDeleteTarget(name);
                }}
                className="text-text-2 hover:text-red-400 p-0.5"
                title="Delete"
              >
                <Trash2 size={10} />
              </button>
            </div>
          </div>
        ))}
      </div>

      {deleteTarget && (
        <ConfirmDialog
          title="Delete Script"
          message={`Delete "${deleteTarget}"? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={() => handleDelete(deleteTarget)}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}
