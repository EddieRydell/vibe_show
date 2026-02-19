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

export default function App() {
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

  const handleOpenShow = useCallback(
    (profileSlug: string, showSlug: string) => {
      setScreen({ kind: "editor", profileSlug, showSlug });
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

  switch (screen.kind) {
    case "loading":
      return <LoadingScreen />;
    case "first_launch":
      return <FirstLaunchScreen onComplete={handleFirstLaunchComplete} />;
    case "home":
      return (
        <HomeScreen
          onOpenProfile={handleOpenProfile}
          onOpenSettings={handleOpenSettings}
        />
      );
    case "settings":
      return <SettingsScreen onBack={handleCloseSettings} />;
    case "profile":
      return (
        <ProfileScreen
          slug={screen.slug}
          onBack={handleBackToHome}
          onOpenShow={(showSlug) => handleOpenShow(screen.slug, showSlug)}
          onOpenSettings={handleOpenSettings}
        />
      );
    case "editor":
      return (
        <EditorScreen
          profileSlug={screen.profileSlug}
          showSlug={screen.showSlug}
          onBack={() => handleBackToProfile(screen.profileSlug)}
          onOpenSettings={handleOpenSettings}
        />
      );
  }
}
