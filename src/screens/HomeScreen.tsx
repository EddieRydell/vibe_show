import { useCallback, useEffect, useState } from "react";
import type { Setup, SetupSummary } from "../types";
import { cmd } from "../commands";
import { ScreenShell } from "../components/ScreenShell";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ImportWizard } from "../components/ImportWizard";
import { TabBar } from "../components/TabBar";
import { SequencesTab } from "./setup/SequencesTab";
import { MusicTab } from "./setup/MusicTab";
import { HouseSetupTab } from "./setup/HouseSetupTab";
import { LayoutTab } from "./setup/LayoutTab";
import { EffectsTab } from "./setup/EffectsTab";
import { GradientsTab } from "./setup/GradientsTab";
import { CurvesTab } from "./setup/CurvesTab";

interface Props {
  activeSetupSlug: string | null;
  activeTab: string;
  onTabChange: (tab: string) => void;
  onOpenSetup: (slug: string) => void;
  onCloseSetup: () => void;
  onOpenSequence: (sequenceSlug: string) => void;
  onOpenScript: (name: string | null) => void;
  onOpenAnalysis: (filename: string) => void;
}

export function HomeScreen({
  activeSetupSlug,
  activeTab,
  onTabChange,
  onOpenSetup,
  onCloseSetup,
  onOpenSequence,
  onOpenScript,
  onOpenAnalysis,
}: Props) {
  const [setups, setSetups] = useState<SetupSummary[]>([]);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    cmd.listSetups()
      .then(setSetups)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  useEffect(refresh, [refresh]);

  // Load setup data when activeSetupSlug changes
  useEffect(() => {
    if (!activeSetupSlug) {
      setSetup(null);
      return;
    }
    cmd.openSetup(activeSetupSlug)
      .then(setSetup)
      .catch((e: unknown) => {
        setError(String(e));
        setSetup(null);
      });
  }, [activeSetupSlug]);

  const handleCreate = useCallback(() => {
    if (!newName.trim()) return;
    cmd.createSetup(newName.trim())
      .then(() => {
        setNewName("");
        setShowCreate(false);
        refresh();
      })
      .catch((e: unknown) => setError(String(e)));
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
    cmd.deleteSetup(deleteTarget.slug)
      .then(() => {
        refresh();
        if (activeSetupSlug === deleteTarget.slug) onCloseSetup();
      })
      .catch((e: unknown) => setError(String(e)));
    setDeleteTarget(null);
  }, [deleteTarget, refresh, activeSetupSlug, onCloseSetup]);

  const [showImportWizard, setShowImportWizard] = useState(false);

  const handleImportComplete = useCallback(
    (setupSlug: string) => {
      setShowImportWizard(false);
      refresh();
      onOpenSetup(setupSlug);
    },
    [refresh, onOpenSetup],
  );

  const handleSelectSetup = useCallback(
    (slug: string) => {
      onOpenSetup(slug);
    },
    [onOpenSetup],
  );

  return (
    <ScreenShell title="VibeLights">
      <TabBar
        activeTab={activeTab}
        onTabChange={onTabChange}
        activeSetupName={setup?.name ?? null}
        onCloseSetup={onCloseSetup}
      />

      {/* Error */}
      {error && (
        <div className="bg-error/10 border-error/20 text-error border-b px-6 py-2 text-xs">
          {error}
          <button onClick={() => setError(null)} className="ml-2 underline">
            dismiss
          </button>
        </div>
      )}

      <div className="flex-1 overflow-y-auto">
        {/* Setups tab */}
        {activeTab === "setups" && (
          <div className="p-6">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="text-text text-sm font-medium">Setups</h3>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowImportWizard(true)}
                  className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-3 py-1 text-xs transition-colors"
                >
                  Import from Vixen
                </button>
                <button
                  onClick={() => setShowCreate(true)}
                  className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
                >
                  New Setup
                </button>
              </div>
            </div>

            {showCreate && (
              <div className="border-border mb-6 flex items-center gap-3 rounded border px-4 py-3">
                <input
                  type="text"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleCreate()}
                  placeholder="Setup name (e.g. My House)"
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

            {setups.length === 0 && !showCreate ? (
              <div className="text-text-2 mt-20 text-center">
                <p className="text-lg">No setups yet</p>
                <p className="mt-2 text-sm">
                  Create a new setup or import from Vixen to get started.
                </p>
              </div>
            ) : (
              <div className="border-border divide-border divide-y rounded border">
                {setups.map((p) => (
                  <div
                    key={p.slug}
                    onClick={() => handleSelectSetup(p.slug)}
                    className={`hover:bg-surface-2 group flex cursor-pointer items-center gap-4 px-4 py-3 transition-colors ${
                      activeSetupSlug === p.slug ? "bg-primary/5 border-l-2 border-l-primary" : ""
                    }`}
                  >
                    <div className="min-w-0 flex-1">
                      <span className="text-text text-sm font-medium">{p.name}</span>
                      <div className="text-text-2 mt-0.5 flex gap-4 text-xs">
                        <span>
                          {p.fixture_count} fixture{p.fixture_count !== 1 ? "s" : ""}
                        </span>
                        <span>
                          {p.sequence_count} sequence{p.sequence_count !== 1 ? "s" : ""}
                        </span>
                        <span>{p.created_at.split("T")[0]}</span>
                      </div>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(p.slug, p.name);
                      }}
                      className="text-text-2 hover:text-error text-[10px] opacity-0 transition-all group-hover:opacity-100"
                    >
                      Delete
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Setup-dependent tabs */}
        {setup && activeSetupSlug && activeTab === "sequences" && (
          <SequencesTab slug={activeSetupSlug} onOpenSequence={onOpenSequence} setError={setError} />
        )}
        {setup && activeTab === "music" && (
          <MusicTab setError={setError} onOpenAnalysis={onOpenAnalysis} />
        )}
        {setup && activeTab === "house" && (
          <HouseSetupTab setup={setup} onSetupUpdate={setSetup} setError={setError} />
        )}
        {setup && activeTab === "layout" && (
          <LayoutTab setup={setup} onSetupUpdate={setSetup} setError={setError} />
        )}

        {/* Global tabs */}
        {activeTab === "effects" && <EffectsTab setError={setError} onOpenScript={onOpenScript} />}
        {activeTab === "gradients" && <GradientsTab setError={setError} />}
        {activeTab === "curves" && <CurvesTab setError={setError} />}
      </div>

      {showImportWizard && (
        <ImportWizard
          onComplete={handleImportComplete}
          onCancel={() => setShowImportWizard(false)}
        />
      )}

      {deleteTarget && (
        <ConfirmDialog
          title="Delete setup"
          message={`Delete setup "${deleteTarget.name}" and all its data? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </ScreenShell>
  );
}
