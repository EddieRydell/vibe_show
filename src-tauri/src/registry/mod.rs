pub mod catalog;
pub mod execute;
pub mod handlers;
pub mod params;
pub mod reference;

use serde::{Deserialize, Serialize};

// ── Param types (used in Command enum) ──────────────────────────
use params::{
    AddEffectParams, AddTrackParams, BatchEditParams, CheckVixenPreviewFileParams,
    CompileScriptParams, CompileScriptPreviewParams, ConversationIdParams, CreateSequenceParams,
    CreateSetupParams, DeleteEffectsParams, DeleteTrackParams, GetAnalysisDetailParams,
    GetBeatsInRangeParams, GetEffectDetailParams, ImportMediaParams, ImportVixenParams,
    ImportVixenSequenceParams, ImportVixenSetupParams, InitializeDataDirParams,
    MoveEffectToTrackParams, NameParams, RenameParams, ScanVixenDirectoryParams, SeekParams,
    SetGlobalCurveParams, SetGlobalGradientParams, SetLlmConfigParams, SetLoopingParams,
    SetRegionParams, SlugParams, UpdateEffectParamParams, UpdateEffectTimeRangeParams,
    UpdateSequenceSettingsParams, UpdateSetupFixturesParams, UpdateSetupLayoutParams,
    UpdateSetupOutputsParams, WriteScriptParams,
};

// ── Return types (used in CommandResult enum) ───────────────────
use crate::chat::{ChatHistoryEntry, ConversationSummary};
use crate::commands::{ScriptCompileResult, ScriptParamInfo};
use crate::dispatcher::UndoState;
use crate::import::vixen::VixenDiscovery;
use crate::model::{AudioAnalysis, ColorGradient, Curve, Show, SongSection};
use crate::settings::{AppSettings, LlmConfigInfo};
use crate::setup::{MediaFile, SequenceSummary, Setup, SetupSummary};
use crate::state::{EffectDetail, EffectInfo, PlaybackInfo};

use handlers::analysis::{AnalysisSummary, BeatsInRange};
use handlers::chat::NewConversationResult;
use handlers::global_lib::GlobalLibrarySummary;
use handlers::query::EffectCatalogEntry;

// ── Handler modules (dispatch targets) ──────────────────────────
use handlers::{
    analysis, chat, edit, global_lib, import, media, playback, query, script, sequence, settings,
    setup,
};

// ── JsonValue newtype ───────────────────────────────────────────

/// Transparent newtype for `serde_json::Value` with a ts-rs override.
/// Used for `GetAnalysisDetail` which returns dynamic JSON.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
#[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
pub struct JsonValue(
    #[cfg_attr(feature = "tauri-app", ts(type = "unknown"))]
    pub serde_json::Value,
);

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
    Setup,
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
    pub fn is_llm_hidden(&self) -> bool {
        matches!(self.name, "set_llm_config" | "get_llm_config")
    }
}

// ── Command output ──────────────────────────────────────────────

/// Internal result of executing a Command.
/// `message` serves LLM/CLI, `result` carries typed data for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct CommandOutput {
    pub message: String,
    pub result: CommandResult,
}

impl CommandOutput {
    pub fn new(message: impl Into<String>, result: CommandResult) -> Self {
        Self {
            message: message.into(),
            result,
        }
    }
}

// ── define_commands! macro ──────────────────────────────────────

/// Single source of truth for all commands. Generates 6 artifacts:
/// 1. `Command` enum (serde-tagged, ts-rs exported)
/// 2. `CommandResult` enum (serde-tagged, ts-rs exported)
/// 3. `Command::info()` — metadata (name, description, category, undoable)
/// 4. `Command::dispatch()` — execute against AppState
/// 5. `Command::registry_entries()` — catalog entries with JSON schemas
/// 6. `Command::from_tool_call()` — deserialize from (name, JSON) pair
macro_rules! define_commands {
    (
        params {
            $(
                [ $pc:expr $(, $pf:ident)? ]
                $pv:ident ( $pp:ty ) $( -> $pr:ty )?
                => $ph:path, $pn:literal : $pd:literal ;
            )*
        }
        no_params {
            $(
                [ $nc:expr $(, $nf:ident)? ]
                $nv:ident $( -> $nr:ty )?
                => $nh:path, $nn:literal : $nd:literal ;
            )*
        }
    ) => {
        // ── 1. Command enum ──
        /// Unified command type. Every surface (GUI, CLI, AI, REST, MCP) dispatches
        /// through the same executor. Adding a variant causes compiler errors until
        /// it's fully handled.
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
        #[cfg_attr(feature = "tauri-app", ts(export))]
        #[serde(tag = "command", content = "params")]
        pub enum Command {
            $( $pv($pp), )*
            $( $nv, )*
        }

        // ── 2. CommandResult enum ──
        /// Typed result for every command. ts-rs generates a discriminated union
        /// that TypeScript can narrow by `command`.
        #[derive(Debug, Clone, Serialize)]
        #[cfg_attr(feature = "tauri-app", derive(ts_rs::TS))]
        #[cfg_attr(feature = "tauri-app", ts(export))]
        #[serde(tag = "command", content = "data")]
        pub enum CommandResult {
            $( $pv $( ($pr) )?, )*
            $( $nv $( ($nr) )?, )*
        }

        // ── 3. Command::info() ──
        impl Command {
            pub fn info(&self) -> CommandInfo {
                match self {
                    $( Command::$pv(_) => CommandInfo {
                        name: $pn,
                        description: $pd,
                        category: $pc,
                        undoable: define_commands!(@flag $($pf)?),
                    }, )*
                    $( Command::$nv => CommandInfo {
                        name: $nn,
                        description: $nd,
                        category: $nc,
                        undoable: define_commands!(@flag $($nf)?),
                    }, )*
                }
            }
        }

        // ── 4. Command::dispatch() ──
        impl Command {
            pub(crate) fn dispatch(
                self,
                state: &std::sync::Arc<crate::state::AppState>,
            ) -> Result<CommandOutput, crate::error::AppError> {
                match self {
                    $( Command::$pv(p) => $ph(state, p), )*
                    $( Command::$nv => $nh(state), )*
                }
            }
        }

        // ── 5. Command::registry_entries() ──
        impl Command {
            pub(crate) fn registry_entries() -> Vec<catalog::CommandRegistryEntry> {
                vec![
                    $( catalog::entry(
                        CommandInfo {
                            name: $pn,
                            description: $pd,
                            category: $pc,
                            undoable: define_commands!(@flag $($pf)?),
                        },
                        catalog::schema_value::<$pp>(),
                    ), )*
                    $( catalog::entry(
                        CommandInfo {
                            name: $nn,
                            description: $nd,
                            category: $nc,
                            undoable: define_commands!(@flag $($nf)?),
                        },
                        catalog::empty_object_schema(),
                    ), )*
                ]
            }
        }

        // ── 6. Command::from_tool_call() ──
        impl Command {
            pub(crate) fn from_tool_call(
                name: &str,
                input: &serde_json::Value,
            ) -> Result<Command, String> {
                match name {
                    $( $pn => Ok(Command::$pv(catalog::de(input)?)), )*
                    $( $nn => Ok(Command::$nv), )*
                    _ => Err(format!("Unknown command: {name}")),
                }
            }
        }
    };

    // Flag helpers
    (@flag undoable) => { true };
    (@flag) => { false };
}

// ── Command definitions ─────────────────────────────────────────

define_commands! {
    params {
        // ── Edit (9, all undoable) ──────────────────────────────
        [CommandCategory::Edit, undoable]
        AddEffect(AddEffectParams) -> usize
        => edit::add_effect, "add_effect": "Add an effect to a track. Returns the new effect index.";

        [CommandCategory::Edit, undoable]
        DeleteEffects(DeleteEffectsParams)
        => edit::delete_effects, "delete_effects": "Delete effects by (track_index, effect_index) pairs.";

        [CommandCategory::Edit, undoable]
        UpdateEffectParam(UpdateEffectParamParams)
        => edit::update_effect_param, "update_effect_param": "Set a parameter on an effect.";

        [CommandCategory::Edit, undoable]
        UpdateEffectTimeRange(UpdateEffectTimeRangeParams)
        => edit::update_effect_time_range, "update_effect_time_range": "Change the start/end time of an effect.";

        [CommandCategory::Edit, undoable]
        AddTrack(AddTrackParams) -> usize
        => edit::add_track, "add_track": "Create a new track targeting a fixture. Returns the new track index.";

        [CommandCategory::Edit, undoable]
        DeleteTrack(DeleteTrackParams)
        => edit::delete_track, "delete_track": "Delete a track and all its effects by track index.";

        [CommandCategory::Edit, undoable]
        MoveEffectToTrack(MoveEffectToTrackParams) -> usize
        => edit::move_effect_to_track, "move_effect_to_track": "Move an effect from one track to another.";

        [CommandCategory::Edit, undoable]
        UpdateSequenceSettings(UpdateSequenceSettingsParams)
        => edit::update_sequence_settings, "update_sequence_settings": "Update sequence name, audio file, duration, or frame rate.";

        [CommandCategory::Edit, undoable]
        BatchEdit(BatchEditParams)
        => edit::batch_edit, "batch_edit": "Execute multiple edit commands as a single undoable operation.";

        // ── Playback (3) ────────────────────────────────────────
        [CommandCategory::Playback]
        Seek(SeekParams)
        => playback::seek, "seek": "Seek to a time in seconds.";

        [CommandCategory::Playback]
        SetRegion(SetRegionParams)
        => playback::set_region, "set_region": "Set or clear the playback region.";

        [CommandCategory::Playback]
        SetLooping(SetLoopingParams)
        => playback::set_looping, "set_looping": "Enable or disable playback looping.";

        // ── Query (1) ───────────────────────────────────────────
        [CommandCategory::Query]
        GetEffectDetail(GetEffectDetailParams) -> EffectDetail
        => query::get_effect_detail, "get_effect_detail": "Get schema and current params for a placed effect.";

        // ── Analysis (2) ────────────────────────────────────────
        [CommandCategory::Analysis]
        GetBeatsInRange(GetBeatsInRangeParams) -> BeatsInRange
        => analysis::get_beats_in_range, "get_beats_in_range": "Get beat timestamps within a time range.";

        [CommandCategory::Analysis]
        GetAnalysisDetail(GetAnalysisDetailParams) -> JsonValue
        => analysis::get_analysis_detail, "get_analysis_detail": "Get full detail for one analysis feature.";

        // ── Library (6) ─────────────────────────────────────────
        [CommandCategory::Library]
        SetGlobalGradient(SetGlobalGradientParams)
        => global_lib::set_global_gradient, "set_global_gradient": "Create or update a named gradient in the global library.";

        [CommandCategory::Library]
        DeleteGlobalGradient(NameParams)
        => global_lib::delete_global_gradient, "delete_global_gradient": "Delete a gradient from the global library.";

        [CommandCategory::Library]
        RenameGlobalGradient(RenameParams)
        => global_lib::rename_global_gradient, "rename_global_gradient": "Rename a gradient in the global library.";

        [CommandCategory::Library]
        SetGlobalCurve(SetGlobalCurveParams)
        => global_lib::set_global_curve, "set_global_curve": "Create or update a named curve in the global library.";

        [CommandCategory::Library]
        DeleteGlobalCurve(NameParams)
        => global_lib::delete_global_curve, "delete_global_curve": "Delete a curve from the global library.";

        [CommandCategory::Library]
        RenameGlobalCurve(RenameParams)
        => global_lib::rename_global_curve, "rename_global_curve": "Rename a curve in the global library.";

        // ── Script (9) ──────────────────────────────────────────
        [CommandCategory::Script]
        WriteGlobalScript(WriteScriptParams)
        => script::write_global_script, "write_global_script": "Compile and save a DSL script to the global library.";

        [CommandCategory::Script]
        SetGlobalScript(WriteScriptParams)
        => global_lib::set_global_script, "set_global_script": "Save a script to the global library without compiling.";

        [CommandCategory::Script]
        CompileGlobalScript(WriteScriptParams) -> ScriptCompileResult
        => global_lib::compile_global_script, "compile_global_script": "Compile and save a script to the global library.";

        [CommandCategory::Script]
        GetGlobalScriptSource(NameParams) -> String
        => script::get_global_script_source, "get_global_script_source": "Get the source code of a named script from the global library.";

        [CommandCategory::Script]
        DeleteGlobalScript(NameParams)
        => script::delete_global_script, "delete_global_script": "Delete a script from the global library.";

        [CommandCategory::Script]
        CompileScript(CompileScriptParams) -> ScriptCompileResult
        => script::compile_global_script, "compile_script": "Compile and save a DSL script to the global library.";

        [CommandCategory::Script]
        CompileScriptPreview(CompileScriptPreviewParams) -> ScriptCompileResult
        => script::compile_script_preview, "compile_script_preview": "Compile a DSL script without saving. Returns compile result.";

        [CommandCategory::Script]
        RenameGlobalScript(RenameParams)
        => script::rename_global_script, "rename_global_script": "Rename a script in the global library.";

        [CommandCategory::Script]
        GetScriptParams(NameParams) -> Vec<ScriptParamInfo>
        => script::get_script_params, "get_script_params": "Get the parameter definitions for a compiled script.";

        // ── Settings (2) ────────────────────────────────────────
        [CommandCategory::Settings]
        InitializeDataDir(InitializeDataDirParams) -> AppSettings
        => settings::initialize_data_dir, "initialize_data_dir": "Initialize the data directory on first launch.";

        [CommandCategory::Settings]
        SetLlmConfig(SetLlmConfigParams)
        => settings::set_llm_config, "set_llm_config": "Configure the LLM provider, API key, and model.";

        // ── Setup (6) ───────────────────────────────────────────
        [CommandCategory::Setup]
        CreateSetup(CreateSetupParams) -> SetupSummary
        => setup::create_setup, "create_setup": "Create a new setup.";

        [CommandCategory::Setup]
        OpenSetup(SlugParams) -> Box<Setup>
        => setup::open_setup, "open_setup": "Open a setup by slug, set as current.";

        [CommandCategory::Setup]
        DeleteSetup(SlugParams)
        => setup::delete_setup, "delete_setup": "Delete a setup by slug.";

        [CommandCategory::Setup]
        UpdateSetupFixtures(UpdateSetupFixturesParams)
        => setup::update_setup_fixtures, "update_setup_fixtures": "Update fixtures and groups in the current setup.";

        [CommandCategory::Setup]
        UpdateSetupOutputs(UpdateSetupOutputsParams)
        => setup::update_setup_outputs, "update_setup_outputs": "Update controllers and patches in the current setup.";

        [CommandCategory::Setup]
        UpdateSetupLayout(UpdateSetupLayoutParams)
        => setup::update_setup_layout, "update_setup_layout": "Update the layout in the current setup.";

        // ── Sequence (3) ────────────────────────────────────────
        [CommandCategory::Sequence]
        CreateSequence(CreateSequenceParams) -> SequenceSummary
        => sequence::create_sequence, "create_sequence": "Create a new sequence in the current setup.";

        [CommandCategory::Sequence]
        OpenSequence(SlugParams)
        => sequence::open_sequence, "open_sequence": "Open a sequence by slug. Loads it into the editor.";

        [CommandCategory::Sequence]
        DeleteSequence(SlugParams)
        => sequence::delete_sequence, "delete_sequence": "Delete a sequence by slug.";

        // ── Media (3) ───────────────────────────────────────────
        [CommandCategory::Media]
        ImportMedia(ImportMediaParams) -> MediaFile
        => media::import_media, "import_media": "Import a media file into the current setup.";

        [CommandCategory::Media]
        DeleteMedia(NameParams)
        => media::delete_media, "delete_media": "Delete a media file from the current setup.";

        [CommandCategory::Media]
        ResolveMediaPath(NameParams) -> String
        => media::resolve_media_path, "resolve_media_path": "Get the absolute path for a media filename.";

        // ── Chat (2) ────────────────────────────────────────────
        [CommandCategory::Chat]
        SwitchAgentConversation(ConversationIdParams)
        => chat::switch_agent_conversation, "switch_agent_conversation": "Switch to a different agent conversation by ID.";

        [CommandCategory::Chat]
        DeleteAgentConversation(ConversationIdParams)
        => chat::delete_agent_conversation, "delete_agent_conversation": "Delete an agent conversation by ID.";

        // ── Import (5) ──────────────────────────────────────────
        [CommandCategory::Import]
        ImportVixen(ImportVixenParams) -> SetupSummary
        => import::import_vixen, "import_vixen": "Import a Vixen 3 project (setup + sequences).";

        [CommandCategory::Import]
        ImportVixenSetup(ImportVixenSetupParams) -> SetupSummary
        => import::import_vixen_setup, "import_vixen_setup": "Import only the setup from a Vixen 3 project.";

        [CommandCategory::Import]
        ImportVixenSequence(ImportVixenSequenceParams) -> SequenceSummary
        => import::import_vixen_sequence, "import_vixen_sequence": "Import a single Vixen .tim sequence into an existing setup.";

        [CommandCategory::Import]
        ScanVixenDirectory(ScanVixenDirectoryParams) -> Box<VixenDiscovery>
        => import::scan_vixen_directory, "scan_vixen_directory": "Scan a Vixen 3 directory and return discovery info.";

        [CommandCategory::Import]
        CheckVixenPreviewFile(CheckVixenPreviewFileParams) -> usize
        => import::check_vixen_preview_file, "check_vixen_preview_file": "Validate a Vixen preview file and return item count.";
    }
    no_params {
        // ── Playback (6) ────────────────────────────────────────
        [CommandCategory::Playback]
        Play => playback::play, "play": "Start playback.";

        [CommandCategory::Playback]
        Pause => playback::pause, "pause": "Pause playback.";

        [CommandCategory::Playback]
        Undo => playback::undo, "undo": "Undo the last editing action.";

        [CommandCategory::Playback]
        Redo => playback::redo, "redo": "Redo the last undone action.";

        [CommandCategory::Playback]
        GetPlayback -> PlaybackInfo
        => playback::get_playback, "get_playback": "Get playback state: playing, current time, duration, region, looping.";

        [CommandCategory::Playback]
        GetUndoState -> UndoState
        => playback::get_undo_state, "get_undo_state": "Get undo/redo availability and descriptions.";

        // ── Query (4) ───────────────────────────────────────────
        [CommandCategory::Query]
        GetShow -> Box<Show>
        => query::get_show, "get_show": "Get the full show model including fixtures, tracks, and effects.";

        [CommandCategory::Query]
        GetEffectCatalog -> Vec<EffectCatalogEntry>
        => query::get_effect_catalog, "get_effect_catalog": "Get all available effect types with their parameter schemas.";

        [CommandCategory::Query]
        GetDesignGuide -> String
        => query::get_design_guide, "get_design_guide": "Get best practices for light show design.";

        [CommandCategory::Query]
        ListEffects -> Vec<EffectInfo>
        => query::list_effects, "list_effects": "List all available effect types with parameter schemas.";

        // ── Analysis (3) ────────────────────────────────────────
        [CommandCategory::Analysis]
        GetAnalysisSummary -> AnalysisSummary
        => analysis::get_analysis_summary, "get_analysis_summary": "Get a lightweight summary of the audio analysis: tempo, key, mood, energy.";

        [CommandCategory::Analysis]
        GetSections -> Vec<SongSection>
        => analysis::get_sections, "get_sections": "Get all structural sections with time ranges and labels.";

        [CommandCategory::Analysis]
        GetAnalysis -> Option<Box<AudioAnalysis>>
        => analysis::get_analysis, "get_analysis": "Get the cached audio analysis for the current sequence.";

        // ── Library (3) ─────────────────────────────────────────
        [CommandCategory::Library]
        ListGlobalLibrary -> GlobalLibrarySummary
        => global_lib::list_global_library, "list_global_library": "List all gradients, curves, and scripts in the global library.";

        [CommandCategory::Library]
        ListGlobalGradients -> Vec<(String, ColorGradient)>
        => global_lib::list_global_gradients, "list_global_gradients": "List all gradients in the global library with their data.";

        [CommandCategory::Library]
        ListGlobalCurves -> Vec<(String, Curve)>
        => global_lib::list_global_curves, "list_global_curves": "List all curves in the global library with their data.";

        // ── Script (2) ──────────────────────────────────────────
        [CommandCategory::Script]
        GetDslReference -> String
        => script::get_dsl_reference, "get_dsl_reference": "Get the complete DSL language reference.";

        [CommandCategory::Script]
        ListGlobalScripts -> Vec<(String, String)>
        => script::list_global_scripts, "list_global_scripts": "List all script names in the global library.";

        // ── Settings (3) ────────────────────────────────────────
        [CommandCategory::Settings]
        GetSettings -> Option<AppSettings>
        => settings::get_settings, "get_settings": "Get the application settings.";

        [CommandCategory::Settings]
        GetApiPort -> u16
        => settings::get_api_port, "get_api_port": "Get the HTTP API server port.";

        [CommandCategory::Settings]
        GetLlmConfig -> LlmConfigInfo
        => settings::get_llm_config, "get_llm_config": "Get the current LLM configuration (key is masked).";

        // ── Setup (2) ───────────────────────────────────────────
        [CommandCategory::Setup]
        ListSetups -> Vec<SetupSummary>
        => setup::list_setups, "list_setups": "List all setups.";

        [CommandCategory::Setup]
        SaveSetup => setup::save_setup, "save_setup": "Save the current setup to disk.";

        // ── Sequence (2) ────────────────────────────────────────
        [CommandCategory::Sequence]
        ListSequences -> Vec<SequenceSummary>
        => sequence::list_sequences, "list_sequences": "List all sequences in the current setup.";

        [CommandCategory::Sequence]
        SaveCurrentSequence
        => sequence::save_current_sequence, "save_current_sequence": "Save the current sequence to disk.";

        // ── Media (1) ───────────────────────────────────────────
        [CommandCategory::Media]
        ListMedia -> Vec<MediaFile>
        => media::list_media, "list_media": "List all media files in the current setup.";

        // ── Chat (6) ────────────────────────────────────────────
        [CommandCategory::Chat]
        GetChatHistory -> Vec<ChatHistoryEntry>
        => chat::get_chat_history, "get_chat_history": "Get the chat history for the current sequence.";

        [CommandCategory::Chat]
        GetAgentChatHistory -> Vec<ChatHistoryEntry>
        => chat::get_agent_chat_history, "get_agent_chat_history": "Get the agent chat history for the current sequence.";

        [CommandCategory::Chat]
        ClearChat => chat::clear_chat, "clear_chat": "Clear the chat history.";

        [CommandCategory::Chat]
        StopChat => chat::stop_chat, "stop_chat": "Cancel the in-flight chat request.";

        [CommandCategory::Chat]
        ListAgentConversations -> Vec<ConversationSummary>
        => chat::list_agent_conversations, "list_agent_conversations": "List all agent conversations for the current sequence.";

        [CommandCategory::Chat]
        NewAgentConversation -> NewConversationResult
        => chat::new_agent_conversation, "new_agent_conversation": "Archive the current agent conversation and start a new one.";
    }
}
