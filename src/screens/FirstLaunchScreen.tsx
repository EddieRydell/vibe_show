import { useCallback, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  onComplete: () => void;
}

export function FirstLaunchScreen({ onComplete }: Props) {
  const [parentDir, setParentDir] = useState<string | null>(null);
  const [folderName, setFolderName] = useState("VibeLights");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleChooseParent = useCallback(async () => {
    const selected = await open({
      directory: true,
      title: "Choose where to create your VibeLights folder",
    });
    if (selected) {
      setParentDir(selected);
      setError(null);
    }
  }, []);

  const handleCreate = useCallback(async () => {
    if (!parentDir || !folderName.trim()) return;

    setLoading(true);
    setError(null);
    try {
      const dataDir = parentDir.replace(/[\\/]+$/, "") + "/" + folderName.trim();
      await invoke("initialize_data_dir", { dataDir });
      onComplete();
    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  }, [parentDir, folderName, onComplete]);

  return (
    <div className="bg-bg flex h-screen items-center justify-center">
      <div className="max-w-lg text-center">
        <h1 className="text-text text-4xl font-bold">Vibe Lights</h1>
        <p className="text-text mt-6 text-sm">
          Welcome! Pick a location and name for your Vibe Lights data folder. All profiles, shows,
          and media will be stored inside it.
        </p>

        {/* Step 1: Choose parent directory */}
        <div className="mt-6">
          <button
            onClick={handleChooseParent}
            disabled={loading}
            className="border-border bg-surface text-text hover:bg-surface-2 rounded border px-4 py-2 text-sm transition-colors disabled:opacity-50"
          >
            {parentDir ? "Change Location" : "Choose Location"}
          </button>
          {parentDir && (
            <p className="text-text-2 mt-2 text-xs">
              Location: <span className="text-text-2">{parentDir}</span>
            </p>
          )}
        </div>

        {/* Step 2: Name the folder */}
        {parentDir && (
          <div className="mt-4">
            <label className="text-text-2 mb-1 block text-xs">Folder name</label>
            <input
              type="text"
              value={folderName}
              onChange={(e) => setFolderName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              className="border-border bg-surface text-text w-64 rounded border px-3 py-1.5 text-center text-sm outline-none focus:border-primary"
            />
            <p className="text-text-2 mt-1.5 text-[11px]">
              Will create:{" "}
              <span className="text-text-2">
                {parentDir.replace(/[\\/]+$/, "")}/{folderName.trim() || "..."}
              </span>
            </p>

            <button
              onClick={handleCreate}
              disabled={loading || !folderName.trim()}
              className="bg-primary hover:bg-primary-hover mt-4 rounded-lg px-6 py-2.5 text-sm font-medium text-white transition-colors disabled:opacity-50"
            >
              {loading ? "Setting up..." : "Create & Continue"}
            </button>
          </div>
        )}

        {error && <p className="text-error mt-4 text-sm">{error}</p>}
      </div>
    </div>
  );
}
