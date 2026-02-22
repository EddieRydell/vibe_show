use std::sync::Arc;

use crate::error::AppError;
use crate::state::AppState;

use super::handlers::{
    analysis, chat, edit, import, library, media, playback, profile, profile_lib, query, script,
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

        // ── Library (sequence) ──────────────────────────────
        Command::ListLibrary => library::list_library(state),
        Command::SetLibraryGradient(p) => library::set_library_gradient(state, p),
        Command::SetLibraryCurve(p) => library::set_library_curve(state, p),
        Command::DeleteLibraryGradient(p) => library::delete_library_gradient(state, p),
        Command::DeleteLibraryCurve(p) => library::delete_library_curve(state, p),
        Command::LinkEffectToLibrary(p) => library::link_effect_to_library(state, p),
        Command::ListLibraryGradients => library::list_library_gradients(state),
        Command::ListLibraryCurves => library::list_library_curves(state),
        Command::RenameLibraryGradient(p) => library::rename_library_gradient(state, p),
        Command::RenameLibraryCurve(p) => library::rename_library_curve(state, p),

        // ── Library (profile) ───────────────────────────────
        Command::ListProfileGradients => profile_lib::list_profile_gradients(state),
        Command::SetProfileGradient(p) => profile_lib::set_profile_gradient(state, p),
        Command::DeleteProfileGradient(p) => profile_lib::delete_profile_gradient(state, p),
        Command::RenameProfileGradient(p) => profile_lib::rename_profile_gradient(state, p),
        Command::ListProfileCurves => profile_lib::list_profile_curves(state),
        Command::SetProfileCurve(p) => profile_lib::set_profile_curve(state, p),
        Command::DeleteProfileCurve(p) => profile_lib::delete_profile_curve(state, p),
        Command::RenameProfileCurve(p) => profile_lib::rename_profile_curve(state, p),
        Command::SetProfileScript(p) => profile_lib::set_profile_script(state, p),
        Command::CompileProfileScript(p) => profile_lib::compile_profile_script(state, p),

        // ── Script ──────────────────────────────────────────
        Command::GetDslReference => script::get_dsl_reference(),
        Command::WriteScript(p) => script::write_script(state, p),
        Command::GetScriptSource(p) => script::get_script_source(state, p),
        Command::DeleteScript(p) => script::delete_script(state, p),
        Command::ListScripts => script::list_scripts(state),
        Command::WriteProfileScript(p) => script::write_profile_script(state, p),
        Command::ListProfileScripts => script::list_profile_scripts(state),
        Command::GetProfileScriptSource(p) => script::get_profile_script_source(state, p),
        Command::DeleteProfileScript(p) => script::delete_profile_script(state, p),
        Command::CompileScript(p) => script::compile_script(state, p),
        Command::CompileScriptPreview(p) => script::compile_script_preview(p),
        Command::RenameScript(p) => script::rename_script(state, p),
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
