pub mod catalog;
pub mod execute;
pub mod handlers;
pub mod params;
pub mod reference;
pub mod validation;

use serde::{Deserialize, Serialize};

// ── Param types (used in Command enum) ──────────────────────────
use params::{
    AddEffectParams, AddTrackParams, AnalyzeAudioParams, BatchEditParams,
    CancelOperationParams, CheckVixenPreviewFileParams,
    CompileScriptPreviewParams, ConversationIdParams, CreateSequenceParams, CreateSetupParams,
    DeleteEffectsParams, DeleteTrackParams, GetAnalysisDetailParams, GetBeatsInRangeParams,
    GetEffectDetailParams, GetFrameFilteredParams, GetFrameParams, HelpParams, ImportMediaParams,
    ImportVixenParams, ImportVixenSequenceParams, ImportVixenSetupParams, InitializeDataDirParams,
    MoveEffectToTrackParams, NameParams, PreviewScriptFrameParams, PreviewScriptParams,
    RenameParams, RenderEffectThumbnailParams, ScanVixenDirectoryParams, SeekParams,
    SendAgentMessageParams, SetGlobalCurveParams, SetGlobalGradientParams,
    SetLlmConfigParams, SetLoopingParams, SetRegionParams, SlugParams, TickParams,
    UpdateEffectParamParams, UpdateEffectTimeRangeParams, UpdateSequenceSettingsParams,
    UpdateSetupFixturesParams, UpdateSetupLayoutParams, UpdateSetupOutputsParams, WriteScriptParams,
};

// ── Return types (used in CommandResult enum) ───────────────────
use crate::chat::{ChatHistoryEntry, ConversationSummary};
use crate::commands::{EffectThumbnail, ScriptCompileResult, ScriptParamInfo, ScriptPreviewData, TickResult};
use crate::dispatcher::UndoState;
use crate::engine::Frame;
use crate::import::vixen::{VixenDiscovery, VixenImportResult};
use crate::model::{AudioAnalysis, ColorGradient, Curve, PythonEnvStatus, Show, SongSection};
use crate::settings::{AppSettings, LlmConfigInfo};
use crate::setup::{MediaFile, SequenceSummary, Setup, SetupSummary};
use crate::state::{EffectDetail, EffectInfo, PlaybackInfo};

use handlers::analysis::{AnalysisSummary, BeatsInRange};
use handlers::chat::NewConversationResult;

// ── Handler modules (dispatch targets) ──────────────────────────
use handlers::{
    agent, analysis, chat, common, edit, global_lib, hot, import, media, playback, python, query,
    script, sequence, settings, setup,
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
    Python,
    Agent,
}

impl CommandCategory {
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Playback => "playback",
            Self::Query => "query",
            Self::Analysis => "analysis",
            Self::Library => "library",
            Self::Script => "script",
            Self::Settings => "settings",
            Self::Setup => "setup",
            Self::Sequence => "sequence",
            Self::Media => "media",
            Self::Chat => "chat",
            Self::Import => "import",
            Self::Python => "python",
            Self::Agent => "agent",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Edit => "Add, delete, move effects and tracks, update params",
            Self::Playback => "Play, pause, seek, undo, redo",
            Self::Query => "Inspect show state and effect types",
            Self::Analysis => "Audio analysis: beats, sections, mood",
            Self::Library => "Manage gradients, curves, scripts",
            Self::Script => "Write and compile DSL scripts",
            Self::Settings => "App settings and data directory",
            Self::Setup => "Setup CRUD: list, create, open, delete",
            Self::Sequence => "Sequence CRUD: list, create, open, delete",
            Self::Media => "Audio file management",
            Self::Chat => "Chat history management",
            Self::Import => "Vixen 3 project import",
            Self::Python => "Python environment management",
            Self::Agent => "Agent sidecar communication",
        }
    }

    pub fn all() -> &'static [CommandCategory] {
        &[
            Self::Edit,
            Self::Playback,
            Self::Query,
            Self::Analysis,
            Self::Library,
            Self::Script,
            Self::Settings,
            Self::Setup,
            Self::Sequence,
            Self::Media,
            Self::Chat,
            Self::Import,
            Self::Python,
            Self::Agent,
        ]
    }
}

pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub category: CommandCategory,
    pub undoable: bool,
    pub llm_hidden: bool,
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

/// Single source of truth for all commands. Generates 8 artifacts:
/// 1. `Command` enum (serde-tagged, ts-rs exported)
/// 2. `CommandResult` enum (serde-tagged, ts-rs exported)
/// 3. `Command::info()` — metadata (name, description, category, undoable)
/// 4. `Command::dispatch()` — execute sync variants; errors on async
/// 5. `Command::registry_entries()` — catalog entries with JSON schemas
/// 6. `Command::from_tool_call()` — deserialize from (name, JSON) pair
/// 7. `Command::dispatch_async()` — execute ALL variants (feature-gated `tauri-app`)
/// 8. `Command::is_async()` — returns true for async variants
macro_rules! define_commands {
    (
        params {
            $(
                [ $pc:expr $(, $pf:ident)* ]
                $pv:ident ( $pp:ty ) $( -> $pr:ty )?
                => $ph:path, $pn:literal : $pd:literal ;
            )*
        }
        no_params {
            $(
                [ $nc:expr $(, $nf:ident)* ]
                $nv:ident $( -> $nr:ty )?
                => $nh:path, $nn:literal : $nd:literal ;
            )*
        }
        async_params {
            $(
                [ $apc:expr $(, $apf:ident)* ]
                $apv:ident ( $app:ty ) $( -> $apr:ty )?
                => $aph:path, $apn:literal : $apd:literal ;
            )*
        }
        async_no_params {
            $(
                [ $anc:expr $(, $anf:ident)* ]
                $anv:ident $( -> $anr:ty )?
                => $anh:path, $ann:literal : $and:literal ;
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
            $( $apv($app), )*
            $( $anv, )*
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
            $( $apv $( ($apr) )?, )*
            $( $anv $( ($anr) )?, )*
        }

        // ── 3. Command::info() ──
        impl Command {
            pub fn info(&self) -> CommandInfo {
                match self {
                    $( Command::$pv(_) => CommandInfo {
                        name: $pn,
                        description: $pd,
                        category: $pc,
                        undoable: define_commands!(@has_flag undoable; $($pf)*),
                        llm_hidden: define_commands!(@has_flag llm_hidden; $($pf)*),
                    }, )*
                    $( Command::$nv => CommandInfo {
                        name: $nn,
                        description: $nd,
                        category: $nc,
                        undoable: define_commands!(@has_flag undoable; $($nf)*),
                        llm_hidden: define_commands!(@has_flag llm_hidden; $($nf)*),
                    }, )*
                    $( Command::$apv(_) => CommandInfo {
                        name: $apn,
                        description: $apd,
                        category: $apc,
                        undoable: define_commands!(@has_flag undoable; $($apf)*),
                        llm_hidden: define_commands!(@has_flag llm_hidden; $($apf)*),
                    }, )*
                    $( Command::$anv => CommandInfo {
                        name: $ann,
                        description: $and,
                        category: $anc,
                        undoable: define_commands!(@has_flag undoable; $($anf)*),
                        llm_hidden: define_commands!(@has_flag llm_hidden; $($anf)*),
                    }, )*
                }
            }
        }

        // ── 4. Command::dispatch() — sync only ──
        impl Command {
            pub(crate) fn dispatch(
                self,
                state: &std::sync::Arc<crate::state::AppState>,
            ) -> Result<CommandOutput, crate::error::AppError> {
                match self {
                    $( Command::$pv(p) => $ph(state, p), )*
                    $( Command::$nv => $nh(state), )*
                    $( Command::$apv(_) => Err(crate::error::AppError::ApiError {
                        message: format!(
                            "Command '{}' requires async dispatch",
                            $apn,
                        ),
                    }), )*
                    $( Command::$anv => Err(crate::error::AppError::ApiError {
                        message: format!(
                            "Command '{}' requires async dispatch",
                            $ann,
                        ),
                    }), )*
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
                            undoable: define_commands!(@has_flag undoable; $($pf)*),
                            llm_hidden: define_commands!(@has_flag llm_hidden; $($pf)*),
                        },
                        catalog::schema_value::<$pp>(),
                    ), )*
                    $( catalog::entry(
                        CommandInfo {
                            name: $nn,
                            description: $nd,
                            category: $nc,
                            undoable: define_commands!(@has_flag undoable; $($nf)*),
                            llm_hidden: define_commands!(@has_flag llm_hidden; $($nf)*),
                        },
                        catalog::empty_object_schema(),
                    ), )*
                    $( catalog::entry(
                        CommandInfo {
                            name: $apn,
                            description: $apd,
                            category: $apc,
                            undoable: define_commands!(@has_flag undoable; $($apf)*),
                            llm_hidden: define_commands!(@has_flag llm_hidden; $($apf)*),
                        },
                        catalog::schema_value::<$app>(),
                    ), )*
                    $( catalog::entry(
                        CommandInfo {
                            name: $ann,
                            description: $and,
                            category: $anc,
                            undoable: define_commands!(@has_flag undoable; $($anf)*),
                            llm_hidden: define_commands!(@has_flag llm_hidden; $($anf)*),
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
                    $( $apn => Ok(Command::$apv(catalog::de(input)?)), )*
                    $( $ann => Ok(Command::$anv), )*
                    _ => Err(format!("Unknown command: {name}")),
                }
            }
        }

        // ── 7. Command::dispatch_async() — all variants (feature-gated) ──
        #[cfg(feature = "tauri-app")]
        impl Command {
            pub(crate) async fn dispatch_async(
                self,
                state: std::sync::Arc<crate::state::AppState>,
                app: Option<tauri::AppHandle>,
            ) -> Result<CommandOutput, crate::error::AppError> {
                match self {
                    // Sync params — run inline
                    $( Command::$pv(p) => $ph(&state, p), )*
                    // Sync no_params — run inline
                    $( Command::$nv => $nh(&state), )*
                    // Async params — .await
                    $( Command::$apv(p) => $aph(state, app, p).await, )*
                    // Async no_params — .await
                    $( Command::$anv => $anh(state, app).await, )*
                }
            }
        }

        // ── 8. Command::is_async() ──
        impl Command {
            pub fn is_async(&self) -> bool {
                match self {
                    $( Command::$pv(_) => false, )*
                    $( Command::$nv => false, )*
                    $( Command::$apv(_) => true, )*
                    $( Command::$anv => true, )*
                }
            }
        }
    };

    // Flag helpers — check whether a specific flag appears in a list of flags.
    // Literal tokens match before metavariables, so e.g. `undoable` matches the
    // first arm and any other ident falls through to the recursive second arm.
    (@has_flag undoable; undoable $($rest:ident)*) => { true };
    (@has_flag undoable; $_other:ident $($rest:ident)*) => { define_commands!(@has_flag undoable; $($rest)*) };
    (@has_flag undoable;) => { false };

    (@has_flag llm_hidden; llm_hidden $($rest:ident)*) => { true };
    (@has_flag llm_hidden; $_other:ident $($rest:ident)*) => { define_commands!(@has_flag llm_hidden; $($rest)*) };
    (@has_flag llm_hidden;) => { false };
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

        // ── Query (2) ───────────────────────────────────────────
        [CommandCategory::Query]
        GetEffectDetail(GetEffectDetailParams) -> EffectDetail
        => query::get_effect_detail, "get_effect_detail": "Get schema and current params for a placed effect.";

        [CommandCategory::Query]
        Help(HelpParams) -> String
        => query::help, "help": "Discover available commands and categories. Call with no args for all categories, or with a topic for details.";

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
        CompileGlobalScript(WriteScriptParams) -> ScriptCompileResult
        => global_lib::compile_global_script, "compile_global_script": "Compile and save a script to the global library.";

        [CommandCategory::Script]
        GetGlobalScriptSource(NameParams) -> String
        => script::get_global_script_source, "get_global_script_source": "Get the source code of a named script from the global library.";

        [CommandCategory::Script]
        DeleteGlobalScript(NameParams)
        => script::delete_global_script, "delete_global_script": "Delete a script from the global library.";

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

        [CommandCategory::Settings, llm_hidden]
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
        OpenSequence(SlugParams) -> Box<Show>
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

        // ── Hot-path (7) ────────────────────────────────────────
        [CommandCategory::Playback]
        Tick(TickParams) -> Option<TickResult>
        => hot::tick, "tick": "Advance playback by one frame tick. Returns frame if playing.";

        [CommandCategory::Query]
        GetFrame(GetFrameParams) -> Frame
        => hot::get_frame, "get_frame": "Evaluate and return a single frame at the given time.";

        [CommandCategory::Query]
        GetFrameFiltered(GetFrameFilteredParams) -> Frame
        => hot::get_frame_filtered, "get_frame_filtered": "Evaluate a frame rendering only specified effects.";

        [CommandCategory::Query]
        RenderEffectThumbnail(RenderEffectThumbnailParams) -> Option<EffectThumbnail>
        => hot::render_effect_thumbnail, "render_effect_thumbnail": "Pre-render an effect as a thumbnail for the timeline.";

        [CommandCategory::Script]
        PreviewScript(PreviewScriptParams) -> ScriptPreviewData
        => hot::preview_script, "preview_script": "Generate a spacetime heatmap preview for a compiled script.";

        [CommandCategory::Script]
        PreviewScriptFrame(PreviewScriptFrameParams) -> Vec<[u8; 4]>
        => hot::preview_script_frame, "preview_script_frame": "Evaluate a single frame of a compiled script.";

        // ── Cancellation (1) ────────────────────────────────────
        [CommandCategory::Settings]
        CancelOperation(CancelOperationParams) -> bool
        => common::cancel_operation, "cancel_operation": "Cancel a long-running operation by name.";
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
        GetDesignGuide -> String
        => query::get_design_guide, "get_design_guide": "Get best practices for light show design.";

        [CommandCategory::Query]
        ListEffects -> Vec<EffectInfo>
        => query::list_effects, "list_effects": "List all available effect types with parameter schemas.";

        [CommandCategory::Query]
        DescribeShow -> String
        => query::describe_show, "describe_show": "Get a human-readable description of the current show and sequence.";

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

        // ── Library (2) ─────────────────────────────────────────
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

        [CommandCategory::Settings, llm_hidden]
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

        // ── Chat (4) ────────────────────────────────────────────
        [CommandCategory::Chat]
        GetAgentChatHistory -> Vec<ChatHistoryEntry>
        => chat::get_agent_chat_history, "get_agent_chat_history": "Get the agent chat history for the current sequence.";

        [CommandCategory::Chat]
        ListAgentConversations -> Vec<ConversationSummary>
        => chat::list_agent_conversations, "list_agent_conversations": "List all agent conversations for the current sequence.";

        [CommandCategory::Chat]
        NewAgentConversation -> NewConversationResult
        => chat::new_agent_conversation, "new_agent_conversation": "Archive the current agent conversation and start a new one.";
    }
    async_params {
        // ── Import (1) ──────────────────────────────────────────
        [CommandCategory::Import]
        ExecuteVixenImport(crate::import::vixen::VixenImportConfig) -> VixenImportResult
        => import::execute_vixen_import, "execute_vixen_import": "Execute a full Vixen import from wizard configuration.";

        // ── Analysis (1) ────────────────────────────────────────
        [CommandCategory::Analysis]
        AnalyzeAudio(AnalyzeAudioParams) -> Box<AudioAnalysis>
        => analysis::analyze_audio, "analyze_audio": "Run audio analysis on the current sequence's audio file.";

        // ── Agent (1) ───────────────────────────────────────────
        [CommandCategory::Agent]
        SendAgentMessage(SendAgentMessageParams)
        => agent::send_agent_message, "send_agent_message": "Send a message to the agent sidecar.";
    }
    async_no_params {
        // ── Python (4) ──────────────────────────────────────────
        [CommandCategory::Python]
        GetPythonStatus -> PythonEnvStatus
        => python::get_python_status, "get_python_status": "Check the Python analysis environment status.";

        [CommandCategory::Python]
        SetupPythonEnv
        => python::setup_python_env, "setup_python_env": "Bootstrap the Python environment.";

        [CommandCategory::Python]
        StartPythonSidecar -> u16
        => python::start_python_sidecar, "start_python_sidecar": "Start the Python analysis sidecar. Returns port.";

        [CommandCategory::Python]
        StopPythonSidecar
        => python::stop_python_sidecar, "stop_python_sidecar": "Stop the Python analysis sidecar.";

        // ── Agent (2) ───────────────────────────────────────────
        [CommandCategory::Agent]
        CancelAgentMessage
        => agent::cancel_agent_message, "cancel_agent_message": "Cancel the in-flight agent query.";

        [CommandCategory::Agent]
        ClearAgentSession
        => agent::clear_agent_session, "clear_agent_session": "Clear the agent session and reset conversation context.";
    }
}
