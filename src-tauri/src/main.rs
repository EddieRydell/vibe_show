// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use tauri::Manager;

use vibe_show::commands::{self, AppState, PlaybackState};
use vibe_show::model::Show;
use vibe_show::settings;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_config_dir = app
                .path()
                .app_config_dir()
                .expect("failed to resolve app config dir");

            let loaded_settings = settings::load_settings(&app_config_dir);

            app.manage(AppState {
                show: Mutex::new(Show::empty()),
                playback: Mutex::new(PlaybackState {
                    playing: false,
                    current_time: 0.0,
                    sequence_index: 0,
                }),
                app_config_dir,
                settings: Mutex::new(loaded_settings),
                current_profile: Mutex::new(None),
                current_show: Mutex::new(None),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Settings
            commands::get_settings,
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
            // Shows
            commands::list_shows,
            commands::create_show,
            commands::open_show,
            commands::save_current_show,
            commands::delete_show,
            // Media
            commands::list_media,
            commands::import_media,
            commands::delete_media,
            // Effects
            commands::list_effects,
            // Engine / playback
            commands::get_show,
            commands::get_frame,
            commands::play,
            commands::pause,
            commands::seek,
            commands::get_playback,
            commands::tick,
            commands::render_effect_thumbnail,
            commands::get_effect_detail,
            commands::update_effect_param,
            commands::add_effect,
            commands::add_track,
            commands::delete_effects,
            commands::update_effect_time_range,
            commands::move_effect_to_track,
            commands::select_sequence,
            // Import
            commands::import_vixen,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VibeShow");
}
