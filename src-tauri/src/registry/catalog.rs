#![allow(clippy::needless_pass_by_value)]

use schemars::schema_for;
use serde::Serialize;
use serde_json::Value;

use super::params::{
    AddEffectParams, AddTrackParams, BatchEditParams, CheckVixenPreviewFileParams,
    CompileScriptParams, CompileScriptPreviewParams, ConversationIdParams, CreateProfileParams,
    CreateSequenceParams, DeleteEffectsParams, DeleteTrackParams, GetAnalysisDetailParams,
    GetBeatsInRangeParams, GetEffectDetailParams, ImportMediaParams, ImportVixenParams,
    ImportVixenProfileParams, ImportVixenSequenceParams, InitializeDataDirParams,
    MoveEffectToTrackParams, NameParams, RenameParams,
    ScanVixenDirectoryParams, SeekParams,
    SetLlmConfigParams, SetLoopingParams, SetGlobalCurveParams, SetGlobalGradientParams,
    SetRegionParams, SlugParams, UpdateEffectParamParams, UpdateEffectTimeRangeParams,
    UpdateProfileFixturesParams, UpdateProfileLayoutParams, UpdateProfileSetupParams,
    UpdateSequenceSettingsParams, WriteScriptParams,
};
use super::{CommandCategory, CommandInfo};

/// A registry entry: metadata + JSON schema for the params.
#[derive(Debug, Clone, Serialize)]
pub struct CommandRegistryEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub category: CommandCategory,
    pub undoable: bool,
    pub llm_hidden: bool,
    pub param_schema: Value,
}

fn empty_object_schema() -> Value {
    serde_json::json!({ "type": "object", "properties": {} })
}

fn schema_value<T: schemars::JsonSchema>() -> Value {
    let root = schema_for!(T);
    serde_json::to_value(root).unwrap_or(empty_object_schema())
}

fn entry(info: CommandInfo, param_schema: Value) -> CommandRegistryEntry {
    CommandRegistryEntry {
        name: info.name,
        description: info.description,
        category: info.category,
        undoable: info.undoable,
        llm_hidden: info.is_llm_hidden(),
        param_schema,
    }
}

/// The complete command registry, auto-generated from param struct schemas.
#[allow(clippy::too_many_lines)]
pub fn command_registry() -> Vec<CommandRegistryEntry> {
    use super::Command;

    // We use dummy instances just to get the CommandInfo metadata.
    let specs: Vec<(CommandInfo, Value)> = vec![
        // ── Edit ────────────────────────────────────────────
        (
            Command::AddEffect(AddEffectParams {
                track_index: 0, kind: crate::model::EffectKind::Solid,
                start: 0.0, end: 0.0,
                blend_mode: crate::model::BlendMode::Override, opacity: 1.0,
            }).info(),
            schema_value::<AddEffectParams>(),
        ),
        (
            Command::DeleteEffects(DeleteEffectsParams { targets: vec![] }).info(),
            schema_value::<DeleteEffectsParams>(),
        ),
        (
            Command::UpdateEffectParam(UpdateEffectParamParams {
                track_index: 0, effect_index: 0,
                key: crate::model::ParamKey::Color,
                value: crate::model::ParamValue::Float(0.0),
            }).info(),
            schema_value::<UpdateEffectParamParams>(),
        ),
        (
            Command::UpdateEffectTimeRange(UpdateEffectTimeRangeParams {
                track_index: 0, effect_index: 0, start: 0.0, end: 0.0,
            }).info(),
            schema_value::<UpdateEffectTimeRangeParams>(),
        ),
        (
            Command::AddTrack(AddTrackParams { name: String::new(), fixture_id: 0 }).info(),
            schema_value::<AddTrackParams>(),
        ),
        (
            Command::DeleteTrack(DeleteTrackParams { track_index: 0 }).info(),
            schema_value::<DeleteTrackParams>(),
        ),
        (
            Command::MoveEffectToTrack(MoveEffectToTrackParams {
                from_track: 0, effect_index: 0, to_track: 0,
            }).info(),
            schema_value::<MoveEffectToTrackParams>(),
        ),
        (
            Command::UpdateSequenceSettings(UpdateSequenceSettingsParams {
                name: None, audio_file: None, duration: None, frame_rate: None,
            }).info(),
            schema_value::<UpdateSequenceSettingsParams>(),
        ),
        (
            Command::BatchEdit(BatchEditParams {
                description: String::new(), commands: vec![],
            }).info(),
            schema_value::<BatchEditParams>(),
        ),

        // ── Playback ────────────────────────────────────────
        (Command::Play.info(), empty_object_schema()),
        (Command::Pause.info(), empty_object_schema()),
        (
            Command::Seek(SeekParams { time: 0.0 }).info(),
            schema_value::<SeekParams>(),
        ),
        (Command::Undo.info(), empty_object_schema()),
        (Command::Redo.info(), empty_object_schema()),
        (Command::GetPlayback.info(), empty_object_schema()),
        (
            Command::SetRegion(SetRegionParams { region: None }).info(),
            schema_value::<SetRegionParams>(),
        ),
        (
            Command::SetLooping(SetLoopingParams { looping: false }).info(),
            schema_value::<SetLoopingParams>(),
        ),
        (Command::GetUndoState.info(), empty_object_schema()),

        // ── Query ───────────────────────────────────────────
        (Command::GetShow.info(), empty_object_schema()),
        (Command::GetEffectCatalog.info(), empty_object_schema()),
        (Command::GetDesignGuide.info(), empty_object_schema()),
        (Command::ListEffects.info(), empty_object_schema()),
        (
            Command::GetEffectDetail(GetEffectDetailParams {
                sequence_index: 0, track_index: 0, effect_index: 0,
            }).info(),
            schema_value::<GetEffectDetailParams>(),
        ),

        // ── Analysis ────────────────────────────────────────
        (Command::GetAnalysisSummary.info(), empty_object_schema()),
        (
            Command::GetBeatsInRange(GetBeatsInRangeParams { start: 0.0, end: 0.0 }).info(),
            schema_value::<GetBeatsInRangeParams>(),
        ),
        (Command::GetSections.info(), empty_object_schema()),
        (
            Command::GetAnalysisDetail(GetAnalysisDetailParams { feature: String::new() }).info(),
            schema_value::<GetAnalysisDetailParams>(),
        ),
        (Command::GetAnalysis.info(), empty_object_schema()),

        // ── Library (global) ─────────────────────────────────
        (Command::ListGlobalLibrary.info(), empty_object_schema()),
        (Command::ListGlobalGradients.info(), empty_object_schema()),
        (
            Command::SetGlobalGradient(SetGlobalGradientParams {
                name: String::new(), gradient: crate::model::ColorGradient::default(),
            }).info(),
            schema_value::<SetGlobalGradientParams>(),
        ),
        (
            Command::DeleteGlobalGradient(NameParams { name: String::new() }).info(),
            schema_value::<NameParams>(),
        ),
        (
            Command::RenameGlobalGradient(RenameParams { old_name: String::new(), new_name: String::new() }).info(),
            schema_value::<RenameParams>(),
        ),
        (Command::ListGlobalCurves.info(), empty_object_schema()),
        (
            Command::SetGlobalCurve(SetGlobalCurveParams {
                name: String::new(), curve: crate::model::Curve::default(),
            }).info(),
            schema_value::<SetGlobalCurveParams>(),
        ),
        (
            Command::DeleteGlobalCurve(NameParams { name: String::new() }).info(),
            schema_value::<NameParams>(),
        ),
        (
            Command::RenameGlobalCurve(RenameParams { old_name: String::new(), new_name: String::new() }).info(),
            schema_value::<RenameParams>(),
        ),

        // ── Script (global) ────────────────────────────────
        (Command::GetDslReference.info(), empty_object_schema()),
        (
            Command::WriteGlobalScript(WriteScriptParams { name: String::new(), source: String::new() }).info(),
            schema_value::<WriteScriptParams>(),
        ),
        (
            Command::SetGlobalScript(WriteScriptParams { name: String::new(), source: String::new() }).info(),
            schema_value::<WriteScriptParams>(),
        ),
        (
            Command::CompileGlobalScript(WriteScriptParams { name: String::new(), source: String::new() }).info(),
            schema_value::<WriteScriptParams>(),
        ),
        (Command::ListGlobalScripts.info(), empty_object_schema()),
        (
            Command::GetGlobalScriptSource(NameParams { name: String::new() }).info(),
            schema_value::<NameParams>(),
        ),
        (
            Command::DeleteGlobalScript(NameParams { name: String::new() }).info(),
            schema_value::<NameParams>(),
        ),
        (
            Command::CompileScript(CompileScriptParams { name: String::new(), source: String::new() }).info(),
            schema_value::<CompileScriptParams>(),
        ),
        (
            Command::CompileScriptPreview(CompileScriptPreviewParams { source: String::new() }).info(),
            schema_value::<CompileScriptPreviewParams>(),
        ),
        (
            Command::RenameGlobalScript(RenameParams { old_name: String::new(), new_name: String::new() }).info(),
            schema_value::<RenameParams>(),
        ),
        (
            Command::GetScriptParams(NameParams { name: String::new() }).info(),
            schema_value::<NameParams>(),
        ),

        // ── Settings ────────────────────────────────────────
        (Command::GetSettings.info(), empty_object_schema()),
        (Command::GetApiPort.info(), empty_object_schema()),
        (
            Command::InitializeDataDir(InitializeDataDirParams { data_dir: String::new() }).info(),
            schema_value::<InitializeDataDirParams>(),
        ),
        (
            Command::SetLlmConfig(SetLlmConfigParams {
                provider: crate::settings::LlmProvider::Anthropic,
                api_key: String::new(), base_url: None, model: None, chat_mode: None,
            }).info(),
            schema_value::<SetLlmConfigParams>(),
        ),
        (Command::GetLlmConfig.info(), empty_object_schema()),

        // ── Profile CRUD ────────────────────────────────────
        (Command::ListProfiles.info(), empty_object_schema()),
        (
            Command::CreateProfile(CreateProfileParams { name: String::new() }).info(),
            schema_value::<CreateProfileParams>(),
        ),
        (
            Command::OpenProfile(SlugParams { slug: String::new() }).info(),
            schema_value::<SlugParams>(),
        ),
        (
            Command::DeleteProfile(SlugParams { slug: String::new() }).info(),
            schema_value::<SlugParams>(),
        ),
        (Command::SaveProfile.info(), empty_object_schema()),
        (
            Command::UpdateProfileFixtures(UpdateProfileFixturesParams {
                fixtures: vec![], groups: vec![],
            }).info(),
            schema_value::<UpdateProfileFixturesParams>(),
        ),
        (
            Command::UpdateProfileSetup(UpdateProfileSetupParams {
                controllers: vec![], patches: vec![],
            }).info(),
            schema_value::<UpdateProfileSetupParams>(),
        ),
        (
            Command::UpdateProfileLayout(UpdateProfileLayoutParams {
                layout: crate::model::Layout { fixtures: vec![] },
            }).info(),
            schema_value::<UpdateProfileLayoutParams>(),
        ),

        // ── Sequence CRUD ───────────────────────────────────
        (Command::ListSequences.info(), empty_object_schema()),
        (
            Command::CreateSequence(CreateSequenceParams { name: String::new() }).info(),
            schema_value::<CreateSequenceParams>(),
        ),
        (
            Command::OpenSequence(SlugParams { slug: String::new() }).info(),
            schema_value::<SlugParams>(),
        ),
        (
            Command::DeleteSequence(SlugParams { slug: String::new() }).info(),
            schema_value::<SlugParams>(),
        ),
        (Command::SaveCurrentSequence.info(), empty_object_schema()),

        // ── Media ───────────────────────────────────────────
        (Command::ListMedia.info(), empty_object_schema()),
        (
            Command::ImportMedia(ImportMediaParams { source_path: String::new() }).info(),
            schema_value::<ImportMediaParams>(),
        ),
        (
            Command::DeleteMedia(NameParams { name: String::new() }).info(),
            schema_value::<NameParams>(),
        ),
        (
            Command::ResolveMediaPath(NameParams { name: String::new() }).info(),
            schema_value::<NameParams>(),
        ),

        // ── Chat ────────────────────────────────────────────
        (Command::GetChatHistory.info(), empty_object_schema()),
        (Command::GetAgentChatHistory.info(), empty_object_schema()),
        (Command::ClearChat.info(), empty_object_schema()),
        (Command::StopChat.info(), empty_object_schema()),
        (Command::ListAgentConversations.info(), empty_object_schema()),
        (Command::NewAgentConversation.info(), empty_object_schema()),
        (
            Command::SwitchAgentConversation(ConversationIdParams { conversation_id: String::new() }).info(),
            schema_value::<ConversationIdParams>(),
        ),
        (
            Command::DeleteAgentConversation(ConversationIdParams { conversation_id: String::new() }).info(),
            schema_value::<ConversationIdParams>(),
        ),

        // ── Vixen Import ────────────────────────────────────
        (
            Command::ImportVixen(ImportVixenParams {
                system_config_path: String::new(), sequence_paths: vec![],
            }).info(),
            schema_value::<ImportVixenParams>(),
        ),
        (
            Command::ImportVixenProfile(ImportVixenProfileParams {
                system_config_path: String::new(),
            }).info(),
            schema_value::<ImportVixenProfileParams>(),
        ),
        (
            Command::ImportVixenSequence(ImportVixenSequenceParams {
                profile_slug: String::new(), tim_path: String::new(),
            }).info(),
            schema_value::<ImportVixenSequenceParams>(),
        ),
        (
            Command::ScanVixenDirectory(ScanVixenDirectoryParams {
                vixen_dir: String::new(),
            }).info(),
            schema_value::<ScanVixenDirectoryParams>(),
        ),
        (
            Command::CheckVixenPreviewFile(CheckVixenPreviewFileParams {
                file_path: String::new(),
            }).info(),
            schema_value::<CheckVixenPreviewFileParams>(),
        ),
    ];

    specs.into_iter().map(|(info, schema)| entry(info, schema)).collect()
}

/// Generate the minimal `tools` array for LLM chat.
/// Instead of dumping all command schemas (which blows past token limits),
/// we expose just 3 meta-tools: `help`, `run`, and `batch`.
/// The LLM discovers specific commands via `help` and invokes them via `run`/`batch`.
pub fn to_llm_tools() -> Value {
    serde_json::json!([
        {
            "name": "help",
            "description": "Discover available commands. No args = list categories. Provide a category name to see its commands, or a command name to see its full parameter schema.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "Category name (e.g. 'edit') or command name (e.g. 'add_effect')"
                    }
                }
            }
        },
        {
            "name": "run",
            "description": "Execute a single command. Use help() first to discover command names and parameters.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Command name (e.g. 'add_effect', 'get_show')" },
                    "params": { "type": "object", "description": "Command parameters (see help for schema)" }
                },
                "required": ["command"]
            }
        },
        {
            "name": "batch",
            "description": "Execute multiple edit commands as a single undoable operation. Each entry has 'command' and optional 'params'.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "description": { "type": "string", "description": "Human-readable description of the batch" },
                    "commands": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" },
                                "params": { "type": "object" }
                            },
                            "required": ["command"]
                        }
                    }
                },
                "required": ["description", "commands"]
            }
        }
    ])
}

/// Generate help text for LLM command discovery.
/// Three tiers: no topic → categories, category → command list, command → full schema.
pub fn help_text(topic: Option<&str>) -> String {
    let registry = command_registry();
    let visible: Vec<&CommandRegistryEntry> = registry.iter().filter(|e| !e.llm_hidden).collect();

    match topic {
        None => {
            // Tier 1: category overview
            let categories = [
                ("edit", CommandCategory::Edit, "Add, delete, move effects and tracks, update params"),
                ("playback", CommandCategory::Playback, "Play, pause, seek, undo, redo"),
                ("query", CommandCategory::Query, "Inspect show state and effect types"),
                ("analysis", CommandCategory::Analysis, "Audio analysis: beats, sections, mood"),
                ("library", CommandCategory::Library, "Manage gradients, curves, scripts"),
                ("script", CommandCategory::Script, "Write and compile DSL scripts"),
                ("settings", CommandCategory::Settings, "App settings and data directory"),
                ("profile", CommandCategory::Profile, "Profile CRUD: list, create, open, delete"),
                ("sequence", CommandCategory::Sequence, "Sequence CRUD: list, create, open, delete"),
                ("media", CommandCategory::Media, "Audio file management"),
                ("chat", CommandCategory::Chat, "Chat history management"),
                ("import", CommandCategory::Import, "Vixen 3 project import"),
            ];

            let mut lines = vec!["Available command categories:".to_string()];
            for (name, cat, desc) in &categories {
                let count = visible.iter().filter(|e| e.category == *cat).count();
                if count > 0 {
                    lines.push(format!("  {name} ({count}) — {desc}"));
                }
            }
            lines.push(String::new());
            lines.push("Use help({topic: \"edit\"}) to list commands in a category.".to_string());
            lines.push("Use help({topic: \"add_effect\"}) for full parameter details.".to_string());
            lines.join("\n")
        }
        Some(topic) => {
            // Try as command name first (tier 3: full schema)
            if let Some(entry) = visible.iter().find(|e| e.name == topic) {
                let schema_str = serde_json::to_string_pretty(&entry.param_schema)
                    .unwrap_or_else(|_| "{}".to_string());
                return format!(
                    "{}: {}\nCategory: {:?} | Undoable: {}\n\nParameters:\n{}",
                    entry.name,
                    entry.description,
                    entry.category,
                    if entry.undoable { "yes" } else { "no" },
                    schema_str,
                );
            }

            // Try as category name (tier 2: command list)
            let cat_lower = topic.to_lowercase();
            let matching: Vec<&&CommandRegistryEntry> = visible
                .iter()
                .filter(|e| {
                    format!("{:?}", e.category).to_lowercase() == cat_lower
                })
                .collect();

            if matching.is_empty() {
                format!(
                    "Unknown topic: \"{topic}\". Use help() to see categories and commands."
                )
            } else {
                let mut lines = vec![format!("{topic} commands:")];
                for entry in &matching {
                    lines.push(format!("  - {}: {}", entry.name, entry.description));
                }
                lines.push(String::new());
                lines.push(
                    "Use help({topic: \"command_name\"}) for parameter details.".to_string(),
                );
                lines.join("\n")
            }
        }
    }
}

/// Generate JSON Schema formatted tool list (for MCP / REST).
pub fn to_json_schema() -> Value {
    Value::Array(
        command_registry()
            .iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "description": e.description,
                    "category": e.category,
                    "undoable": e.undoable,
                    "inputSchema": e.param_schema,
                })
            })
            .collect(),
    )
}

/// Deserialize a tool call (name + JSON input) into a Command.
#[allow(clippy::too_many_lines)]
pub fn deserialize_from_tool_call(name: &str, input: &Value) -> Result<super::Command, String> {
    use super::Command;

    match name {
        // Edit
        "add_effect" => Ok(Command::AddEffect(de(input)?)),
        "delete_effects" => Ok(Command::DeleteEffects(de(input)?)),
        "update_effect_param" => Ok(Command::UpdateEffectParam(de(input)?)),
        "update_effect_time_range" => Ok(Command::UpdateEffectTimeRange(de(input)?)),
        "add_track" => Ok(Command::AddTrack(de(input)?)),
        "delete_track" => Ok(Command::DeleteTrack(de(input)?)),
        "move_effect_to_track" => Ok(Command::MoveEffectToTrack(de(input)?)),
        "update_sequence_settings" => Ok(Command::UpdateSequenceSettings(de(input)?)),
        "batch_edit" => Ok(Command::BatchEdit(de(input)?)),
        // Playback
        "play" => Ok(Command::Play),
        "pause" => Ok(Command::Pause),
        "seek" => Ok(Command::Seek(de(input)?)),
        "undo" => Ok(Command::Undo),
        "redo" => Ok(Command::Redo),
        "get_playback" => Ok(Command::GetPlayback),
        "set_region" => Ok(Command::SetRegion(de(input)?)),
        "set_looping" => Ok(Command::SetLooping(de(input)?)),
        "get_undo_state" => Ok(Command::GetUndoState),
        // Query
        "get_show" => Ok(Command::GetShow),
        "get_effect_catalog" => Ok(Command::GetEffectCatalog),
        "get_design_guide" => Ok(Command::GetDesignGuide),
        "list_effects" => Ok(Command::ListEffects),
        "get_effect_detail" => Ok(Command::GetEffectDetail(de(input)?)),
        // Analysis
        "get_analysis_summary" => Ok(Command::GetAnalysisSummary),
        "get_beats_in_range" => Ok(Command::GetBeatsInRange(de(input)?)),
        "get_sections" => Ok(Command::GetSections),
        "get_analysis_detail" => Ok(Command::GetAnalysisDetail(de(input)?)),
        "get_analysis" => Ok(Command::GetAnalysis),
        // Library (global)
        "list_global_library" => Ok(Command::ListGlobalLibrary),
        "list_global_gradients" => Ok(Command::ListGlobalGradients),
        "set_global_gradient" => Ok(Command::SetGlobalGradient(de(input)?)),
        "delete_global_gradient" => Ok(Command::DeleteGlobalGradient(de(input)?)),
        "rename_global_gradient" => Ok(Command::RenameGlobalGradient(de(input)?)),
        "list_global_curves" => Ok(Command::ListGlobalCurves),
        "set_global_curve" => Ok(Command::SetGlobalCurve(de(input)?)),
        "delete_global_curve" => Ok(Command::DeleteGlobalCurve(de(input)?)),
        "rename_global_curve" => Ok(Command::RenameGlobalCurve(de(input)?)),
        // Script (global)
        "get_dsl_reference" => Ok(Command::GetDslReference),
        "write_global_script" => Ok(Command::WriteGlobalScript(de(input)?)),
        "set_global_script" => Ok(Command::SetGlobalScript(de(input)?)),
        "compile_global_script" => Ok(Command::CompileGlobalScript(de(input)?)),
        "list_global_scripts" => Ok(Command::ListGlobalScripts),
        "get_global_script_source" => Ok(Command::GetGlobalScriptSource(de(input)?)),
        "delete_global_script" => Ok(Command::DeleteGlobalScript(de(input)?)),
        "compile_script" => Ok(Command::CompileScript(de(input)?)),
        "compile_script_preview" => Ok(Command::CompileScriptPreview(de(input)?)),
        "rename_global_script" => Ok(Command::RenameGlobalScript(de(input)?)),
        "get_script_params" => Ok(Command::GetScriptParams(de(input)?)),
        // Settings
        "get_settings" => Ok(Command::GetSettings),
        "get_api_port" => Ok(Command::GetApiPort),
        "initialize_data_dir" => Ok(Command::InitializeDataDir(de(input)?)),
        "set_llm_config" => Ok(Command::SetLlmConfig(de(input)?)),
        "get_llm_config" => Ok(Command::GetLlmConfig),
        // Profile CRUD
        "list_profiles" => Ok(Command::ListProfiles),
        "create_profile" => Ok(Command::CreateProfile(de(input)?)),
        "open_profile" => Ok(Command::OpenProfile(de(input)?)),
        "delete_profile" => Ok(Command::DeleteProfile(de(input)?)),
        "save_profile" => Ok(Command::SaveProfile),
        "update_profile_fixtures" => Ok(Command::UpdateProfileFixtures(de(input)?)),
        "update_profile_setup" => Ok(Command::UpdateProfileSetup(de(input)?)),
        "update_profile_layout" => Ok(Command::UpdateProfileLayout(de(input)?)),
        // Sequence CRUD
        "list_sequences" => Ok(Command::ListSequences),
        "create_sequence" => Ok(Command::CreateSequence(de(input)?)),
        "open_sequence" => Ok(Command::OpenSequence(de(input)?)),
        "delete_sequence" => Ok(Command::DeleteSequence(de(input)?)),
        "save_current_sequence" => Ok(Command::SaveCurrentSequence),
        // Media
        "list_media" => Ok(Command::ListMedia),
        "import_media" => Ok(Command::ImportMedia(de(input)?)),
        "delete_media" => Ok(Command::DeleteMedia(de(input)?)),
        "resolve_media_path" => Ok(Command::ResolveMediaPath(de(input)?)),
        // Chat
        "get_chat_history" => Ok(Command::GetChatHistory),
        "get_agent_chat_history" => Ok(Command::GetAgentChatHistory),
        "clear_chat" => Ok(Command::ClearChat),
        "stop_chat" => Ok(Command::StopChat),
        "list_agent_conversations" => Ok(Command::ListAgentConversations),
        "new_agent_conversation" => Ok(Command::NewAgentConversation),
        "switch_agent_conversation" => Ok(Command::SwitchAgentConversation(de(input)?)),
        "delete_agent_conversation" => Ok(Command::DeleteAgentConversation(de(input)?)),
        // Vixen Import
        "import_vixen" => Ok(Command::ImportVixen(de(input)?)),
        "import_vixen_profile" => Ok(Command::ImportVixenProfile(de(input)?)),
        "import_vixen_sequence" => Ok(Command::ImportVixenSequence(de(input)?)),
        "scan_vixen_directory" => Ok(Command::ScanVixenDirectory(de(input)?)),
        "check_vixen_preview_file" => Ok(Command::CheckVixenPreviewFile(de(input)?)),
        _ => Err(format!("Unknown command: {name}")),
    }
}

fn de<T: serde::de::DeserializeOwned>(input: &Value) -> Result<T, String> {
    serde_json::from_value(input.clone()).map_err(|e| e.to_string())
}
