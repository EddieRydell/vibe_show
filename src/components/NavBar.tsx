import { Settings, MessageSquare } from "lucide-react";
import { useAppShell } from "./ScreenShell";

interface NavBarProps {
  title: string;
  subtitle?: string;
  onBack?: () => void;
  backLabel?: string;
  hideSettings?: boolean;
}

export function NavBar({ title, subtitle, onBack, backLabel = "Back", hideSettings }: NavBarProps) {
  const { chatOpen, toggleChat, openSettings } = useAppShell();

  return (
    <div className="border-border bg-surface flex select-none items-center gap-2 border-b px-4 py-1.5">
      {onBack && (
        <button
          onClick={onBack}
          className="text-text-2 hover:text-text mr-1 text-sm transition-colors"
        >
          &larr; {backLabel}
        </button>
      )}
      <span className="text-text text-sm font-bold">{title}</span>
      {subtitle && (
        <span className="text-text-2 text-xs">{subtitle}</span>
      )}
      <div className="flex-1" />
      {!hideSettings && (
        <button
          onClick={openSettings}
          className="text-text-2 hover:text-text p-1 transition-colors"
          title="Settings"
        >
          <Settings size={14} />
        </button>
      )}
      <button
        onClick={toggleChat}
        className={`p-1 transition-colors ${chatOpen ? "text-primary" : "text-text-2 hover:text-text"}`}
        title="Chat"
      >
        <MessageSquare size={14} />
      </button>
    </div>
  );
}
