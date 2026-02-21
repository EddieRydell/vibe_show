// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use tauri::Manager;

use vibe_lights::api;
use vibe_lights::chat::ChatManager;
use vibe_lights::commands;
use vibe_lights::dispatcher::CommandDispatcher;
use vibe_lights::model::Show;
use vibe_lights::settings;
use vibe_lights::state::{AppState, PlaybackState};

#[allow(clippy::expect_used)] // app cannot start without config dir / Tauri runtime
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_config_dir = app
                .path()
                .app_config_dir()
                .expect("failed to resolve app config dir");

            let loaded_settings = settings::load_settings(&app_config_dir);

            let state = Arc::new(AppState {
                show: Mutex::new(Show::empty()),
                playback: Mutex::new(PlaybackState {
                    playing: false,
                    current_time: 0.0,
                    sequence_index: 0,
                    last_tick: None,
                    region: None,
                    looping: false,
                }),
                dispatcher: Mutex::new(CommandDispatcher::new()),
                chat: Mutex::new(ChatManager::new()),
                api_port: AtomicU16::new(0),
                app_config_dir: app_config_dir.clone(),
                settings: Mutex::new(loaded_settings),
                current_profile: Mutex::new(None),
                current_sequence: Mutex::new(None),
                script_cache: Mutex::new(std::collections::HashMap::new()),
                python_sidecar: Mutex::new(None),
                python_port: AtomicU16::new(0),
                analysis_cache: Mutex::new(std::collections::HashMap::new()),
            });

            app.manage(state.clone());

            // Start the HTTP API server on a background task
            let api_state = state.clone();
            tauri::async_runtime::spawn(async move {
                let port = api::start_api_server(api_state.clone()).await;
                api_state.api_port.store(port, Ordering::Relaxed);

                // Write port file to app config dir for external tool discovery
                let port_file = app_config_dir.join(".vibelights-port");
                let _ = std::fs::write(&port_file, port.to_string());

                eprintln!("[VibeLights] API server listening on http://127.0.0.1:{port}");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Settings
            commands::get_settings,
            commands::get_api_port,
            commands::initialize_data_dir,
            // Profiles
            commands::list_profiles,
            commands::create_profile,
            commands::open_profile,
            commands::delete_profile,
            commands::save_profile,
            commands::update_profile_fixtures,
            commands::update_profile_setup,
            commands::update_profile_layout,
            // Sequences
            commands::list_sequences,
            commands::create_sequence,
            commands::open_sequence,
            commands::save_current_sequence,
            commands::delete_sequence,
            // Media
            commands::list_media,
            commands::import_media,
            commands::delete_media,
            commands::resolve_media_path,
            // Sequence settings
            commands::update_sequence_settings,
            // Effects
            commands::list_effects,
            // Undo / Redo
            commands::undo,
            commands::redo,
            commands::get_undo_state,
            // Chat
            commands::send_chat_message,
            commands::get_chat_history,
            commands::clear_chat,
            commands::stop_chat,
            commands::set_claude_api_key,
            commands::has_claude_api_key,
            // Engine / playback
            commands::get_show,
            commands::get_frame,
            commands::get_frame_filtered,
            commands::play,
            commands::pause,
            commands::seek,
            commands::get_playback,
            commands::set_region,
            commands::set_looping,
            commands::tick,
            commands::render_effect_thumbnail,
            commands::get_effect_detail,
            commands::update_effect_param,
            commands::add_effect,
            commands::add_track,
            commands::delete_track,
            commands::delete_effects,
            commands::update_effect_time_range,
            commands::move_effect_to_track,
            // Import
            commands::import_vixen,
            commands::import_vixen_profile,
            commands::import_vixen_sequence,
            commands::scan_vixen_directory,
            commands::check_vixen_preview_file,
            commands::execute_vixen_import,
            // DSL Scripts
            commands::compile_script,
            commands::list_scripts,
            commands::get_script_source,
            commands::delete_script,
            // Profile libraries
            commands::list_profile_gradients,
            commands::set_profile_gradient,
            commands::delete_profile_gradient,
            commands::rename_profile_gradient,
            commands::list_profile_curves,
            commands::set_profile_curve,
            commands::delete_profile_curve,
            commands::rename_profile_curve,
            commands::list_profile_scripts,
            commands::set_profile_script,
            commands::delete_profile_script,
            commands::compile_profile_script,
            // Sequence resource libraries
            commands::list_library_gradients,
            commands::set_library_gradient,
            commands::delete_library_gradient,
            commands::rename_library_gradient,
            commands::list_library_curves,
            commands::set_library_curve,
            commands::delete_library_curve,
            commands::rename_library_curve,
            // Python / Analysis
            commands::get_python_status,
            commands::setup_python_env,
            commands::start_python_sidecar,
            commands::stop_python_sidecar,
            commands::get_analysis,
            commands::analyze_audio,
        ])
        .build(tauri::generate_context!())
        .expect("error while building VibeLights")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Stop Python sidecar on app exit
                let state = app.state::<Arc<AppState>>();
                let port = state.python_port.load(Ordering::Relaxed);
                if port > 0 {
                    // Best-effort shutdown — send POST and kill
                    let shutdown_url = format!("http://127.0.0.1:{port}/shutdown");
                    let _ = reqwest::blocking::Client::new()
                        .post(shutdown_url)
                        .timeout(std::time::Duration::from_secs(2))
                        .send();
                    let mut child_opt = state.python_sidecar.lock().take();
                    if let Some(ref mut child) = child_opt {
                        let _ = child.start_kill();
                    }
                    state.python_port.store(0, Ordering::Relaxed);
                }
            }
        });
}
