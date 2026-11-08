mod clipboard;
mod commands;
mod storage;
mod tray;

use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use commands::Db;
use storage::Storage;

/// Shows, restores and focuses one of the app's windows by label.
pub fn show_window(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

/// Runs on the watcher thread after every clipboard change.
fn on_clipboard_change(app: &AppHandle) {
    if tray::PAUSED.load(Ordering::Relaxed) {
        return;
    }
    let Some(text) = clipboard::read_text() else { return };
    if text.trim().is_empty() {
        return;
    }
    let db = app.state::<Db>();
    match db.lock().unwrap().insert("text", &text, None, None, now_ms()) {
        Ok(_) => {
            let _ = app.emit("history-changed", ());
        }
        Err(e) => eprintln!("failed to save clip: {e}"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first: a second launch just opens the existing app.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_window(app, "main");
        }))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(Mutex::new(Storage::open(&dir.join("history.db"))?));

            tray::create(app.handle())?;

            let handle = app.handle().clone();
            clipboard::watch(move || on_clipboard_change(&handle));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing a window only hides it; the app keeps living in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![commands::list_items])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
