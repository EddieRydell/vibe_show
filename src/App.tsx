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
import { AppBar } from "./components/AppBar";
import { ChatPanel } from "./components/ChatPanel";

const EditorScreen = lazy(() => import("./screens/EditorScreen").then(m => ({ default: m.EditorScreen })));
const ScriptScreen = lazy(() => import("./screens/ScriptScreen").then(m => ({ default: m.ScriptScreen })));
const SettingsScreen = lazy(() => import("./screens/SettingsScreen").then(m => ({ default: m.SettingsScreen })));
const AnalysisScreen = lazy(() => import("./screens/AnalysisScreen").then(m => ({ default: m.AnalysisScreen })));

export default function App() {
  const progressOps = useProgress();
  const [screen, setScreen] = useState<AppScreen>({ kind: "loading" });
  const [chatOpen, setChatOpen] = useState(false);
  const refreshRef = useRef<(() => void) | null>(null);

  // Setup/tab state for the home screen hub
  const [activeSetupSlug, setActiveSetupSlug] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<string>("setups");

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

  const handleOpenSetup = useCallback((slug: string) => {
    setActiveSetupSlug(slug);
    setActiveTab("sequences");
  }, []);

  const handleCloseSetup = useCallback(() => {
    setActiveSetupSlug(null);
    setActiveTab("setups");
  }, []);

  const handleOpenSequence = useCallback(
    (sequenceSlug: string) => {
      if (!activeSetupSlug) return;
      setScreen({ kind: "editor", setupSlug: activeSetupSlug, sequenceSlug });
    },
    [activeSetupSlug],
  );

  const handleOpenSettings = useCallback(() => {
    setScreen((current) => ({ kind: "settings", returnTo: current }));
  }, []);

  const handleCloseSettings = useCallback(() => {
    setScreen((current) => {
      if (current.kind === "settings") return current.returnTo;
      return current;
    });
  }, []);

  const handleOpenScript = useCallback((scriptName: string | null) => {
    setScreen((current) => ({
      kind: "script",
      scriptName,
      returnTo: current,
    }));
  }, []);

  const handleOpenAnalysis = useCallback((filename: string) => {
    if (!activeSetupSlug) return;
    setScreen((current) => ({
      kind: "analysis",
      setupSlug: activeSetupSlug,
      filename,
      returnTo: current,
    }));
  }, [activeSetupSlug]);

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
            activeSetupSlug={activeSetupSlug}
            activeTab={activeTab}
            onTabChange={setActiveTab}
            onOpenSetup={handleOpenSetup}
            onCloseSetup={handleCloseSetup}
            onOpenSequence={handleOpenSequence}
            onOpenScript={handleOpenScript}
            onOpenAnalysis={handleOpenAnalysis}
          />
        </ErrorBoundary>
      );
      break;
    case "settings":
      content = <ErrorBoundary><Suspense fallback={<LoadingScreen />}><SettingsScreen onBack={handleCloseSettings} /></Suspense></ErrorBoundary>;
      break;
    case "editor":
      content = (
        <ErrorBoundary>
          <Suspense fallback={<LoadingScreen />}>
            <EditorScreen
              setupSlug={screen.setupSlug}
              sequenceSlug={screen.sequenceSlug}
              onBack={() => {
                setScreen({ kind: "home" });
                setActiveSetupSlug(screen.setupSlug);
                setActiveTab("sequences");
              }}
              onOpenScript={(name) => {
                setScreen((current) => ({
                  kind: "script",
                  scriptName: name,
                  returnTo: current,
                }));
              }}
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
              initialScriptName={screen.scriptName}
              onBack={() => setScreen(screen.returnTo)}
              onOpenScript={(name) => {
                setScreen((current) => ({
                  kind: "script",
                  scriptName: name,
                  returnTo: current.kind === "script" ? current.returnTo : current,
                }));
              }}
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
              setupSlug={screen.setupSlug}
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
        <div className="bg-bg text-text flex h-full flex-col">
          <AppBar />
          <div className="flex min-h-0 flex-1">
            <main className="min-w-0 flex-1">{content}</main>
            <ChatPanel
              open={chatOpen}
              onClose={toggleChat}
              onRefresh={() => refreshRef.current?.()}
              screen={screen}
            />
          </div>
        </div>
        <ProgressOverlay operations={progressOps} />
      </AppShellContext.Provider>
    </ErrorBoundary>
  );
}
