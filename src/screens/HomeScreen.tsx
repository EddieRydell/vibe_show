import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ProfileSummary } from "../types";

interface Props {
  onOpenProfile: (slug: string) => void;
  onOpenSettings: () => void;
}

export function HomeScreen({ onOpenProfile, onOpenSettings }: Props) {
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    invoke<ProfileSummary[]>("list_profiles")
      .then(setProfiles)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    if (!newName.trim()) return;
    invoke<ProfileSummary>("create_profile", { name: newName.trim() })
      .then(() => {
        setNewName("");
        setShowCreate(false);
        refresh();
      })
      .catch((e) => setError(String(e)));
  }, [newName, refresh]);

  const handleDelete = useCallback(
    (slug: string, name: string) => {
      if (!confirm(`Delete profile "${name}" and all its data?`)) return;
      invoke("delete_profile", { slug })
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh],
  );

  const handleImportVixen = useCallback(async () => {
    const configPath = await open({
      title: "Select Vixen SystemConfig.xml",
      filters: [{ name: "XML Files", extensions: ["xml"] }],
    });
    if (!configPath) return;

    // Ask if they also want to import sequences
    let seqPaths: string[] = [];
    const wantSequences = confirm(
      "Would you also like to import sequence files (.tim)?\n\nClick OK to select sequences, or Cancel to import just the profile.",
    );
    if (wantSequences) {
      const picked = await open({
        title: "Select Vixen Sequence Files (.tim)",
        filters: [{ name: "Vixen Sequences", extensions: ["tim"] }],
        multiple: true,
      });
      seqPaths = picked ?? [];
    }

    invoke<ProfileSummary>("import_vixen", {
      systemConfigPath: configPath,
      sequencePaths: seqPaths,
    })
      .then((imported) => {
        refresh();
        onOpenProfile(imported.slug);
      })
      .catch((e) => setError(String(e)));
  }, [refresh, onOpenProfile]);

  return (
    <div className="bg-bg flex h-screen flex-col">
      {/* Header */}
      <div className="border-border flex items-center border-b px-6 py-4">
        <h1 className="text-text text-xl font-bold">VibeShow</h1>
        <div className="ml-auto flex items-center gap-2">
          <button
            onClick={handleImportVixen}
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
          <button
            onClick={onOpenSettings}
            className="text-text-2 hover:text-text ml-1 p-1.5 transition-colors"
            title="Settings"
          >
            <GearIcon />
          </button>
        </div>
      </div>

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
                    {p.show_count} show{p.show_count !== 1 ? "s" : ""}
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
    </div>
  );
}
