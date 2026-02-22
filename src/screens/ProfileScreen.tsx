import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { cmd } from "../commands";
import type {
  Profile,
  SequenceSummary,
  MediaFile,
  EffectInfo,
  FixtureDef,
  FixtureGroup,
  FixtureLayout,
  ColorGradient,
  ColorStop,
  Curve,
  CurvePoint,
} from "../types";
import { ScreenShell } from "../components/ScreenShell";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { HouseTree } from "../components/house/HouseTree";
import { FixtureEditor } from "../components/house/FixtureEditor";
import { GroupEditor } from "../components/house/GroupEditor";
import { LayoutCanvas } from "../components/layout/LayoutCanvas";
import { LayoutToolbar } from "../components/layout/LayoutToolbar";
import { ShapeConfigurator } from "../components/layout/ShapeConfigurator";
import { FixturePlacer } from "../components/layout/FixturePlacer";
import { GradientEditor } from "../components/controls/GradientEditor";
import { CurveEditor } from "../components/controls/CurveEditor";


type Tab = "sequences" | "music" | "house" | "layout" | "effects" | "gradients" | "curves";

interface Props {
  slug: string;
  onBack: () => void;
  onOpenSequence: (sequenceSlug: string) => void;
  onOpenScript: (name: string | null) => void;
}

export function ProfileScreen({ slug, onBack, onOpenSequence, onOpenScript }: Props) {
  const [profile, setProfile] = useState<Profile | null>(null);
  const [tab, setTab] = useState<Tab>("sequences");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    cmd.openProfile(slug)
      .then(setProfile)
      .catch((e) => setError(String(e)));
  }, [slug]);

  const tabs: { key: Tab; label: string }[] = [
    { key: "sequences", label: "Sequences" },
    { key: "music", label: "Music" },
    { key: "house", label: "House Setup" },
    { key: "layout", label: "Layout" },
    { key: "effects", label: "Effects" },
    { key: "gradients", label: "Gradients" },
    { key: "curves", label: "Curves" },
  ];

  return (
    <ScreenShell title={profile?.name ?? "Loading..."} onBack={onBack} backLabel="Home">
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
        {profile && tab === "effects" && <EffectsTab setError={setError} onOpenScript={onOpenScript} />}
        {profile && tab === "gradients" && <GradientsTab setError={setError} />}
        {profile && tab === "curves" && <CurvesTab setError={setError} />}
      </div>
    </ScreenShell>
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
    cmd.listMedia()
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
    cmd.importMedia(selected)
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
    cmd.deleteMedia(deleteFilename)
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
      await cmd.updateProfileFixtures(fixtures, groups);
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
      await cmd.updateProfileLayout(layout);
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

// ── Effects Tab ────────────────────────────────────────────────────

function EffectsTab({ setError, onOpenScript }: { setError: (e: string | null) => void; onOpenScript: (name: string | null) => void }) {
  const [effects, setEffects] = useState<EffectInfo[]>([]);
  const [scripts, setScripts] = useState<[string, string][]>([]);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const refreshEffects = useCallback(() => {
    cmd.listEffects().then(setEffects).catch(console.error);
  }, []);

  const refreshScripts = useCallback(() => {
    cmd.listProfileScripts()
      .then(setScripts)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refreshEffects, [refreshEffects]);
  useEffect(refreshScripts, [refreshScripts]);

  const handleDeleteScript = useCallback(() => {
    if (!deleteTarget) return;
    cmd.deleteProfileScript(deleteTarget)
      .then(refreshScripts)
      .catch((e) => setError(String(e)));
    setDeleteTarget(null);
  }, [deleteTarget, refreshScripts, setError]);

  return (
    <div className="p-6 space-y-8">
      {/* Built-in effects */}
      <section>
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
                    <div key={String(param.key)} className="flex justify-between">
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
      </section>

      {/* Custom scripts */}
      <section>
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-text text-sm font-medium">Custom Scripts</h3>
          <div className="flex items-center gap-2">
            <button
              onClick={() => onOpenScript(null)}
              className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-3 py-1 text-xs transition-colors"
            >
              Open Script Studio
            </button>
            <button
              onClick={() => onOpenScript(null)}
              className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
            >
              New Script
            </button>
          </div>
        </div>

        {scripts.length === 0 ? (
          <p className="text-text-2 text-center text-sm">
            No custom scripts yet. Create one to define custom effects.
          </p>
        ) : (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {scripts.map(([name, source]) => (
              <div
                key={name}
                onClick={() => onOpenScript(name)}
                className="border-border bg-surface hover:border-primary group cursor-pointer rounded-lg border p-4 transition-colors"
              >
                <h4 className="text-text text-sm font-medium">{name}</h4>
                <span className="text-text-2 text-xs">{source.split("\n").length} lines</span>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setDeleteTarget(name);
                  }}
                  className="text-text-2 hover:text-error ml-2 text-[10px] opacity-0 transition-all group-hover:opacity-100"
                >
                  Delete
                </button>
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

// ── Gradients Tab ──────────────────────────────────────────────────

function GradientsTab({ setError }: { setError: (e: string | null) => void }) {
  const [gradients, setGradients] = useState<[string, ColorGradient][]>([]);
  const [expandedName, setExpandedName] = useState<string | null>(null);
  const [renamingName, setRenamingName] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const refresh = useCallback(() => {
    cmd.listProfileGradients()
      .then(setGradients)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    const existingNames = gradients.map(([n]) => n);
    let idx = 1;
    while (existingNames.includes(`Gradient ${idx}`)) idx++;
    const name = `Gradient ${idx}`;
    const defaultGradient: ColorGradient = {
      stops: [
        { position: 0, color: { r: 255, g: 0, b: 0, a: 255 } },
        { position: 1, color: { r: 0, g: 0, b: 255, a: 255 } },
      ],
    };
    cmd.setProfileGradient(name, defaultGradient)
      .then(() => {
        refresh();
        setExpandedName(name);
      })
      .catch((e) => setError(String(e)));
  }, [gradients, refresh, setError]);

  const handleUpdate = useCallback(
    (name: string, stops: ColorStop[]) => {
      const gradient: ColorGradient = { stops };
      cmd.setProfileGradient(name, gradient)
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  const handleRename = useCallback(
    (oldName: string) => {
      if (!renameValue.trim() || renameValue === oldName) {
        setRenamingName(null);
        return;
      }
      cmd.renameProfileGradient(oldName, renameValue.trim())
        .then(() => {
          if (expandedName === oldName) setExpandedName(renameValue.trim());
          setRenamingName(null);
          refresh();
        })
        .catch((e) => setError(String(e)));
    },
    [renameValue, expandedName, refresh, setError],
  );

  const confirmDelete = useCallback(() => {
    if (!deleteTarget) return;
    cmd.deleteProfileGradient(deleteTarget)
      .then(() => {
        if (expandedName === deleteTarget) setExpandedName(null);
        refresh();
      })
      .catch((e) => setError(String(e)));
    setDeleteTarget(null);
  }, [deleteTarget, expandedName, refresh, setError]);

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">Gradients</h3>
        <button
          onClick={handleCreate}
          className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          New Gradient
        </button>
      </div>

      {gradients.length === 0 ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          No gradients yet. Create one to use in your effects.
        </p>
      ) : (
        <div className="space-y-3">
          {gradients.map(([name, gradient]) => (
            <div
              key={name}
              className="border-border bg-surface rounded-lg border"
            >
              {/* Card header */}
              <div
                className="flex cursor-pointer items-center gap-3 px-4 py-3"
                onClick={() => setExpandedName(expandedName === name ? null : name)}
              >
                {/* Gradient preview bar */}
                <GradientPreview stops={gradient.stops} className="h-4 w-20 rounded" />

                {/* Name (double-click to rename) */}
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

                <span className="text-text-2 text-xs">{gradient.stops.length} stops</span>

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

              {/* Expanded editor */}
              {expandedName === name && (
                <div className="border-border border-t px-4 py-3">
                  <GradientEditor
                    label="Edit Gradient"
                    value={gradient.stops}
                    minStops={1}
                    maxStops={10}
                    onChange={(stops) => handleUpdate(name, stops)}
                  />
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {deleteTarget && (
        <ConfirmDialog
          title="Delete gradient"
          message={`Delete gradient "${deleteTarget}"? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}

/** Tiny inline gradient preview using CSS linear-gradient. */
function GradientPreview({ stops, className }: { stops: ColorStop[]; className?: string }) {
  const sorted = [...stops].sort((a, b) => a.position - b.position);
  const gradientCSS = sorted
    .map((s) => `rgba(${s.color.r},${s.color.g},${s.color.b},${s.color.a / 255}) ${s.position * 100}%`)
    .join(", ");
  return (
    <div
      className={`border-border border ${className ?? ""}`}
      style={{ background: `linear-gradient(to right, ${gradientCSS})` }}
    />
  );
}

// ── Curves Tab ─────────────────────────────────────────────────────

function CurvesTab({ setError }: { setError: (e: string | null) => void }) {
  const [curves, setCurves] = useState<[string, Curve][]>([]);
  const [expandedName, setExpandedName] = useState<string | null>(null);
  const [renamingName, setRenamingName] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const refresh = useCallback(() => {
    cmd.listProfileCurves()
      .then(setCurves)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    const existingNames = curves.map(([n]) => n);
    let idx = 1;
    while (existingNames.includes(`Curve ${idx}`)) idx++;
    const name = `Curve ${idx}`;
    const defaultCurve: Curve = {
      points: [
        { x: 0, y: 0 },
        { x: 1, y: 1 },
      ],
    };
    cmd.setProfileCurve(name, defaultCurve)
      .then(() => {
        refresh();
        setExpandedName(name);
      })
      .catch((e) => setError(String(e)));
  }, [curves, refresh, setError]);

  const handleUpdate = useCallback(
    (name: string, points: CurvePoint[]) => {
      const curve: Curve = { points };
      cmd.setProfileCurve(name, curve)
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  const handleRename = useCallback(
    (oldName: string) => {
      if (!renameValue.trim() || renameValue === oldName) {
        setRenamingName(null);
        return;
      }
      cmd.renameProfileCurve(oldName, renameValue.trim())
        .then(() => {
          if (expandedName === oldName) setExpandedName(renameValue.trim());
          setRenamingName(null);
          refresh();
        })
        .catch((e) => setError(String(e)));
    },
    [renameValue, expandedName, refresh, setError],
  );

  const confirmDelete = useCallback(() => {
    if (!deleteTarget) return;
    cmd.deleteProfileCurve(deleteTarget)
      .then(() => {
        if (expandedName === deleteTarget) setExpandedName(null);
        refresh();
      })
      .catch((e) => setError(String(e)));
    setDeleteTarget(null);
  }, [deleteTarget, expandedName, refresh, setError]);

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">Curves</h3>
        <button
          onClick={handleCreate}
          className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          New Curve
        </button>
      </div>

      {curves.length === 0 ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          No curves yet. Create one to use in your effects.
        </p>
      ) : (
        <div className="space-y-3">
          {curves.map(([name, curve]) => (
            <div
              key={name}
              className="border-border bg-surface rounded-lg border"
            >
              {/* Card header */}
              <div
                className="flex cursor-pointer items-center gap-3 px-4 py-3"
                onClick={() => setExpandedName(expandedName === name ? null : name)}
              >
                {/* Name (double-click to rename) */}
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

                <span className="text-text-2 text-xs">{curve.points.length} points</span>

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

              {/* Expanded editor */}
              {expandedName === name && (
                <div className="border-border border-t px-4 py-3">
                  <CurveEditor
                    label="Edit Curve"
                    value={curve.points}
                    onChange={(points) => handleUpdate(name, points)}
                  />
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {deleteTarget && (
        <ConfirmDialog
          title="Delete curve"
          message={`Delete curve "${deleteTarget}"? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}

