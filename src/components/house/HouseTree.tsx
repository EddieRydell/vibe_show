import { useMemo, useState } from "react";
import { ChevronDown, ChevronRight, Pencil, Trash2, Plus } from "lucide-react";
import type { FixtureDef, FixtureGroup, GroupMember } from "../../types";

interface Props {
  fixtures: FixtureDef[];
  groups: FixtureGroup[];
  onEditFixture: (fixture: FixtureDef) => void;
  onDeleteFixture: (id: number) => void;
  onEditGroup: (group: FixtureGroup) => void;
  onDeleteGroup: (id: number) => void;
  onAddFixture: () => void;
  onAddGroup: () => void;
}

export function HouseTree({
  fixtures,
  groups,
  onEditFixture,
  onDeleteFixture,
  onEditGroup,
  onDeleteGroup,
  onAddFixture,
  onAddGroup,
}: Props) {
  const [expanded, setExpanded] = useState<Set<number>>(new Set());

  const toggleExpand = (groupId: number) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) next.delete(groupId);
      else next.add(groupId);
      return next;
    });
  };

  // Fixtures that are members of at least one group
  const groupedFixtureIds = useMemo(() => {
    const ids = new Set<number>();
    for (const g of groups) {
      for (const m of g.members) {
        if ("Fixture" in m) ids.add(m.Fixture);
      }
    }
    return ids;
  }, [groups]);

  // Groups that are members of another group
  const nestedGroupIds = useMemo(() => {
    const ids = new Set<number>();
    for (const g of groups) {
      for (const m of g.members) {
        if ("Group" in m) ids.add(m.Group);
      }
    }
    return ids;
  }, [groups]);

  // Top-level groups (not nested inside any other group)
  const rootGroups = groups.filter((g) => !nestedGroupIds.has(g.id));

  // Ungrouped fixtures
  const ungroupedFixtures = fixtures.filter((f) => !groupedFixtureIds.has(f.id));

  const fixtureMap = useMemo(() => {
    const map = new Map<number, FixtureDef>();
    for (const f of fixtures) map.set(f.id, f);
    return map;
  }, [fixtures]);

  const groupMap = useMemo(() => {
    const map = new Map<number, FixtureGroup>();
    for (const g of groups) map.set(g.id, g);
    return map;
  }, [groups]);

  const renderMember = (member: GroupMember, depth: number) => {
    if ("Fixture" in member) {
      const fixture = fixtureMap.get(member.Fixture);
      if (!fixture) return null;
      return renderFixtureNode(fixture, depth);
    } else {
      const group = groupMap.get(member.Group);
      if (!group) return null;
      return renderGroupNode(group, depth);
    }
  };

  const renderFixtureNode = (fixture: FixtureDef, depth: number) => (
    <div
      key={`f-${fixture.id}`}
      className="group/node hover:bg-surface flex items-center gap-2 py-1 pr-2"
      style={{ paddingLeft: `${depth * 16 + 12}px` }}
    >
      <span className="bg-primary/20 text-primary rounded px-1.5 py-0.5 text-[9px] font-mono">FX</span>
      <span className="text-text flex-1 truncate text-sm">{fixture.name}</span>
      <span className="text-text-2 text-[10px]">
        {fixture.color_model} {fixture.pixel_count}px
        {fixture.bulb_shape && fixture.bulb_shape !== "LED" ? ` ${fixture.bulb_shape}` : ""}
      </span>
      <button
        onClick={() => onEditFixture(fixture)}
        className="text-text-2 hover:text-text opacity-0 transition-all group-hover/node:opacity-100"
        title="Edit"
      >
        <Pencil size={12} />
      </button>
      <button
        onClick={() => onDeleteFixture(fixture.id)}
        className="text-text-2 hover:text-error opacity-0 transition-all group-hover/node:opacity-100"
        title="Delete"
      >
        <Trash2 size={12} />
      </button>
    </div>
  );

  const renderGroupNode = (group: FixtureGroup, depth: number) => {
    const isExpanded = expanded.has(group.id);
    return (
      <div key={`g-${group.id}`}>
        <div
          className="group/node hover:bg-surface flex cursor-pointer items-center gap-2 py-1 pr-2"
          style={{ paddingLeft: `${depth * 16 + 4}px` }}
          onClick={() => toggleExpand(group.id)}
        >
          {isExpanded ? (
            <ChevronDown size={14} className="text-text-2 shrink-0" />
          ) : (
            <ChevronRight size={14} className="text-text-2 shrink-0" />
          )}
          <span className="bg-accent/20 text-accent rounded px-1.5 py-0.5 text-[9px] font-mono">GP</span>
          <span className="text-text flex-1 truncate text-sm font-medium">{group.name}</span>
          <span className="text-text-2 text-[10px]">
            {group.members.length} member{group.members.length !== 1 ? "s" : ""}
          </span>
          <button
            onClick={(e) => { e.stopPropagation(); onEditGroup(group); }}
            className="text-text-2 hover:text-text opacity-0 transition-all group-hover/node:opacity-100"
            title="Edit"
          >
            <Pencil size={12} />
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onDeleteGroup(group.id); }}
            className="text-text-2 hover:text-error opacity-0 transition-all group-hover/node:opacity-100"
            title="Delete"
          >
            <Trash2 size={12} />
          </button>
        </div>
        {isExpanded && group.members.map((m, i) => (
          <div key={i}>{renderMember(m, depth + 1)}</div>
        ))}
      </div>
    );
  };

  return (
    <div>
      {/* Actions */}
      <div className="mb-3 flex items-center gap-2">
        <button
          onClick={onAddFixture}
          className="bg-primary hover:bg-primary-hover flex items-center gap-1 rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          <Plus size={12} /> Fixture
        </button>
        <button
          onClick={onAddGroup}
          className="bg-surface-2 border-border text-text hover:bg-surface flex items-center gap-1 rounded border px-3 py-1 text-xs font-medium transition-colors"
        >
          <Plus size={12} /> Group
        </button>
      </div>

      {/* Tree */}
      <div className="border-border divide-border divide-y rounded border">
        {rootGroups.length === 0 && ungroupedFixtures.length === 0 && (
          <p className="text-text-2 px-4 py-6 text-center text-xs">
            No fixtures or groups. Add some to get started.
          </p>
        )}

        {rootGroups.map((g) => renderGroupNode(g, 0))}

        {ungroupedFixtures.length > 0 && rootGroups.length > 0 && (
          <div className="text-text-2 px-3 py-1.5 text-[10px] font-medium uppercase tracking-wider">
            Ungrouped
          </div>
        )}
        {ungroupedFixtures.map((f) => renderFixtureNode(f, 0))}
      </div>
    </div>
  );
}
