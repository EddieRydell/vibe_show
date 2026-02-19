import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Application, Graphics } from "pixi.js";
import type { Frame, Show } from "../types";
import { BULB_SHAPE_RADIUS } from "../types";

interface PreviewProps {
  show: Show | null;
  frame: Frame | null;
  collapsed: boolean;
  onToggle: () => void;
}

const BASE_RADIUS = 6;
const PADDING = 24;

export function Preview({ show, frame, collapsed, onToggle }: PreviewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<Application | null>(null);
  const graphicsRef = useRef<Graphics | null>(null);
  const destroyedRef = useRef(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    if (collapsed || !containerRef.current) return;

    destroyedRef.current = false;
    const app = new Application();

    app
      .init({
        resizeTo: containerRef.current,
        background: "#0E0E0E",
        antialias: true,
      })
      .then(() => {
        if (destroyedRef.current || !containerRef.current) {
          app.destroy(true, { children: true });
          return;
        }
        containerRef.current.appendChild(app.canvas);
        appRef.current = app;

        const graphics = new Graphics();
        app.stage.addChild(graphics);
        graphicsRef.current = graphics;
        setReady(true);
      });

    return () => {
      destroyedRef.current = true;
      graphicsRef.current = null;
      setReady(false);
      if (appRef.current) {
        appRef.current.destroy(true, { children: true });
        appRef.current = null;
      }
    };
  }, [collapsed]);

  // Build a map of fixture_id -> display radius multiplier
  const fixtureRadiusMap = useMemo(() => {
    if (!show) return new Map<number, number>();
    const map = new Map<number, number>();
    for (const f of show.fixtures) {
      const multiplier = f.display_radius_override ?? BULB_SHAPE_RADIUS[f.bulb_shape ?? "LED"] ?? 1.0;
      map.set(f.id, multiplier);
    }
    return map;
  }, [show]);

  const draw = useCallback(() => {
    const graphics = graphicsRef.current;
    const app = appRef.current;
    if (!graphics || !app || !show) return;

    graphics.clear();

    const width = app.screen.width - PADDING * 2;
    const height = app.screen.height - PADDING * 2;

    for (const fixtureLayout of show.layout.fixtures) {
      const fixtureId = fixtureLayout.fixture_id;
      const pixelColors = frame?.fixtures[fixtureId];
      const radiusMultiplier = fixtureRadiusMap.get(fixtureId) ?? 1.0;
      const pixelRadius = BASE_RADIUS * radiusMultiplier;

      for (let i = 0; i < fixtureLayout.pixel_positions.length; i++) {
        const pos = fixtureLayout.pixel_positions[i];
        const x = PADDING + pos.x * width;
        const y = PADDING + pos.y * height;

        let color = 0x000000;
        if (pixelColors?.[i]) {
          const [r, g, b] = pixelColors[i];
          color = (r << 16) | (g << 8) | b;
        }

        graphics.circle(x, y, pixelRadius + 1);
        graphics.fill({ color: 0x1D1D1D });

        graphics.circle(x, y, pixelRadius);
        graphics.fill({ color });

        if (color !== 0x000000) {
          graphics.circle(x, y, pixelRadius * 2.5);
          graphics.fill({ color, alpha: 0.15 });
        }
      }
    }
  }, [show, frame, fixtureRadiusMap]);

  useEffect(() => {
    if (ready) draw();
  }, [ready, draw]);

  return (
    <div className="border-border bg-bg flex flex-col border-t">
      <button
        onClick={onToggle}
        className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text flex items-center gap-1.5 border-b px-3 py-1 text-left text-[11px] tracking-wider uppercase"
      >
        <span className="text-[8px]">{collapsed ? "\u25B6" : "\u25BC"}</span>
        Preview
      </button>

      {!collapsed && <div ref={containerRef} className="h-48 overflow-hidden" />}
    </div>
  );
}
