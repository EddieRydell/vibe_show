import { useState, useEffect, useCallback } from "react";
import { cmd } from "../commands";
import { PROGRESS } from "../events";
import type { PythonEnvStatus, ProgressEvent } from "../types";
import { useTauriListener } from "../hooks/useTauriListener";

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
    void onCheckStatus();
  }, [onCheckStatus]);

  useTauriListener<ProgressEvent>(PROGRESS, (p) => {
    if (p.operation === "python_setup") {
      setProgress(p.progress);
      setProgressMessage(p.phase);
    }
  });

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

  const handleCancelSetup = useCallback(async () => {
    await cmd.cancelOperation("python_setup");
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        if (phase === "install") void handleCancelSetup();
        else onClose();
      }
    },
    [onClose, phase, handleCancelSetup],
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={handleKeyDown}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) {
          if (phase === "install") void handleCancelSetup();
          else onClose();
        }
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
                {Math.round(progress * 100)}% — Press Escape or click Cancel to stop
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
                onClick={() => { void handleInstall(); }}
                className="border-primary bg-primary hover:bg-primary-hover rounded border px-3 py-1.5 text-xs font-medium text-white transition-colors"
              >
                Install
              </button>
            </>
          )}
          {phase === "install" && (
            <button
              onClick={() => { void handleCancelSetup(); }}
              className="border-border bg-surface-2 text-text-2 hover:bg-bg rounded border px-3 py-1.5 text-xs transition-colors"
            >
              Cancel
            </button>
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
