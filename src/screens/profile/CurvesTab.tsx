import { useCallback, useEffect, useState } from "react";
import { cmd } from "../../commands";
import type { Curve, CurvePoint } from "../../types";
import { CollapsibleListEditor } from "../../components/CollapsibleListEditor";
import { CurveEditor } from "../../components/controls/CurveEditor";

interface Props {
  setError: (e: string | null) => void;
}

export function CurvesTab({ setError }: Props) {
  const [curves, setCurves] = useState<[string, Curve][]>([]);

  const refresh = useCallback(() => {
    cmd.listProfileCurves()
      .then(setCurves)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    const existingNames = curves.map(([n]) => n);
    let idx = 1;
    while (existingNames.includes(`Curve ${idx}`)) idx++;
    const name = `Curve ${idx}`;
    const defaultCurve: Curve = {
      points: [
        { x: 0, y: 0 },
        { x: 1, y: 1 },
      ],
    };
    cmd.setProfileCurve(name, defaultCurve)
      .then(refresh)
      .catch((e) => setError(String(e)));
  }, [curves, refresh, setError]);

  const handleUpdate = useCallback(
    (name: string, curve: Curve) => {
      cmd.setProfileCurve(name, curve)
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  const handleRename = useCallback(
    (oldName: string, newName: string) => {
      cmd.renameProfileCurve(oldName, newName)
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  const handleDelete = useCallback(
    (name: string) => {
      cmd.deleteProfileCurve(name)
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  return (
    <CollapsibleListEditor
      items={curves}
      itemLabel="curve"
      countLabel={(c) => `${c.points.length} points`}
      onCreate={handleCreate}
      onUpdate={handleUpdate}
      onRename={handleRename}
      onDelete={handleDelete}
      renderEditor={(name, c) => (
        <CurveEditor
          label="Edit Curve"
          value={c.points}
          onChange={(points: CurvePoint[]) => handleUpdate(name, { points })}
        />
      )}
    />
  );
}
