import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { cmd } from "../../commands";
import type { MediaFile } from "../../types";
import { ConfirmDialog } from "../../components/ConfirmDialog";

interface Props {
  setError: (e: string | null) => void;
  onOpenAnalysis: (filename: string) => void;
}

export function MusicTab({ setError, onOpenAnalysis }: Props) {
  const [files, setFiles] = useState<MediaFile[]>([]);

  const refresh = useCallback(() => {
    cmd.listMedia()
      .then(setFiles)
      .catch((e: unknown) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleImport = useCallback(async () => {
    const selected = await open({
      title: "Import Audio File",
      filters: [
        { name: "Audio Files", extensions: ["mp3", "wav", "ogg", "flac", "m4a", "aac"] },
      ],
    });
    if (!selected) return;
    cmd.importMedia(selected)
      .then(() => refresh())
      .catch((e: unknown) => setError(String(e)));
  }, [refresh, setError]);

  const [deleteFilename, setDeleteFilename] = useState<string | null>(null);

  const handleDelete = useCallback(
    (filename: string) => {
      setDeleteFilename(filename);
    },
    [],
  );

  const confirmDelete = useCallback(() => {
    if (!deleteFilename) return;
    cmd.deleteMedia(deleteFilename)
      .then(refresh)
      .catch((e: unknown) => setError(String(e)));
    setDeleteFilename(null);
  }, [deleteFilename, refresh, setError]);

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">Music / Audio</h3>
        <button
          onClick={() => { void handleImport(); }}
          className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          Import Audio
        </button>
      </div>

      {files.length === 0 ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          No audio files. Import music to use in your shows.
        </p>
      ) : (
        <div className="border-border divide-border divide-y rounded border">
          {files.map((f) => (
            <div
              key={f.filename}
              className="group flex items-center justify-between px-4 py-2.5"
            >
              <div>
                <span className="text-text text-sm">{f.filename}</span>
                <span className="text-text-2 ml-3 text-xs">{formatSize(f.size_bytes)}</span>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => onOpenAnalysis(f.filename)}
                  className="text-text-2 hover:text-primary text-xs opacity-0 transition-all group-hover:opacity-100"
                >
                  Analyze
                </button>
                <button
                  onClick={() => handleDelete(f.filename)}
                  className="text-text-2 hover:text-error text-xs opacity-0 transition-all group-hover:opacity-100"
                >
                  Delete
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {deleteFilename && (
        <ConfirmDialog
          title="Delete file"
          message={`Delete "${deleteFilename}"? This cannot be undone.`}
          confirmLabel="Delete"
          destructive
          onConfirm={confirmDelete}
          onCancel={() => setDeleteFilename(null)}
        />
      )}
    </div>
  );
}
