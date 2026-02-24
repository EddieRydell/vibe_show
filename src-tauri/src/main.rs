// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use tauri::Manager;

use vibe_lights::commands;
use vibe_lights::dispatcher::CommandDispatcher;
use vibe_lights::model::Show;
use vibe_lights::settings;
use vibe_lights::state::{AppState, CancellationRegistry, PlaybackState};

#[allow(clippy::expect_used)] // app cannot start without config dir / Tauri runtime
fn main() {
    let mut ctx = tauri::generate_context!();
    ctx.set_default_window_icon(Some(
        tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
            .expect("failed to load app icon"),
    ));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_config_dir = app
                .path()
                .app_config_dir()
                .expect("failed to resolve app config dir");

            let loaded_settings = settings::load_settings(&app_config_dir);

            // Load global libraries from disk (if data_dir is configured).
            let global_libs = loaded_settings
                .as_ref()
                .and_then(|s| {
                    vibe_lights::setup::load_global_libraries(&s.data_dir).ok()
                })
                .unwrap_or_default();

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
                app_config_dir: app_config_dir.clone(),
                settings: Mutex::new(loaded_settings),
                current_setup: Mutex::new(None),
                current_sequence: Mutex::new(None),
                script_cache: Mutex::new(std::collections::HashMap::new()),
                python_sidecar: Mutex::new(None),
                python_port: AtomicU16::new(0),
                analysis_cache: Mutex::new(std::collections::HashMap::new()),
                agent_sidecar: Mutex::new(None),
                agent_port: AtomicU16::new(0),
                agent_session_id: Mutex::new(None),
                agent_display_messages: Mutex::new(Vec::new()),
                agent_chats: Mutex::new(vibe_lights::chat::AgentChatsData::default()),
                global_libraries: Mutex::new(global_libs),
                cancellation: CancellationRegistry::new(),
            });

            // Load agent chat history
            vibe_lights::chat::load_agent_chats(&state);

            app.manage(state.clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::exec,
            commands::get_command_registry,
        ])
        .build(ctx)
        .expect("error while building VibeLights")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<Arc<AppState>>();

                // Persist agent chat history before shutdown
                vibe_lights::chat::save_agent_chats(&state);

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
