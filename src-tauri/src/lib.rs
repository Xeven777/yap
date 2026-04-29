mod commands;
pub use commands::RecordingState;

pub fn run() {
    eprintln!("[yap] starting up");
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(RecordingState::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::get_api_key,
            commands::save_api_key,
            commands::paste_text,
        ])
        .run(tauri::generate_context!())
        .expect("error running yap");
}
