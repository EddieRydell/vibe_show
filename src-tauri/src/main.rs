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
                agent_sidecar: Mutex::new(None),
                agent_port: AtomicU16::new(0),
                agent_session_id: Mutex::new(None),
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
            // Async commands
            commands::send_chat_message,
            commands::send_agent_message,
            commands::cancel_agent_message,
            commands::clear_agent_session,
            commands::open_sequence,
            commands::execute_vixen_import,
            commands::analyze_audio,
            commands::get_python_status,
            commands::setup_python_env,
            commands::start_python_sidecar,
            commands::stop_python_sidecar,
            // Binary/hot-path commands
            commands::tick,
            commands::get_frame,
            commands::get_frame_filtered,
            commands::render_effect_thumbnail,
            commands::preview_script,
            commands::preview_script_frame,
            // Unified registry
            commands::exec,
            commands::get_command_registry,
        ])
        .build(tauri::generate_context!())
        .expect("error while building VibeLights")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<Arc<AppState>>();

                // Stop Python sidecar on app exit
                let port = state.python_port.load(Ordering::Relaxed);
                if port > 0 {
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

                // Stop Agent sidecar on app exit
                let agent_port = state.agent_port.load(Ordering::Relaxed);
                if agent_port > 0 {
                    let shutdown_url = format!("http://127.0.0.1:{agent_port}/shutdown");
                    let _ = reqwest::blocking::Client::new()
                        .post(shutdown_url)
                        .timeout(std::time::Duration::from_secs(2))
                        .send();
                    let mut child_opt = state.agent_sidecar.lock().take();
                    if let Some(ref mut child) = child_opt {
                        let _ = child.start_kill();
                    }
                    state.agent_port.store(0, Ordering::Relaxed);
                }
            }
        });
}
