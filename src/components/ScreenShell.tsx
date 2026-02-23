import { createContext, useContext } from "react";
import { AppBar } from "./AppBar";
import { NavBar } from "./NavBar";

export interface AppShellContextType {
  chatOpen: boolean;
  toggleChat: () => void;
  openSettings: () => void;
  refreshRef: React.RefObject<(() => void) | null>;
}

export const AppShellContext = createContext<AppShellContextType | null>(null);

export function useAppShell(): AppShellContextType {
  const ctx = useContext(AppShellContext);
  if (!ctx) throw new Error("useAppShell must be used within AppShellContext.Provider");
  return ctx;
}

interface ScreenShellProps {
  title: string;
  subtitle?: string | undefined;
  onBack?: (() => void) | undefined;
  backLabel?: string | undefined;
  toolbar?: React.ReactNode;
  hideSettings?: boolean | undefined;
  children: React.ReactNode;
}

export function ScreenShell({
  title,
  subtitle,
  onBack,
  backLabel,
  toolbar,
  hideSettings,
  children,
}: ScreenShellProps) {
  return (
    <div className="bg-bg text-text flex h-full flex-col">
      <AppBar />
      <NavBar
        title={title}
        subtitle={subtitle}
        onBack={onBack}
        backLabel={backLabel}
        hideSettings={hideSettings}
      />
      {toolbar}
      <div className="flex min-h-0 flex-1">
        <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
          {children}
        </div>
      </div>
    </div>
  );
}
