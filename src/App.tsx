import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AppScreen } from "./screens";
import { cmd } from "./commands";
import { LoadingScreen } from "./screens/LoadingScreen";
import { FirstLaunchScreen } from "./screens/FirstLaunchScreen";
import { HomeScreen } from "./screens/HomeScreen";
import { useProgress } from "./hooks/useProgress";
import { ProgressOverlay } from "./components/ProgressOverlay";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { AppShellContext } from "./components/ScreenShell";
import { ChatPanel } from "./components/ChatPanel";

const ProfileScreen = lazy(() => import("./screens/ProfileScreen").then(m => ({ default: m.ProfileScreen })));
const EditorScreen = lazy(() => import("./screens/EditorScreen").then(m => ({ default: m.EditorScreen })));
const ScriptScreen = lazy(() => import("./screens/ScriptScreen").then(m => ({ default: m.ScriptScreen })));
const SettingsScreen = lazy(() => import("./screens/SettingsScreen").then(m => ({ default: m.SettingsScreen })));
const AnalysisScreen = lazy(() => import("./screens/AnalysisScreen").then(m => ({ default: m.AnalysisScreen })));

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

  // Catch unhandled promise rejections to prevent silent failures
  useEffect(() => {
    const handler = (event: PromiseRejectionEvent) => {
      console.error("[VibeLights] Unhandled promise rejection:", event.reason);
    };
    window.addEventListener("unhandledrejection", handler);
    return () => window.removeEventListener("unhandledrejection", handler);
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

  const handleOpenAnalysis = useCallback((profileSlug: string, filename: string) => {
    setScreen((current) => ({
      kind: "analysis",
      profileSlug,
      filename,
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
        <ErrorBoundary>
          <HomeScreen
            onOpenProfile={handleOpenProfile}
          />
        </ErrorBoundary>
      );
      break;
    case "settings":
      content = <ErrorBoundary><Suspense fallback={<LoadingScreen />}><SettingsScreen onBack={handleCloseSettings} /></Suspense></ErrorBoundary>;
      break;
    case "profile":
      content = (
        <ErrorBoundary>
          <Suspense fallback={<LoadingScreen />}>
            <ProfileScreen
              slug={screen.slug}
              onBack={handleBackToHome}
              onOpenSequence={(sequenceSlug) => handleOpenSequence(screen.slug, sequenceSlug)}
              onOpenScript={(name) => handleOpenScript(screen.slug, name)}
              onOpenAnalysis={(filename) => handleOpenAnalysis(screen.slug, filename)}
            />
          </Suspense>
        </ErrorBoundary>
      );
      break;
    case "editor":
      content = (
        <ErrorBoundary>
          <Suspense fallback={<LoadingScreen />}>
            <EditorScreen
              profileSlug={screen.profileSlug}
              sequenceSlug={screen.sequenceSlug}
              onBack={() => handleBackToProfile(screen.profileSlug)}
              onOpenScript={(name) => handleOpenScript(screen.profileSlug, name)}
            />
          </Suspense>
        </ErrorBoundary>
      );
      break;
    case "script":
      content = (
        <ErrorBoundary>
          <Suspense fallback={<LoadingScreen />}>
            <ScriptScreen
              profileSlug={screen.profileSlug}
              initialScriptName={screen.scriptName}
              onBack={() => setScreen(screen.returnTo)}
              onOpenScript={(name) => handleOpenScript(screen.profileSlug, name)}
            />
          </Suspense>
        </ErrorBoundary>
      );
      break;
    case "analysis":
      content = (
        <ErrorBoundary>
          <Suspense fallback={<LoadingScreen />}>
            <AnalysisScreen
              profileSlug={screen.profileSlug}
              filename={screen.filename}
              onBack={() => setScreen(screen.returnTo)}
            />
          </Suspense>
        </ErrorBoundary>
      );
      break;
  }

  return (
    <ErrorBoundary>
      <AppShellContext.Provider value={shellContext}>
        <div className="flex h-full">
          <main className="min-w-0 flex-1">{content}</main>
          <ChatPanel
            open={chatOpen}
            onClose={toggleChat}
            onRefresh={() => refreshRef.current?.()}
            sequenceKey={sequenceKey}
          />
        </div>
        <ProgressOverlay operations={progressOps} />
      </AppShellContext.Provider>
    </ErrorBoundary>
  );
}
