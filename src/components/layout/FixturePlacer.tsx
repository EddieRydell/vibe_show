import type { FixtureDef, FixtureLayout } from "../../types";
import { generatePositions } from "../../utils/layoutShapes";

interface Props {
  fixtures: FixtureDef[];
  layouts: FixtureLayout[];
  onPlace: (layout: FixtureLayout) => void;
}

export function FixturePlacer({ fixtures, layouts, onPlace }: Props) {
  const placedIds = new Set(layouts.map((l) => l.fixture_id));
  const unplaced = fixtures.filter((f) => !placedIds.has(f.id));

  if (unplaced.length === 0) {
    return (
      <div className="border-border bg-surface w-48 shrink-0 border-r p-3">
        <h4 className="text-text mb-2 text-xs font-medium">Fixtures</h4>
        <p className="text-text-2 text-[10px]">All fixtures placed.</p>
      </div>
    );
  }

  const handlePlace = (fixture: FixtureDef) => {
    // Place as a horizontal line in the middle by default
    const yPos = 0.3 + (layouts.length % 5) * 0.1;
    const shape = { Line: { start: { x: 0.1, y: yPos }, end: { x: 0.9, y: yPos } } } as const;
    const positions = generatePositions(shape, fixture.pixel_count) ?? [];
    onPlace({
      fixture_id: fixture.id,
      pixel_positions: positions,
      shape,
    });
  };

  return (
    <div className="border-border bg-surface w-48 shrink-0 overflow-y-auto border-r p-3">
      <h4 className="text-text mb-2 text-xs font-medium">
        Unplaced ({unplaced.length})
      </h4>
      <div className="space-y-1">
        {unplaced.map((f) => (
          <button
            key={f.id}
            onClick={() => handlePlace(f)}
            className="bg-surface-2 border-border text-text hover:border-primary w-full rounded border px-2 py-1.5 text-left text-xs transition-colors"
          >
            <div className="font-medium">{f.name}</div>
            <div className="text-text-2 text-[10px]">
              {f.pixel_count}px {f.bulb_shape && f.bulb_shape !== "LED" ? `(${f.bulb_shape})` : ""}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
