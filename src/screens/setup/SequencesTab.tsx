import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { cmd } from "../../commands";
import type { SequenceSummary } from "../../types";
import { ConfirmDialog } from "../../components/ConfirmDialog";

interface Props {
  slug: string;
  onOpenSequence: (slug: string) => void;
  setError: (e: string | null) => void;
}

export function SequencesTab({ slug, onOpenSequence, setError }: Props) {
  const [sequences, setSequences] = useState<SequenceSummary[]>([]);
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [importingVixen, setImportingVixen] = useState(false);

  const refresh = useCallback(() => {
    cmd.listSequences()
      .then(setSequences)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    if (!newName.trim()) return;
    cmd.createSequence(newName.trim())
      .then(() => {
        setNewName("");
        setShowCreate(false);
        refresh();
      })
      .catch((e) => setError(String(e)));
  }, [newName, refresh, setError]);

  const [deleteTarget, setDeleteTarget] = useState<{ slug: string; name: string } | null>(null);

  const handleDelete = useCallback(
    (seqSlug: string, name: string) => {
      setDeleteTarget({ slug: seqSlug, name });
    },
    [],
  );

  const confirmDelete = useCallback(() => {
    if (!deleteTarget) return;
    cmd.deleteSequence(deleteTarget.slug)
      .then(refresh)
      .catch((e) => setError(String(e)));
    setDeleteTarget(null);
  }, [deleteTarget, refresh, setError]);

  const handleImportVixenSequence = useCallback(async () => {
    const picked = await open({
      title: "Select Vixen Sequence Files (.tim)",
      filters: [{ name: "Vixen Sequences", extensions: ["tim"] }],
      multiple: true,
    });
    if (!picked || picked.length === 0) return;

    setImportingVixen(true);
    try {
      for (const timPath of picked) {
        try {
          await cmd.importVixenSequence(slug, timPath);
        } catch (e) {
          setError(`Failed to import sequence: ${e}`);
        }
      }
      refresh();
    } finally {
      setImportingVixen(false);
    }
  }, [slug, refresh, setError]);

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">Sequences</h3>
        <div className="flex items-center gap-2">
          <button
            onClick={handleImportVixenSequence}
            disabled={importingVixen}
            className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-3 py-1 text-xs transition-colors disabled:opacity-50"
          >
            {importingVixen ? "Importing..." : "Import from Vixen (.tim)"}
          </button>
          <button
            onClick={() => setShowCreate(true)}
            className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
          >
            New Sequence
          </button>
        </div>
      </div>

      {showCreate && (
        <div className="border-border bg-surface mb-4 flex items-center gap-3 rounded border p-3">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            placeholder="Sequence name"
            autoFocus
            className="border-border bg-surface-2 text-text placeholder:text-text-2 flex-1 rounded border px-3 py-1.5 text-sm outline-none focus:border-primary"
          />
          <button
            onClick={handleCreate}
            disabled={!newName.trim()}
            className="bg-primary hover:bg-primary-hover rounded px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
          >
            Create
          </button>
          <button
            onClick={() => {
              setShowCreate(false);
              setNewName("");
            }}
            className="text-text-2 hover:text-text text-xs"
          >
            Cancel
          </button>
        </div>
      )}

      {sequences.length === 0 && !showCreate ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          No sequences yet. Create one to start sequencing.
        </p>
      ) : (
        <div className="border-border divide-border divide-y rounded border">
          {sequences.map((s) => (
            <div
              key={s.slug}
              onClick={() => onOpenSequence(s.slug)}
              className="hover:bg-surface-2 group flex cursor-pointer items-center justify-between px-4 py-2.5 transition-colors"
            >
              <span className="text-text text-sm font-medium">{s.name}</span>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleDelete(s.slug, s.name);
                }}
                className="text-text-2 hover:text-error text-[10px] opacity-0 transition-all group-hover:opacity-100"
              >
                Delete
              </button>
            </div>
          ))}
        </div>
      )}

      {deleteTarget && (
        <ConfirmDialog
          title="Delete sequence"
          message={`Delete sequence "${deleteTarget.name}"? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}
