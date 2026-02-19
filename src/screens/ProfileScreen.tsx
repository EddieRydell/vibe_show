import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Settings } from "lucide-react";
import type {
  Profile,
  ShowSummary,
  MediaFile,
  EffectInfo,
  FixtureDef,
  FixtureGroup,
  FixtureLayout,
} from "../types";
import { HouseTree } from "../components/house/HouseTree";
import { FixtureEditor } from "../components/house/FixtureEditor";
import { GroupEditor } from "../components/house/GroupEditor";
import { LayoutCanvas } from "../components/layout/LayoutCanvas";
import { LayoutToolbar } from "../components/layout/LayoutToolbar";
import { ShapeConfigurator } from "../components/layout/ShapeConfigurator";
import { FixturePlacer } from "../components/layout/FixturePlacer";

type Tab = "shows" | "music" | "house" | "layout" | "effects";

interface Props {
  slug: string;
  onBack: () => void;
  onOpenShow: (showSlug: string) => void;
  onOpenSettings: () => void;
}

export function ProfileScreen({ slug, onBack, onOpenShow, onOpenSettings }: Props) {
  const [profile, setProfile] = useState<Profile | null>(null);
  const [tab, setTab] = useState<Tab>("shows");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<Profile>("open_profile", { slug })
      .then(setProfile)
      .catch((e) => setError(String(e)));
  }, [slug]);

  const tabs: { key: Tab; label: string }[] = [
    { key: "shows", label: "Shows" },
    { key: "music", label: "Music" },
    { key: "house", label: "House Setup" },
    { key: "layout", label: "Layout" },
    { key: "effects", label: "Effects" },
  ];

  return (
    <div className="bg-bg text-text flex h-screen flex-col">
      {/* Header */}
      <div className="border-border flex items-center border-b px-6 py-3">
        <button
          onClick={onBack}
          className="text-text-2 hover:text-text mr-3 text-sm transition-colors"
        >
          &larr; Home
        </button>
        <h2 className="text-text text-lg font-bold">{profile?.name ?? "Loading..."}</h2>
        <button
          onClick={onOpenSettings}
          className="text-text-2 hover:text-text ml-auto p-1.5 transition-colors"
          title="Settings"
        >
          <Settings size={16} />
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
        {profile && tab === "shows" && <ShowsTab onOpenShow={onOpenShow} setError={setError} />}
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

// ── Shows Tab ──────────────────────────────────────────────────────

function ShowsTab({
  onOpenShow,
  setError,
}: {
  onOpenShow: (slug: string) => void;
  setError: (e: string | null) => void;
}) {
  const [shows, setShows] = useState<ShowSummary[]>([]);
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);

  const refresh = useCallback(() => {
    invoke<ShowSummary[]>("list_shows")
      .then(setShows)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    if (!newName.trim()) return;
    invoke<ShowSummary>("create_show", { name: newName.trim() })
      .then(() => {
        setNewName("");
        setShowCreate(false);
        refresh();
      })
      .catch((e) => setError(String(e)));
  }, [newName, refresh, setError]);

  const handleDelete = useCallback(
    (showSlug: string, name: string) => {
      if (!confirm(`Delete show "${name}"?`)) return;
      invoke("delete_show", { slug: showSlug })
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">Shows</h3>
        <button
          onClick={() => setShowCreate(true)}
          className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          New Show
        </button>
      </div>

      {showCreate && (
        <div className="border-border bg-surface mb-4 flex items-center gap-3 rounded border p-3">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            placeholder="Show name"
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

      {shows.length === 0 && !showCreate ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          No shows yet. Create one to start sequencing.
        </p>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {shows.map((s) => (
            <div
              key={s.slug}
              onClick={() => onOpenShow(s.slug)}
              className="border-border bg-surface hover:border-primary group cursor-pointer rounded-lg border p-4 transition-colors"
            >
              <h4 className="text-text text-sm font-medium">{s.name}</h4>
              <p className="text-text-2 mt-1 text-xs">
                {s.sequence_count} sequence{s.sequence_count !== 1 ? "s" : ""}
              </p>
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

  const handleDelete = useCallback(
    (filename: string) => {
      if (!confirm(`Delete "${filename}"?`)) return;
      invoke("delete_media", { filename })
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

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
    if (!confirm("Delete this fixture?")) return;
    // Remove from fixtures
    setFixtures((prev) => prev.filter((f) => f.id !== id));
    // Remove from all group members
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
    if (!confirm("Delete this group?")) return;
    // Remove the group itself
    setGroups((prev) => {
      const without = prev.filter((g) => g.id !== id);
      // Also remove from other group members
      return without.map((g) => ({
        ...g,
        members: g.members.filter((m) => !("Group" in m && m.Group === id)),
      }));
    });
    setDirty(true);
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

