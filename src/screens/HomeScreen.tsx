import { useCallback, useEffect, useState } from "react";
import type { ProfileSummary } from "../types";
import { cmd } from "../commands";
import { ScreenShell } from "../components/ScreenShell";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ImportWizard } from "../components/ImportWizard";

interface Props {
  onOpenProfile: (slug: string) => void;
}

export function HomeScreen({ onOpenProfile }: Props) {
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    cmd.listProfiles()
      .then(setProfiles)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    if (!newName.trim()) return;
    cmd.createProfile(newName.trim())
      .then(() => {
        setNewName("");
        setShowCreate(false);
        refresh();
      })
      .catch((e) => setError(String(e)));
  }, [newName, refresh]);

  const [deleteTarget, setDeleteTarget] = useState<{ slug: string; name: string } | null>(null);

  const handleDelete = useCallback(
    (slug: string, name: string) => {
      setDeleteTarget({ slug, name });
    },
    [],
  );

  const confirmDelete = useCallback(() => {
    if (!deleteTarget) return;
    cmd.deleteProfile(deleteTarget.slug)
      .then(refresh)
      .catch((e) => setError(String(e)));
    setDeleteTarget(null);
  }, [deleteTarget, refresh]);

  const [showImportWizard, setShowImportWizard] = useState(false);

  const handleImportComplete = useCallback(
    (profileSlug: string) => {
      setShowImportWizard(false);
      refresh();
      onOpenProfile(profileSlug);
    },
    [refresh, onOpenProfile],
  );

  const toolbar = (
    <div className="border-border bg-surface flex select-none items-center gap-2 border-b px-4 py-1.5">
      <div className="flex-1" />
      <button
        onClick={() => setShowImportWizard(true)}
        className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-3 py-1.5 text-xs transition-colors"
      >
        Import from Vixen
      </button>
      <button
        onClick={() => setShowCreate(true)}
        className="bg-primary hover:bg-primary-hover rounded px-3 py-1.5 text-xs font-medium text-white transition-colors"
      >
        New Profile
      </button>
    </div>
  );

  return (
    <ScreenShell title="VibeLights" toolbar={toolbar}>
      {/* Error */}
      {error && (
        <div className="bg-error/10 border-error/20 text-error border-b px-6 py-2 text-xs">
          {error}
          <button onClick={() => setError(null)} className="ml-2 underline">
            dismiss
          </button>
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6">
        {/* Create form */}
        {showCreate && (
          <div className="border-border bg-surface mb-6 flex items-center gap-3 rounded-lg border p-4">
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              placeholder="Profile name (e.g. My House)"
              autoFocus
              className="border-border bg-surface-2 text-text placeholder:text-text-2 flex-1 rounded border px-3 py-1.5 text-sm outline-none focus:border-primary"
            />
            <button
              onClick={handleCreate}
              disabled={!newName.trim()}
              className="bg-primary hover:bg-primary-hover rounded px-4 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-50"
            >
              Create
            </button>
            <button
              onClick={() => {
                setShowCreate(false);
                setNewName("");
              }}
              className="text-text-2 hover:text-text text-xs transition-colors"
            >
              Cancel
            </button>
          </div>
        )}

        {/* Profile grid */}
        {profiles.length === 0 && !showCreate ? (
          <div className="text-text-2 mt-20 text-center">
            <p className="text-lg">No profiles yet</p>
            <p className="mt-2 text-sm">
              Create a new profile or import from Vixen to get started.
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {profiles.map((p) => (
              <div
                key={p.slug}
                onClick={() => onOpenProfile(p.slug)}
                className="border-border bg-surface hover:border-primary group cursor-pointer rounded-lg border p-4 transition-colors"
              >
                <h3 className="text-text text-sm font-medium">{p.name}</h3>
                <div className="text-text-2 mt-2 flex gap-4 text-xs">
                  <span>
                    {p.fixture_count} fixture{p.fixture_count !== 1 ? "s" : ""}
                  </span>
                  <span>
                    {p.sequence_count} sequence{p.sequence_count !== 1 ? "s" : ""}
                  </span>
                </div>
                <div className="text-text-2 mt-1 text-[10px]">
                  Created {p.created_at.split("T")[0]}
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDelete(p.slug, p.name);
                  }}
                  className="text-text-2 hover:text-error mt-2 text-[10px] opacity-0 transition-all group-hover:opacity-100"
                >
                  Delete
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {showImportWizard && (
        <ImportWizard
          onComplete={handleImportComplete}
          onCancel={() => setShowImportWizard(false)}
        />
      )}

      {deleteTarget && (
        <ConfirmDialog
          title="Delete profile"
          message={`Delete profile "${deleteTarget.name}" and all its data? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </ScreenShell>
  );
}
