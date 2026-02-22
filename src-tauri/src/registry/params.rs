use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{
    BlendMode, ColorGradient, ColorStop, Controller, Curve, CurvePoint, EffectKind, FixtureDef,
    FixtureGroup, Layout, Patch, ParamKey, ParamValue,
};
use crate::settings::{ChatMode, LlmProvider};

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
pub struct EffectTarget {
    pub track_index: usize,
    pub effect_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct DeleteEffectsParams {
    /// Array of effect targets to delete.
    pub targets: Vec<EffectTarget>,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct BatchEditParams {
    #[serde(default)]
    pub description: String,
    #[cfg_attr(feature = "tauri-app", ts(type = "any[]"))]
    #[schemars(schema_with = "batch_commands_schema")]
    pub commands: Vec<serde_json::Value>,
}

fn batch_commands_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    use schemars::schema::{
        ArrayValidation, InstanceType, ObjectValidation, Schema, SchemaObject, SingleOrVec,
    };

    let string_schema = SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
        ..SchemaObject::default()
    };
    let object_schema = SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
        ..SchemaObject::default()
    };

    let mut properties = schemars::Map::new();
    properties.insert("action".to_string(), Schema::Object(string_schema));
    properties.insert("params".to_string(), Schema::Object(object_schema));

    let item_schema = SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
        object: Some(Box::new(ObjectValidation {
            properties,
            required: ["action".to_string()].into(),
            ..ObjectValidation::default()
        })),
        ..SchemaObject::default()
    };

    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Array))),
        array: Some(Box::new(ArrayValidation {
            items: Some(schemars::schema::SingleOrVec::Single(Box::new(
                Schema::Object(item_schema),
            ))),
            ..ArrayValidation::default()
        })),
        ..SchemaObject::default()
    })
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
    #[serde(default)]
    pub audio_file: Option<Option<String>>,
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

// ── Library params ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetLibraryGradientParams {
    pub name: String,
    pub stops: Vec<ColorStop>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetLibraryCurveParams {
    pub name: String,
    pub points: Vec<CurvePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct LinkEffectToLibraryParams {
    pub track_index: usize,
    pub effect_index: usize,
    pub key: String,
    pub ref_type: String,
    pub library_name: String,
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
    pub provider: LlmProvider,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub chat_mode: Option<ChatMode>,
}

// ── Profile params ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CreateProfileParams {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateProfileFixturesParams {
    pub fixtures: Vec<FixtureDef>,
    pub groups: Vec<FixtureGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateProfileSetupParams {
    pub controllers: Vec<Controller>,
    pub patches: Vec<Patch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct UpdateProfileLayoutParams {
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

// ── Query extended params ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct GetEffectDetailParams {
    pub sequence_index: usize,
    pub track_index: usize,
    pub effect_index: usize,
}

// ── Profile library params ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetProfileGradientParams {
    pub name: String,
    pub gradient: ColorGradient,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct SetProfileCurveParams {
    pub name: String,
    pub curve: Curve,
}

// ── Script extended params ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CompileScriptParams {
    pub name: String,
    pub source: String,
}

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
pub struct ImportVixenProfileParams {
    pub system_config_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct ImportVixenSequenceParams {
    pub profile_slug: String,
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
