use std::sync::Arc;

use crate::error::AppError;
use crate::state::AppState;

use super::handlers::{
    analysis, chat, edit, global_lib, import, media, playback, profile, query, script,
    sequence, settings,
};
use super::{Command, CommandOutput};

/// Execute a Command against the application state.
/// This is the single dispatch point for all surfaces (GUI, CLI, AI, REST, MCP).
/// The exhaustive match ensures a compiler error if a new variant is added
/// without providing an implementation.
#[allow(clippy::too_many_lines)]
pub fn execute(state: &Arc<AppState>, cmd: Command) -> Result<CommandOutput, AppError> {
    match cmd {
        // ── Edit (undoable) ─────────────────────────────────
        Command::AddEffect(p) => edit::add_effect(state, p),
        Command::DeleteEffects(p) => edit::delete_effects(state, p),
        Command::UpdateEffectParam(p) => edit::update_effect_param(state, p),
        Command::UpdateEffectTimeRange(p) => edit::update_effect_time_range(state, p),
        Command::AddTrack(p) => edit::add_track(state, p),
        Command::DeleteTrack(p) => edit::delete_track(state, p),
        Command::MoveEffectToTrack(p) => edit::move_effect_to_track(state, p),
        Command::UpdateSequenceSettings(p) => edit::update_sequence_settings(state, p),
        Command::BatchEdit(p) => edit::batch_edit(state, p),

        // ── Playback ────────────────────────────────────────
        Command::Play => playback::play(state),
        Command::Pause => playback::pause(state),
        Command::Seek(p) => playback::seek(state, p),
        Command::Undo => playback::undo(state),
        Command::Redo => playback::redo(state),
        Command::GetPlayback => playback::get_playback(state),
        Command::SetRegion(p) => playback::set_region(state, p),
        Command::SetLooping(p) => playback::set_looping(state, p),
        Command::GetUndoState => playback::get_undo_state(state),

        // ── Query ───────────────────────────────────────────
        Command::GetShow => query::get_show(state),
        Command::GetEffectCatalog => query::get_effect_catalog(state),
        Command::GetDesignGuide => query::get_design_guide(),
        Command::ListEffects => query::list_effects(),
        Command::GetEffectDetail(p) => query::get_effect_detail(state, p),

        // ── Analysis ────────────────────────────────────────
        Command::GetAnalysisSummary => analysis::get_analysis_summary(state),
        Command::GetBeatsInRange(p) => analysis::get_beats_in_range(state, p),
        Command::GetSections => analysis::get_sections(state),
        Command::GetAnalysisDetail(p) => analysis::get_analysis_detail(state, p),
        Command::GetAnalysis => analysis::get_analysis(state),

        // ── Library (global) ────────────────────────────────
        Command::ListGlobalLibrary => global_lib::list_global_library(state),
        Command::ListGlobalGradients => global_lib::list_global_gradients(state),
        Command::SetGlobalGradient(p) => global_lib::set_global_gradient(state, p),
        Command::DeleteGlobalGradient(p) => global_lib::delete_global_gradient(state, p),
        Command::RenameGlobalGradient(p) => global_lib::rename_global_gradient(state, p),
        Command::ListGlobalCurves => global_lib::list_global_curves(state),
        Command::SetGlobalCurve(p) => global_lib::set_global_curve(state, p),
        Command::DeleteGlobalCurve(p) => global_lib::delete_global_curve(state, p),
        Command::RenameGlobalCurve(p) => global_lib::rename_global_curve(state, p),

        // ── Script (global) ────────────────────────────────
        Command::GetDslReference => script::get_dsl_reference(),
        Command::WriteGlobalScript(p) => script::write_global_script(state, p),
        Command::SetGlobalScript(p) => global_lib::set_global_script(state, p),
        Command::CompileGlobalScript(p) => global_lib::compile_global_script(state, p),
        Command::ListGlobalScripts => script::list_global_scripts(state),
        Command::GetGlobalScriptSource(p) => script::get_global_script_source(state, p),
        Command::DeleteGlobalScript(p) => script::delete_global_script(state, p),
        Command::CompileScript(p) => script::compile_global_script(state, p),
        Command::CompileScriptPreview(p) => script::compile_script_preview(p),
        Command::RenameGlobalScript(p) => script::rename_global_script(state, p),
        Command::GetScriptParams(p) => script::get_script_params(state, p),

        // ── Settings ────────────────────────────────────────
        Command::GetSettings => settings::get_settings(state),
        Command::GetApiPort => settings::get_api_port(state),
        Command::InitializeDataDir(p) => settings::initialize_data_dir(state, p),
        Command::SetLlmConfig(p) => settings::set_llm_config(state, p),
        Command::GetLlmConfig => settings::get_llm_config(state),

        // ── Profile CRUD ────────────────────────────────────
        Command::ListProfiles => profile::list_profiles(state),
        Command::CreateProfile(p) => profile::create_profile(state, p),
        Command::OpenProfile(p) => profile::open_profile(state, p),
        Command::DeleteProfile(p) => profile::delete_profile(state, p),
        Command::SaveProfile => profile::save_profile(state),
        Command::UpdateProfileFixtures(p) => profile::update_profile_fixtures(state, p),
        Command::UpdateProfileSetup(p) => profile::update_profile_setup(state, p),
        Command::UpdateProfileLayout(p) => profile::update_profile_layout(state, p),

        // ── Sequence CRUD ───────────────────────────────────
        Command::ListSequences => sequence::list_sequences(state),
        Command::CreateSequence(p) => sequence::create_sequence(state, p),
        Command::OpenSequence(p) => sequence::open_sequence(state, p),
        Command::DeleteSequence(p) => sequence::delete_sequence(state, p),
        Command::SaveCurrentSequence => sequence::save_current_sequence(state),

        // ── Media ───────────────────────────────────────────
        Command::ListMedia => media::list_media(state),
        Command::ImportMedia(p) => media::import_media(state, p),
        Command::DeleteMedia(p) => media::delete_media(state, p),
        Command::ResolveMediaPath(p) => media::resolve_media_path(state, p),

        // ── Chat ────────────────────────────────────────────
        Command::GetChatHistory => chat::get_chat_history(state),
        Command::GetAgentChatHistory => chat::get_agent_chat_history(state),
        Command::ClearChat => chat::clear_chat(state),
        Command::StopChat => chat::stop_chat(state),
        Command::ListAgentConversations => chat::list_agent_conversations(state),
        Command::NewAgentConversation => chat::new_agent_conversation(state),
        Command::SwitchAgentConversation(p) => chat::switch_agent_conversation(state, p),
        Command::DeleteAgentConversation(p) => chat::delete_agent_conversation(state, p),

        // ── Vixen Import (sync) ─────────────────────────────
        Command::ImportVixen(p) => import::import_vixen(state, p),
        Command::ImportVixenProfile(p) => import::import_vixen_profile(state, p),
        Command::ImportVixenSequence(p) => import::import_vixen_sequence(state, p),
        Command::ScanVixenDirectory(p) => import::scan_vixen_directory(p),
        Command::CheckVixenPreviewFile(p) => import::check_vixen_preview_file(p),
    }
}
