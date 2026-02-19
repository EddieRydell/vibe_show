import type { GroupMember, Show } from "../types";

/** Recursively collect fixture IDs from group members. */
function resolveMembers(members: GroupMember[], allGroups: Show["groups"], visited: Set<number>): number[] {
  const ids: number[] = [];
  for (const m of members) {
    if ("Fixture" in m) {
      ids.push((m as { Fixture: number }).Fixture);
    } else if ("Group" in m) {
      const gid = (m as { Group: number }).Group;
      if (!visited.has(gid)) {
        visited.add(gid);
        const sub = allGroups.find((g) => g.id === gid);
        if (sub) ids.push(...resolveMembers(sub.members, allGroups, visited));
      }
    }
  }
  return ids;
}

interface FixtureListProps {
  show: Show | null;
}

export function FixtureList({ show }: FixtureListProps) {
  if (!show) return null;

  // Build a map: fixture_id → group names it belongs to.
  const fixtureGroups = new Map<number, string[]>();
  for (const group of show.groups) {
    const fids = resolveMembers(group.members, show.groups, new Set([group.id]));
    for (const fid of fids) {
      const existing = fixtureGroups.get(fid) ?? [];
      existing.push(group.name);
      fixtureGroups.set(fid, existing);
    }
  }

  return (
    <div className="border-border bg-surface flex w-52 shrink-0 flex-col overflow-y-auto border-r">
      {/* Groups section */}
      {show.groups.length > 0 && (
        <div className="border-border border-b p-3">
          <h3 className="text-text-2 mb-2 text-[10px] tracking-wider uppercase">Groups</h3>
          {show.groups.map((group) => (
            <div
              key={group.id}
              className="border-border bg-surface-2 hover:border-text-2/30 mb-1 rounded border p-2 transition-colors"
            >
              <div className="text-text text-xs font-medium">{group.name}</div>
              <div className="text-text-2 mt-0.5 text-[10px]">
                {group.members
                  .map((m) => {
                    if ("Fixture" in m) {
                      const fid = (m as { Fixture: number }).Fixture;
                      return show.fixtures.find((f) => f.id === fid)?.name ?? `#${fid}`;
                    }
                    const gid = (m as { Group: number }).Group;
                    return show.groups.find((g) => g.id === gid)?.name ?? `Group #${gid}`;
                  })
                  .join(", ")}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Fixtures section */}
      <div className="p-3">
        <h3 className="text-text-2 mb-2 text-[10px] tracking-wider uppercase">Fixtures</h3>
        {show.fixtures.map((fixture) => {
          const groups = fixtureGroups.get(fixture.id);
          return (
            <div
              key={fixture.id}
              className="border-border bg-surface-2 hover:border-text-2/30 mb-1 rounded border p-2 transition-colors"
            >
              <div className="flex items-baseline justify-between">
                <span className="text-text text-xs font-medium">{fixture.name}</span>
                <span className="text-text-2 text-[10px]">{fixture.pixel_count}px</span>
              </div>
              <div className="text-text-2 mt-0.5 flex items-center gap-1.5 text-[10px]">
                <span>{fixture.color_model}</span>
                {groups && (
                  <>
                    <span className="text-border">&middot;</span>
                    <span className="text-text-2 truncate">{groups.join(", ")}</span>
                  </>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
