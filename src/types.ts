// TypeScript types mirroring the Rust data model.
// These are the shapes we receive from Tauri IPC commands.

export interface Color {
  r: number;
  g: number;
  b: number;
  a: number;
}

export interface Position2D {
  x: number;
  y: number;
}

export interface FixtureLayout {
  fixture_id: number;
  pixel_positions: Position2D[];
}

export interface Layout {
  fixtures: FixtureLayout[];
}

export interface FixtureDef {
  id: number;
  name: string;
  color_model: "Single" | "Rgb" | "Rgbw";
  pixel_count: number;
}

export type GroupMember = { Fixture: number } | { Group: number };

export interface FixtureGroup {
  id: number;
  name: string;
  members: GroupMember[];
}

export type EffectTarget = { Group: number } | { Fixtures: number[] } | "All";

export type BlendMode = "Override" | "Add" | "Multiply" | "Max" | "Alpha";

export type EffectKind = "Solid" | "Chase" | "Rainbow" | "Strobe" | "Gradient" | "Twinkle";

export interface TimeRange {
  start: number;
  end: number;
}

export type ParamType =
  | { Float: { min: number; max: number; step: number } }
  | { Int: { min: number; max: number } }
  | "Bool"
  | "Color"
  | { ColorList: { min_colors: number; max_colors: number } };

export type ParamValue =
  | { Float: number }
  | { Int: number }
  | { Bool: boolean }
  | { Color: Color }
  | { ColorList: Color[] }
  | { Text: string };

export interface ParamSchema {
  key: string;
  label: string;
  param_type: ParamType;
  default: ParamValue;
}

export interface EffectDetail {
  kind: EffectKind;
  schema: ParamSchema[];
  params: Record<string, ParamValue>;
  time_range: TimeRange;
  track_name: string;
  blend_mode: BlendMode;
}

export interface EffectInstance {
  kind: EffectKind;
  params: Record<string, ParamValue>;
  time_range: TimeRange;
}

export interface Track {
  name: string;
  target: EffectTarget;
  effects: EffectInstance[];
  blend_mode: BlendMode;
}

export interface Sequence {
  name: string;
  duration: number;
  frame_rate: number;
  audio_file: string | null;
  tracks: Track[];
}

export interface Show {
  name: string;
  fixtures: FixtureDef[];
  groups: FixtureGroup[];
  layout: Layout;
  sequences: Sequence[];
}

/** Frame data from the engine: fixture_id -> array of [r,g,b,a] per pixel */
export interface Frame {
  fixtures: Record<number, number[][]>;
}

export interface PlaybackInfo {
  playing: boolean;
  current_time: number;
  duration: number;
  sequence_index: number;
}

export interface TickResult {
  frame: Frame;
  current_time: number;
}

export interface EffectThumbnail {
  width: number;
  height: number;
  /** RGBA pixel data, row-major: [height * width * 4] bytes */
  pixels: number[];
  start_time: number;
  end_time: number;
}

// ── Settings & Profile types ───────────────────────────────────────

export interface AppSettings {
  version: number;
  data_dir: string;
  last_profile: string | null;
}

export interface ProfileSummary {
  name: string;
  slug: string;
  created_at: string;
  show_count: number;
  fixture_count: number;
}

export interface Profile {
  name: string;
  slug: string;
  fixtures: FixtureDef[];
  groups: FixtureGroup[];
  controllers: Controller[];
  patches: Patch[];
  layout: Layout;
}

export interface ShowSummary {
  name: string;
  slug: string;
  sequence_count: number;
}

export interface MediaFile {
  filename: string;
  size_bytes: number;
}

export interface EffectInfo {
  kind: EffectKind;
  name: string;
  schema: ParamSchema[];
}

// ── Controller / Patch types ───────────────────────────────────────

export type ControllerProtocol =
  | { E131: { unicast_address: string | null } }
  | { ArtNet: { address: string | null } }
  | { Serial: { port: string; baud_rate: number } };

export interface Controller {
  id: number;
  name: string;
  protocol: ControllerProtocol;
}

export type ChannelOrder = "Rgb" | "Grb" | "Brg" | "Rbg" | "Gbr" | "Bgr";

export type OutputMapping =
  | { Dmx: { universe: number; start_address: number; channel_order: ChannelOrder } }
  | { PixelPort: { controller_id: number; port: number; channel_order: ChannelOrder } };

export interface Patch {
  fixture_id: number;
  output: OutputMapping;
}
