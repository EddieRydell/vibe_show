import { useState, useCallback, type ReactNode } from "react";
import { ConfirmDialog } from "./ConfirmDialog";

export interface CollapsibleListEditorProps<T> {
  items: [string, T][];
  itemLabel: string;
  countLabel: (item: T) => string;
  onCreate: () => void;
  onUpdate: (name: string, item: T) => void;
  onRename: (oldName: string, newName: string) => void;
  onDelete: (name: string) => void;
  renderEditor: (name: string, item: T) => ReactNode;
  renderPreview?: (name: string, item: T) => ReactNode;
  emptyMessage?: string;
}

export function CollapsibleListEditor<T>({
  items,
  itemLabel,
  countLabel,
  onCreate,
  onRename,
  onDelete,
  renderEditor,
  renderPreview,
  emptyMessage,
}: CollapsibleListEditorProps<T>) {
  const [expandedName, setExpandedName] = useState<string | null>(null);
  const [renamingName, setRenamingName] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const handleRename = useCallback(
    (oldName: string) => {
      if (!renameValue.trim() || renameValue === oldName) {
        setRenamingName(null);
        return;
      }
      onRename(oldName, renameValue.trim());
      if (expandedName === oldName) setExpandedName(renameValue.trim());
      setRenamingName(null);
    },
    [renameValue, expandedName, onRename],
  );

  const confirmDelete = useCallback(() => {
    if (!deleteTarget) return;
    onDelete(deleteTarget);
    if (expandedName === deleteTarget) setExpandedName(null);
    setDeleteTarget(null);
  }, [deleteTarget, expandedName, onDelete]);

  const capitalLabel = itemLabel.charAt(0).toUpperCase() + itemLabel.slice(1);

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">{capitalLabel}s</h3>
        <button
          onClick={onCreate}
          className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          New {capitalLabel}
        </button>
      </div>

      {items.length === 0 ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          {emptyMessage ?? `No ${itemLabel}s yet. Create one to use in your effects.`}
        </p>
      ) : (
        <div className="border-border divide-border divide-y rounded border">
          {items.map(([name, item]) => (
            <div key={name}>
              <div
                className="hover:bg-surface-2 group flex cursor-pointer items-center gap-3 px-4 py-2.5 transition-colors"
                onClick={() => setExpandedName(expandedName === name ? null : name)}
              >
                {renderPreview?.(name, item)}

                {renamingName === name ? (
                  <input
                    type="text"
                    value={renameValue}
                    onChange={(e) => setRenameValue(e.target.value)}
                    onBlur={() => handleRename(name)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleRename(name);
                      if (e.key === "Escape") setRenamingName(null);
                    }}
                    onClick={(e) => e.stopPropagation()}
                    autoFocus
                    className="border-border bg-surface-2 text-text rounded border px-2 py-0.5 text-sm outline-none focus:border-primary"
                  />
                ) : (
                  <span
                    className="text-text flex-1 text-sm font-medium"
                    onDoubleClick={(e) => {
                      e.stopPropagation();
                      setRenamingName(name);
                      setRenameValue(name);
                    }}
                  >
                    {name}
                  </span>
                )}

                <span className="text-text-2 text-xs">{countLabel(item)}</span>

                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setDeleteTarget(name);
                  }}
                  className="text-text-2 hover:text-error text-xs opacity-0 transition-all group-hover:opacity-100 hover:opacity-100"
                >
                  Delete
                </button>
              </div>

              {expandedName === name && (
                <div className="border-border border-t px-4 py-3">
                  {renderEditor(name, item)}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {deleteTarget && (
        <ConfirmDialog
          title={`Delete ${itemLabel}`}
          message={`Delete ${itemLabel} "${deleteTarget}"? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}
