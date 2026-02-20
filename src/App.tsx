import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppScreen } from "./screens";
import type { AppSettings } from "./types";
import { LoadingScreen } from "./screens/LoadingScreen";
import { FirstLaunchScreen } from "./screens/FirstLaunchScreen";
import { HomeScreen } from "./screens/HomeScreen";
import { ProfileScreen } from "./screens/ProfileScreen";
import { EditorScreen } from "./screens/EditorScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { useProgress } from "./hooks/useProgress";
import { ProgressOverlay } from "./components/ProgressOverlay";

export default function App() {
  const progressOps = useProgress();
  const [screen, setScreen] = useState<AppScreen>({ kind: "loading" });

  useEffect(() => {
    invoke<AppSettings | null>("get_settings").then((settings) => {
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
          onOpenSettings={handleOpenSettings}
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
          onOpenSettings={handleOpenSettings}
        />
      );
      break;
    case "editor":
      content = (
        <EditorScreen
          profileSlug={screen.profileSlug}
          sequenceSlug={screen.sequenceSlug}
          onBack={() => handleBackToProfile(screen.profileSlug)}
          onOpenSettings={handleOpenSettings}
        />
      );
      break;
  }

  return (
    <>
      {content}
      <ProgressOverlay operations={progressOps} />
    </>
  );
}
