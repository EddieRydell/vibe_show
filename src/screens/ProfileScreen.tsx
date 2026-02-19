import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  Profile,
  ShowSummary,
  MediaFile,
  EffectInfo,
} from "../types";

type Tab = "shows" | "music" | "house" | "layout" | "effects";

interface Props {
  slug: string;
  onBack: () => void;
  onOpenShow: (showSlug: string) => void;
}

export function ProfileScreen({ slug, onBack, onOpenShow }: Props) {
  const [profile, setProfile] = useState<Profile | null>(null);
  const [tab, setTab] = useState<Tab>("shows");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<Profile>("open_profile", { slug })
      .then(setProfile)
      .catch((e) => setError(String(e)));
  }, [slug]);

  const tabs: { key: Tab; label: string }[] = [
    { key: "shows", label: "Shows" },
    { key: "music", label: "Music" },
    { key: "house", label: "House Setup" },
    { key: "layout", label: "Layout" },
    { key: "effects", label: "Effects" },
  ];

  return (
    <div className="bg-bg text-text flex h-screen flex-col">
      {/* Header */}
      <div className="border-border flex items-center border-b px-6 py-3">
        <button
          onClick={onBack}
          className="text-text-2 hover:text-text mr-3 text-sm transition-colors"
        >
          &larr; Home
        </button>
        <h2 className="text-text text-lg font-bold">{profile?.name ?? "Loading..."}</h2>
      </div>

      {/* Error */}
      {error && (
        <div className="bg-error/10 border-error/20 text-error border-b px-6 py-2 text-xs">
          {error}
          <button onClick={() => setError(null)} className="ml-2 underline">
            dismiss
          </button>
        </div>
      )}

      {/* Tab bar */}
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

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto">
        {tab === "shows" && <ShowsTab onOpenShow={onOpenShow} setError={setError} />}
        {tab === "music" && <MusicTab setError={setError} />}
        {tab === "house" && <HouseSetupTab profile={profile} />}
        {tab === "layout" && <LayoutTab profile={profile} />}
        {tab === "effects" && <EffectsTab />}
      </div>
    </div>
  );
}

// ── Shows Tab ──────────────────────────────────────────────────────

function ShowsTab({
  onOpenShow,
  setError,
}: {
  onOpenShow: (slug: string) => void;
  setError: (e: string | null) => void;
}) {
  const [shows, setShows] = useState<ShowSummary[]>([]);
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);

  const refresh = useCallback(() => {
    invoke<ShowSummary[]>("list_shows")
      .then(setShows)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleCreate = useCallback(() => {
    if (!newName.trim()) return;
    invoke<ShowSummary>("create_show", { name: newName.trim() })
      .then(() => {
        setNewName("");
        setShowCreate(false);
        refresh();
      })
      .catch((e) => setError(String(e)));
  }, [newName, refresh, setError]);

  const handleDelete = useCallback(
    (showSlug: string, name: string) => {
      if (!confirm(`Delete show "${name}"?`)) return;
      invoke("delete_show", { slug: showSlug })
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">Shows</h3>
        <button
          onClick={() => setShowCreate(true)}
          className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          New Show
        </button>
      </div>

      {showCreate && (
        <div className="border-border bg-surface mb-4 flex items-center gap-3 rounded border p-3">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            placeholder="Show name"
            autoFocus
            className="border-border bg-surface-2 text-text placeholder:text-text-2 flex-1 rounded border px-3 py-1.5 text-sm outline-none focus:border-primary"
          />
          <button
            onClick={handleCreate}
            disabled={!newName.trim()}
            className="bg-primary hover:bg-primary-hover rounded px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
          >
            Create
          </button>
          <button
            onClick={() => {
              setShowCreate(false);
              setNewName("");
            }}
            className="text-text-2 hover:text-text text-xs"
          >
            Cancel
          </button>
        </div>
      )}

      {shows.length === 0 && !showCreate ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          No shows yet. Create one to start sequencing.
        </p>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {shows.map((s) => (
            <div
              key={s.slug}
              onClick={() => onOpenShow(s.slug)}
              className="border-border bg-surface hover:border-primary group cursor-pointer rounded-lg border p-4 transition-colors"
            >
              <h4 className="text-text text-sm font-medium">{s.name}</h4>
              <p className="text-text-2 mt-1 text-xs">
                {s.sequence_count} sequence{s.sequence_count !== 1 ? "s" : ""}
              </p>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleDelete(s.slug, s.name);
                }}
                className="text-text-2 hover:text-error mt-2 text-[10px] opacity-0 transition-all group-hover:opacity-100"
              >
                Delete
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Music Tab ──────────────────────────────────────────────────────

function MusicTab({ setError }: { setError: (e: string | null) => void }) {
  const [files, setFiles] = useState<MediaFile[]>([]);

  const refresh = useCallback(() => {
    invoke<MediaFile[]>("list_media")
      .then(setFiles)
      .catch((e) => setError(String(e)));
  }, [setError]);

  useEffect(refresh, [refresh]);

  const handleImport = useCallback(async () => {
    const selected = await open({
      title: "Import Audio File",
      filters: [
        { name: "Audio Files", extensions: ["mp3", "wav", "ogg", "flac", "m4a", "aac"] },
      ],
    });
    if (!selected) return;
    invoke<MediaFile>("import_media", { sourcePath: selected })
      .then(() => refresh())
      .catch((e) => setError(String(e)));
  }, [refresh, setError]);

  const handleDelete = useCallback(
    (filename: string) => {
      if (!confirm(`Delete "${filename}"?`)) return;
      invoke("delete_media", { filename })
        .then(refresh)
        .catch((e) => setError(String(e)));
    },
    [refresh, setError],
  );

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-text text-sm font-medium">Music / Audio</h3>
        <button
          onClick={handleImport}
          className="bg-primary hover:bg-primary-hover rounded px-3 py-1 text-xs font-medium text-white transition-colors"
        >
          Import Audio
        </button>
      </div>

      {files.length === 0 ? (
        <p className="text-text-2 mt-8 text-center text-sm">
          No audio files. Import music to use in your shows.
        </p>
      ) : (
        <div className="border-border divide-border divide-y rounded border">
          {files.map((f) => (
            <div
              key={f.filename}
              className="group flex items-center justify-between px-4 py-2.5"
            >
              <div>
                <span className="text-text text-sm">{f.filename}</span>
                <span className="text-text-2 ml-3 text-xs">{formatSize(f.size_bytes)}</span>
              </div>
              <button
                onClick={() => handleDelete(f.filename)}
                className="text-text-2 hover:text-error text-xs opacity-0 transition-all group-hover:opacity-100"
              >
                Delete
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── House Setup Tab (Read-only V1) ─────────────────────────────────

function HouseSetupTab({ profile }: { profile: Profile | null }) {
  if (!profile) return null;

  return (
    <div className="p-6 space-y-6">
      {/* Fixtures */}
      <section>
        <h3 className="text-text mb-3 text-sm font-medium">
          Fixtures ({profile.fixtures.length})
        </h3>
        {profile.fixtures.length === 0 ? (
          <p className="text-text-2 text-xs">No fixtures configured.</p>
        ) : (
          <div className="border-border divide-border divide-y rounded border">
            {profile.fixtures.map((f) => (
              <div key={f.id} className="flex items-center justify-between px-4 py-2">
                <span className="text-text text-sm">{f.name}</span>
                <span className="text-text-2 text-xs">
                  {f.color_model} &middot; {f.pixel_count}px
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Groups */}
      <section>
        <h3 className="text-text mb-3 text-sm font-medium">
          Groups ({profile.groups.length})
        </h3>
        {profile.groups.length === 0 ? (
          <p className="text-text-2 text-xs">No groups configured.</p>
        ) : (
          <div className="border-border divide-border divide-y rounded border">
            {profile.groups.map((g) => (
              <div key={g.id} className="flex items-center justify-between px-4 py-2">
                <span className="text-text text-sm">{g.name}</span>
                <span className="text-text-2 text-xs">
                  {g.members.length} member{g.members.length !== 1 ? "s" : ""}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Controllers */}
      <section>
        <h3 className="text-text mb-3 text-sm font-medium">
          Controllers ({profile.controllers.length})
        </h3>
        {profile.controllers.length === 0 ? (
          <p className="text-text-2 text-xs">No controllers configured.</p>
        ) : (
          <div className="border-border divide-border divide-y rounded border">
            {profile.controllers.map((c) => (
              <div key={c.id} className="flex items-center justify-between px-4 py-2">
                <span className="text-text text-sm">{c.name}</span>
                <span className="text-text-2 text-xs">
                  {"E131" in c.protocol
                    ? "E1.31"
                    : "ArtNet" in c.protocol
                      ? "ArtNet"
                      : "Serial"}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Patches */}
      <section>
        <h3 className="text-text mb-3 text-sm font-medium">
          Patches ({profile.patches.length})
        </h3>
        {profile.patches.length === 0 ? (
          <p className="text-text-2 text-xs">No patches configured.</p>
        ) : (
          <div className="border-border divide-border divide-y rounded border">
            {profile.patches.map((p, i) => (
              <div key={i} className="flex items-center justify-between px-4 py-2">
                <span className="text-text text-sm">Fixture {p.fixture_id}</span>
                <span className="text-text-2 text-xs">
                  {"Dmx" in p.output
                    ? `DMX ${p.output.Dmx.universe}/${p.output.Dmx.start_address}`
                    : `Port ${p.output.PixelPort.port}`}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

// ── Layout Tab (Read-only V1) ──────────────────────────────────────

function LayoutTab({ profile }: { profile: Profile | null }) {
  if (!profile) return null;

  const fixtureCount = profile.layout.fixtures.length;
  const totalPixels = profile.layout.fixtures.reduce(
    (sum, f) => sum + f.pixel_positions.length,
    0,
  );

  return (
    <div className="p-6">
      <h3 className="text-text mb-3 text-sm font-medium">Layout Preview</h3>
      {fixtureCount === 0 ? (
        <p className="text-text-2 text-xs">No fixture layout configured.</p>
      ) : (
        <>
          <p className="text-text-2 mb-4 text-xs">
            {fixtureCount} fixture{fixtureCount !== 1 ? "s" : ""}, {totalPixels} total pixels
          </p>
          <div className="border-border bg-surface relative h-80 overflow-hidden rounded border">
            {profile.layout.fixtures.map((fl) =>
              fl.pixel_positions.map((pos, pi) => (
                <div
                  key={`${fl.fixture_id}-${pi}`}
                  className="bg-primary absolute h-2 w-2 rounded-full opacity-60"
                  style={{
                    left: `${pos.x * 100}%`,
                    top: `${pos.y * 100}%`,
                  }}
                />
              )),
            )}
          </div>
        </>
      )}
    </div>
  );
}

// ── Effects Tab (Read-only) ────────────────────────────────────────

function EffectsTab() {
  const [effects, setEffects] = useState<EffectInfo[]>([]);

  useEffect(() => {
    invoke<EffectInfo[]>("list_effects")
      .then(setEffects)
      .catch(console.error);
  }, []);

  const effectColors: Record<string, string> = {
    Solid: "bg-fx-solid",
    Chase: "bg-fx-chase",
    Rainbow: "bg-fx-rainbow",
    Strobe: "bg-fx-strobe",
    Gradient: "bg-fx-gradient",
    Twinkle: "bg-fx-twinkle",
  };

  return (
    <div className="p-6">
      <h3 className="text-text mb-4 text-sm font-medium">Built-in Effects</h3>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {effects.map((fx) => (
          <div
            key={fx.name}
            className="border-border bg-surface rounded-lg border p-4"
          >
            <div className="mb-2 flex items-center gap-2">
              <div className={`h-3 w-3 rounded-full ${effectColors[fx.kind] ?? "bg-primary"}`} />
              <h4 className="text-text text-sm font-medium">{fx.name}</h4>
            </div>
            {fx.schema.length > 0 && (
              <div className="text-text-2 space-y-1 text-xs">
                {fx.schema.map((param) => (
                  <div key={param.key} className="flex justify-between">
                    <span>{param.label}</span>
                    <span className="text-text-2">
                      {typeof param.param_type === "string"
                        ? param.param_type
                        : Object.keys(param.param_type)[0]}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
