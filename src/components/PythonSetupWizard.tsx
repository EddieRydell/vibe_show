import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import type { PythonEnvStatus, ProgressEvent } from "../types";

interface PythonSetupWizardProps {
  pythonStatus: PythonEnvStatus | null;
  onSetup: () => Promise<void>;
  onCheckStatus: () => Promise<PythonEnvStatus>;
  onClose: () => void;
}

export function PythonSetupWizard({
  pythonStatus,
  onSetup,
  onCheckStatus,
  onClose,
}: PythonSetupWizardProps) {
  const [phase, setPhase] = useState<"check" | "install" | "done" | "error">(
    "check",
  );
  const [progress, setProgress] = useState(0);
  const [progressMessage, setProgressMessage] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    onCheckStatus();
  }, [onCheckStatus]);

  useEffect(() => {
    const unlisten = listen<ProgressEvent>("progress", (event) => {
      if (event.payload.operation === "python_setup") {
        setProgress(event.payload.progress);
        setProgressMessage(event.payload.phase);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Check if already ready
  useEffect(() => {
    if (pythonStatus?.deps_installed) {
      setPhase("done");
    }
  }, [pythonStatus]);

  const handleInstall = useCallback(async () => {
    setPhase("install");
    setProgress(0);
    setProgressMessage("Starting setup...");
    try {
      await onSetup();
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }, [onSetup]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape" && phase !== "install") onClose();
    },
    [onClose, phase],
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={handleKeyDown}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && phase !== "install") onClose();
      }}
    >
      <div className="bg-surface border-border w-[480px] rounded-lg border shadow-xl">
        <div className="border-border border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">
            Audio Analysis Setup
          </h3>
        </div>

        <div className="px-5 py-4">
          {phase === "check" && (
            <div className="space-y-3">
              <p className="text-text-2 text-sm">
                Audio analysis requires a Python environment with ML libraries.
                This is a one-time setup (~2-5 GB download).
              </p>
              <div className="space-y-2 text-xs">
                <StatusRow
                  label="Python runtime (uv)"
                  ok={pythonStatus?.uv_available ?? false}
                />
                <StatusRow
                  label="Python 3.12"
                  ok={pythonStatus?.python_installed ?? false}
                />
                <StatusRow
                  label="Virtual environment"
                  ok={pythonStatus?.venv_exists ?? false}
                />
                <StatusRow
                  label="ML dependencies"
                  ok={pythonStatus?.deps_installed ?? false}
                />
                {pythonStatus?.gpu_available && (
                  <StatusRow label="GPU acceleration" ok />
                )}
              </div>
            </div>
          )}

          {phase === "install" && (
            <div className="space-y-3">
              <p className="text-text-2 text-sm">{progressMessage}</p>
              <div className="bg-bg border-border h-2 overflow-hidden rounded-full border">
                <div
                  className="bg-primary h-full transition-all duration-300"
                  style={{ width: `${Math.round(progress * 100)}%` }}
                />
              </div>
              <p className="text-text-2 text-xs">
                {Math.round(progress * 100)}% — Do not close this window
              </p>
            </div>
          )}

          {phase === "done" && (
            <div className="space-y-2">
              <p className="text-sm text-green-500">
                Python environment is ready.
              </p>
              <p className="text-text-2 text-xs">
                You can now analyze audio files.
              </p>
            </div>
          )}

          {phase === "error" && (
            <div className="space-y-2">
              <p className="text-sm text-red-500">Setup failed</p>
              <p className="text-text-2 break-all text-xs">{error}</p>
            </div>
          )}
        </div>

        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          {phase === "check" && (
            <>
              <button
                onClick={onClose}
                className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-3 py-1.5 text-xs transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleInstall}
                className="border-primary bg-primary hover:bg-primary-hover rounded border px-3 py-1.5 text-xs font-medium text-white transition-colors"
              >
                Install
              </button>
            </>
          )}
          {(phase === "done" || phase === "error") && (
            <button
              onClick={onClose}
              className="border-primary bg-primary hover:bg-primary-hover rounded border px-3 py-1.5 text-xs font-medium text-white transition-colors"
            >
              Close
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function StatusRow({ label, ok }: { label: string; ok: boolean }) {
  return (
    <div className="flex items-center gap-2">
      <span className={ok ? "text-green-500" : "text-text-2"}>
        {ok ? "\u2713" : "\u2717"}
      </span>
      <span className="text-text">{label}</span>
    </div>
  );
}
