use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{
    BlendMode, ColorGradient, Controller, Curve, EffectKind, EffectParams, FixtureDef,
    FixtureGroup, Layout, Patch, ParamKey, ParamValue,
};
use crate::model::AnalysisFeatures;

/// Represents a field update that distinguishes "absent" from "null" from "value".
/// Use as `Option<FieldUpdate<T>>` with `#[serde(default, deserialize_with = "field_update_opt::deserialize")]`.
///
/// - `None` (field absent via `#[serde(default)]`) → skip / unchanged
/// - `Some(FieldUpdate::Clear)` (JSON `null`) → clear the field
/// - `Some(FieldUpdate::Set(v))` (JSON value) → set the field
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
#[serde(untagged)]
pub enum FieldUpdate<T> {
    Clear,
    Set(T),
}

/// Serde helper for `Option<FieldUpdate<T>>` fields.
/// Prevents `Option` from swallowing JSON `null` — instead maps it to `Some(FieldUpdate::Clear)`.
pub mod field_update_opt {
    use super::FieldUpdate;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<FieldUpdate<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        FieldUpdate::<T>::deserialize(deserializer).map(Some)
    }
}

fn default_blend_mode() -> BlendMode {
    BlendMode::Override
}

fn default_opacity() -> f64 {
    1.0
}

// ── Edit params ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct AddEffectParams {
    pub track_index: usize,
    pub kind: EffectKind,
    pub start: f64,
    pub end: f64,
    #[serde(default = "default_blend_mode")]
    pub blend_mode: BlendMode,
    #[serde(default = "default_opacity")]
    pub opacity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct EffectLocation {
    pub track_index: usize,
    pub effect_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct DeleteEffectsParams {
    /// Array of effect locations to delete.
    pub targets: Vec<EffectLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateEffectParamParams {
    pub track_index: usize,
    pub effect_index: usize,
    pub key: ParamKey,
    pub value: ParamValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateEffectTimeRangeParams {
    pub track_index: usize,
    pub effect_index: usize,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct AddTrackParams {
    pub name: String,
    pub fixture_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct DeleteTrackParams {
    pub track_index: usize,
}

/// A single action within a batch edit operation.
/// Typed union — adding a variant without handling it is a compiler error.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
#[serde(tag = "action", content = "params")]
pub enum BatchAction {
    #[serde(rename = "add_effect")]
    AddEffect(AddEffectParams),
    #[serde(rename = "delete_effects")]
    DeleteEffects(DeleteEffectsParams),
    #[serde(rename = "update_effect_param")]
    UpdateEffectParam(UpdateEffectParamParams),
    #[serde(rename = "update_effect_time_range")]
    UpdateEffectTimeRange(UpdateEffectTimeRangeParams),
    #[serde(rename = "add_track")]
    AddTrack(AddTrackParams),
    #[serde(rename = "delete_track")]
    DeleteTrack(DeleteTrackParams),
    #[serde(rename = "move_effect_to_track")]
    MoveEffectToTrack(MoveEffectToTrackParams),
    #[serde(rename = "update_sequence_settings")]
    UpdateSequenceSettings(UpdateSequenceSettingsParams),
    #[serde(rename = "write_script")]
    WriteScript(WriteScriptParams),
}

impl BatchAction {
    /// Convert a batch action into an `EditCommand`. Returns `Ok(None)` for
    /// actions that are pre-processed (e.g. `WriteScript` is compiled before
    /// the batch executes and has no corresponding `EditCommand`).
    pub fn into_edit_command(
        self,
        sequence_index: usize,
    ) -> Result<Option<crate::dispatcher::EditCommand>, String> {
        use crate::dispatcher::EditCommand;
        use crate::model::{EffectTarget, FixtureId};

        match self {
            BatchAction::AddEffect(p) => Ok(Some(EditCommand::AddEffect {
                sequence_index,
                track_index: p.track_index,
                kind: p.kind,
                start: p.start,
                end: p.end,
                blend_mode: p.blend_mode,
                opacity: p.opacity,
            })),
            BatchAction::DeleteEffects(p) => Ok(Some(EditCommand::DeleteEffects {
                sequence_index,
                targets: p.targets.into_iter().map(|t| (t.track_index, t.effect_index)).collect(),
            })),
            BatchAction::UpdateEffectParam(p) => Ok(Some(EditCommand::UpdateEffectParam {
                sequence_index,
                track_index: p.track_index,
                effect_index: p.effect_index,
                key: p.key,
                value: p.value,
            })),
            BatchAction::UpdateEffectTimeRange(p) => Ok(Some(EditCommand::UpdateEffectTimeRange {
                sequence_index,
                track_index: p.track_index,
                effect_index: p.effect_index,
                start: p.start,
                end: p.end,
            })),
            BatchAction::AddTrack(p) => Ok(Some(EditCommand::AddTrack {
                sequence_index,
                name: p.name,
                target: EffectTarget::Fixtures(vec![FixtureId(p.fixture_id)]),
            })),
            BatchAction::DeleteTrack(p) => Ok(Some(EditCommand::DeleteTrack {
                sequence_index,
                track_index: p.track_index,
            })),
            BatchAction::MoveEffectToTrack(p) => Ok(Some(EditCommand::MoveEffectToTrack {
                sequence_index,
                from_track: p.from_track,
                effect_index: p.effect_index,
                to_track: p.to_track,
            })),
            BatchAction::UpdateSequenceSettings(p) => {
                Ok(Some(EditCommand::UpdateSequenceSettings {
                    sequence_index,
                    name: p.name,
                    audio_file: p.audio_file,
                    duration: p.duration,
                    frame_rate: p.frame_rate,
                }))
            }
            BatchAction::WriteScript(_) => Ok(None), // Pre-processed
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct BatchEditParams {
    #[serde(default)]
    pub description: String,
    pub commands: Vec<BatchAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct MoveEffectToTrackParams {
    pub from_track: usize,
    pub effect_index: usize,
    pub to_track: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateSequenceSettingsParams {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "field_update_opt::deserialize")]
    pub audio_file: Option<FieldUpdate<String>>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub frame_rate: Option<f64>,
}

// ── Playback params ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SeekParams {
    pub time: f64,
}

// ── Analysis params ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct GetBeatsInRangeParams {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct GetAnalysisDetailParams {
    pub feature: String,
}

// ── Script params ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct WriteScriptParams {
    pub name: String,
    pub source: String,
}

// ── Shared params ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct NameParams {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SlugParams {
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct RenameParams {
    pub old_name: String,
    pub new_name: String,
}

// ── Settings params ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct InitializeDataDirParams {
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetLlmConfigParams {
    pub api_key: String,
    #[serde(default)]
    pub model: Option<String>,
}

// ── Setup params ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CreateSetupParams {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateSetupFixturesParams {
    pub fixtures: Vec<FixtureDef>,
    pub groups: Vec<FixtureGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateSetupOutputsParams {
    pub controllers: Vec<Controller>,
    pub patches: Vec<Patch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateSetupLayoutParams {
    pub layout: Layout,
}

// ── Sequence params ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CreateSequenceParams {
    pub name: String,
}

// ── Media params ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct ImportMediaParams {
    pub source_path: String,
}

// ── Playback extended params ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct Region {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetRegionParams {
    pub region: Option<Region>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetLoopingParams {
    pub looping: bool,
}

// ── Help params ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct HelpParams {
    /// Category name or command name to get details for (e.g. "edit", "add_effect").
    /// Omit to see all categories.
    #[serde(default)]
    pub topic: Option<String>,
}

// ── Query extended params ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct GetEffectDetailParams {
    pub sequence_index: usize,
    pub track_index: usize,
    pub effect_index: usize,
}

// ── Global library params ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetGlobalGradientParams {
    pub name: String,
    pub gradient: ColorGradient,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetGlobalCurveParams {
    pub name: String,
    pub curve: Curve,
}

// ── Script extended params ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CompileScriptPreviewParams {
    pub source: String,
}

// ── Conversation params ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct ConversationIdParams {
    pub conversation_id: String,
}

// ── Vixen import params ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct ImportVixenParams {
    pub system_config_path: String,
    pub sequence_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct ImportVixenSetupParams {
    pub system_config_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct ImportVixenSequenceParams {
    pub setup_slug: String,
    pub tim_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct ScanVixenDirectoryParams {
    pub vixen_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CheckVixenPreviewFileParams {
    pub file_path: String,
}

// ── Hot-path params ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct TickParams {
    pub dt: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct GetFrameParams {
    pub time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct GetFrameFilteredParams {
    pub time: f64,
    pub effects: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct RenderEffectThumbnailParams {
    pub sequence_index: usize,
    pub track_index: usize,
    pub effect_index: usize,
    pub time_samples: usize,
    pub pixel_rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct PreviewScriptParams {
    pub name: String,
    pub params: EffectParams,
    pub pixel_count: usize,
    pub time_samples: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct PreviewScriptFrameParams {
    pub name: String,
    pub params: EffectParams,
    pub pixel_count: usize,
    pub t: f64,
}

// ── Cancellation params ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CancelOperationParams {
    pub operation: String,
}

// ── Async command params ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct AnalyzeAudioParams {
    #[serde(default)]
    pub features: Option<AnalysisFeatures>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SendAgentMessageParams {
    pub message: String,
    #[serde(default)]
    pub context: Option<String>,
}
