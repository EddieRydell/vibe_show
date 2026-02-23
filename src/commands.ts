/**
 * Typed command helpers wrapping the unified `exec` registry command.
 * All operations go through a single Tauri IPC endpoint.
 *
 * Commands that STAY as direct `invoke()` (async/streaming/binary/hot-path):
 *   - send_chat_message, open_sequence, execute_vixen_import, analyze_audio
 *   - tick, get_frame, get_frame_filtered
 *   - render_effect_thumbnail, preview_script, preview_script_frame
 *   - get_python_status, setup_python_env, start_python_sidecar, stop_python_sidecar
 */

import { invoke } from "@tauri-apps/api/core";
import type { CommandOutput } from "../src-tauri/bindings/CommandOutput";
import type {
  AppSettings,
  ChatMode,
  ColorGradient,
  Controller,
  Curve,
  EffectDetail,
  EffectInfo,
  EffectKind,
  FixtureDef,
  FixtureGroup,
  Layout,
  LlmConfigInfo,
  LlmProvider,
  MediaFile,
  Patch,
  PlaybackInfo,
  Profile,
  ProfileSummary,
  ScriptCompileResult,
  ScriptParamInfo,
  SequenceSummary,
  Show,
  UndoState,
  AudioAnalysis,
  ParamKey,
  ParamValue,
  BlendMode,
} from "./types";
import type { VixenDiscovery } from "../src-tauri/bindings/VixenDiscovery";

// ── Conversation types ────────────────────────────────────────────

export interface ConversationSummary {
  id: string;
  title: string;
  created_at: string;
  message_count: number;
  is_active: boolean;
}

// ── Core exec helpers ─────────────────────────────────────────────

async function exec(
  command: string,
  params?: unknown,
): Promise<CommandOutput> {
  return invoke("exec", {
    cmd: params ? { command, params } : { command },
  });
}

async function execData<T>(
  command: string,
  params?: unknown,
): Promise<T> {
  const output = await exec(command, params);
  return output.data as T;
}

// ── Typed command functions ───────────────────────────────────────

export const cmd = {
  // ── Playback ────────────────────────────────────────────
  play: () => exec("Play"),
  pause: () => exec("Pause"),
  seek: (time: number) => exec("Seek", { time }),
  undo: () => exec("Undo"),
  redo: () => exec("Redo"),
  getPlayback: () => execData<PlaybackInfo>("GetPlayback"),
  setRegion: (region: [number, number] | null) =>
    exec("SetRegion", {
      region: region ? { start: region[0], end: region[1] } : null,
    }),
  setLooping: (looping: boolean) => exec("SetLooping", { looping }),
  getUndoState: () => execData<UndoState>("GetUndoState"),

  // ── Query ───────────────────────────────────────────────
  getShow: () => execData<Show>("GetShow"),
  listEffects: () => execData<EffectInfo[]>("ListEffects"),
  getEffectDetail: (
    sequenceIndex: number,
    trackIndex: number,
    effectIndex: number,
  ) =>
    execData<EffectDetail | null>("GetEffectDetail", {
      sequence_index: sequenceIndex,
      track_index: trackIndex,
      effect_index: effectIndex,
    }),

  // ── Edit ────────────────────────────────────────────────
  addEffect: (
    trackIndex: number,
    kind: EffectKind,
    start: number,
    end: number,
    blendMode?: BlendMode,
    opacity?: number,
  ) =>
    execData<number>("AddEffect", {
      track_index: trackIndex,
      kind,
      start,
      end,
      ...(blendMode !== undefined && { blend_mode: blendMode }),
      ...(opacity !== undefined && { opacity }),
    }),
  deleteEffects: (targets: [number, number][]) =>
    exec("DeleteEffects", {
      targets: targets.map(([t, e]) => ({ track_index: t, effect_index: e })),
    }),
  updateEffectParam: (
    trackIndex: number,
    effectIndex: number,
    key: ParamKey,
    value: ParamValue,
  ) =>
    exec("UpdateEffectParam", {
      track_index: trackIndex,
      effect_index: effectIndex,
      key,
      value,
    }),
  updateEffectTimeRange: (
    trackIndex: number,
    effectIndex: number,
    start: number,
    end: number,
  ) =>
    exec("UpdateEffectTimeRange", {
      track_index: trackIndex,
      effect_index: effectIndex,
      start,
      end,
    }),
  addTrack: (name: string, fixtureId: number) =>
    execData<number>("AddTrack", { name, fixture_id: fixtureId }),
  deleteTrack: (trackIndex: number) =>
    exec("DeleteTrack", { track_index: trackIndex }),
  moveEffectToTrack: (
    fromTrack: number,
    effectIndex: number,
    toTrack: number,
  ) =>
    execData<number>("MoveEffectToTrack", {
      from_track: fromTrack,
      effect_index: effectIndex,
      to_track: toTrack,
    }),
  updateSequenceSettings: (p: {
    name?: string;
    audioFile?: string | null;
    duration?: number;
    frameRate?: number;
  }) =>
    exec("UpdateSequenceSettings", {
      name: p.name ?? null,
      audio_file: p.audioFile !== undefined ? p.audioFile : null,
      duration: p.duration ?? null,
      frame_rate: p.frameRate ?? null,
    }),

  // ── Settings ────────────────────────────────────────────
  getSettings: () => execData<AppSettings | null>("GetSettings"),
  getApiPort: () => execData<number>("GetApiPort"),
  initializeDataDir: (dataDir: string) =>
    execData<AppSettings>("InitializeDataDir", { data_dir: dataDir }),
  setLlmConfig: (p: {
    provider: LlmProvider;
    apiKey: string;
    baseUrl?: string | null;
    model?: string | null;
    chatMode?: ChatMode;
  }) =>
    exec("SetLlmConfig", {
      provider: p.provider,
      api_key: p.apiKey,
      base_url: p.baseUrl ?? null,
      model: p.model ?? null,
      ...(p.chatMode !== undefined && { chat_mode: p.chatMode }),
    }),
  getLlmConfig: () => execData<LlmConfigInfo>("GetLlmConfig"),

  // ── Profile CRUD ────────────────────────────────────────
  listProfiles: () => execData<ProfileSummary[]>("ListProfiles"),
  createProfile: (name: string) =>
    execData<ProfileSummary>("CreateProfile", { name }),
  openProfile: (slug: string) =>
    execData<Profile>("OpenProfile", { slug }),
  deleteProfile: (slug: string) => exec("DeleteProfile", { slug }),
  saveProfile: () => exec("SaveProfile"),
  updateProfileFixtures: (
    fixtures: FixtureDef[],
    groups: FixtureGroup[],
  ) => exec("UpdateProfileFixtures", { fixtures, groups }),
  updateProfileSetup: (controllers: Controller[], patches: Patch[]) =>
    exec("UpdateProfileSetup", { controllers, patches }),
  updateProfileLayout: (layout: Layout) =>
    exec("UpdateProfileLayout", { layout }),

  // ── Sequence CRUD ───────────────────────────────────────
  listSequences: () => execData<SequenceSummary[]>("ListSequences"),
  createSequence: (name: string) =>
    execData<SequenceSummary>("CreateSequence", { name }),
  openSequence: (slug: string) => exec("OpenSequence", { slug }),
  deleteSequence: (slug: string) => exec("DeleteSequence", { slug }),
  saveCurrentSequence: () => exec("SaveCurrentSequence"),

  // ── Media ───────────────────────────────────────────────
  listMedia: () => execData<MediaFile[]>("ListMedia"),
  importMedia: (sourcePath: string) =>
    execData<MediaFile>("ImportMedia", { source_path: sourcePath }),
  deleteMedia: (name: string) => exec("DeleteMedia", { name }),
  resolveMediaPath: (name: string) =>
    execData<string>("ResolveMediaPath", { name }),

  // ── Chat ────────────────────────────────────────────────
  getChatHistory: () => execData<unknown[]>("GetChatHistory"),
  getAgentChatHistory: () => execData<unknown[]>("GetAgentChatHistory"),
  clearChat: () => exec("ClearChat"),
  stopChat: () => exec("StopChat"),
  listAgentConversations: () => execData<ConversationSummary[]>("ListAgentConversations"),
  newAgentConversation: () => execData<{ id: string }>("NewAgentConversation"),
  switchAgentConversation: (id: string) => exec("SwitchAgentConversation", { conversation_id: id }),
  deleteAgentConversation: (id: string) => exec("DeleteAgentConversation", { conversation_id: id }),

  // ── Library (sequence) ──────────────────────────────────
  listLibraryGradients: () =>
    execData<[string, ColorGradient][]>("ListLibraryGradients"),
  listLibraryCurves: () =>
    execData<[string, Curve][]>("ListLibraryCurves"),
  setLibraryGradient: (name: string, stops: unknown[]) =>
    exec("SetLibraryGradient", { name, stops }),
  deleteLibraryGradient: (name: string) =>
    exec("DeleteLibraryGradient", { name }),
  renameLibraryGradient: (oldName: string, newName: string) =>
    exec("RenameLibraryGradient", { old_name: oldName, new_name: newName }),
  setLibraryCurve: (name: string, points: unknown[]) =>
    exec("SetLibraryCurve", { name, points }),
  deleteLibraryCurve: (name: string) =>
    exec("DeleteLibraryCurve", { name }),
  renameLibraryCurve: (oldName: string, newName: string) =>
    exec("RenameLibraryCurve", { old_name: oldName, new_name: newName }),
  listScripts: () => execData<string[]>("ListScripts"),
  getScriptSource: (name: string) =>
    execData<string | null>("GetScriptSource", { name }),
  deleteScript: (name: string) => exec("DeleteScript", { name }),

  // ── Library (profile) ──────────────────────────────────
  listProfileGradients: () =>
    execData<[string, ColorGradient][]>("ListProfileGradients"),
  setProfileGradient: (name: string, gradient: ColorGradient) =>
    exec("SetProfileGradient", { name, gradient }),
  deleteProfileGradient: (name: string) =>
    exec("DeleteProfileGradient", { name }),
  renameProfileGradient: (oldName: string, newName: string) =>
    exec("RenameProfileGradient", { old_name: oldName, new_name: newName }),
  listProfileCurves: () =>
    execData<[string, Curve][]>("ListProfileCurves"),
  setProfileCurve: (name: string, curve: Curve) =>
    exec("SetProfileCurve", { name, curve }),
  deleteProfileCurve: (name: string) =>
    exec("DeleteProfileCurve", { name }),
  renameProfileCurve: (oldName: string, newName: string) =>
    exec("RenameProfileCurve", { old_name: oldName, new_name: newName }),
  listProfileScripts: () =>
    execData<[string, string][]>("ListProfileScripts"),
  deleteProfileScript: (name: string) =>
    exec("DeleteProfileScript", { name }),
  setProfileScript: (name: string, source: string) =>
    exec("SetProfileScript", { name, source }),
  compileProfileScript: (name: string, source: string) =>
    execData<ScriptCompileResult>("CompileProfileScript", { name, source }),

  // ── Script ──────────────────────────────────────────────
  compileScript: (name: string, source: string) =>
    execData<ScriptCompileResult>("CompileScript", { name, source }),
  compileScriptPreview: (source: string) =>
    execData<ScriptCompileResult>("CompileScriptPreview", { source }),
  renameScript: (oldName: string, newName: string) =>
    exec("RenameScript", { old_name: oldName, new_name: newName }),
  getScriptParams: (name: string) =>
    execData<ScriptParamInfo[]>("GetScriptParams", { name }),

  // ── Cancellation ────────────────────────────────────────
  cancelOperation: (operation: string) =>
    invoke<boolean>("cancel_operation", { operation }),

  // ── Analysis ────────────────────────────────────────────
  getAnalysis: () => execData<AudioAnalysis | null>("GetAnalysis"),

  // ── Vixen Import (sync) ─────────────────────────────────
  importVixenSequence: (profileSlug: string, timPath: string) =>
    exec("ImportVixenSequence", {
      profile_slug: profileSlug,
      tim_path: timPath,
    }),
  scanVixenDirectory: (vixenDir: string) =>
    execData<VixenDiscovery>("ScanVixenDirectory", { vixen_dir: vixenDir }),
  checkVixenPreviewFile: (filePath: string) =>
    execData<number>("CheckVixenPreviewFile", { file_path: filePath }),
};
