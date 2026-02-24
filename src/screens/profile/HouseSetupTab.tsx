import { useEffect, useState } from "react";
import { cmd } from "../../commands";
import type { Profile, FixtureDef, FixtureGroup } from "../../types";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { HouseTree } from "../../components/house/HouseTree";
import { FixtureEditor } from "../../components/house/FixtureEditor";
import { GroupEditor } from "../../components/house/GroupEditor";

interface Props {
  profile: Profile;
  onProfileUpdate: (p: Profile) => void;
  setError: (e: string | null) => void;
}

export function HouseSetupTab({ profile, onProfileUpdate, setError }: Props) {
  const [fixtures, setFixtures] = useState<FixtureDef[]>(profile.fixtures);
  const [groups, setGroups] = useState<FixtureGroup[]>(profile.groups);
  const [dirty, setDirty] = useState(false);
  const [editingFixture, setEditingFixture] = useState<FixtureDef | null | "new">(null);
  const [editingGroup, setEditingGroup] = useState<FixtureGroup | null | "new">(null);
  const [deleteConfirm, setDeleteConfirm] = useState<{ kind: "fixture" | "group"; id: number } | null>(null);

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

      {editingFixture !== null && (
        <FixtureEditor
          fixture={editingFixture === "new" ? null : editingFixture}
          onSave={handleSaveFixture}
          onCancel={() => setEditingFixture(null)}
          nextId={nextFixtureId}
        />
      )}

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
