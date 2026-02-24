interface Tab {
  key: string;
  label: string;
  requiresSetup?: boolean;
}

const TABS: Tab[] = [
  { key: "setups", label: "Setups" },
  { key: "sequences", label: "Sequences", requiresSetup: true },
  { key: "music", label: "Music", requiresSetup: true },
  { key: "house", label: "House Setup", requiresSetup: true },
  { key: "layout", label: "Layout", requiresSetup: true },
  { key: "effects", label: "Effects" },
  { key: "gradients", label: "Gradients" },
  { key: "curves", label: "Curves" },
];

interface Props {
  activeTab: string;
  onTabChange: (tab: string) => void;
  activeSetupName: string | null;
  onCloseSetup: () => void;
}

export function TabBar({ activeTab, onTabChange, activeSetupName, onCloseSetup }: Props) {
  const hasSetup = activeSetupName !== null;

  return (
    <div className="border-border bg-surface flex items-center gap-0 border-b">
      {TABS.filter((t) => !t.requiresSetup || hasSetup).map((t) => (
        <button
          key={t.key}
          onClick={() => onTabChange(t.key)}
          className={`px-5 py-2 text-xs font-medium transition-colors ${
            activeTab === t.key
              ? "border-primary text-primary border-b-2"
              : "text-text-2 hover:text-text border-b-2 border-transparent"
          }`}
        >
          {t.label}
        </button>
      ))}

      {hasSetup && (
        <div className="ml-auto flex items-center gap-1.5 pr-3">
          <span className="text-text-2 text-[10px]">Setup:</span>
          <span className="text-text text-xs font-medium">{activeSetupName}</span>
          <button
            onClick={onCloseSetup}
            className="text-text-2 hover:text-text ml-1 text-xs transition-colors"
            title="Close setup"
          >
            &times;
          </button>
        </div>
      )}
    </div>
  );
}
