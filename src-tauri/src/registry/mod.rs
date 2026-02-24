pub mod catalog;
pub mod execute;
pub mod handlers;
pub mod params;
pub mod reference;

use serde::{Deserialize, Serialize};

use params::{
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

// ── Command enum ────────────────────────────────────────────────

/// Unified command type. Every surface (GUI, CLI, AI, REST, MCP) dispatches
/// through the same executor. Adding a variant causes compiler errors until
/// it's fully handled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
#[serde(tag = "command", content = "params")]
pub enum Command {
    // ── Edit (undoable) ─────────────────────────────────────
    AddEffect(AddEffectParams),
    DeleteEffects(DeleteEffectsParams),
    UpdateEffectParam(UpdateEffectParamParams),
    UpdateEffectTimeRange(UpdateEffectTimeRangeParams),
    AddTrack(AddTrackParams),
    DeleteTrack(DeleteTrackParams),
    MoveEffectToTrack(MoveEffectToTrackParams),
    UpdateSequenceSettings(UpdateSequenceSettingsParams),
    BatchEdit(BatchEditParams),

    // ── Playback ────────────────────────────────────────────
    Play,
    Pause,
    Seek(SeekParams),
    Undo,
    Redo,
    GetPlayback,
    SetRegion(SetRegionParams),
    SetLooping(SetLoopingParams),
    GetUndoState,

    // ── Query ───────────────────────────────────────────────
    GetShow,
    GetEffectCatalog,
    GetDesignGuide,
    ListEffects,
    GetEffectDetail(GetEffectDetailParams),

    // ── Analysis ────────────────────────────────────────────
    GetAnalysisSummary,
    GetBeatsInRange(GetBeatsInRangeParams),
    GetSections,
    GetAnalysisDetail(GetAnalysisDetailParams),
    GetAnalysis,

    // ── Library (global) ────────────────────────────────────
    ListGlobalLibrary,
    ListGlobalGradients,
    SetGlobalGradient(SetGlobalGradientParams),
    DeleteGlobalGradient(NameParams),
    RenameGlobalGradient(RenameParams),
    ListGlobalCurves,
    SetGlobalCurve(SetGlobalCurveParams),
    DeleteGlobalCurve(NameParams),
    RenameGlobalCurve(RenameParams),

    // ── Script (global) ────────────────────────────────────
    GetDslReference,
    WriteGlobalScript(WriteScriptParams),
    SetGlobalScript(WriteScriptParams),
    CompileGlobalScript(WriteScriptParams),
    ListGlobalScripts,
    GetGlobalScriptSource(NameParams),
    DeleteGlobalScript(NameParams),
    CompileScript(CompileScriptParams),
    CompileScriptPreview(CompileScriptPreviewParams),
    RenameGlobalScript(RenameParams),
    GetScriptParams(NameParams),

    // ── Settings ────────────────────────────────────────────
    GetSettings,
    GetApiPort,
    InitializeDataDir(InitializeDataDirParams),
    SetLlmConfig(SetLlmConfigParams),
    GetLlmConfig,

    // ── Profile CRUD ────────────────────────────────────────
    ListProfiles,
    CreateProfile(CreateProfileParams),
    OpenProfile(SlugParams),
    DeleteProfile(SlugParams),
    SaveProfile,
    UpdateProfileFixtures(UpdateProfileFixturesParams),
    UpdateProfileSetup(UpdateProfileSetupParams),
    UpdateProfileLayout(UpdateProfileLayoutParams),

    // ── Sequence CRUD ───────────────────────────────────────
    ListSequences,
    CreateSequence(CreateSequenceParams),
    OpenSequence(SlugParams),
    DeleteSequence(SlugParams),
    SaveCurrentSequence,

    // ── Media ───────────────────────────────────────────────
    ListMedia,
    ImportMedia(ImportMediaParams),
    DeleteMedia(NameParams),
    ResolveMediaPath(NameParams),

    // ── Chat ────────────────────────────────────────────────
    GetChatHistory,
    GetAgentChatHistory,
    ClearChat,
    StopChat,
    ListAgentConversations,
    NewAgentConversation,
    SwitchAgentConversation(ConversationIdParams),
    DeleteAgentConversation(ConversationIdParams),

    // ── Vixen Import (sync) ─────────────────────────────────
    ImportVixen(ImportVixenParams),
    ImportVixenProfile(ImportVixenProfileParams),
    ImportVixenSequence(ImportVixenSequenceParams),
    ScanVixenDirectory(ScanVixenDirectoryParams),
    CheckVixenPreviewFile(CheckVixenPreviewFileParams),
}

// ── Command metadata ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub enum CommandCategory {
    Edit,
    Playback,
    Query,
    Analysis,
    Library,
    Script,
    Settings,
    Profile,
    Sequence,
    Media,
    Chat,
    Import,
}

pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub category: CommandCategory,
    pub undoable: bool,
}

impl CommandInfo {
    /// Whether this command should be hidden from the LLM help system.
    /// Almost everything is visible. Only commands that could break the
    /// LLM's own configuration are excluded.
    pub fn is_llm_hidden(&self) -> bool {
        matches!(self.name, "set_llm_config" | "get_llm_config")
    }
}

impl Command {
    /// Metadata for this command. Exhaustive match ensures compiler errors
    /// if a new variant is added without providing metadata.
    pub fn info(&self) -> CommandInfo {
        match self {
            // ── Edit ────────────────────────────────────────
            Command::AddEffect(_) => CommandInfo {
                name: "add_effect",
                description: "Add an effect to a track. Returns the new effect index.",
                category: CommandCategory::Edit,
                undoable: true,
            },
            Command::DeleteEffects(_) => CommandInfo {
                name: "delete_effects",
                description: "Delete effects by (track_index, effect_index) pairs.",
                category: CommandCategory::Edit,
                undoable: true,
            },
            Command::UpdateEffectParam(_) => CommandInfo {
                name: "update_effect_param",
                description: "Set a parameter on an effect.",
                category: CommandCategory::Edit,
                undoable: true,
            },
            Command::UpdateEffectTimeRange(_) => CommandInfo {
                name: "update_effect_time_range",
                description: "Change the start/end time of an effect.",
                category: CommandCategory::Edit,
                undoable: true,
            },
            Command::AddTrack(_) => CommandInfo {
                name: "add_track",
                description: "Create a new track targeting a fixture. Returns the new track index.",
                category: CommandCategory::Edit,
                undoable: true,
            },
            Command::DeleteTrack(_) => CommandInfo {
                name: "delete_track",
                description: "Delete a track and all its effects by track index.",
                category: CommandCategory::Edit,
                undoable: true,
            },
            Command::MoveEffectToTrack(_) => CommandInfo {
                name: "move_effect_to_track",
                description: "Move an effect from one track to another.",
                category: CommandCategory::Edit,
                undoable: true,
            },
            Command::UpdateSequenceSettings(_) => CommandInfo {
                name: "update_sequence_settings",
                description: "Update sequence name, audio file, duration, or frame rate.",
                category: CommandCategory::Edit,
                undoable: true,
            },
            Command::BatchEdit(_) => CommandInfo {
                name: "batch_edit",
                description: "Execute multiple edit commands as a single undoable operation.",
                category: CommandCategory::Edit,
                undoable: true,
            },

            // ── Playback ────────────────────────────────────
            Command::Play => CommandInfo {
                name: "play",
                description: "Start playback.",
                category: CommandCategory::Playback,
                undoable: false,
            },
            Command::Pause => CommandInfo {
                name: "pause",
                description: "Pause playback.",
                category: CommandCategory::Playback,
                undoable: false,
            },
            Command::Seek(_) => CommandInfo {
                name: "seek",
                description: "Seek to a time in seconds.",
                category: CommandCategory::Playback,
                undoable: false,
            },
            Command::Undo => CommandInfo {
                name: "undo",
                description: "Undo the last editing action.",
                category: CommandCategory::Playback,
                undoable: false,
            },
            Command::Redo => CommandInfo {
                name: "redo",
                description: "Redo the last undone action.",
                category: CommandCategory::Playback,
                undoable: false,
            },
            Command::GetPlayback => CommandInfo {
                name: "get_playback",
                description: "Get playback state: playing, current time, duration, region, looping.",
                category: CommandCategory::Playback,
                undoable: false,
            },
            Command::SetRegion(_) => CommandInfo {
                name: "set_region",
                description: "Set or clear the playback region.",
                category: CommandCategory::Playback,
                undoable: false,
            },
            Command::SetLooping(_) => CommandInfo {
                name: "set_looping",
                description: "Enable or disable playback looping.",
                category: CommandCategory::Playback,
                undoable: false,
            },
            Command::GetUndoState => CommandInfo {
                name: "get_undo_state",
                description: "Get undo/redo availability and descriptions.",
                category: CommandCategory::Playback,
                undoable: false,
            },

            // ── Query ───────────────────────────────────────
            Command::GetShow => CommandInfo {
                name: "get_show",
                description: "Get the full show model including fixtures, tracks, and effects.",
                category: CommandCategory::Query,
                undoable: false,
            },
            Command::GetEffectCatalog => CommandInfo {
                name: "get_effect_catalog",
                description: "Get all available effect types with their parameter schemas.",
                category: CommandCategory::Query,
                undoable: false,
            },
            Command::GetDesignGuide => CommandInfo {
                name: "get_design_guide",
                description: "Get best practices for light show design.",
                category: CommandCategory::Query,
                undoable: false,
            },
            Command::ListEffects => CommandInfo {
                name: "list_effects",
                description: "List all available effect types with parameter schemas.",
                category: CommandCategory::Query,
                undoable: false,
            },
            Command::GetEffectDetail(_) => CommandInfo {
                name: "get_effect_detail",
                description: "Get schema and current params for a placed effect.",
                category: CommandCategory::Query,
                undoable: false,
            },

            // ── Analysis ────────────────────────────────────
            Command::GetAnalysisSummary => CommandInfo {
                name: "get_analysis_summary",
                description: "Get a lightweight summary of the audio analysis: tempo, key, mood, energy.",
                category: CommandCategory::Analysis,
                undoable: false,
            },
            Command::GetBeatsInRange(_) => CommandInfo {
                name: "get_beats_in_range",
                description: "Get beat timestamps within a time range.",
                category: CommandCategory::Analysis,
                undoable: false,
            },
            Command::GetSections => CommandInfo {
                name: "get_sections",
                description: "Get all structural sections with time ranges and labels.",
                category: CommandCategory::Analysis,
                undoable: false,
            },
            Command::GetAnalysisDetail(_) => CommandInfo {
                name: "get_analysis_detail",
                description: "Get full detail for one analysis feature.",
                category: CommandCategory::Analysis,
                undoable: false,
            },
            Command::GetAnalysis => CommandInfo {
                name: "get_analysis",
                description: "Get the cached audio analysis for the current sequence.",
                category: CommandCategory::Analysis,
                undoable: false,
            },

            // ── Library (global) ────────────────────────────
            Command::ListGlobalLibrary => CommandInfo {
                name: "list_global_library",
                description: "List all gradients, curves, and scripts in the global library.",
                category: CommandCategory::Library,
                undoable: false,
            },
            Command::ListGlobalGradients => CommandInfo {
                name: "list_global_gradients",
                description: "List all gradients in the global library with their data.",
                category: CommandCategory::Library,
                undoable: false,
            },
            Command::SetGlobalGradient(_) => CommandInfo {
                name: "set_global_gradient",
                description: "Create or update a named gradient in the global library.",
                category: CommandCategory::Library,
                undoable: false,
            },
            Command::DeleteGlobalGradient(_) => CommandInfo {
                name: "delete_global_gradient",
                description: "Delete a gradient from the global library.",
                category: CommandCategory::Library,
                undoable: false,
            },
            Command::RenameGlobalGradient(_) => CommandInfo {
                name: "rename_global_gradient",
                description: "Rename a gradient in the global library.",
                category: CommandCategory::Library,
                undoable: false,
            },
            Command::ListGlobalCurves => CommandInfo {
                name: "list_global_curves",
                description: "List all curves in the global library with their data.",
                category: CommandCategory::Library,
                undoable: false,
            },
            Command::SetGlobalCurve(_) => CommandInfo {
                name: "set_global_curve",
                description: "Create or update a named curve in the global library.",
                category: CommandCategory::Library,
                undoable: false,
            },
            Command::DeleteGlobalCurve(_) => CommandInfo {
                name: "delete_global_curve",
                description: "Delete a curve from the global library.",
                category: CommandCategory::Library,
                undoable: false,
            },
            Command::RenameGlobalCurve(_) => CommandInfo {
                name: "rename_global_curve",
                description: "Rename a curve in the global library.",
                category: CommandCategory::Library,
                undoable: false,
            },

            // ── Script (global) ────────────────────────────
            Command::GetDslReference => CommandInfo {
                name: "get_dsl_reference",
                description: "Get the complete DSL language reference.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::WriteGlobalScript(_) => CommandInfo {
                name: "write_global_script",
                description: "Compile and save a DSL script to the global library.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::SetGlobalScript(_) => CommandInfo {
                name: "set_global_script",
                description: "Save a script to the global library without compiling.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::CompileGlobalScript(_) => CommandInfo {
                name: "compile_global_script",
                description: "Compile and save a script to the global library.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::ListGlobalScripts => CommandInfo {
                name: "list_global_scripts",
                description: "List all script names in the global library.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::GetGlobalScriptSource(_) => CommandInfo {
                name: "get_global_script_source",
                description: "Get the source code of a named script from the global library.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::DeleteGlobalScript(_) => CommandInfo {
                name: "delete_global_script",
                description: "Delete a script from the global library.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::CompileScript(_) => CommandInfo {
                name: "compile_script",
                description: "Compile and save a DSL script to the global library.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::CompileScriptPreview(_) => CommandInfo {
                name: "compile_script_preview",
                description: "Compile a DSL script without saving. Returns compile result.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::RenameGlobalScript(_) => CommandInfo {
                name: "rename_global_script",
                description: "Rename a script in the global library.",
                category: CommandCategory::Script,
                undoable: false,
            },
            Command::GetScriptParams(_) => CommandInfo {
                name: "get_script_params",
                description: "Get the parameter definitions for a compiled script.",
                category: CommandCategory::Script,
                undoable: false,
            },

            // ── Settings ─────────────────────────────────────
            Command::GetSettings => CommandInfo {
                name: "get_settings",
                description: "Get the application settings.",
                category: CommandCategory::Settings,
                undoable: false,
            },
            Command::GetApiPort => CommandInfo {
                name: "get_api_port",
                description: "Get the HTTP API server port.",
                category: CommandCategory::Settings,
                undoable: false,
            },
            Command::InitializeDataDir(_) => CommandInfo {
                name: "initialize_data_dir",
                description: "Initialize the data directory on first launch.",
                category: CommandCategory::Settings,
                undoable: false,
            },
            Command::SetLlmConfig(_) => CommandInfo {
                name: "set_llm_config",
                description: "Configure the LLM provider, API key, and model.",
                category: CommandCategory::Settings,
                undoable: false,
            },
            Command::GetLlmConfig => CommandInfo {
                name: "get_llm_config",
                description: "Get the current LLM configuration (key is masked).",
                category: CommandCategory::Settings,
                undoable: false,
            },

            // ── Profile CRUD ─────────────────────────────────
            Command::ListProfiles => CommandInfo {
                name: "list_profiles",
                description: "List all profiles.",
                category: CommandCategory::Profile,
                undoable: false,
            },
            Command::CreateProfile(_) => CommandInfo {
                name: "create_profile",
                description: "Create a new profile.",
                category: CommandCategory::Profile,
                undoable: false,
            },
            Command::OpenProfile(_) => CommandInfo {
                name: "open_profile",
                description: "Open a profile by slug, set as current.",
                category: CommandCategory::Profile,
                undoable: false,
            },
            Command::DeleteProfile(_) => CommandInfo {
                name: "delete_profile",
                description: "Delete a profile by slug.",
                category: CommandCategory::Profile,
                undoable: false,
            },
            Command::SaveProfile => CommandInfo {
                name: "save_profile",
                description: "Save the current profile to disk.",
                category: CommandCategory::Profile,
                undoable: false,
            },
            Command::UpdateProfileFixtures(_) => CommandInfo {
                name: "update_profile_fixtures",
                description: "Update fixtures and groups in the current profile.",
                category: CommandCategory::Profile,
                undoable: false,
            },
            Command::UpdateProfileSetup(_) => CommandInfo {
                name: "update_profile_setup",
                description: "Update controllers and patches in the current profile.",
                category: CommandCategory::Profile,
                undoable: false,
            },
            Command::UpdateProfileLayout(_) => CommandInfo {
                name: "update_profile_layout",
                description: "Update the layout in the current profile.",
                category: CommandCategory::Profile,
                undoable: false,
            },

            // ── Sequence CRUD ────────────────────────────────
            Command::ListSequences => CommandInfo {
                name: "list_sequences",
                description: "List all sequences in the current profile.",
                category: CommandCategory::Sequence,
                undoable: false,
            },
            Command::CreateSequence(_) => CommandInfo {
                name: "create_sequence",
                description: "Create a new sequence in the current profile.",
                category: CommandCategory::Sequence,
                undoable: false,
            },
            Command::OpenSequence(_) => CommandInfo {
                name: "open_sequence",
                description: "Open a sequence by slug. Loads it into the editor.",
                category: CommandCategory::Sequence,
                undoable: false,
            },
            Command::DeleteSequence(_) => CommandInfo {
                name: "delete_sequence",
                description: "Delete a sequence by slug.",
                category: CommandCategory::Sequence,
                undoable: false,
            },
            Command::SaveCurrentSequence => CommandInfo {
                name: "save_current_sequence",
                description: "Save the current sequence to disk.",
                category: CommandCategory::Sequence,
                undoable: false,
            },

            // ── Media ────────────────────────────────────────
            Command::ListMedia => CommandInfo {
                name: "list_media",
                description: "List all media files in the current profile.",
                category: CommandCategory::Media,
                undoable: false,
            },
            Command::ImportMedia(_) => CommandInfo {
                name: "import_media",
                description: "Import a media file into the current profile.",
                category: CommandCategory::Media,
                undoable: false,
            },
            Command::DeleteMedia(_) => CommandInfo {
                name: "delete_media",
                description: "Delete a media file from the current profile.",
                category: CommandCategory::Media,
                undoable: false,
            },
            Command::ResolveMediaPath(_) => CommandInfo {
                name: "resolve_media_path",
                description: "Get the absolute path for a media filename.",
                category: CommandCategory::Media,
                undoable: false,
            },

            // ── Chat ─────────────────────────────────────────
            Command::GetChatHistory => CommandInfo {
                name: "get_chat_history",
                description: "Get the chat history for the current sequence.",
                category: CommandCategory::Chat,
                undoable: false,
            },
            Command::GetAgentChatHistory => CommandInfo {
                name: "get_agent_chat_history",
                description: "Get the agent chat history for the current sequence.",
                category: CommandCategory::Chat,
                undoable: false,
            },
            Command::ClearChat => CommandInfo {
                name: "clear_chat",
                description: "Clear the chat history.",
                category: CommandCategory::Chat,
                undoable: false,
            },
            Command::StopChat => CommandInfo {
                name: "stop_chat",
                description: "Cancel the in-flight chat request.",
                category: CommandCategory::Chat,
                undoable: false,
            },
            Command::ListAgentConversations => CommandInfo {
                name: "list_agent_conversations",
                description: "List all agent conversations for the current sequence.",
                category: CommandCategory::Chat,
                undoable: false,
            },
            Command::NewAgentConversation => CommandInfo {
                name: "new_agent_conversation",
                description: "Archive the current agent conversation and start a new one.",
                category: CommandCategory::Chat,
                undoable: false,
            },
            Command::SwitchAgentConversation(_) => CommandInfo {
                name: "switch_agent_conversation",
                description: "Switch to a different agent conversation by ID.",
                category: CommandCategory::Chat,
                undoable: false,
            },
            Command::DeleteAgentConversation(_) => CommandInfo {
                name: "delete_agent_conversation",
                description: "Delete an agent conversation by ID.",
                category: CommandCategory::Chat,
                undoable: false,
            },

            // ── Vixen Import ─────────────────────────────────
            Command::ImportVixen(_) => CommandInfo {
                name: "import_vixen",
                description: "Import a Vixen 3 project (profile + sequences).",
                category: CommandCategory::Import,
                undoable: false,
            },
            Command::ImportVixenProfile(_) => CommandInfo {
                name: "import_vixen_profile",
                description: "Import only the profile from a Vixen 3 project.",
                category: CommandCategory::Import,
                undoable: false,
            },
            Command::ImportVixenSequence(_) => CommandInfo {
                name: "import_vixen_sequence",
                description: "Import a single Vixen .tim sequence into an existing profile.",
                category: CommandCategory::Import,
                undoable: false,
            },
            Command::ScanVixenDirectory(_) => CommandInfo {
                name: "scan_vixen_directory",
                description: "Scan a Vixen 3 directory and return discovery info.",
                category: CommandCategory::Import,
                undoable: false,
            },
            Command::CheckVixenPreviewFile(_) => CommandInfo {
                name: "check_vixen_preview_file",
                description: "Validate a Vixen preview file and return item count.",
                category: CommandCategory::Import,
                undoable: false,
            },
        }
    }
}

// ── Command output ──────────────────────────────────────────────

/// Result of executing a Command. Dual output serves both audiences:
/// LLM/CLI gets `message`, frontend/API gets `data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
#[cfg_attr(feature = "tauri-app", ts(export))]
pub struct CommandOutput {
    pub message: String,
    #[cfg_attr(feature = "tauri-app", ts(type = "any"))]
    pub data: serde_json::Value,
}

impl CommandOutput {
    pub fn unit(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            data: serde_json::Value::Null,
        }
    }

    pub fn data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            message: message.into(),
            data,
        }
    }

    pub fn json(message: impl Into<String>, value: &impl Serialize) -> Self {
        Self {
            message: message.into(),
            data: serde_json::to_value(value).unwrap_or(serde_json::Value::Null),
        }
    }
}
