import { useCallback, useEffect, useRef, useState } from "react";
import type { FixtureDef, FixtureLayout, Position2D } from "../../types";
import { cssMouseOffset } from "../../utils/cssZoom";

// Distinct fixture identity colors for the editor
const FIXTURE_COLORS = [
  "#4A9EFF", "#FF6B6B", "#51CF66", "#FFD43B", "#CC5DE8",
  "#FF922B", "#20C997", "#F06595", "#74C0FC", "#A9E34B",
];

interface Props {
  layouts: FixtureLayout[];
  fixtures: FixtureDef[];
  selectedFixtureId: number | null;
  onLayoutChange: (layouts: FixtureLayout[]) => void;
  onSelectFixture: (id: number | null) => void;
}

const PADDING = 30;
const PIXEL_RADIUS = 5;
const HIT_RADIUS = 10;

export function LayoutCanvas({
  layouts,
  fixtures,
  selectedFixtureId,
  onLayoutChange,
  onSelectFixture,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 800, height: 500 });
  const [dragging, setDragging] = useState<{
    fixtureId: number;
    startMouse: Position2D;
    startPositions: Position2D[];
  } | null>(null);

  // Observe container size
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        setSize({ width: Math.floor(width), height: Math.floor(height) });
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  // Convert normalized (0-1) coords to canvas pixels
  const toCanvas = useCallback(
    (pos: Position2D) => ({
      x: PADDING + pos.x * (size.width - PADDING * 2),
      y: PADDING + pos.y * (size.height - PADDING * 2),
    }),
    [size],
  );

  // Draw
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = size.width * dpr;
    canvas.height = size.height * dpr;
    ctx.scale(dpr, dpr);

    // Background
    ctx.fillStyle = "#0E0E0E";
    ctx.fillRect(0, 0, size.width, size.height);

    // Grid at 10% intervals
    ctx.strokeStyle = "#1A1A1A";
    ctx.lineWidth = 1;
    for (let i = 0; i <= 10; i++) {
      const frac = i / 10;
      const x = PADDING + frac * (size.width - PADDING * 2);
      const y = PADDING + frac * (size.height - PADDING * 2);
      ctx.beginPath();
      ctx.moveTo(x, PADDING);
      ctx.lineTo(x, size.height - PADDING);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(PADDING, y);
      ctx.lineTo(size.width - PADDING, y);
      ctx.stroke();
    }

    // Border
    ctx.strokeStyle = "#2A2A2A";
    ctx.lineWidth = 1;
    ctx.strokeRect(PADDING, PADDING, size.width - PADDING * 2, size.height - PADDING * 2);

    // Draw fixtures
    for (let fi = 0; fi < layouts.length; fi++) {
      const layout = layouts[fi]!;
      const isSelected = layout.fixture_id === selectedFixtureId;
      const color = FIXTURE_COLORS[fi % FIXTURE_COLORS.length]!;

      for (const pos of layout.pixel_positions) {
        const { x, y } = toCanvas(pos);

        // Glow for selected
        if (isSelected) {
          ctx.beginPath();
          ctx.arc(x, y, PIXEL_RADIUS * 2.5, 0, Math.PI * 2);
          ctx.fillStyle = color + "30";
          ctx.fill();
        }

        // Pixel dot
        ctx.beginPath();
        ctx.arc(x, y, PIXEL_RADIUS, 0, Math.PI * 2);
        ctx.fillStyle = isSelected ? color : color + "AA";
        ctx.fill();

        // Outline for selected
        if (isSelected) {
          ctx.strokeStyle = "#FFFFFF55";
          ctx.lineWidth = 1;
          ctx.stroke();
        }
      }

      // Label for selected fixture
      if (isSelected && layout.pixel_positions.length > 0) {
        const firstPos = toCanvas(layout.pixel_positions[0]!);
        const fixture = fixtures.find((f) => f.id === layout.fixture_id);
        if (fixture) {
          ctx.font = "11px sans-serif";
          ctx.fillStyle = "#FFFFFF";
          ctx.fillText(fixture.name, firstPos.x + PIXEL_RADIUS + 4, firstPos.y + 4);
        }
      }
    }
  }, [layouts, fixtures, selectedFixtureId, size, toCanvas]);

  // Find fixture at canvas coordinates
  const hitTest = useCallback(
    (cx: number, cy: number): number | null => {
      // Check in reverse order (top-most first)
      for (let fi = layouts.length - 1; fi >= 0; fi--) {
        const layout = layouts[fi]!;
        for (const pos of layout.pixel_positions) {
          const { x, y } = toCanvas(pos);
          const dx = cx - x;
          const dy = cy - y;
          if (dx * dx + dy * dy <= HIT_RADIUS * HIT_RADIUS) {
            return layout.fixture_id;
          }
        }
      }
      return null;
    },
    [layouts, toCanvas],
  );

  const getCanvasCoords = (e: React.MouseEvent) => {
    const canvas = canvasRef.current;
    if (!canvas) return { cx: 0, cy: 0 };
    const { x, y } = cssMouseOffset(e, canvas);
    return { cx: x, cy: y };
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    const { cx, cy } = getCanvasCoords(e);
    const hitId = hitTest(cx, cy);

    if (hitId != null) {
      onSelectFixture(hitId);
      const layout = layouts.find((l) => l.fixture_id === hitId);
      if (layout) {
        setDragging({
          fixtureId: hitId,
          startMouse: { x: cx, y: cy },
          startPositions: layout.pixel_positions.map((p) => ({ ...p })),
        });
      }
    } else {
      onSelectFixture(null);
    }
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!dragging) return;
    const { cx, cy } = getCanvasCoords(e);
    const dx = cx - dragging.startMouse.x;
    const dy = cy - dragging.startMouse.y;
    const normDx = dx / (size.width - PADDING * 2);
    const normDy = dy / (size.height - PADDING * 2);

    const updated = layouts.map((l) => {
      if (l.fixture_id !== dragging.fixtureId) return l;
      return {
        ...l,
        pixel_positions: dragging.startPositions.map((p) => ({
          x: Math.max(0, Math.min(1, p.x + normDx)),
          y: Math.max(0, Math.min(1, p.y + normDy)),
        })),
      };
    });
    onLayoutChange(updated);
  };

  const handleMouseUp = () => {
    setDragging(null);
  };

  return (
    <div ref={containerRef} className="size-full ">
      <canvas
        ref={canvasRef}
        style={{ width: size.width, height: size.height, cursor: dragging ? "grabbing" : "default" }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
      />
    </div>
  );
}
