import { useCallback, useEffect, useRef, useState } from "react";
import { Plus, Trash2, Copy, Pencil } from "lucide-react";
import { cmd } from "../../commands";
import { ConfirmDialog } from "../ConfirmDialog";

interface Props {
  currentScript: string | null;
  onSelectScript: (name: string) => void;
  onNewScript: (name: string) => void;
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
  const [renamingScript, setRenamingScript] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [showNewPrompt, setShowNewPrompt] = useState(false);
  const [newName, setNewName] = useState("");
  const renameInputRef = useRef<HTMLInputElement>(null);
  const newInputRef = useRef<HTMLInputElement>(null);

  const refresh = useCallback(() => {
    cmd.listGlobalScripts()
      .then((pairs) => setScripts(pairs.map(([name]) => name)))
      .catch(console.warn);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  // Focus inputs when they appear
  useEffect(() => {
    if (renamingScript) renameInputRef.current?.focus();
  }, [renamingScript]);

  useEffect(() => {
    if (showNewPrompt) newInputRef.current?.focus();
  }, [showNewPrompt]);

  const handleDelete = useCallback(
    async (name: string) => {
      await cmd.deleteGlobalScript(name);
      setDeleteTarget(null);
      refresh();
      if (currentScript === name) {
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
      const allScripts = await cmd.listGlobalScripts();
      const source = allScripts.find(([n]) => n === name)?.[1];
      if (!source) return;
      const existingNames = allScripts.map(([n]) => n);
      let newName = `${name}_copy`;
      let counter = 2;
      while (existingNames.includes(newName)) {
        newName = `${name}_copy${counter}`;
        counter++;
      }
      await cmd.compileGlobalScript(newName, source);
      refresh();
      onSelectScript(newName);
    },
    [onSelectScript, refresh],
  );

  const startRename = useCallback((name: string) => {
    setRenamingScript(name);
    setRenameValue(name);
  }, []);

  const commitRename = useCallback(async () => {
    if (!renamingScript) return;
    const trimmed = renameValue.trim();
    if (!trimmed || trimmed === renamingScript) {
      setRenamingScript(null);
      return;
    }
    try {
      await cmd.renameGlobalScript(renamingScript, trimmed);
      refresh();
      if (currentScript === renamingScript) {
        onSelectScript(trimmed);
      }
    } catch (e) {
      console.warn("Rename failed:", e);
    }
    setRenamingScript(null);
  }, [renamingScript, renameValue, currentScript, onSelectScript, refresh]);

  const commitNewScript = useCallback(() => {
    const trimmed = newName.trim();
    if (!trimmed) {
      setShowNewPrompt(false);
      setNewName("");
      return;
    }
    setShowNewPrompt(false);
    setNewName("");
    onNewScript(trimmed);
  }, [newName, onNewScript]);

  return (
    <div className="border-border flex h-full w-[200px] shrink-0 flex-col border-r">
      <div className="border-border flex items-center justify-between border-b px-3 py-2">
        <span className="text-text-2 text-[10px] font-medium uppercase tracking-wide">
          Scripts
        </span>
        <button
          onClick={() => setShowNewPrompt(true)}
          className="text-text-2 hover:text-text"
          title="New Script"
        >
          <Plus size={14} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {/* New script name prompt */}
        {showNewPrompt && (
          <div className="border-border border-b px-2 py-1.5">
            <input
              ref={newInputRef}
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") commitNewScript();
                if (e.key === "Escape") { setShowNewPrompt(false); setNewName(""); }
              }}
              onBlur={commitNewScript}
              placeholder="Script name"
              className="border-border bg-surface-2 text-text placeholder:text-text-2 w-full rounded border px-2 py-1 text-[11px] outline-none focus:border-primary"
            />
          </div>
        )}

        {scripts.length === 0 && !showNewPrompt && (
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
            {renamingScript === name ? (
              <input
                ref={renameInputRef}
                type="text"
                value={renameValue}
                onChange={(e) => setRenameValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") commitRename();
                  if (e.key === "Escape") setRenamingScript(null);
                }}
                onBlur={commitRename}
                onClick={(e) => e.stopPropagation()}
                className="border-border bg-surface-2 text-text min-w-0 flex-1 rounded border px-1.5 py-0.5 text-[11px] outline-none focus:border-primary"
              />
            ) : (
              <>
                <span className="min-w-0 flex-1 truncate">{name}</span>
                <div className="flex gap-0.5 opacity-0 group-hover:opacity-100">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      startRename(name);
                    }}
                    className="text-text-2 hover:text-text p-0.5"
                    title="Rename"
                  >
                    <Pencil size={10} />
                  </button>
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
              </>
            )}
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
