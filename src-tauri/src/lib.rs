mod commands;
pub use commands::{HttpClient, LocalWhisperState, RecordingState};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, PhysicalPosition, WindowEvent,
};

pub fn run() {
    eprintln!("[yap] starting up");

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(RecordingState::default())
        .manage(HttpClient(reqwest::Client::new()))
        .manage(LocalWhisperState::default())
        .setup(|app| {
            if let Some(win) = app.get_webview_window("main") {
                win.show().ok();
            }

            if let Some(pill) = app.get_webview_window("pill") {
                if let Ok(Some(monitor)) = app.primary_monitor() {
                    let sz = monitor.size();
                    let x = sz.width as i32 / 2 - 150;
                    let y = sz.height as i32 - 100;
                    pill.set_position(PhysicalPosition::new(x, y)).ok();
                }
            }

            let settings_item =
                MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_item =
                MenuItem::with_id(app, "quit", "Quit Yap", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        if let Some(win) = app.get_webview_window("main") {
                            win.show().ok();
                            win.set_focus().ok();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|win, event| {
            if win.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    win.hide().ok();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::get_api_key,
            commands::save_api_key,
            commands::paste_text,
            commands::get_hotkey,
            commands::save_hotkey,
            commands::get_language,
            commands::save_language,
            commands::get_backend,
            commands::save_backend,
            commands::get_active_model,
            commands::save_active_model,
            commands::list_models,
            commands::download_model,
            commands::cancel_download,
            commands::delete_model,
        ])
        .run(tauri::generate_context!())
        .expect("error running yap");
}
