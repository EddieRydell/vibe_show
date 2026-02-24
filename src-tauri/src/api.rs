use std::sync::Arc;

use axum::extract::{Path, Query, State as AxumState};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::chat::{ChatHistoryEntry, ChatManager, NoopChatEmitter};
use crate::describe;
use crate::dispatcher::{CommandResult, EditCommand, UndoState};
use crate::effects::resolve_effect;
use crate::engine::{self, Frame};
use crate::model::Show;
use crate::profile::{self, MediaFile, ProfileSummary, SequenceSummary, MEDIA_EXTENSIONS};
use crate::settings::{self, AppSettings};
use crate::state::{self, AppState, EffectDetail, EffectInfo, PlaybackInfo};
use crate::registry;

/// JSON response envelope for API calls.
#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn success(data: T) -> Json<Self> {
        Json(Self {
            ok: true,
            data: Some(data),
            description: None,
            error: None,
        })
    }

    fn success_with_desc(data: T, description: String) -> Json<Self> {
        Json(Self {
            ok: true,
            data: Some(data),
            description: Some(description),
            error: None,
        })
    }
}

fn error_response(msg: String) -> (StatusCode, Json<ApiResponse<()>>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiResponse {
            ok: false,
            data: None,
            description: None,
            error: Some(msg),
        }),
    )
}

type ApiResult<T> = Result<Json<ApiResponse<T>>, (StatusCode, Json<ApiResponse<()>>)>;
type AppArc = AxumState<Arc<AppState>>;

fn get_data_dir(state: &AppState) -> Result<std::path::PathBuf, (StatusCode, Json<ApiResponse<()>>)> {
    state::get_data_dir(state).map_err(error_response)
}

fn require_profile(state: &AppState) -> Result<String, (StatusCode, Json<ApiResponse<()>>)> {
    state
        .current_profile
        .lock()
        .clone()
        .ok_or_else(|| error_response("No profile open".into()))
}

fn require_sequence(state: &AppState) -> Result<String, (StatusCode, Json<ApiResponse<()>>)> {
    state
        .current_sequence
        .lock()
        .clone()
        .ok_or_else(|| error_response("No sequence open".into()))
}

// ══════════════════════════════════════════════════════════════════
// GET handlers (existing)
// ══════════════════════════════════════════════════════════════════

async fn get_show(AxumState(state): AppArc) -> Json<ApiResponse<Show>> {
    let show = state.show.lock().clone();
    ApiResponse::success(show)
}

async fn get_effects(_: AppArc) -> Json<ApiResponse<Vec<EffectInfo>>> {
    ApiResponse::success(state::all_effect_info())
}

async fn get_playback(AxumState(state): AppArc) -> Json<ApiResponse<PlaybackInfo>> {
    let playback = state.playback.lock();
    let show = state.show.lock();
    let duration = show
        .sequences
        .get(playback.sequence_index)
        .map_or(0.0, |s| s.duration);
    ApiResponse::success(PlaybackInfo {
        playing: playback.playing,
        current_time: playback.current_time,
        duration,
        sequence_index: playback.sequence_index,
        region: playback.region,
        looping: playback.looping,
    })
}

async fn get_effect_detail(
    AxumState(state): AppArc,
    Path((seq, track, idx)): Path<(usize, usize, usize)>,
) -> ApiResult<EffectDetail> {
    let show = state.show.lock();
    let sequence = show.sequences.get(seq).ok_or_else(|| error_response("Invalid sequence index".into()))?;
    let t = sequence.tracks.get(track).ok_or_else(|| error_response("Invalid track index".into()))?;
    let effect_instance = t.effects.get(idx).ok_or_else(|| error_response("Invalid effect index".into()))?;
    let schema = resolve_effect(&effect_instance.kind)
        .map_or_else(Vec::new, |e| e.param_schema());
    Ok(ApiResponse::success(EffectDetail {
        kind: effect_instance.kind.clone(),
        schema,
        params: effect_instance.params.clone(),
        time_range: effect_instance.time_range,
        track_name: t.name.clone(),
        blend_mode: effect_instance.blend_mode,
        opacity: effect_instance.opacity,
    }))
}

async fn get_undo_state(AxumState(state): AppArc) -> Json<ApiResponse<UndoState>> {
    let dispatcher = state.dispatcher.lock();
    ApiResponse::success(dispatcher.undo_state())
}

// ══════════════════════════════════════════════════════════════════
// POST handlers (existing editing commands)
// ══════════════════════════════════════════════════════════════════

#[derive(Serialize)]
struct CommandResponse {
    result: String,
}

async fn post_command(
    AxumState(state): AppArc,
    Json(cmd): Json<EditCommand>,
) -> ApiResult<CommandResponse> {
    let description = cmd.description();
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let result = dispatcher
        .execute(&mut show, &cmd)
        .map_err(|e| error_response(e.to_string()))?;
    let result_str = match result {
        CommandResult::Index(i) => format!("{i}"),
        CommandResult::Bool(b) => format!("{b}"),
        CommandResult::Unit => "ok".to_string(),
    };
    Ok(ApiResponse::success_with_desc(
        CommandResponse { result: result_str },
        description,
    ))
}

async fn post_undo(AxumState(state): AppArc) -> ApiResult<String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let desc = dispatcher.undo(&mut show).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(desc))
}

async fn post_redo(AxumState(state): AppArc) -> ApiResult<String> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    let desc = dispatcher.redo(&mut show).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(desc))
}

async fn post_play(AxumState(state): AppArc) -> Json<ApiResponse<()>> {
    let mut playback = state.playback.lock();
    playback.playing = true;
    playback.last_tick = Some(std::time::Instant::now());
    ApiResponse::success(())
}

async fn post_pause(AxumState(state): AppArc) -> Json<ApiResponse<()>> {
    let mut playback = state.playback.lock();
    playback.playing = false;
    playback.last_tick = None;
    ApiResponse::success(())
}

#[derive(Deserialize)]
struct SeekBody {
    time: f64,
}

async fn post_seek(
    AxumState(state): AppArc,
    Json(body): Json<SeekBody>,
) -> Json<ApiResponse<()>> {
    let mut playback = state.playback.lock();
    playback.current_time = body.time.max(0.0);
    if playback.playing {
        playback.last_tick = Some(std::time::Instant::now());
    }
    ApiResponse::success(())
}

async fn post_save(AxumState(state): AppArc) -> ApiResult<()> {
    let data_dir = get_data_dir(&state)?;
    let profile_slug = require_profile(&state)?;
    let seq_slug = require_sequence(&state)?;
    let show = state.show.lock();
    let sequence = show
        .sequences
        .first()
        .ok_or_else(|| error_response("No sequence in show".into()))?;
    profile::save_sequence(&data_dir, &profile_slug, &seq_slug, sequence)
        .map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

// ══════════════════════════════════════════════════════════════════
// Settings endpoints
// ══════════════════════════════════════════════════════════════════

async fn get_settings(AxumState(state): AppArc) -> Json<ApiResponse<Option<AppSettings>>> {
    let s = state.settings.lock().clone();
    ApiResponse::success(s)
}

#[derive(Deserialize)]
struct InitBody {
    data_dir: String,
}

async fn post_initialize(
    AxumState(state): AppArc,
    Json(body): Json<InitBody>,
) -> ApiResult<AppSettings> {
    let data_path = std::path::PathBuf::from(&body.data_dir);
    std::fs::create_dir_all(data_path.join("profiles")).map_err(|e| error_response(e.to_string()))?;
    let new_settings = AppSettings::new(data_path);
    settings::save_settings(&state.app_config_dir, &new_settings)
        .map_err(|e| error_response(e.to_string()))?;
    *state.settings.lock() = Some(new_settings.clone());
    Ok(ApiResponse::success(new_settings))
}

// ══════════════════════════════════════════════════════════════════
// Profile endpoints
// ══════════════════════════════════════════════════════════════════

async fn get_profiles(AxumState(state): AppArc) -> ApiResult<Vec<ProfileSummary>> {
    let data_dir = get_data_dir(&state)?;
    let profiles = profile::list_profiles(&data_dir).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(profiles))
}

#[derive(Deserialize)]
struct CreateProfileBody {
    name: String,
}

async fn post_profiles(
    AxumState(state): AppArc,
    Json(body): Json<CreateProfileBody>,
) -> ApiResult<ProfileSummary> {
    let data_dir = get_data_dir(&state)?;
    let summary = profile::create_profile(&data_dir, &body.name).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(summary))
}

async fn get_profile(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
) -> ApiResult<crate::profile::Profile> {
    let data_dir = get_data_dir(&state)?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(|e| error_response(e.to_string()))?;
    *state.current_profile.lock() = Some(slug.clone());
    *state.current_sequence.lock() = None;

    // Update last_profile
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.last_profile = Some(slug);
        if let Err(e) = settings::save_settings(&state.app_config_dir, s) {
            eprintln!("[VibeLights] Failed to save settings: {e}");
        }
    }

    Ok(ApiResponse::success(loaded))
}

async fn delete_profile_handler(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
) -> ApiResult<()> {
    let data_dir = get_data_dir(&state)?;
    profile::delete_profile(&data_dir, &slug).map_err(|e| error_response(e.to_string()))?;

    let mut current = state.current_profile.lock();
    if current.as_deref() == Some(&slug) {
        *current = None;
        *state.current_sequence.lock() = None;
    }

    Ok(ApiResponse::success(()))
}

async fn post_profile_save(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
) -> ApiResult<()> {
    let data_dir = get_data_dir(&state)?;
    let loaded = profile::load_profile(&data_dir, &slug).map_err(|e| error_response(e.to_string()))?;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

#[derive(Deserialize)]
struct UpdateFixturesBody {
    fixtures: Vec<crate::model::FixtureDef>,
    groups: Vec<crate::model::FixtureGroup>,
}

async fn put_profile_fixtures(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
    Json(body): Json<UpdateFixturesBody>,
) -> ApiResult<()> {
    let data_dir = get_data_dir(&state)?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(|e| error_response(e.to_string()))?;
    loaded.fixtures = body.fixtures;
    loaded.groups = body.groups;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

#[derive(Deserialize)]
struct UpdateSetupBody {
    controllers: Vec<crate::model::Controller>,
    patches: Vec<crate::model::fixture::Patch>,
}

async fn put_profile_setup(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
    Json(body): Json<UpdateSetupBody>,
) -> ApiResult<()> {
    let data_dir = get_data_dir(&state)?;
    let mut loaded = profile::load_profile(&data_dir, &slug).map_err(|e| error_response(e.to_string()))?;
    loaded.controllers = body.controllers;
    loaded.patches = body.patches;
    profile::save_profile(&data_dir, &slug, &loaded).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

// ══════════════════════════════════════════════════════════════════
// Sequence endpoints
// ══════════════════════════════════════════════════════════════════

async fn get_sequences(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
) -> ApiResult<Vec<SequenceSummary>> {
    let data_dir = get_data_dir(&state)?;
    let seqs = profile::list_sequences(&data_dir, &slug).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(seqs))
}

#[derive(Deserialize)]
struct CreateSequenceBody {
    name: String,
}

async fn post_sequences(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
    Json(body): Json<CreateSequenceBody>,
) -> ApiResult<SequenceSummary> {
    let data_dir = get_data_dir(&state)?;
    let summary = profile::create_sequence(&data_dir, &slug, &body.name)
        .map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(summary))
}

async fn get_sequence(
    AxumState(state): AppArc,
    Path((profile_slug, seq_slug)): Path<(String, String)>,
) -> ApiResult<Show> {
    let data_dir = get_data_dir(&state)?;
    let profile_data = profile::load_profile(&data_dir, &profile_slug)
        .map_err(|e| error_response(e.to_string()))?;
    let sequence = profile::load_sequence(&data_dir, &profile_slug, &seq_slug)
        .map_err(|e| error_response(e.to_string()))?;
    let assembled = profile::assemble_show(&profile_data, &sequence);

    *state.show.lock() = assembled.clone();
    state.dispatcher.lock().clear();
    {
        let mut playback = state.playback.lock();
        playback.playing = false;
        playback.current_time = 0.0;
        playback.sequence_index = 0;
    }
    *state.current_profile.lock() = Some(profile_slug);
    *state.current_sequence.lock() = Some(seq_slug);

    Ok(ApiResponse::success(assembled))
}

async fn delete_sequence_handler(
    AxumState(state): AppArc,
    Path((profile_slug, seq_slug)): Path<(String, String)>,
) -> ApiResult<()> {
    let data_dir = get_data_dir(&state)?;
    profile::delete_sequence(&data_dir, &profile_slug, &seq_slug)
        .map_err(|e| error_response(e.to_string()))?;

    let mut current = state.current_sequence.lock();
    if current.as_deref() == Some(&seq_slug) {
        *current = None;
    }

    Ok(ApiResponse::success(()))
}

// ══════════════════════════════════════════════════════════════════
// Media endpoints
// ══════════════════════════════════════════════════════════════════

async fn get_media(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
) -> ApiResult<Vec<MediaFile>> {
    let data_dir = get_data_dir(&state)?;
    let files = profile::list_media(&data_dir, &slug).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(files))
}

#[derive(Deserialize)]
struct ImportMediaBody {
    source_path: String,
}

async fn post_media(
    AxumState(state): AppArc,
    Path(slug): Path<String>,
    Json(body): Json<ImportMediaBody>,
) -> ApiResult<MediaFile> {
    let data_dir = get_data_dir(&state)?;
    let file = profile::import_media(&data_dir, &slug, std::path::Path::new(&body.source_path))
        .map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(file))
}

async fn delete_media_handler(
    AxumState(state): AppArc,
    Path((slug, filename)): Path<(String, String)>,
) -> ApiResult<()> {
    let data_dir = get_data_dir(&state)?;
    profile::delete_media(&data_dir, &slug, &filename).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

// ══════════════════════════════════════════════════════════════════
// Rendering / describe endpoints
// ══════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
struct FrameQuery {
    time: f64,
}

async fn get_frame(
    AxumState(state): AppArc,
    Query(q): Query<FrameQuery>,
) -> Json<ApiResponse<Frame>> {
    let show = state.show.lock();
    let playback = state.playback.lock();
    let frame = engine::evaluate(&show, playback.sequence_index, q.time, None, None);
    ApiResponse::success(frame)
}

#[derive(Deserialize)]
struct DescribeQuery {
    frame_time: Option<f64>,
}

async fn get_describe(
    AxumState(state): AppArc,
    Query(q): Query<DescribeQuery>,
) -> Json<ApiResponse<String>> {
    let show = state.show.lock();
    let mut text = describe::describe_show(&show);

    if let Some(seq) = show.sequences.first() {
        text.push_str("\n\n");
        text.push_str(&describe::describe_sequence(seq));
    }

    if let Some(t) = q.frame_time {
        let playback = state.playback.lock();
        let frame = engine::evaluate(&show, playback.sequence_index, t, None, None);
        text.push_str("\n\n");
        text.push_str(&describe::describe_frame(&show, &frame));
    }

    ApiResponse::success(text)
}

// ══════════════════════════════════════════════════════════════════
// Chat endpoints
// ══════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
struct ChatBody {
    message: String,
}

#[derive(Serialize)]
struct ChatResponse {
    response: Option<String>,
}

async fn post_chat(
    AxumState(state): AppArc,
    Json(body): Json<ChatBody>,
) -> ApiResult<ChatResponse> {
    let emitter = NoopChatEmitter;
    ChatManager::send_message(state.clone(), &emitter, body.message)
        .await
        .map_err(error_response)?;
    let response = state.chat.lock().last_assistant_text();
    Ok(ApiResponse::success(ChatResponse { response }))
}

async fn get_chat_history(AxumState(state): AppArc) -> Json<ApiResponse<Vec<ChatHistoryEntry>>> {
    let history = state.chat.lock().history_for_display();
    ApiResponse::success(history)
}

async fn post_chat_clear(AxumState(state): AppArc) -> Json<ApiResponse<()>> {
    state.chat.lock().clear();
    ApiResponse::success(())
}

async fn post_chat_stop(AxumState(state): AppArc) -> Json<ApiResponse<()>> {
    state.chat.lock().cancel();
    ApiResponse::success(())
}

#[derive(Deserialize)]
struct LlmConfigBody {
    provider: crate::settings::LlmProvider,
    api_key: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

async fn put_chat_api_key(
    AxumState(state): AppArc,
    Json(body): Json<LlmConfigBody>,
) -> ApiResult<()> {
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.llm = crate::settings::LlmProviderConfig {
            provider: body.provider,
            api_key: if body.api_key.is_empty() {
                None
            } else {
                Some(body.api_key.clone())
            },
            base_url: body.base_url,
            model: body.model,
        };
        settings::save_settings(&state.app_config_dir, s)
            .map_err(|e| error_response(e.to_string()))?;
        settings::save_api_key(&state.app_config_dir, &body.api_key)
            .map_err(|e| error_response(e.to_string()))?;
    }
    Ok(ApiResponse::success(()))
}

// ══════════════════════════════════════════════════════════════════
// Vixen import endpoints
// ══════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
struct VixenScanBody {
    vixen_dir: String,
}

async fn post_vixen_scan(
    Json(body): Json<VixenScanBody>,
) -> ApiResult<crate::import::vixen::VixenDiscovery> {
    let vixen_path = std::path::Path::new(&body.vixen_dir);
    let config_path = vixen_path.join(crate::import::VIXEN_SYSTEM_DATA_DIR).join(crate::import::VIXEN_SYSTEM_CONFIG_FILE);
    if !config_path.exists() {
        return Err(error_response(format!(
            "Not a valid Vixen 3 directory: SystemData/SystemConfig.xml not found in {}",
            body.vixen_dir
        )));
    }

    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(&config_path)
        .map_err(|e| error_response(e.to_string()))?;

    let fixtures_found = importer.fixture_count();
    let groups_found = importer.group_count();
    let controllers_found = importer.controller_count();

    let preview_file = crate::import::vixen_preview::find_preview_file(vixen_path);
    let (preview_available, preview_item_count) = if let Some(ref pf) = preview_file {
        match crate::import::vixen_preview::parse_preview_file(pf) {
            Ok(data) => (!data.display_items.is_empty(), data.display_items.len()),
            Err(_) => (false, 0),
        }
    } else {
        (false, 0)
    };
    let preview_file_path = preview_file.map(|p| p.to_string_lossy().to_string());

    let mut sequences = Vec::new();
    let seq_dir = vixen_path.join("Sequence");
    if seq_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&seq_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if ext == crate::import::VIXEN_SEQUENCE_EXT {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        sequences.push(crate::import::vixen::VixenSequenceInfo {
                            filename,
                            path: path.to_string_lossy().to_string(),
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    sequences.sort_by(|a, b| a.filename.cmp(&b.filename));

    let mut media_files = Vec::new();
    let media_dir = vixen_path.join("Media");
    if media_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&media_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if MEDIA_EXTENSIONS.contains(&ext.as_str()) {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        media_files.push(crate::import::vixen::VixenMediaInfo {
                            filename,
                            path: path.to_string_lossy().to_string(),
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    media_files.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(ApiResponse::success(crate::import::vixen::VixenDiscovery {
        vixen_dir: body.vixen_dir,
        fixtures_found,
        groups_found,
        controllers_found,
        preview_available,
        preview_item_count,
        preview_file_path,
        sequences,
        media_files,
    }))
}

#[derive(Deserialize)]
struct VixenCheckPreviewBody {
    file_path: String,
}

async fn post_vixen_check_preview(
    Json(body): Json<VixenCheckPreviewBody>,
) -> ApiResult<usize> {
    let path = std::path::Path::new(&body.file_path);
    if !path.exists() {
        return Err(error_response("File not found".into()));
    }
    let data = crate::import::vixen_preview::parse_preview_file(path)
        .map_err(|e| error_response(e.to_string()))?;
    if data.display_items.is_empty() {
        return Err(error_response("No display items found".into()));
    }
    Ok(ApiResponse::success(data.display_items.len()))
}

async fn post_vixen_execute(
    AxumState(state): AppArc,
    Json(config): Json<crate::import::vixen::VixenImportConfig>,
) -> ApiResult<crate::import::vixen::VixenImportResult> {
    let data_dir = get_data_dir(&state)?;

    let result = tokio::task::spawn_blocking(move || {
        let vixen_path = std::path::Path::new(&config.vixen_dir);
        let config_path = vixen_path.join(crate::import::VIXEN_SYSTEM_DATA_DIR).join(crate::import::VIXEN_SYSTEM_CONFIG_FILE);

        let mut importer = crate::import::vixen::VixenImporter::new();
        importer.parse_system_config(&config_path).map_err(|e| e.to_string())?;

        let layout_items = if config.import_layout {
            let override_path = config.preview_file_override.as_deref().map(std::path::Path::new);
            importer.parse_preview(vixen_path, override_path).unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut sequences_imported = 0usize;
        for seq_path in &config.sequence_paths {
            if importer.parse_sequence(std::path::Path::new(seq_path), None).is_ok() {
                sequences_imported += 1;
            }
        }

        let guid_map = importer.guid_map().clone();
        let warnings: Vec<String> = importer.warnings().to_vec();
        let show = importer.into_show();

        let fixtures_imported = show.fixtures.len();
        let groups_imported = show.groups.len();
        let controllers_imported = if config.import_controllers { show.controllers.len() } else { 0 };
        let layout_items_imported = layout_items.len();

        let profile_name = if config.profile_name.trim().is_empty() {
            "Vixen Import".to_string()
        } else {
            config.profile_name.trim().to_string()
        };
        let summary = profile::create_profile(&data_dir, &profile_name).map_err(|e| e.to_string())?;

        let layout = if layout_items.is_empty() {
            show.layout.clone()
        } else {
            crate::model::show::Layout { fixtures: layout_items }
        };

        let prof = crate::profile::Profile {
            name: profile_name,
            slug: summary.slug.clone(),
            fixtures: show.fixtures.clone(),
            groups: show.groups.clone(),
            controllers: if config.import_controllers { show.controllers.clone() } else { Vec::new() },
            patches: if config.import_controllers { show.patches.clone() } else { Vec::new() },
            layout,
        };
        profile::save_profile(&data_dir, &summary.slug, &prof).map_err(|e| e.to_string())?;
        profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map).map_err(|e| e.to_string())?;

        // Copy media first so audio_file remapping can reference imported files
        let mut media_imported = 0usize;
        for media_filename in &config.media_filenames {
            let source = vixen_path.join("Media").join(media_filename);
            if source.exists()
                && profile::import_media(&data_dir, &summary.slug, &source).is_ok()
            {
                media_imported += 1;
            }
        }

        // Save sequences with audio_file remapped to local media filename
        for seq in &show.sequences {
            let mut seq = seq.clone();
            if let Some(ref audio_path) = seq.audio_file {
                let audio_basename = std::path::Path::new(audio_path)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string());
                if let Some(ref basename) = audio_basename {
                    if config.media_filenames.iter().any(|m| m == basename) {
                        seq.audio_file = Some(basename.clone());
                    } else {
                        seq.audio_file = None;
                    }
                }
            }
            let _ = profile::create_sequence(&data_dir, &summary.slug, &seq.name);
            let seq_slug = crate::project::slugify(&seq.name);
            profile::save_sequence(&data_dir, &summary.slug, &seq_slug, &seq).map_err(|e| e.to_string())?;
        }

        Ok::<_, String>(crate::import::vixen::VixenImportResult {
            profile_slug: summary.slug,
            fixtures_imported,
            groups_imported,
            controllers_imported,
            layout_items_imported,
            sequences_imported,
            media_imported,
            warnings,
        })
    })
    .await
    .map_err(|e| error_response(e.to_string()))?
    .map_err(error_response)?;

    Ok(ApiResponse::success(result))
}

// ══════════════════════════════════════════════════════════════════
// Tool registry endpoints
// ══════════════════════════════════════════════════════════════════

async fn get_tools() -> Json<ApiResponse<serde_json::Value>> {
    ApiResponse::success(registry::catalog::to_json_schema())
}

async fn post_tool_dispatch(
    AxumState(state): AppArc,
    Path(tool_name): Path<String>,
    Json(params): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    let result = crate::chat::execute_tool_api(&state, &tool_name, &params)
        .map_err(error_response)?;
    Ok(ApiResponse::success(serde_json::Value::String(result)))
}

// ══════════════════════════════════════════════════════════════════
// Analysis endpoints
// ══════════════════════════════════════════════════════════════════

/// Helper: get the current analysis for the loaded sequence.
fn get_current_analysis(state: &AppState) -> Option<crate::model::analysis::AudioAnalysis> {
    let show = state.show.lock();
    let audio_file = show.sequences.first()?.audio_file.as_ref()?;
    let cache = state.analysis_cache.lock();
    cache.get(audio_file).cloned()
}

async fn get_analysis_summary(AxumState(state): AppArc) -> ApiResult<serde_json::Value> {
    let analysis = get_current_analysis(&state)
        .ok_or_else(|| error_response("No audio analysis available".into()))?;

    let mut summary = serde_json::Map::new();
    if let Some(ref beats) = analysis.beats {
        summary.insert("tempo".to_string(), serde_json::json!(beats.tempo));
        summary.insert("time_signature".to_string(), serde_json::json!(beats.time_signature));
        summary.insert("beat_count".to_string(), serde_json::json!(beats.beats.len()));
    }
    if let Some(ref harmony) = analysis.harmony {
        summary.insert("key".to_string(), serde_json::json!(harmony.key));
    }
    if let Some(ref mood) = analysis.mood {
        summary.insert("valence".to_string(), serde_json::json!(mood.valence));
        summary.insert("arousal".to_string(), serde_json::json!(mood.arousal));
        summary.insert("danceability".to_string(), serde_json::json!(mood.danceability));
        summary.insert("genres".to_string(), serde_json::json!(mood.genres));
    }
    if let Some(ref structure) = analysis.structure {
        let sections: Vec<serde_json::Value> = structure.sections.iter().map(|s| {
            serde_json::json!({"label": s.label, "start": s.start, "end": s.end})
        }).collect();
        summary.insert("sections".to_string(), serde_json::Value::Array(sections));
    }

    Ok(ApiResponse::success(serde_json::Value::Object(summary)))
}

#[derive(Deserialize)]
struct BeatsQuery {
    start: f64,
    end: f64,
}

async fn get_analysis_beats(
    AxumState(state): AppArc,
    Query(q): Query<BeatsQuery>,
) -> ApiResult<serde_json::Value> {
    let analysis = get_current_analysis(&state)
        .ok_or_else(|| error_response("No audio analysis available".into()))?;
    let beats = analysis.beats.as_ref()
        .ok_or_else(|| error_response("No beat analysis available".into()))?;
    let filtered: Vec<f64> = beats.beats.iter().copied().filter(|&b| b >= q.start && b <= q.end).collect();
    let downbeats: Vec<f64> = beats.downbeats.iter().copied().filter(|&b| b >= q.start && b <= q.end).collect();
    Ok(ApiResponse::success(serde_json::json!({
        "beats": filtered, "downbeats": downbeats, "count": filtered.len(), "tempo": beats.tempo,
    })))
}

async fn get_analysis_sections(AxumState(state): AppArc) -> ApiResult<serde_json::Value> {
    let analysis = get_current_analysis(&state)
        .ok_or_else(|| error_response("No audio analysis available".into()))?;
    let structure = analysis.structure.as_ref()
        .ok_or_else(|| error_response("No structure analysis available".into()))?;
    Ok(ApiResponse::success(serde_json::to_value(&structure.sections).unwrap_or_default()))
}

async fn get_analysis_detail_handler(
    AxumState(state): AppArc,
    Path(feature): Path<String>,
) -> ApiResult<serde_json::Value> {
    let analysis = get_current_analysis(&state)
        .ok_or_else(|| error_response("No audio analysis available".into()))?;
    let detail = match feature.as_str() {
        "beats" => serde_json::to_value(&analysis.beats),
        "structure" => serde_json::to_value(&analysis.structure),
        "mood" => serde_json::to_value(&analysis.mood),
        "harmony" => serde_json::to_value(&analysis.harmony),
        "lyrics" => serde_json::to_value(&analysis.lyrics),
        "pitch" => serde_json::to_value(&analysis.pitch),
        "drums" => serde_json::to_value(&analysis.drums),
        "vocal_presence" => serde_json::to_value(&analysis.vocal_presence),
        "low_level" => serde_json::to_value(&analysis.low_level),
        _ => return Err(error_response(format!("Unknown feature: {feature}"))),
    }.unwrap_or_default();
    Ok(ApiResponse::success(detail))
}

// ══════════════════════════════════════════════════════════════════
// Script endpoints
// ══════════════════════════════════════════════════════════════════

async fn get_scripts(AxumState(state): AppArc) -> Json<ApiResponse<Vec<String>>> {
    let show = state.show.lock();
    let names: Vec<String> = show.sequences.first()
        .map(|seq| seq.scripts.keys().cloned().collect())
        .unwrap_or_default();
    ApiResponse::success(names)
}

async fn get_script_source_handler(
    AxumState(state): AppArc,
    Path(name): Path<String>,
) -> ApiResult<String> {
    let show = state.show.lock();
    let source = show.sequences.first()
        .and_then(|seq| seq.scripts.get(&name))
        .cloned()
        .ok_or_else(|| error_response(format!("Script \"{name}\" not found")))?;
    Ok(ApiResponse::success(source))
}

#[derive(Deserialize)]
struct CompileScriptBody {
    name: String,
    source: String,
}

async fn post_script(
    AxumState(state): AppArc,
    Json(body): Json<CompileScriptBody>,
) -> ApiResult<String> {
    match crate::dsl::compile_source(&body.source) {
        Ok(compiled) => {
            state.script_cache.lock().insert(body.name.clone(), std::sync::Arc::new(compiled));
            let cmd = EditCommand::SetScript {
                sequence_index: 0,
                name: body.name.clone(),
                source: body.source,
            };
            let mut dispatcher = state.dispatcher.lock();
            let mut show = state.show.lock();
            dispatcher.execute(&mut show, &cmd).map_err(|e| error_response(e.to_string()))?;
            Ok(ApiResponse::success(format!("Script \"{}\" compiled and saved", body.name)))
        }
        Err(errors) => {
            let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            Err(error_response(format!("Compile errors: {}", msgs.join("; "))))
        }
    }
}

async fn delete_script_handler(
    AxumState(state): AppArc,
    Path(name): Path<String>,
) -> ApiResult<()> {
    let cmd = EditCommand::DeleteScript {
        sequence_index: 0,
        name: name.clone(),
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd).map_err(|e| error_response(e.to_string()))?;
    state.script_cache.lock().remove(&name);
    Ok(ApiResponse::success(()))
}

// ══════════════════════════════════════════════════════════════════
// Library endpoints
// ══════════════════════════════════════════════════════════════════

async fn get_library_gradients(AxumState(state): AppArc) -> ApiResult<serde_json::Value> {
    let show = state.show.lock();
    let seq = show.sequences.first().ok_or_else(|| error_response("No sequence".into()))?;
    Ok(ApiResponse::success(serde_json::to_value(&seq.gradient_library).unwrap_or_default()))
}

#[derive(Deserialize)]
struct GradientBody {
    name: String,
    stops: serde_json::Value,
}

async fn post_library_gradient(
    AxumState(state): AppArc,
    Json(body): Json<GradientBody>,
) -> ApiResult<()> {
    let stops: Vec<crate::model::ColorStop> = serde_json::from_value(body.stops)
        .map_err(|e| error_response(format!("Invalid stops: {e}")))?;
    let gradient = crate::model::ColorGradient::new(stops)
        .ok_or_else(|| error_response("Gradient needs at least 2 stops".into()))?;
    let cmd = EditCommand::SetGradient { sequence_index: 0, name: body.name, gradient };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

async fn delete_library_gradient_handler(
    AxumState(state): AppArc,
    Path(name): Path<String>,
) -> ApiResult<()> {
    let cmd = EditCommand::DeleteGradient { sequence_index: 0, name };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

async fn get_library_curves(AxumState(state): AppArc) -> ApiResult<serde_json::Value> {
    let show = state.show.lock();
    let seq = show.sequences.first().ok_or_else(|| error_response("No sequence".into()))?;
    Ok(ApiResponse::success(serde_json::to_value(&seq.curve_library).unwrap_or_default()))
}

#[derive(Deserialize)]
struct CurveBody {
    name: String,
    points: serde_json::Value,
}

async fn post_library_curve(
    AxumState(state): AppArc,
    Json(body): Json<CurveBody>,
) -> ApiResult<()> {
    let points: Vec<crate::model::CurvePoint> = serde_json::from_value(body.points)
        .map_err(|e| error_response(format!("Invalid points: {e}")))?;
    let curve = crate::model::Curve::new(points)
        .ok_or_else(|| error_response("Curve needs at least 2 points".into()))?;
    let cmd = EditCommand::SetCurve { sequence_index: 0, name: body.name, curve };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

async fn delete_library_curve_handler(
    AxumState(state): AppArc,
    Path(name): Path<String>,
) -> ApiResult<()> {
    let cmd = EditCommand::DeleteCurve { sequence_index: 0, name };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd).map_err(|e| error_response(e.to_string()))?;
    Ok(ApiResponse::success(()))
}

// ══════════════════════════════════════════════════════════════════
// Batch edit endpoint
// ══════════════════════════════════════════════════════════════════

async fn post_batch(
    AxumState(state): AppArc,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<String> {
    // Delegate to the chat tool executor for consistent behavior
    let result = crate::chat::execute_tool_api(&state, "batch_edit", &body)
        .map_err(error_response)?;
    Ok(ApiResponse::success(result))
}

// ══════════════════════════════════════════════════════════════════
// DSL reference & design guide endpoints
// ══════════════════════════════════════════════════════════════════

async fn get_dsl_reference() -> Json<ApiResponse<String>> {
    ApiResponse::success(registry::reference::dsl_reference())
}

async fn get_design_guide() -> Json<ApiResponse<String>> {
    ApiResponse::success(registry::reference::design_guide())
}

// ══════════════════════════════════════════════════════════════════
// Server startup
// ══════════════════════════════════════════════════════════════════

/// Build the router with all API endpoints.
pub fn build_router(state: Arc<AppState>) -> Router {
    #[allow(clippy::unwrap_used)] // compile-time string literals always parse
    let origins = [
        "http://localhost:1420".parse().unwrap(),
        "tauri://localhost".parse().unwrap(),
        "http://tauri.localhost".parse().unwrap(),
    ];
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::DELETE,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
        ]);

    Router::new()
        // Existing 12 endpoints
        .route("/api/show", get(get_show))
        .route("/api/effects", get(get_effects))
        .route("/api/playback", get(get_playback))
        .route("/api/effect/{seq}/{track}/{idx}", get(get_effect_detail))
        .route("/api/undo-state", get(get_undo_state))
        .route("/api/command", post(post_command))
        .route("/api/undo", post(post_undo))
        .route("/api/redo", post(post_redo))
        .route("/api/play", post(post_play))
        .route("/api/pause", post(post_pause))
        .route("/api/seek", post(post_seek))
        .route("/api/save", post(post_save))
        // Settings
        .route("/api/settings", get(get_settings))
        .route("/api/settings/initialize", post(post_initialize))
        // Profiles
        .route("/api/profiles", get(get_profiles).post(post_profiles))
        .route("/api/profiles/{slug}", get(get_profile).delete(delete_profile_handler))
        .route("/api/profiles/{slug}/save", post(post_profile_save))
        .route("/api/profiles/{slug}/fixtures", put(put_profile_fixtures))
        .route("/api/profiles/{slug}/setup", put(put_profile_setup))
        // Sequences
        .route("/api/profiles/{slug}/sequences", get(get_sequences).post(post_sequences))
        .route("/api/profiles/{profile_slug}/sequences/{seq_slug}", get(get_sequence).delete(delete_sequence_handler))
        // Media
        .route("/api/profiles/{slug}/media", get(get_media).post(post_media))
        .route("/api/profiles/{slug}/media/{filename}", delete(delete_media_handler))
        // Rendering & describe
        .route("/api/frame", get(get_frame))
        .route("/api/describe", get(get_describe))
        // Chat
        .route("/api/chat", post(post_chat))
        .route("/api/chat/history", get(get_chat_history))
        .route("/api/chat/clear", post(post_chat_clear))
        .route("/api/chat/stop", post(post_chat_stop))
        .route("/api/chat/api-key", put(put_chat_api_key))
        // Vixen import
        .route("/api/vixen/scan", post(post_vixen_scan))
        .route("/api/vixen/check-preview", post(post_vixen_check_preview))
        .route("/api/vixen/execute", post(post_vixen_execute))
        // Tool registry
        .route("/api/tools", get(get_tools))
        .route("/api/tools/{name}", post(post_tool_dispatch))
        // Analysis
        .route("/api/analysis/summary", get(get_analysis_summary))
        .route("/api/analysis/beats", get(get_analysis_beats))
        .route("/api/analysis/sections", get(get_analysis_sections))
        .route("/api/analysis/detail/{feature}", get(get_analysis_detail_handler))
        // Scripts
        .route("/api/scripts", get(get_scripts).post(post_script))
        .route("/api/scripts/{name}", get(get_script_source_handler).delete(delete_script_handler))
        // Library
        .route("/api/library/gradients", get(get_library_gradients).post(post_library_gradient))
        .route("/api/library/gradients/{name}", delete(delete_library_gradient_handler))
        .route("/api/library/curves", get(get_library_curves).post(post_library_curve))
        .route("/api/library/curves/{name}", delete(delete_library_curve_handler))
        // Batch + reference
        .route("/api/batch", post(post_batch))
        .route("/api/dsl-reference", get(get_dsl_reference))
        .route("/api/design-guide", get(get_design_guide))
        .layer(cors)
        .with_state(state)
}

/// Start the API server on a random available port. Returns the bound port.
///
/// # Errors
/// Returns an I/O error if the TCP listener cannot bind or the local address
/// cannot be determined.
pub async fn start_api_server(state: Arc<AppState>) -> Result<u16, std::io::Error> {
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("[VibeLights] API server error: {e}");
        }
    });

    Ok(port)
}

/// Start the API server on a specific port. Returns the bound port.
/// Used by the CLI binary. Blocks until the server exits.
///
/// # Errors
/// Returns an I/O error if the TCP listener cannot bind, the local address
/// cannot be determined, or the server encounters a fatal error.
pub async fn start_api_server_on_port(state: Arc<AppState>, port: u16) -> Result<u16, std::io::Error> {
    let app = build_router(state);

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let actual_port = listener.local_addr()?.port();

    axum::serve(listener, app).await?;

    Ok(actual_port)
}
