/**
 * Typed command helpers wrapping the unified `exec` registry command.
 * All operations go through a single Tauri IPC endpoint.
 *
 * Return types are inferred from CommandReturnMap — no manual `as T` casts.
 * Param types are checked against CommandParams — field name typos and
 * missing fields are compile errors.
 */

import { invoke } from "@tauri-apps/api/core";
import type { CommandResult } from "../src-tauri/bindings/CommandResult";
import type {
  CommandParams,
  CommandReturnMap,
  DataCommand,
  UnitCommand,
} from "./commandMap";
import type {
  ColorGradient,
  Controller,
  Curve,
  EffectKind,
  FixtureDef,
  FixtureGroup,
  Layout,
  Patch,
  ParamKey,
  ParamValue,
  BlendMode,
  AnalysisFeatures,
  EffectParams,
  VixenImportConfig,
} from "./types";

// ── Core exec helpers ─────────────────────────────────────────────

async function exec<C extends UnitCommand>(
  command: C,
  ...args: CommandParams<C> extends undefined ? [] : [params: CommandParams<C>]
): Promise<void> {
  const params = args[0];
  await invoke<CommandResult>("exec", {
    cmd: params !== undefined ? { command, params } : { command },
  });
}

async function execData<C extends DataCommand>(
  command: C,
  ...args: CommandParams<C> extends undefined ? [] : [params: CommandParams<C>]
): Promise<CommandReturnMap[C]> {
  const params = args[0];
  const result = await invoke<CommandResult>("exec", {
    cmd: params !== undefined ? { command, params } : { command },
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
    blendMode: BlendMode = "Override",
    opacity = 1.0,
  ) =>
    execData("AddEffect", {
      track_index: trackIndex,
      kind,
      start,
      end,
      blend_mode: blendMode,
      opacity,
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
  initializeDataDir: (dataDir: string) =>
    execData("InitializeDataDir", { data_dir: dataDir }),
  setLlmConfig: (p: {
    apiKey: string;
    model?: string | null;
  }) =>
    exec("SetLlmConfig", {
      api_key: p.apiKey,
      model: p.model ?? null,
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
  openSequence: (slug: string) => execData("OpenSequence", { slug }),
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
  getAgentChatHistory: () => execData("GetAgentChatHistory"),
  listAgentConversations: () => execData("ListAgentConversations"),
  newAgentConversation: () => execData("NewAgentConversation"),
  switchAgentConversation: (id: string) => exec("SwitchAgentConversation", { conversation_id: id }),
  deleteAgentConversation: (id: string) => exec("DeleteAgentConversation", { conversation_id: id }),
  sendAgentMessage: (message: string, context?: string) =>
    exec("SendAgentMessage", { message, context: context ?? null }),
  cancelAgentMessage: () => exec("CancelAgentMessage"),
  clearAgentSession: () => exec("ClearAgentSession"),

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

  // ── Hot-path (playback/preview) ─────────────────────────
  tick: (dt: number) => execData("Tick", { dt }),
  getFrame: (time: number) => execData("GetFrame", { time }),
  getFrameFiltered: (time: number, effects: [number, number][]) =>
    execData("GetFrameFiltered", { time, effects }),
  renderEffectThumbnail: (
    sequenceIndex: number,
    trackIndex: number,
    effectIndex: number,
    timeSamples: number,
    pixelRows: number,
  ) =>
    execData("RenderEffectThumbnail", {
      sequence_index: sequenceIndex,
      track_index: trackIndex,
      effect_index: effectIndex,
      time_samples: timeSamples,
      pixel_rows: pixelRows,
    }),
  previewScript: (
    name: string,
    params: EffectParams,
    pixelCount: number,
    timeSamples: number,
  ) =>
    execData("PreviewScript", {
      name,
      params,
      pixel_count: pixelCount,
      time_samples: timeSamples,
    }),
  previewScriptFrame: (
    name: string,
    params: EffectParams,
    pixelCount: number,
    t: number,
  ) =>
    execData("PreviewScriptFrame", { name, params, pixel_count: pixelCount, t }),

  // ── Cancellation ────────────────────────────────────────
  cancelOperation: (operation: string) =>
    execData("CancelOperation", { operation }),

  // ── Analysis ────────────────────────────────────────────
  getAnalysis: () => execData("GetAnalysis"),
  analyzeAudio: (features?: AnalysisFeatures) =>
    execData("AnalyzeAudio", { features: features ?? null }),

  // ── Python ──────────────────────────────────────────────
  getPythonStatus: () => execData("GetPythonStatus"),
  setupPythonEnv: () => exec("SetupPythonEnv"),
  startPythonSidecar: () => execData("StartPythonSidecar"),
  stopPythonSidecar: () => exec("StopPythonSidecar"),

  // ── Vixen Import ────────────────────────────────────────
  importVixenSequence: (setupSlug: string, timPath: string) =>
    execData("ImportVixenSequence", {
      setup_slug: setupSlug,
      tim_path: timPath,
    }),
  scanVixenDirectory: (vixenDir: string) =>
    execData("ScanVixenDirectory", { vixen_dir: vixenDir }),
  checkVixenPreviewFile: (filePath: string) =>
    execData("CheckVixenPreviewFile", { file_path: filePath }),
  executeVixenImport: (config: VixenImportConfig) =>
    execData("ExecuteVixenImport", config),
};
