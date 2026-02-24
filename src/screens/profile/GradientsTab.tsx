import { useCallback, useEffect, useState } from "react";
import { cmd } from "../../commands";
import type { ColorGradient, ColorStop } from "../../types";
import { CollapsibleListEditor } from "../../components/CollapsibleListEditor";
import { GradientEditor } from "../../components/controls/GradientEditor";

interface Props {
  setError: (e: string | null) => void;
}

function GradientPreview({ stops, className }: { stops: ColorStop[]; className?: string }) {
  const sorted = [...stops].sort((a, b) => a.position - b.position);
  const gradientCSS = sorted
    .map((s) => `rgba(${s.color.r},${s.color.g},${s.color.b},${s.color.a / 255}) ${s.position * 100}%`)
    .join(", ");
  return (
    <div
      className={`border-border border ${className ?? ""}`}
      style={{ background: `linear-gradient(to right, ${gradientCSS})` }}
    />
  );
}

export function GradientsTab({ setError }: Props) {
  const [gradients, setGradients] = useState<[string, ColorGradient][]>([]);

  const refresh = useCallback(() => {
    cmd.listProfileGradients()
      .then(setGradients)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    const existingNames = gradients.map(([n]) => n);
    let idx = 1;
    while (existingNames.includes(`Gradient ${idx}`)) idx++;
    const name = `Gradient ${idx}`;
    const defaultGradient: ColorGradient = {
      stops: [
        { position: 0, color: { r: 255, g: 0, b: 0, a: 255 } },
        { position: 1, color: { r: 0, g: 0, b: 255, a: 255 } },
      ],
    };
    cmd.setProfileGradient(name, defaultGradient)
      .then(refresh)
      .catch((e) => setError(String(e)));
  }, [gradients, refresh, setError]);

  const handleUpdate = useCallback(
    (name: string, gradient: ColorGradient) => {
      cmd.setProfileGradient(name, gradient)
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  const handleRename = useCallback(
    (oldName: string, newName: string) => {
      cmd.renameProfileGradient(oldName, newName)
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  const handleDelete = useCallback(
    (name: string) => {
      cmd.deleteProfileGradient(name)
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  return (
    <CollapsibleListEditor
      items={gradients}
      itemLabel="gradient"
      countLabel={(g) => `${g.stops.length} stops`}
      onCreate={handleCreate}
      onUpdate={handleUpdate}
      onRename={handleRename}
      onDelete={handleDelete}
      renderPreview={(_name, g) => (
        <GradientPreview stops={g.stops} className="h-3 w-16 rounded-sm" />
      )}
      renderEditor={(name, g) => (
        <GradientEditor
          label="Edit Gradient"
          value={g.stops}
          minStops={1}
          maxStops={10}
          onChange={(stops) => handleUpdate(name, { stops })}
        />
      )}
    />
  );
}
