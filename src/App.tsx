import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AppScreen } from "./screens";
import { cmd } from "./commands";
import { LoadingScreen } from "./screens/LoadingScreen";
import { FirstLaunchScreen } from "./screens/FirstLaunchScreen";
import { HomeScreen } from "./screens/HomeScreen";
import { ProfileScreen } from "./screens/ProfileScreen";
import { EditorScreen } from "./screens/EditorScreen";
import { ScriptScreen } from "./screens/ScriptScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { useProgress } from "./hooks/useProgress";
import { ProgressOverlay } from "./components/ProgressOverlay";
import { AppShellContext } from "./components/ScreenShell";
import { ChatPanel } from "./components/ChatPanel";

export default function App() {
  const progressOps = useProgress();
  const [screen, setScreen] = useState<AppScreen>({ kind: "loading" });
  const [chatOpen, setChatOpen] = useState(false);
  const refreshRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    cmd.getSettings().then((settings) => {
      if (settings) {
        setScreen({ kind: "home" });
      } else {
        setScreen({ kind: "first_launch" });
      }
    });
  }, []);

  const handleFirstLaunchComplete = useCallback(() => {
    setScreen({ kind: "home" });
  }, []);

  const handleOpenProfile = useCallback((slug: string) => {
    setScreen({ kind: "profile", slug });
  }, []);

  const handleBackToHome = useCallback(() => {
    setScreen({ kind: "home" });
  }, []);

  const handleOpenSequence = useCallback(
    (profileSlug: string, sequenceSlug: string) => {
      setScreen({ kind: "editor", profileSlug, sequenceSlug });
    },
    [],
  );

  const handleBackToProfile = useCallback((slug: string) => {
    setScreen({ kind: "profile", slug });
  }, []);

  const handleOpenSettings = useCallback(() => {
    setScreen((current) => ({ kind: "settings", returnTo: current }));
  }, []);

  const handleCloseSettings = useCallback(() => {
    setScreen((current) => {
      if (current.kind === "settings") return current.returnTo;
      return current;
    });
  }, []);

  const handleOpenScript = useCallback((profileSlug: string, scriptName: string | null) => {
    setScreen((current) => ({
      kind: "script",
      profileSlug,
      scriptName,
      returnTo: current,
    }));
  }, []);

  const toggleChat = useCallback(() => {
    setChatOpen((o) => !o);
  }, []);

  const shellContext = useMemo(
    () => ({
      chatOpen,
      toggleChat,
      openSettings: handleOpenSettings,
      refreshRef,
    }),
    [chatOpen, toggleChat, handleOpenSettings],
  );

  // Derive a key that changes when the active sequence changes
  const sequenceKey = screen.kind === "editor" ? `${screen.profileSlug}/${screen.sequenceSlug}` : "";

  let content: React.ReactNode;
  switch (screen.kind) {
    case "loading":
      content = <LoadingScreen />;
      break;
    case "first_launch":
      content = <FirstLaunchScreen onComplete={handleFirstLaunchComplete} />;
      break;
    case "home":
      content = (
        <HomeScreen
          onOpenProfile={handleOpenProfile}
        />
      );
      break;
    case "settings":
      content = <SettingsScreen onBack={handleCloseSettings} />;
      break;
    case "profile":
      content = (
        <ProfileScreen
          slug={screen.slug}
          onBack={handleBackToHome}
          onOpenSequence={(sequenceSlug) => handleOpenSequence(screen.slug, sequenceSlug)}
          onOpenScript={(name) => handleOpenScript(screen.slug, name)}
        />
      );
      break;
    case "editor":
      content = (
        <EditorScreen
          profileSlug={screen.profileSlug}
          sequenceSlug={screen.sequenceSlug}
          onBack={() => handleBackToProfile(screen.profileSlug)}
          onOpenScript={(name) => handleOpenScript(screen.profileSlug, name)}
        />
      );
      break;
    case "script":
      content = (
        <ScriptScreen
          profileSlug={screen.profileSlug}
          initialScriptName={screen.scriptName}
          onBack={() => setScreen(screen.returnTo)}
          onOpenScript={(name) => handleOpenScript(screen.profileSlug, name)}
        />
      );
      break;
  }

  return (
    <AppShellContext.Provider value={shellContext}>
      <div className="flex h-full">
        <div className="min-w-0 flex-1">{content}</div>
        <ChatPanel
          open={chatOpen}
          onClose={toggleChat}
          onRefresh={() => refreshRef.current?.()}
          sequenceKey={sequenceKey}
        />
      </div>
      <ProgressOverlay operations={progressOps} />
    </AppShellContext.Provider>
  );
}
