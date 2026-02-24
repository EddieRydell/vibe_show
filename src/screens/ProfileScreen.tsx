import { useEffect, useState } from "react";
import { cmd } from "../commands";
import type { Profile } from "../types";
import { ScreenShell } from "../components/ScreenShell";
import { SequencesTab } from "./profile/SequencesTab";
import { MusicTab } from "./profile/MusicTab";
import { HouseSetupTab } from "./profile/HouseSetupTab";
import { LayoutTab } from "./profile/LayoutTab";
import { EffectsTab } from "./profile/EffectsTab";
import { GradientsTab } from "./profile/GradientsTab";
import { CurvesTab } from "./profile/CurvesTab";

type Tab = "sequences" | "music" | "house" | "layout" | "effects" | "gradients" | "curves";

interface Props {
  slug: string;
  onBack: () => void;
  onOpenSequence: (sequenceSlug: string) => void;
  onOpenScript: (name: string | null) => void;
  onOpenAnalysis: (filename: string) => void;
}

export function ProfileScreen({ slug, onBack, onOpenSequence, onOpenScript, onOpenAnalysis }: Props) {
  const [profile, setProfile] = useState<Profile | null>(null);
  const [tab, setTab] = useState<Tab>("sequences");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    cmd.openProfile(slug)
      .then(setProfile)
      .catch((e) => setError(String(e)));
  }, [slug]);

  const tabs: { key: Tab; label: string }[] = [
    { key: "sequences", label: "Sequences" },
    { key: "music", label: "Music" },
    { key: "house", label: "House Setup" },
    { key: "layout", label: "Layout" },
    { key: "effects", label: "Effects" },
    { key: "gradients", label: "Gradients" },
    { key: "curves", label: "Curves" },
  ];

  return (
    <ScreenShell title={profile?.name ?? "Loading..."} onBack={onBack} backLabel="Home">
      {error && (
        <div className="bg-error/10 border-error/20 text-error border-b px-6 py-2 text-xs">
          {error}
          <button onClick={() => setError(null)} className="ml-2 underline">
            dismiss
          </button>
        </div>
      )}

      <div className="border-border flex gap-0 border-b">
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`px-5 py-2 text-xs font-medium transition-colors ${
              tab === t.key
                ? "border-primary text-primary border-b-2"
                : "text-text-2 hover:text-text border-b-2 border-transparent"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto">
        {profile && tab === "sequences" && <SequencesTab slug={slug} onOpenSequence={onOpenSequence} setError={setError} />}
        {profile && tab === "music" && <MusicTab setError={setError} onOpenAnalysis={onOpenAnalysis} />}
        {profile && tab === "house" && (
          <HouseSetupTab profile={profile} onProfileUpdate={setProfile} setError={setError} />
        )}
        {profile && tab === "layout" && (
          <LayoutTab profile={profile} onProfileUpdate={setProfile} setError={setError} />
        )}
        {profile && tab === "effects" && <EffectsTab setError={setError} onOpenScript={onOpenScript} />}
        {profile && tab === "gradients" && <GradientsTab setError={setError} />}
        {profile && tab === "curves" && <CurvesTab setError={setError} />}
      </div>
    </ScreenShell>
  );
}
