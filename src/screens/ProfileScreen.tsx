import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  Profile,
  SequenceSummary,
  MediaFile,
  EffectInfo,
  FixtureDef,
  FixtureGroup,
  FixtureLayout,
} from "../types";
import { Settings } from "lucide-react";
import { AppBar } from "../components/AppBar";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { HouseTree } from "../components/house/HouseTree";
import { FixtureEditor } from "../components/house/FixtureEditor";
import { GroupEditor } from "../components/house/GroupEditor";
import { LayoutCanvas } from "../components/layout/LayoutCanvas";
import { LayoutToolbar } from "../components/layout/LayoutToolbar";
import { ShapeConfigurator } from "../components/layout/ShapeConfigurator";
import { FixturePlacer } from "../components/layout/FixturePlacer";

type Tab = "sequences" | "music" | "house" | "layout" | "effects";

interface Props {
  slug: string;
  onBack: () => void;
  onOpenSequence: (sequenceSlug: string) => void;
  onOpenSettings: () => void;
}

export function ProfileScreen({ slug, onBack, onOpenSequence, onOpenSettings }: Props) {
  const [profile, setProfile] = useState<Profile | null>(null);
  const [tab, setTab] = useState<Tab>("sequences");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<Profile>("open_profile", { slug })
      .then(setProfile)
      .catch((e) => setError(String(e)));
  }, [slug]);

  const tabs: { key: Tab; label: string }[] = [
    { key: "sequences", label: "Sequences" },
    { key: "music", label: "Music" },
    { key: "house", label: "House Setup" },
    { key: "layout", label: "Layout" },
    { key: "effects", label: "Effects" },
  ];

  return (
    <div className="bg-bg text-text flex h-screen flex-col">
      {/* Title bar */}
      <AppBar />

      {/* Screen toolbar */}
      <div className="border-border bg-surface flex select-none items-center gap-2 border-b px-4 py-1.5">
        <button
          onClick={onBack}
          className="text-text-2 hover:text-text mr-1 text-sm transition-colors"
        >
          &larr; Home
        </button>
        <h2 className="text-text text-sm font-bold">{profile?.name ?? "Loading..."}</h2>
        <div className="flex-1" />
        <button
          onClick={onOpenSettings}
          className="text-text-2 hover:text-text p-1 transition-colors"
          title="Settings"
        >
          <Settings size={14} />
        </button>
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

      {/* Tab bar */}
      <div className="border-border flex gap-0 border-b">
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`px-5 py-2 text-xs font-medium transition-colors ${
              tab === t.key
                ? "border-primary text-primary border-b-2"
                : "text-text-2 hover:text-text border-b-2 border-transparent"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto">
        {profile && tab === "sequences" && <SequencesTab slug={slug} onOpenSequence={onOpenSequence} setError={setError} />}
        {profile && tab === "music" && <MusicTab setError={setError} />}
        {profile && tab === "house" && (
          <HouseSetupTab profile={profile} onProfileUpdate={setProfile} setError={setError} />
        )}
        {profile && tab === "layout" && (
          <LayoutTab profile={profile} onProfileUpdate={setProfile} setError={setError} />
        )}
        {profile && tab === "effects" && <EffectsTab />}
      </div>
    </div>
  );
}

// ── Sequences Tab ──────────────────────────────────────────────────

function SequencesTab({
  slug,
  onOpenSequence,
  setError,
}: {
  slug: string;
  onOpenSequence: (slug: string) => void;
  setError: (e: string | null) => void;
}) {
  const [sequences, setSequences] = useState<SequenceSummary[]>([]);
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [importingVixen, setImportingVixen] = useState(false);

  const refresh = useCallback(() => {
    invoke<SequenceSummary[]>("list_sequences")
      .then(setSequences)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    if (!newName.trim()) return;
    invoke<SequenceSummary>("create_sequence", { name: newName.trim() })
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
    invoke("delete_sequence", { slug: deleteTarget.slug })
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
          await invoke("import_vixen_sequence", {
            profileSlug: slug,
            timPath,
          });
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
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {sequences.map((s) => (
            <div
              key={s.slug}
              onClick={() => onOpenSequence(s.slug)}
              className="border-border bg-surface hover:border-primary group cursor-pointer rounded-lg border p-4 transition-colors"
            >
              <h4 className="text-text text-sm font-medium">{s.name}</h4>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleDelete(s.slug, s.name);
                }}
                className="text-text-2 hover:text-error mt-2 text-[10px] opacity-0 transition-all group-hover:opacity-100"
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

// ── Music Tab ──────────────────────────────────────────────────────

function MusicTab({ setError }: { setError: (e: string | null) => void }) {
  const [files, setFiles] = useState<MediaFile[]>([]);

  const refresh = useCallback(() => {
    invoke<MediaFile[]>("list_media")
      .then(setFiles)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleImport = useCallback(async () => {
    const selected = await open({
      title: "Import Audio File",
      filters: [
        { name: "Audio Files", extensions: ["mp3", "wav", "ogg", "flac", "m4a", "aac"] },
      ],
    });
    if (!selected) return;
    invoke<MediaFile>("import_media", { sourcePath: selected })
      .then(() => refresh())
      .catch((e) => setError(String(e)));
  }, [refresh, setError]);

  const [deleteFilename, setDeleteFilename] = useState<string | null>(null);

  const handleDelete = useCallback(
    (filename: string) => {
      setDeleteFilename(filename);
    },
    [],
  );

  const confirmDelete = useCallback(() => {
    if (!deleteFilename) return;
    invoke("delete_media", { filename: deleteFilename })
      .then(refresh)
      .catch((e) => setError(String(e)));
    setDeleteFilename(null);
  }, [deleteFilename, refresh, setError]);

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">Music / Audio</h3>
        <button
          onClick={handleImport}
          className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          Import Audio
        </button>
      </div>

      {files.length === 0 ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          No audio files. Import music to use in your shows.
        </p>
      ) : (
        <div className="border-border divide-border divide-y rounded border">
          {files.map((f) => (
            <div
              key={f.filename}
              className="group flex items-center justify-between px-4 py-2.5"
            >
              <div>
                <span className="text-text text-sm">{f.filename}</span>
                <span className="text-text-2 ml-3 text-xs">{formatSize(f.size_bytes)}</span>
              </div>
              <button
                onClick={() => handleDelete(f.filename)}
                className="text-text-2 hover:text-error text-xs opacity-0 transition-all group-hover:opacity-100"
              >
                Delete
              </button>
            </div>
          ))}
        </div>
      )}

      {deleteFilename && (
        <ConfirmDialog
          title="Delete file"
          message={`Delete "${deleteFilename}"? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDelete}
          onCancel={() => setDeleteFilename(null)}
        />
      )}
    </div>
  );
}

// ── House Setup Tab ─────────────────────────────────────────────────

function HouseSetupTab({
  profile,
  onProfileUpdate,
  setError,
}: {
  profile: Profile;
  onProfileUpdate: (p: Profile) => void;
  setError: (e: string | null) => void;
}) {
  const [fixtures, setFixtures] = useState<FixtureDef[]>(profile.fixtures);
  const [groups, setGroups] = useState<FixtureGroup[]>(profile.groups);
  const [dirty, setDirty] = useState(false);
  const [editingFixture, setEditingFixture] = useState<FixtureDef | null | "new">(null);
  const [editingGroup, setEditingGroup] = useState<FixtureGroup | null | "new">(null);
  const [deleteConfirm, setDeleteConfirm] = useState<{ kind: "fixture" | "group"; id: number } | null>(null);

  // Sync with profile when it changes externally
  useEffect(() => {
    setFixtures(profile.fixtures);
    setGroups(profile.groups);
    setDirty(false);
  }, [profile]);

  const nextFixtureId = Math.max(0, ...fixtures.map((f) => f.id)) + 1;
  const nextGroupId = Math.max(0, ...groups.map((g) => g.id)) + 1;

  const handleSaveFixture = (fixture: FixtureDef) => {
    const exists = fixtures.some((f) => f.id === fixture.id);
    const updated = exists
      ? fixtures.map((f) => (f.id === fixture.id ? fixture : f))
      : [...fixtures, fixture];
    setFixtures(updated);
    setEditingFixture(null);
    setDirty(true);
  };

  const handleDeleteFixture = (id: number) => {
    setDeleteConfirm({ kind: "fixture", id });
  };

  const doDeleteFixture = (id: number) => {
    setFixtures((prev) => prev.filter((f) => f.id !== id));
    setGroups((prev) =>
      prev.map((g) => ({
        ...g,
        members: g.members.filter((m) => !("Fixture" in m && m.Fixture === id)),
      })),
    );
    setDirty(true);
  };

  const handleSaveGroup = (group: FixtureGroup) => {
    const exists = groups.some((g) => g.id === group.id);
    const updated = exists
      ? groups.map((g) => (g.id === group.id ? group : g))
      : [...groups, group];
    setGroups(updated);
    setEditingGroup(null);
    setDirty(true);
  };

  const handleDeleteGroup = (id: number) => {
    setDeleteConfirm({ kind: "group", id });
  };

  const doDeleteGroup = (id: number) => {
    setGroups((prev) => {
      const without = prev.filter((g) => g.id !== id);
      return without.map((g) => ({
        ...g,
        members: g.members.filter((m) => !("Group" in m && m.Group === id)),
      }));
    });
    setDirty(true);
  };

  const confirmDeleteItem = () => {
    if (!deleteConfirm) return;
    if (deleteConfirm.kind === "fixture") doDeleteFixture(deleteConfirm.id);
    else doDeleteGroup(deleteConfirm.id);
    setDeleteConfirm(null);
  };

  const handleSave = async () => {
    try {
      await invoke("update_profile_fixtures", { fixtures, groups });
      const updated = { ...profile, fixtures, groups };
      onProfileUpdate(updated);
      setDirty(false);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="p-6 space-y-6">
      {/* Save bar */}
      {dirty && (
        <div className="bg-primary/10 border-primary/20 flex items-center justify-between rounded border px-4 py-2">
          <span className="text-primary text-xs font-medium">Unsaved changes</span>
          <button
            onClick={handleSave}
            className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white"
          >
            Save
          </button>
        </div>
      )}

      {/* Fixtures & Groups tree */}
      <section>
        <h3 className="text-text mb-3 text-sm font-medium">
          Fixtures & Groups
        </h3>
        <HouseTree
          fixtures={fixtures}
          groups={groups}
          onEditFixture={(f) => setEditingFixture(f)}
          onDeleteFixture={handleDeleteFixture}
          onEditGroup={(g) => setEditingGroup(g)}
          onDeleteGroup={handleDeleteGroup}
          onAddFixture={() => setEditingFixture("new")}
          onAddGroup={() => setEditingGroup("new")}
        />
      </section>

      {/* Controllers (read-only for now) */}
      <section>
        <h3 className="text-text mb-3 text-sm font-medium">
          Controllers ({profile.controllers.length})
        </h3>
        {profile.controllers.length === 0 ? (
          <p className="text-text-2 text-xs">No controllers configured.</p>
        ) : (
          <div className="border-border divide-border divide-y rounded border">
            {profile.controllers.map((c) => (
              <div key={c.id} className="flex items-center justify-between px-4 py-2">
                <span className="text-text text-sm">{c.name}</span>
                <span className="text-text-2 text-xs">
                  {"E131" in c.protocol
                    ? "E1.31"
                    : "ArtNet" in c.protocol
                      ? "ArtNet"
                      : "Serial"}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Patches (read-only for now) */}
      <section>
        <h3 className="text-text mb-3 text-sm font-medium">
          Patches ({profile.patches.length})
        </h3>
        {profile.patches.length === 0 ? (
          <p className="text-text-2 text-xs">No patches configured.</p>
        ) : (
          <div className="border-border divide-border divide-y rounded border">
            {profile.patches.map((p, i) => (
              <div key={i} className="flex items-center justify-between px-4 py-2">
                <span className="text-text text-sm">Fixture {p.fixture_id}</span>
                <span className="text-text-2 text-xs">
                  {"Dmx" in p.output
                    ? `DMX ${p.output.Dmx.universe}/${p.output.Dmx.start_address}`
                    : `Port ${p.output.PixelPort.port}`}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Fixture editor modal */}
      {editingFixture !== null && (
        <FixtureEditor
          fixture={editingFixture === "new" ? null : editingFixture}
          onSave={handleSaveFixture}
          onCancel={() => setEditingFixture(null)}
          nextId={nextFixtureId}
        />
      )}

      {/* Group editor modal */}
      {editingGroup !== null && (
        <GroupEditor
          group={editingGroup === "new" ? null : editingGroup}
          fixtures={fixtures}
          groups={groups}
          onSave={handleSaveGroup}
          onCancel={() => setEditingGroup(null)}
          nextId={nextGroupId}
        />
      )}

      {deleteConfirm && (
        <ConfirmDialog
          title={deleteConfirm.kind === "fixture" ? "Delete fixture" : "Delete group"}
          message={`Delete this ${deleteConfirm.kind}? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDeleteItem}
          onCancel={() => setDeleteConfirm(null)}
        />
      )}
    </div>
  );
}

// ── Layout Tab ─────────────────────────────────────────────────────

function LayoutTab({
  profile,
  onProfileUpdate,
  setError,
}: {
  profile: Profile;
  onProfileUpdate: (p: Profile) => void;
  setError: (e: string | null) => void;
}) {
  const [layouts, setLayouts] = useState<FixtureLayout[]>(profile.layout.fixtures);
  const [selectedFixtureId, setSelectedFixtureId] = useState<number | null>(null);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    setLayouts(profile.layout.fixtures);
    setDirty(false);
  }, [profile]);

  const handleLayoutChange = (updated: FixtureLayout[]) => {
    setLayouts(updated);
    setDirty(true);
  };

  const handlePlace = (layout: FixtureLayout) => {
    setLayouts((prev) => [...prev, layout]);
    setSelectedFixtureId(layout.fixture_id);
    setDirty(true);
  };

  const handleSave = async () => {
    try {
      const layout = { fixtures: layouts };
      await invoke("update_profile_layout", { layout });
      onProfileUpdate({ ...profile, layout });
      setDirty(false);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex h-full flex-col">
      {/* Save bar */}
      {dirty && (
        <div className="bg-primary/10 border-primary/20 flex items-center justify-between border-b px-4 py-2">
          <span className="text-primary text-xs font-medium">Unsaved layout changes</span>
          <button
            onClick={handleSave}
            className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white"
          >
            Save
          </button>
        </div>
      )}

      {/* Layout toolbar */}
      <LayoutToolbar
        selectedFixtureId={selectedFixtureId}
        fixtures={profile.fixtures}
        layouts={layouts}
        onLayoutChange={handleLayoutChange}
      />

      {/* Main content: placer + canvas + shape config */}
      <div className="flex flex-1 overflow-hidden">
        <FixturePlacer
          fixtures={profile.fixtures}
          layouts={layouts}
          onPlace={handlePlace}
        />

        <div className="flex-1 overflow-hidden">
          <LayoutCanvas
            layouts={layouts}
            fixtures={profile.fixtures}
            selectedFixtureId={selectedFixtureId}
            onLayoutChange={handleLayoutChange}
            onSelectFixture={setSelectedFixtureId}
          />
        </div>

        <ShapeConfigurator
          selectedFixtureId={selectedFixtureId}
          fixtures={profile.fixtures}
          layouts={layouts}
          onLayoutChange={handleLayoutChange}
        />
      </div>
    </div>
  );
}

// ── Effects Tab (Read-only) ────────────────────────────────────────

function EffectsTab() {
  const [effects, setEffects] = useState<EffectInfo[]>([]);

  useEffect(() => {
    invoke<EffectInfo[]>("list_effects")
      .then(setEffects)
      .catch(console.error);
  }, []);

  return (
    <div className="p-6">
      <h3 className="text-text mb-4 text-sm font-medium">Built-in Effects</h3>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {effects.map((fx) => (
          <div
            key={fx.name}
            className="border-border bg-surface rounded-lg border p-4"
          >
            <h4 className="text-text mb-2 text-sm font-medium">{fx.name}</h4>
            {fx.schema.length > 0 && (
              <div className="text-text-2 space-y-1 text-xs">
                {fx.schema.map((param) => (
                  <div key={param.key} className="flex justify-between">
                    <span>{param.label}</span>
                    <span className="text-text-2">
                      {typeof param.param_type === "string"
                        ? param.param_type
                        : Object.keys(param.param_type)[0]}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

