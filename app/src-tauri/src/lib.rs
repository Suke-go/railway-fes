pub mod transcript;
mod printer;
mod scheduler;
mod calibration;
mod audio;
mod audio_processing;
mod alignment;
mod whisper_cache;
mod settings;
mod commands;

use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(commands::SessionState::default())
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::load_transcript,
            commands::load_session,
            commands::realign,
            commands::start_playback,
            commands::stop_playback,
            commands::clear_session,
            commands::get_api_key_status,
            commands::set_api_key,
            commands::clear_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
