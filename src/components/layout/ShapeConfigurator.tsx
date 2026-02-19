import type { FixtureDef, FixtureLayout, LayoutShape, Position2D } from "../../types";
import { generatePositions } from "../../utils/layoutShapes";

interface Props {
  selectedFixtureId: number | null;
  fixtures: FixtureDef[];
  layouts: FixtureLayout[];
  onLayoutChange: (layouts: FixtureLayout[]) => void;
}

export function ShapeConfigurator({ selectedFixtureId, fixtures, layouts, onLayoutChange }: Props) {
  const selectedLayout = layouts.find((l) => l.fixture_id === selectedFixtureId);
  const selectedFixture = fixtures.find((f) => f.id === selectedFixtureId);
  const shape = selectedLayout?.shape;

  if (!selectedLayout || !selectedFixture || !shape || shape === "Custom") {
    return null;
  }

  const updateShapeAndPositions = (newShape: LayoutShape) => {
    const positions = generatePositions(newShape, selectedFixture.pixel_count) ?? selectedLayout.pixel_positions;
    onLayoutChange(
      layouts.map((l) =>
        l.fixture_id === selectedFixtureId
          ? { ...l, shape: newShape, pixel_positions: positions }
          : l,
      ),
    );
  };

  const posInput = (label: string, pos: Position2D, onChange: (p: Position2D) => void) => (
    <div className="flex gap-2">
      <label className="flex-1">
        <span className="text-text-2 text-[10px]">{label} X</span>
        <input
          type="number"
          step={0.01}
          min={0}
          max={1}
          value={pos.x}
          onChange={(e) => onChange({ ...pos, x: parseFloat(e.target.value) || 0 })}
          className="border-border bg-surface-2 text-text w-full rounded border px-2 py-1 text-xs outline-none focus:border-primary"
        />
      </label>
      <label className="flex-1">
        <span className="text-text-2 text-[10px]">{label} Y</span>
        <input
          type="number"
          step={0.01}
          min={0}
          max={1}
          value={pos.y}
          onChange={(e) => onChange({ ...pos, y: parseFloat(e.target.value) || 0 })}
          className="border-border bg-surface-2 text-text w-full rounded border px-2 py-1 text-xs outline-none focus:border-primary"
        />
      </label>
    </div>
  );

  const numInput = (label: string, value: number, onChange: (v: number) => void, opts?: { min?: number; max?: number; step?: number }) => (
    <label className="block">
      <span className="text-text-2 text-[10px]">{label}</span>
      <input
        type="number"
        step={opts?.step ?? 0.01}
        min={opts?.min}
        max={opts?.max}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value) || 0)}
        className="border-border bg-surface-2 text-text w-full rounded border px-2 py-1 text-xs outline-none focus:border-primary"
      />
    </label>
  );

  return (
    <div className="border-border bg-surface w-56 shrink-0 space-y-3 overflow-y-auto border-l p-3">
      <h4 className="text-text text-xs font-medium">Shape Parameters</h4>

      {"Line" in shape && (
        <div className="space-y-2">
          {posInput("Start", shape.Line.start, (start) =>
            updateShapeAndPositions({ Line: { ...shape.Line, start } }),
          )}
          {posInput("End", shape.Line.end, (end) =>
            updateShapeAndPositions({ Line: { ...shape.Line, end } }),
          )}
        </div>
      )}

      {"Arc" in shape && (
        <div className="space-y-2">
          {posInput("Center", shape.Arc.center, (center) =>
            updateShapeAndPositions({ Arc: { ...shape.Arc, center } }),
          )}
          {numInput("Radius", shape.Arc.radius, (radius) =>
            updateShapeAndPositions({ Arc: { ...shape.Arc, radius } }),
          )}
          {numInput("Start Angle", shape.Arc.start_angle, (start_angle) =>
            updateShapeAndPositions({ Arc: { ...shape.Arc, start_angle } }),
            { step: 0.1 },
          )}
          {numInput("End Angle", shape.Arc.end_angle, (end_angle) =>
            updateShapeAndPositions({ Arc: { ...shape.Arc, end_angle } }),
            { step: 0.1 },
          )}
        </div>
      )}

      {"Rectangle" in shape && (
        <div className="space-y-2">
          {posInput("Top Left", shape.Rectangle.top_left, (top_left) =>
            updateShapeAndPositions({ Rectangle: { ...shape.Rectangle, top_left } }),
          )}
          {posInput("Bottom Right", shape.Rectangle.bottom_right, (bottom_right) =>
            updateShapeAndPositions({ Rectangle: { ...shape.Rectangle, bottom_right } }),
          )}
        </div>
      )}

      {"Grid" in shape && (
        <div className="space-y-2">
          {posInput("Top Left", shape.Grid.top_left, (top_left) =>
            updateShapeAndPositions({ Grid: { ...shape.Grid, top_left } }),
          )}
          {posInput("Bottom Right", shape.Grid.bottom_right, (bottom_right) =>
            updateShapeAndPositions({ Grid: { ...shape.Grid, bottom_right } }),
          )}
          {numInput("Columns", shape.Grid.columns, (columns) =>
            updateShapeAndPositions({ Grid: { ...shape.Grid, columns: Math.max(1, Math.round(columns)) } }),
            { step: 1, min: 1 },
          )}
        </div>
      )}
    </div>
  );
}
