import { useState } from "react";
import type { FixtureDef, FixtureGroup, GroupMember } from "../../types";

interface Props {
  group: FixtureGroup | null; // null = create mode
  fixtures: FixtureDef[];
  groups: FixtureGroup[];
  onSave: (group: FixtureGroup) => void;
  onCancel: () => void;
  nextId: number;
}

function getMemberId(m: GroupMember): { type: "fixture" | "group"; id: number } {
  if ("Fixture" in m) return { type: "fixture", id: m.Fixture };
  return { type: "group", id: m.Group };
}

/** Collect all group IDs that are descendants of the given group (to prevent cycles). */
function getDescendantGroupIds(
  groupId: number,
  allGroups: FixtureGroup[],
  visited = new Set<number>(),
): Set<number> {
  if (visited.has(groupId)) return visited;
  visited.add(groupId);
  const group = allGroups.find((g) => g.id === groupId);
  if (!group) return visited;
  for (const m of group.members) {
    if ("Group" in m) {
      getDescendantGroupIds(m.Group, allGroups, visited);
    }
  }
  return visited;
}

export function GroupEditor({ group, fixtures, groups, onSave, onCancel, nextId }: Props) {
  const [name, setName] = useState(group?.name ?? "");
  const [members, setMembers] = useState<GroupMember[]>(group?.members ?? []);

  const editingId = group?.id ?? nextId;

  // Groups that would cause a cycle if added as members
  const forbiddenGroupIds = new Set<number>();
  forbiddenGroupIds.add(editingId);
  // Any group that contains us as a descendant is also forbidden
  for (const g of groups) {
    const descendants = getDescendantGroupIds(g.id, groups);
    if (descendants.has(editingId)) {
      forbiddenGroupIds.add(g.id);
    }
  }

  const isMember = (type: "fixture" | "group", id: number) =>
    members.some((m) => {
      const mid = getMemberId(m);
      return mid.type === type && mid.id === id;
    });

  const toggleMember = (type: "fixture" | "group", id: number) => {
    if (isMember(type, id)) {
      setMembers(members.filter((m) => {
        const mid = getMemberId(m);
        return !(mid.type === type && mid.id === id);
      }));
    } else {
      const newMember: GroupMember = type === "fixture" ? { Fixture: id } : { Group: id };
      setMembers([...members, newMember]);
    }
  };

  const handleSave = () => {
    if (!name.trim()) return;
    onSave({ id: editingId, name: name.trim(), members });
  };

  const availableFixtures = fixtures;
  const availableGroups = groups.filter((g) => !forbiddenGroupIds.has(g.id));

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-surface border-border w-[420px] rounded-lg border shadow-xl">
        <div className="border-border border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">
            {group ? "Edit Group" : "New Group"}
          </h3>
        </div>

        <div className="space-y-3 px-5 py-4">
          {/* Name */}
          <label className="block">
            <span className="text-text-2 mb-1 block text-xs">Name</span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Roofline"
              autoFocus
              className="border-border bg-surface-2 text-text placeholder:text-text-2 w-full rounded border px-3 py-1.5 text-sm outline-none focus:border-primary"
            />
          </label>

          {/* Member picker */}
          <div>
            <span className="text-text-2 mb-1 block text-xs">
              Members ({members.length})
            </span>
            <div className="border-border bg-surface-2 max-h-48 overflow-y-auto rounded border">
              {availableFixtures.length === 0 && availableGroups.length === 0 && (
                <p className="text-text-2 px-3 py-2 text-xs">No fixtures or groups available.</p>
              )}

              {availableFixtures.length > 0 && (
                <div className="border-border border-b px-3 py-1.5">
                  <span className="text-text-2 text-[10px] font-medium uppercase tracking-wider">Fixtures</span>
                </div>
              )}
              {availableFixtures.map((f) => (
                <label
                  key={`f-${f.id}`}
                  className="text-text hover:bg-surface flex cursor-pointer items-center gap-2 px-3 py-1.5 text-sm"
                >
                  <input
                    type="checkbox"
                    checked={isMember("fixture", f.id)}
                    onChange={() => toggleMember("fixture", f.id)}
                    className="accent-primary"
                  />
                  <span>{f.name}</span>
                  <span className="text-text-2 ml-auto text-xs">{f.pixel_count}px</span>
                </label>
              ))}

              {availableGroups.length > 0 && (
                <div className="border-border border-y  px-3 py-1.5">
                  <span className="text-text-2 text-[10px] font-medium uppercase tracking-wider">Groups</span>
                </div>
              )}
              {availableGroups.map((g) => (
                <label
                  key={`g-${g.id}`}
                  className="text-text hover:bg-surface flex cursor-pointer items-center gap-2 px-3 py-1.5 text-sm"
                >
                  <input
                    type="checkbox"
                    checked={isMember("group", g.id)}
                    onChange={() => toggleMember("group", g.id)}
                    className="accent-primary"
                  />
                  <span>{g.name}</span>
                  <span className="text-text-2 ml-auto text-xs">{g.members.length} members</span>
                </label>
              ))}
            </div>
          </div>
        </div>

        {/* Actions */}
        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          <button
            onClick={onCancel}
            className="text-text-2 hover:text-text rounded px-3 py-1.5 text-xs transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={!name.trim()}
            className="bg-primary hover:bg-primary-hover rounded px-4 py-1.5 text-xs font-medium text-white disabled:opacity-50"
          >
            {group ? "Save" : "Create"}
          </button>
        </div>
      </div>
    </div>
  );
}
