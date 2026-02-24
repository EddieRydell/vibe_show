/**
 * Typed command helpers wrapping the unified `exec` registry command.
 * All operations go through a single Tauri IPC endpoint.
 *
 * Return types are inferred from CommandReturnMap — no manual `as T` casts.
 *
 * Commands that STAY as direct `invoke()` (async/streaming/binary/hot-path):
 *   - send_chat_message, open_sequence, execute_vixen_import, analyze_audio
 *   - tick, get_frame, get_frame_filtered
 *   - render_effect_thumbnail, preview_script, preview_script_frame
 *   - get_python_status, setup_python_env, start_python_sidecar, stop_python_sidecar
 */

import { invoke } from "@tauri-apps/api/core";
import type { CommandResult } from "../src-tauri/bindings/CommandResult";
import type {
  CommandReturnMap,
  DataCommand,
  UnitCommand,
} from "./commandMap";
import type {
  ChatMode,
  ColorGradient,
  Controller,
  Curve,
  EffectKind,
  FixtureDef,
  FixtureGroup,
  Layout,
  LlmProvider,
  Patch,
  ParamKey,
  ParamValue,
  BlendMode,
} from "./types";

// ── Core exec helpers ─────────────────────────────────────────────

async function exec<C extends UnitCommand>(
  command: C,
  params?: unknown,
): Promise<void> {
  await invoke<CommandResult>("exec", {
    cmd: params ? { command, params } : { command },
  });
}

async function execData<C extends DataCommand>(
  command: C,
  params?: unknown,
): Promise<CommandReturnMap[C]> {
  const result = await invoke<CommandResult>("exec", {
    cmd: params ? { command, params } : { command },
  });
  return (result as { data: CommandReturnMap[C] }).data;
}

// ── Typed command functions ───────────────────────────────────────

export const cmd = {
  // ── Playback ────────────────────────────────────────────
  play: () => exec("Play"),
  pause: () => exec("Pause"),
  seek: (time: number) => exec("Seek", { time }),
  undo: () => exec("Undo"),
  redo: () => exec("Redo"),
  getPlayback: () => execData("GetPlayback"),
  setRegion: (region: [number, number] | null) =>
    exec("SetRegion", {
      region: region ? { start: region[0], end: region[1] } : null,
    }),
  setLooping: (looping: boolean) => exec("SetLooping", { looping }),
  getUndoState: () => execData("GetUndoState"),

  // ── Query ───────────────────────────────────────────────
  getShow: () => execData("GetShow"),
  listEffects: () => execData("ListEffects"),
  getEffectDetail: (
    sequenceIndex: number,
    trackIndex: number,
    effectIndex: number,
  ) =>
    execData("GetEffectDetail", {
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
    execData("AddEffect", {
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
    execData("AddTrack", { name, fixture_id: fixtureId }),
  deleteTrack: (trackIndex: number) =>
    exec("DeleteTrack", { track_index: trackIndex }),
  moveEffectToTrack: (
    fromTrack: number,
    effectIndex: number,
    toTrack: number,
  ) =>
    execData("MoveEffectToTrack", {
      from_track: fromTrack,
      effect_index: effectIndex,
      to_track: toTrack,
    }),
  updateSequenceSettings: (p: {
    name?: string;
    audioFile?: string | null | undefined;
    duration?: number | undefined;
    frameRate?: number | undefined;
  }) =>
    exec("UpdateSequenceSettings", {
      name: p.name ?? null,
      audio_file: p.audioFile !== undefined ? p.audioFile : null,
      duration: p.duration ?? null,
      frame_rate: p.frameRate ?? null,
    }),

  // ── Settings ────────────────────────────────────────────
  getSettings: () => execData("GetSettings"),
  getApiPort: () => execData("GetApiPort"),
  initializeDataDir: (dataDir: string) =>
    execData("InitializeDataDir", { data_dir: dataDir }),
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
  getLlmConfig: () => execData("GetLlmConfig"),

  // ── Setup CRUD ─────────────────────────────────────────
  listSetups: () => execData("ListSetups"),
  createSetup: (name: string) =>
    execData("CreateSetup", { name }),
  openSetup: (slug: string) =>
    execData("OpenSetup", { slug }),
  deleteSetup: (slug: string) => exec("DeleteSetup", { slug }),
  saveSetup: () => exec("SaveSetup"),
  updateSetupFixtures: (
    fixtures: FixtureDef[],
    groups: FixtureGroup[],
  ) => exec("UpdateSetupFixtures", { fixtures, groups }),
  updateSetupOutputs: (controllers: Controller[], patches: Patch[]) =>
    exec("UpdateSetupOutputs", { controllers, patches }),
  updateSetupLayout: (layout: Layout) =>
    exec("UpdateSetupLayout", { layout }),

  // ── Sequence CRUD ───────────────────────────────────────
  listSequences: () => execData("ListSequences"),
  createSequence: (name: string) =>
    execData("CreateSequence", { name }),
  openSequence: (slug: string) => exec("OpenSequence", { slug }),
  deleteSequence: (slug: string) => exec("DeleteSequence", { slug }),
  saveCurrentSequence: () => exec("SaveCurrentSequence"),

  // ── Media ───────────────────────────────────────────────
  listMedia: () => execData("ListMedia"),
  importMedia: (sourcePath: string) =>
    execData("ImportMedia", { source_path: sourcePath }),
  deleteMedia: (name: string) => exec("DeleteMedia", { name }),
  resolveMediaPath: (name: string) =>
    execData("ResolveMediaPath", { name }),

  // ── Chat ────────────────────────────────────────────────
  getChatHistory: () => execData("GetChatHistory"),
  getAgentChatHistory: () => execData("GetAgentChatHistory"),
  clearChat: () => exec("ClearChat"),
  stopChat: () => exec("StopChat"),
  listAgentConversations: () => execData("ListAgentConversations"),
  newAgentConversation: () => execData("NewAgentConversation"),
  switchAgentConversation: (id: string) => exec("SwitchAgentConversation", { conversation_id: id }),
  deleteAgentConversation: (id: string) => exec("DeleteAgentConversation", { conversation_id: id }),

  // ── Global Library ──────────────────────────────────────
  listGlobalGradients: () => execData("ListGlobalGradients"),
  setGlobalGradient: (name: string, gradient: ColorGradient) =>
    exec("SetGlobalGradient", { name, gradient }),
  deleteGlobalGradient: (name: string) =>
    exec("DeleteGlobalGradient", { name }),
  renameGlobalGradient: (oldName: string, newName: string) =>
    exec("RenameGlobalGradient", { old_name: oldName, new_name: newName }),
  listGlobalCurves: () => execData("ListGlobalCurves"),
  setGlobalCurve: (name: string, curve: Curve) =>
    exec("SetGlobalCurve", { name, curve }),
  deleteGlobalCurve: (name: string) =>
    exec("DeleteGlobalCurve", { name }),
  renameGlobalCurve: (oldName: string, newName: string) =>
    exec("RenameGlobalCurve", { old_name: oldName, new_name: newName }),
  listGlobalScripts: () => execData("ListGlobalScripts"),
  getGlobalScriptSource: (name: string) =>
    execData("GetGlobalScriptSource", { name }),
  deleteGlobalScript: (name: string) =>
    exec("DeleteGlobalScript", { name }),
  writeGlobalScript: (name: string, source: string) =>
    exec("WriteGlobalScript", { name, source }),
  compileGlobalScript: (name: string, source: string) =>
    execData("CompileGlobalScript", { name, source }),
  renameGlobalScript: (oldName: string, newName: string) =>
    exec("RenameGlobalScript", { old_name: oldName, new_name: newName }),

  // ── Script ──────────────────────────────────────────────
  compileScriptPreview: (source: string) =>
    execData("CompileScriptPreview", { source }),
  getScriptParams: (name: string) =>
    execData("GetScriptParams", { name }),

  // ── Cancellation ────────────────────────────────────────
  cancelOperation: (operation: string) =>
    invoke<boolean>("cancel_operation", { operation }),

  // ── Analysis ────────────────────────────────────────────
  getAnalysis: () => execData("GetAnalysis"),

  // ── Vixen Import (sync) ─────────────────────────────────
  importVixenSequence: (setupSlug: string, timPath: string) =>
    execData("ImportVixenSequence", {
      setup_slug: setupSlug,
      tim_path: timPath,
    }),
  scanVixenDirectory: (vixenDir: string) =>
    execData("ScanVixenDirectory", { vixen_dir: vixenDir }),
  checkVixenPreviewFile: (filePath: string) =>
    execData("CheckVixenPreviewFile", { file_path: filePath }),
};
