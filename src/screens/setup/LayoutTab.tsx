import { useEffect, useState } from "react";
import { cmd } from "../../commands";
import type { Setup, FixtureLayout } from "../../types";
import { LayoutCanvas } from "../../components/layout/LayoutCanvas";
import { LayoutToolbar } from "../../components/layout/LayoutToolbar";
import { ShapeConfigurator } from "../../components/layout/ShapeConfigurator";
import { FixturePlacer } from "../../components/layout/FixturePlacer";

interface Props {
  setup: Setup;
  onSetupUpdate: (s: Setup) => void;
  setError: (e: string | null) => void;
}

export function LayoutTab({ setup, onSetupUpdate, setError }: Props) {
  const [layouts, setLayouts] = useState<FixtureLayout[]>(setup.layout.fixtures);
  const [selectedFixtureId, setSelectedFixtureId] = useState<number | null>(null);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    setLayouts(setup.layout.fixtures);
    setDirty(false);
  }, [setup]);

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
      await cmd.updateSetupLayout(layout);
      onSetupUpdate({ ...setup, layout });
      setDirty(false);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex h-full flex-col">
      {dirty && (
        <div className="bg-primary/10 border-primary/20 flex items-center justify-between border-b px-4 py-2">
          <span className="text-primary text-xs font-medium">Unsaved layout changes</span>
          <button
            onClick={() => { void handleSave(); }}
            className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white"
          >
            Save
          </button>
        </div>
      )}

      <LayoutToolbar
        selectedFixtureId={selectedFixtureId}
        fixtures={setup.fixtures}
        layouts={layouts}
        onLayoutChange={handleLayoutChange}
      />

      <div className="flex flex-1 overflow-hidden">
        <FixturePlacer
          fixtures={setup.fixtures}
          layouts={layouts}
          onPlace={handlePlace}
        />

        <div className="flex-1 overflow-hidden">
          <LayoutCanvas
            layouts={layouts}
            fixtures={setup.fixtures}
            selectedFixtureId={selectedFixtureId}
            onLayoutChange={handleLayoutChange}
            onSelectFixture={setSelectedFixtureId}
          />
        </div>

        <ShapeConfigurator
          selectedFixtureId={selectedFixtureId}
          fixtures={setup.fixtures}
          layouts={layouts}
          onLayoutChange={handleLayoutChange}
        />
      </div>
    </div>
  );
}
