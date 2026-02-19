import type { FixtureDef, FixtureLayout, LayoutShape } from "../../types";
import { generatePositions } from "../../utils/layoutShapes";

interface Props {
  selectedFixtureId: number | null;
  fixtures: FixtureDef[];
  layouts: FixtureLayout[];
  onLayoutChange: (layouts: FixtureLayout[]) => void;
}

type ShapeType = "Custom" | "Line" | "Arc" | "Rectangle" | "Grid";

function getShapeType(shape?: LayoutShape): ShapeType {
  if (!shape || shape === "Custom") return "Custom";
  if ("Line" in shape) return "Line";
  if ("Arc" in shape) return "Arc";
  if ("Rectangle" in shape) return "Rectangle";
  if ("Grid" in shape) return "Grid";
  return "Custom";
}

function makeDefaultShape(type: ShapeType): LayoutShape {
  switch (type) {
    case "Line":
      return { Line: { start: { x: 0.1, y: 0.5 }, end: { x: 0.9, y: 0.5 } } };
    case "Arc":
      return { Arc: { center: { x: 0.5, y: 0.5 }, radius: 0.3, start_angle: 0, end_angle: Math.PI } };
    case "Rectangle":
      return { Rectangle: { top_left: { x: 0.2, y: 0.2 }, bottom_right: { x: 0.8, y: 0.8 } } };
    case "Grid":
      return { Grid: { top_left: { x: 0.1, y: 0.1 }, bottom_right: { x: 0.9, y: 0.9 }, columns: 10 } };
    case "Custom":
    default:
      return "Custom";
  }
}

const SHAPE_TYPES: ShapeType[] = ["Custom", "Line", "Arc", "Rectangle", "Grid"];

export function LayoutToolbar({ selectedFixtureId, fixtures, layouts, onLayoutChange }: Props) {
  const selectedLayout = layouts.find((l) => l.fixture_id === selectedFixtureId);
  const selectedFixture = fixtures.find((f) => f.id === selectedFixtureId);
  const currentShapeType = getShapeType(selectedLayout?.shape);

  const handleShapeChange = (newType: ShapeType) => {
    if (!selectedFixtureId || !selectedFixture) return;
    const newShape = makeDefaultShape(newType);
    const positions = newType !== "Custom"
      ? generatePositions(newShape, selectedFixture.pixel_count) ?? selectedLayout?.pixel_positions ?? []
      : selectedLayout?.pixel_positions ?? [];

    onLayoutChange(
      layouts.map((l) =>
        l.fixture_id === selectedFixtureId
          ? { ...l, shape: newShape, pixel_positions: positions }
          : l,
      ),
    );
  };

  const handleAutoDistribute = () => {
    if (!selectedLayout || !selectedFixture) return;
    const shape = selectedLayout.shape ?? "Custom";
    if (shape === "Custom") return;
    const positions = generatePositions(shape, selectedFixture.pixel_count);
    if (!positions) return;

    onLayoutChange(
      layouts.map((l) =>
        l.fixture_id === selectedFixtureId
          ? { ...l, pixel_positions: positions }
          : l,
      ),
    );
  };

  return (
    <div className="border-border bg-surface flex items-center gap-3 border-b px-4 py-2">
      {selectedFixture ? (
        <>
          <span className="text-text text-xs font-medium">{selectedFixture.name}</span>
          <span className="text-text-2 text-[10px]">{selectedFixture.pixel_count}px</span>

          <div className="bg-border mx-1 h-4 w-px" />

          <label className="flex items-center gap-1.5">
            <span className="text-text-2 text-[10px]">Shape</span>
            <select
              value={currentShapeType}
              onChange={(e) => handleShapeChange(e.target.value as ShapeType)}
              className="border-border bg-surface-2 text-text rounded border px-2 py-0.5 text-xs outline-none focus:border-primary"
            >
              {SHAPE_TYPES.map((s) => (
                <option key={s} value={s}>{s}</option>
              ))}
            </select>
          </label>

          {currentShapeType !== "Custom" && (
            <button
              onClick={handleAutoDistribute}
              className="bg-surface-2 border-border text-text-2 hover:text-text rounded border px-2 py-0.5 text-xs transition-colors"
            >
              Auto-distribute
            </button>
          )}
        </>
      ) : (
        <span className="text-text-2 text-xs">Select a fixture to edit its layout</span>
      )}
    </div>
  );
}
