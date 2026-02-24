import { useCallback, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { cmd } from "../commands";
import type {
  ProgressEvent,
  VixenDiscovery,
  VixenImportConfig,
  VixenImportResult,
} from "../types";
import { useProgress } from "../hooks/useProgress";

interface Props {
  onComplete: (setupSlug: string) => void;
  onCancel: () => void;
}

type WizardStep = "select" | "review" | "importing" | "done";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function ImportWizard({ onComplete, onCancel }: Props) {
  const progressOps = useProgress();
  const [step, setStep] = useState<WizardStep>("select");
  const [vixenDir, setVixenDir] = useState("");
  const [scanning, setScanning] = useState(false);
  const [discovery, setDiscovery] = useState<VixenDiscovery | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Review step state
  const [setupName, setProfileName] = useState("Vixen Import");
  const [importControllers, setImportControllers] = useState(true);
  const [importLayout, setImportLayout] = useState(true);
  const [previewFileOverride, setPreviewFileOverride] = useState<string | null>(
    null,
  );
  const [selectedSequences, setSelectedSequences] = useState<Set<string>>(
    new Set(),
  );
  const [selectedMedia, setSelectedMedia] = useState<Set<string>>(new Set());

  // Result
  const [result, setResult] = useState<VixenImportResult | null>(null);

  const handleBrowse = useCallback(async () => {
    const selected = await open({ directory: true, title: "Select Vixen 3 Data Directory" });
    if (selected) setVixenDir(selected);
  }, []);

  const handleScan = useCallback(async () => {
    if (!vixenDir.trim()) return;
    setScanning(true);
    setError(null);
    setPreviewFileOverride(null);
    try {
      const disc = await cmd.scanVixenDirectory(vixenDir.trim());
      setDiscovery(disc);
      // Pre-select all sequences and media
      setSelectedSequences(new Set(disc.sequences.map((s) => s.path)));
      setSelectedMedia(new Set(disc.media_files.map((m) => m.filename)));
      setImportLayout(disc.preview_available);
      setStep("review");
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }, [vixenDir]);

  const handleBrowsePreview = useCallback(async () => {
    const selected = await open({
      title: "Select Vixen Preview File (e.g. ModuleStore.xml)",
      filters: [{ name: "XML Files", extensions: ["xml"] }],
    });
    if (!selected) return;

    setError(null);
    try {
      const itemCount = await cmd.checkVixenPreviewFile(selected);
      setPreviewFileOverride(selected);
      setImportLayout(true);
      // Update discovery to reflect the manual selection
      setDiscovery((prev) =>
        prev
          ? {
              ...prev,
              preview_available: true,
              preview_item_count: itemCount,
              preview_file_path: selected,
            }
          : prev,
      );
    } catch (e) {
      setError(`Preview file error: ${String(e)}`);
    }
  }, []);

  const handleImport = useCallback(async () => {
    if (!discovery) return;
    setStep("importing");
    setError(null);
    try {
      const config: VixenImportConfig = {
        vixen_dir: discovery.vixen_dir,
        setup_name: setupName.trim() || "Vixen Import",
        import_controllers: importControllers,
        import_layout: importLayout,
        preview_file_override: previewFileOverride,
        sequence_paths: Array.from(selectedSequences),
        media_filenames: Array.from(selectedMedia),
      };
      const res = await cmd.executeVixenImport(config);
      setResult(res);
      setStep("done");
    } catch (e) {
      setError(String(e));
      setStep("review");
    }
  }, [
    discovery,
    setupName,
    importControllers,
    importLayout,
    previewFileOverride,
    selectedSequences,
    selectedMedia,
  ]);

  const toggleAllSequences = useCallback(
    (selectAll: boolean) => {
      if (!discovery) return;
      setSelectedSequences(
        selectAll ? new Set(discovery.sequences.map((s) => s.path)) : new Set(),
      );
    },
    [discovery],
  );

  const toggleAllMedia = useCallback(
    (selectAll: boolean) => {
      if (!discovery) return;
      setSelectedMedia(
        selectAll
          ? new Set(discovery.media_files.map((m) => m.filename))
          : new Set(),
      );
    },
    [discovery],
  );

  const handleCancelImport = useCallback(async () => {
    await cmd.cancelOperation("import");
    setError("Import was cancelled");
    setStep("review");
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        if (step === "importing") {
          handleCancelImport();
        } else if (step !== "done") {
          onCancel();
        }
      }
    },
    [step, onCancel, handleCancelImport],
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={handleKeyDown}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) {
          if (step === "importing") handleCancelImport();
          else if (step !== "done") onCancel();
        }
      }}
    >
      <div className="bg-surface border-border flex max-h-[80vh] w-[600px] flex-col rounded-lg border shadow-xl">
        {/* Header */}
        <div className="border-border flex items-center justify-between border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">Import from Vixen 3</h3>
          <StepIndicator current={step} />
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          {error && (
            <div className="bg-error/10 text-error mb-3 rounded px-3 py-2 text-xs">
              {error}
              <button
                onClick={() => setError(null)}
                className="ml-2 underline"
              >
                dismiss
              </button>
            </div>
          )}

          {step === "select" && (
            <StepSelect
              vixenDir={vixenDir}
              onDirChange={setVixenDir}
              onBrowse={handleBrowse}
              onScan={handleScan}
              scanning={scanning}
              scanProgress={scanning ? progressOps.get("scan")?.event : undefined}
            />
          )}

          {step === "review" && discovery && (
            <StepReview
              discovery={discovery}
              setupName={setupName}
              onSetupNameChange={setProfileName}
              importControllers={importControllers}
              onImportControllersChange={setImportControllers}
              importLayout={importLayout}
              onImportLayoutChange={setImportLayout}
              onBrowsePreview={handleBrowsePreview}
              selectedSequences={selectedSequences}
              onToggleSequence={(path) => {
                setSelectedSequences((prev) => {
                  const next = new Set(prev);
                  if (next.has(path)) next.delete(path);
                  else next.add(path);
                  return next;
                });
              }}
              onToggleAllSequences={toggleAllSequences}
              selectedMedia={selectedMedia}
              onToggleMedia={(filename) => {
                setSelectedMedia((prev) => {
                  const next = new Set(prev);
                  if (next.has(filename)) next.delete(filename);
                  else next.add(filename);
                  return next;
                });
              }}
              onToggleAllMedia={toggleAllMedia}
            />
          )}

          {step === "importing" && <StepImporting progress={progressOps.get("import")?.event} />}

          {step === "done" && result && <StepDone result={result} />}
        </div>

        {/* Footer */}
        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          {step === "select" && (
            <button
              onClick={onCancel}
              className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-4 py-1.5 text-xs transition-colors"
            >
              Cancel
            </button>
          )}
          {step === "review" && (
            <>
              <button
                onClick={() => setStep("select")}
                className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-4 py-1.5 text-xs transition-colors"
              >
                Back
              </button>
              <button
                onClick={handleImport}
                className="bg-primary hover:bg-primary/90 rounded px-4 py-1.5 text-xs font-medium text-white transition-colors"
              >
                Import
              </button>
            </>
          )}
          {step === "importing" && (
            <button
              onClick={handleCancelImport}
              className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-4 py-1.5 text-xs transition-colors"
            >
              Cancel Import
            </button>
          )}
          {step === "done" && result && (
            <button
              onClick={() => onComplete(result.setup_slug)}
              className="bg-primary hover:bg-primary/90 rounded px-4 py-1.5 text-xs font-medium text-white transition-colors"
            >
              Open Setup
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Step Indicator ──────────────────────────────────────────────────

const STEPS: { key: WizardStep; label: string }[] = [
  { key: "select", label: "Select" },
  { key: "review", label: "Review" },
  { key: "importing", label: "Import" },
  { key: "done", label: "Done" },
];

function StepIndicator({ current }: { current: WizardStep }) {
  const currentIdx = STEPS.findIndex((s) => s.key === current);
  return (
    <div className="flex items-center gap-1">
      {STEPS.map((s, i) => (
        <span
          key={s.key}
          className={`text-[10px] ${
            i <= currentIdx ? "text-primary font-medium" : "text-text-2"
          }`}
        >
          {i > 0 && <span className="text-text-2 mx-1">&rsaquo;</span>}
          {s.label}
        </span>
      ))}
    </div>
  );
}

// ── Step 1: Select Directory ────────────────────────────────────────

function StepSelect({
  vixenDir,
  onDirChange,
  onBrowse,
  onScan,
  scanning,
  scanProgress,
}: {
  vixenDir: string;
  onDirChange: (dir: string) => void;
  onBrowse: () => void;
  onScan: () => void;
  scanning: boolean;
  scanProgress?: ProgressEvent | undefined;
}) {
  const pct = scanProgress && scanProgress.progress >= 0
    ? Math.round(scanProgress.progress * 100)
    : 0;

  return (
    <div className="space-y-4">
      <p className="text-text-2 text-xs">
        Select your Vixen 3 data directory. This is typically located at{" "}
        <code className="bg-surface-2 rounded px-1">Documents/Vixen 3</code>.
      </p>

      <div className="flex gap-2">
        <input
          type="text"
          value={vixenDir}
          onChange={(e) => onDirChange(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && onScan()}
          placeholder="C:\Users\...\Documents\Vixen 3"
          autoFocus
          disabled={scanning}
          className="border-border bg-surface-2 text-text placeholder:text-text-2 focus:border-primary flex-1 rounded border px-3 py-1.5 text-sm outline-none disabled:opacity-50"
        />
        <button
          onClick={onBrowse}
          disabled={scanning}
          className="border-border bg-surface text-text-2 hover:bg-surface-2 hover:text-text rounded border px-3 py-1.5 text-xs transition-colors disabled:opacity-50"
        >
          Browse
        </button>
      </div>

      <button
        onClick={onScan}
        disabled={!vixenDir.trim() || scanning}
        className="bg-primary hover:bg-primary/90 rounded px-4 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-50"
      >
        {scanning ? "Scanning..." : "Scan Directory"}
      </button>

      {scanning && (
        <div className="space-y-1.5">
          <p className="text-text-2 text-xs">
            {scanProgress?.phase ?? "Starting scan..."}
          </p>
          <div className="bg-border/30 h-1.5 w-full overflow-hidden rounded-full">
            {scanProgress ? (
              <div
                className="bg-primary h-full rounded-full transition-[width] duration-200"
                style={{ width: `${pct}%` }}
              />
            ) : (
              <div className="bg-primary h-full w-1/3 animate-pulse rounded-full" />
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Step 2: Review ──────────────────────────────────────────────────

function StepReview({
  discovery,
  setupName,
  onSetupNameChange,
  importControllers,
  onImportControllersChange,
  importLayout,
  onImportLayoutChange,
  onBrowsePreview,
  selectedSequences,
  onToggleSequence,
  onToggleAllSequences,
  selectedMedia,
  onToggleMedia,
  onToggleAllMedia,
}: {
  discovery: VixenDiscovery;
  setupName: string;
  onSetupNameChange: (name: string) => void;
  importControllers: boolean;
  onImportControllersChange: (v: boolean) => void;
  importLayout: boolean;
  onImportLayoutChange: (v: boolean) => void;
  onBrowsePreview: () => void;
  selectedSequences: Set<string>;
  onToggleSequence: (path: string) => void;
  onToggleAllSequences: (selectAll: boolean) => void;
  selectedMedia: Set<string>;
  onToggleMedia: (filename: string) => void;
  onToggleAllMedia: (selectAll: boolean) => void;
}) {
  return (
    <div className="space-y-4">
      {/* Setup name */}
      <label className="block">
        <span className="text-text-2 mb-1 block text-xs">Setup Name</span>
        <input
          type="text"
          value={setupName}
          onChange={(e) => onSetupNameChange(e.target.value)}
          className="border-border bg-surface-2 text-text focus:border-primary w-full rounded border px-3 py-1.5 text-sm outline-none"
        />
      </label>

      {/* Fixtures & Groups (always on) */}
      <Section
        label={`Fixtures & Groups: ${discovery.fixtures_found} fixtures in ${discovery.groups_found} groups`}
        checked={true}
        locked
      />

      {/* Controllers */}
      <Section
        label={`Controllers: ${discovery.controllers_found} found`}
        checked={importControllers}
        onChange={onImportControllersChange}
      />

      {/* Preview Layout */}
      {discovery.preview_available ? (
        <Section
          label={`Preview Layout: ${discovery.preview_item_count} display items`}
          checked={importLayout}
          onChange={onImportLayoutChange}
        />
      ) : (
        <div className="border-border rounded border px-3 py-2">
          <div className="flex items-center justify-between">
            <span className="text-text text-xs">
              Preview Layout: Not found automatically
            </span>
            <button
              onClick={onBrowsePreview}
              className="text-primary text-[11px] hover:underline"
            >
              Browse...
            </button>
          </div>
          <p className="text-text-2 mt-1 text-[10px]">
            Look for <code className="bg-surface-2 rounded px-0.5">ModuleStore.xml</code> in
            your Vixen <code className="bg-surface-2 rounded px-0.5">SystemData</code> folder,
            or any XML file containing your display preview configuration.
          </p>
        </div>
      )}

      {/* Sequences */}
      {discovery.sequences.length > 0 && (
        <div>
          <div className="mb-1 flex items-center justify-between">
            <span className="text-text text-xs font-medium">
              Sequences ({selectedSequences.size} of{" "}
              {discovery.sequences.length})
            </span>
            <div className="flex gap-2">
              <button
                onClick={() => onToggleAllSequences(true)}
                className="text-primary text-[10px] hover:underline"
              >
                All
              </button>
              <button
                onClick={() => onToggleAllSequences(false)}
                className="text-text-2 text-[10px] hover:underline"
              >
                None
              </button>
            </div>
          </div>
          <div className="border-border bg-surface-2 max-h-[120px] overflow-y-auto rounded border">
            {discovery.sequences.map((seq) => (
              <label
                key={seq.path}
                className="border-border flex cursor-pointer items-center gap-2 border-b px-3 py-1.5 last:border-b-0"
              >
                <input
                  type="checkbox"
                  checked={selectedSequences.has(seq.path)}
                  onChange={() => onToggleSequence(seq.path)}
                  className="accent-primary"
                />
                <span className="text-text flex-1 truncate text-xs">
                  {seq.filename}
                </span>
                <span className="text-text-2 text-[10px]">
                  {formatBytes(seq.size_bytes)}
                </span>
              </label>
            ))}
          </div>
        </div>
      )}

      {/* Media */}
      {discovery.media_files.length > 0 && (
        <div>
          <div className="mb-1 flex items-center justify-between">
            <span className="text-text text-xs font-medium">
              Media ({selectedMedia.size} of {discovery.media_files.length})
            </span>
            <div className="flex gap-2">
              <button
                onClick={() => onToggleAllMedia(true)}
                className="text-primary text-[10px] hover:underline"
              >
                All
              </button>
              <button
                onClick={() => onToggleAllMedia(false)}
                className="text-text-2 text-[10px] hover:underline"
              >
                None
              </button>
            </div>
          </div>
          <div className="border-border bg-surface-2 max-h-[120px] overflow-y-auto rounded border">
            {discovery.media_files.map((m) => (
              <label
                key={m.filename}
                className="border-border flex cursor-pointer items-center gap-2 border-b px-3 py-1.5 last:border-b-0"
              >
                <input
                  type="checkbox"
                  checked={selectedMedia.has(m.filename)}
                  onChange={() => onToggleMedia(m.filename)}
                  className="accent-primary"
                />
                <span className="text-text flex-1 truncate text-xs">
                  {m.filename}
                </span>
                <span className="text-text-2 text-[10px]">
                  {formatBytes(m.size_bytes)}
                </span>
              </label>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Section toggle ──────────────────────────────────────────────────

function Section({
  label,
  checked,
  onChange,
  disabled,
  locked,
}: {
  label: string;
  checked: boolean;
  onChange?: (v: boolean) => void;
  disabled?: boolean;
  locked?: boolean;
}) {
  return (
    <label
      className={`border-border flex items-center gap-2 rounded border px-3 py-2 ${
        disabled ? "opacity-50" : "cursor-pointer"
      }`}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange?.(e.target.checked)}
        disabled={disabled || locked}
        className="accent-primary"
      />
      <span className="text-text text-xs">{label}</span>
      {locked && (
        <span className="text-text-2 ml-auto text-[10px]">Required</span>
      )}
    </label>
  );
}

// ── Step 3: Importing ───────────────────────────────────────────────

function StepImporting({ progress }: { progress?: ProgressEvent | undefined }) {
  const pct = progress && progress.progress >= 0 ? Math.round(progress.progress * 100) : 0;
  const indeterminate = !progress || progress.progress < 0;

  return (
    <div className="flex flex-col items-center gap-4 py-8">
      <div className="border-primary size-8  animate-spin rounded-full border-2 border-t-transparent" />
      <p className="text-text text-sm">{progress?.phase ?? "Importing..."}</p>
      {progress?.detail && (
        <p className="text-text-2 text-xs">{progress.detail}</p>
      )}
      <div className="bg-border/30 h-1.5 w-48 overflow-hidden rounded-full">
        {indeterminate ? (
          <div className="bg-primary h-full w-1/3 animate-pulse rounded-full" />
        ) : (
          <div
            className="bg-primary h-full rounded-full transition-[width] duration-200"
            style={{ width: `${pct}%` }}
          />
        )}
      </div>
      {!progress && (
        <p className="text-text-2 text-xs">
          This may take a moment for large shows.
        </p>
      )}
    </div>
  );
}

// ── Step 4: Done ────────────────────────────────────────────────────

function StepDone({ result }: { result: VixenImportResult }) {
  return (
    <div className="space-y-4">
      <div className="text-center">
        <p className="text-text text-sm font-medium">Import Complete</p>
      </div>

      <div className="border-border bg-surface-2 space-y-1 rounded border px-4 py-3">
        <SummaryRow label="Fixtures" value={result.fixtures_imported} />
        <SummaryRow label="Groups" value={result.groups_imported} />
        {result.controllers_imported > 0 && (
          <SummaryRow label="Controllers" value={result.controllers_imported} />
        )}
        {result.layout_items_imported > 0 && (
          <SummaryRow
            label="Layout items"
            value={result.layout_items_imported}
          />
        )}
        {result.sequences_imported > 0 && (
          <SummaryRow label="Sequences" value={result.sequences_imported} />
        )}
        {result.media_imported > 0 && (
          <SummaryRow label="Media files" value={result.media_imported} />
        )}
      </div>

      {result.warnings.length > 0 && (
        <div>
          <p className="text-text-2 mb-1 text-xs font-medium">Warnings</p>
          <div className="bg-warning/5 border-warning/20 max-h-[100px] overflow-y-auto rounded border px-3 py-2">
            {result.warnings.map((w, i) => (
              <p key={i} className="text-text-2 text-[11px]">
                {w}
              </p>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function SummaryRow({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex justify-between text-xs">
      <span className="text-text-2">{label}</span>
      <span className="text-text font-medium">{value}</span>
    </div>
  );
}
