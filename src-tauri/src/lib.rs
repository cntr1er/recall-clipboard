//! Recall Clipboard: clipboard history for Windows.
//!
//! Module map:
//! - `clipboard`  – change notifications, reading and writing the clipboard
//! - `privacy`    – which changes must never be recorded
//! - `source_app` – which app a copy came from (name + icon)
//! - `storage`    – SQLite history
//! - `images`     – image decoding and on-disk storage
//! - `hotkey`     – global shortcut, popup window, paste into the previous app
//! - `settings`, `tray`, `theme`, `commands` – the app shell around them

mod clipboard;
mod commands;
mod hotkey;
mod images;
mod privacy;
mod settings;
mod source_app;
mod storage;
mod theme;
mod tray;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

use clipboard::Clip;
use settings::Settings;
use source_app::SourceApp;
use storage::Storage;

pub type Db = Mutex<Storage>;
pub type SettingsState = Mutex<Settings>;

/// Where everything lives under the app data directory.
pub struct Paths {
    pub images: PathBuf,
    pub icons: PathBuf,
    pub settings: PathBuf,
}

/// Whether Mica could be applied (false before Windows 11).
pub struct Mica(pub AtomicBool);

/// Shows, restores and focuses one of the app's windows by label.
pub fn show_window(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Changes settings, then applies them everywhere: hotkey, tray, retention.
pub fn update_settings(
    app: &AppHandle,
    change: impl FnOnce(&mut Settings),
) -> Result<Settings, String> {
    let state = app.state::<SettingsState>();
    let mut current = state.lock().unwrap();
    let mut new = current.clone();
    change(&mut new);
    let new = new.sanitized();

    if new.hotkey != current.hotkey {
        hotkey::register(app, &new.hotkey, Some(&current.hotkey))?;
    }
    new.save(&app.state::<Paths>().settings)
        .map_err(|e| e.to_string())?;
    *current = new.clone();
    drop(current);

    tray::set_paused(app, new.paused);
    run_cleanup(app);
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

/// Applies retention settings and deletes the image files of removed items.
fn run_cleanup(app: &AppHandle) {
    let s = app.state::<SettingsState>().lock().unwrap().clone();
    let removed =
        app.state::<Db>()
            .lock()
            .unwrap()
            .cleanup(now_ms(), s.retention_days, s.max_items);
    match removed {
        Ok(hashes) if !hashes.is_empty() => {
            let dir = &app.state::<Paths>().images;
            for hash in &hashes {
                images::remove(dir, hash);
            }
            let _ = app.emit("history-changed", ());
        }
        Ok(_) => {}
        Err(e) => eprintln!("cleanup failed: {e}"),
    }
}

/// Records the app's friendly name and icon the first time we see it.
fn remember_app(app: &AppHandle, source: &SourceApp) {
    let db = app.state::<Db>();
    if db.lock().unwrap().has_app(&source.exe).unwrap_or(true) {
        return;
    }
    let name = source.display_name();
    if let Some(icon) = source.icon() {
        let path = app
            .state::<Paths>()
            .icons
            .join(format!("{}.png", source.exe));
        if let Err(e) = images::save_png(&icon, &path) {
            eprintln!("couldn't save icon for {}: {e}", source.exe);
        }
    }
    let _ = db.lock().unwrap().upsert_app(&source.exe, &name);
}

/// Runs on the watcher thread after every clipboard change.
fn on_clipboard_change(app: &AppHandle) {
    if clipboard::changed_by_us() {
        return;
    }
    let settings = app.state::<SettingsState>().lock().unwrap().clone();
    if settings.paused {
        return;
    }
    let source = source_app::foreground();
    let exe = source.as_ref().map(|s| s.exe.as_str());
    if privacy::is_excluded_app(exe, &settings.excluded_apps) {
        return;
    }
    let Some(clip) = clipboard::read() else {
        return;
    };

    let img = match clip {
        Clip::Text(text) => return record(app, "text", &text, None, source.as_ref()),
        Clip::Png(bytes) => images::decode_png(&bytes),
        Clip::Dib(bytes) => images::decode_dib(&bytes),
    };
    let Some(img) = img else { return };
    match images::store(&app.state::<Paths>().images, &img) {
        Ok(hash) => record(app, "image", &hash, Some(img.dimensions()), source.as_ref()),
        Err(e) => eprintln!("couldn't save image: {e}"),
    }
}

fn record(
    app: &AppHandle,
    kind: &str,
    content: &str,
    size: Option<(u32, u32)>,
    source: Option<&SourceApp>,
) {
    if let Some(s) = source {
        remember_app(app, s);
    }
    let exe = source.map(|s| s.exe.as_str());
    let inserted = app
        .state::<Db>()
        .lock()
        .unwrap()
        .insert(kind, content, size, exe, now_ms());
    match inserted {
        Ok(_) => {
            let _ = app.emit("history-changed", ());
            run_cleanup(app);
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
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        hotkey::toggle_popup(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let paths = Paths {
                images: dir.join("images"),
                icons: dir.join("icons"),
                settings: dir.join("settings.json"),
            };
            std::fs::create_dir_all(&paths.images)?;
            std::fs::create_dir_all(&paths.icons)?;
            let settings = Settings::load(&paths.settings).sanitized();

            app.manage(Mutex::new(Storage::open(&dir.join("history.db"))?));
            app.manage(Mutex::new(settings.clone()));
            app.manage(paths);

            let handle = app.handle();
            tray::create(handle, settings.paused)?;
            if let Err(e) = hotkey::register(handle, &settings.hotkey, None) {
                eprintln!("{e}");
            }

            // Windows are `"create": false` in the config and built here, after all
            // state is managed, so their first commands can't race the setup.
            app.manage(Mica(AtomicBool::new(true)));
            for config in app.config().app.windows.clone() {
                let window = WebviewWindowBuilder::from_config(handle, &config)?.build()?;
                if !theme::apply_mica(&window) {
                    app.state::<Mica>().0.store(false, Ordering::Relaxed);
                }
            }

            // Retention is by age, so it also has to run when nothing is being copied.
            run_cleanup(handle);
            let h = handle.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(3600));
                    run_cleanup(&h);
                }
            });

            let h = handle.clone();
            clipboard::watch(move || on_clipboard_change(&h));

            if !settings.welcomed {
                show_window(handle, "welcome");
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing a window only hides it; the app keeps living in the tray.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) if window.label() == "popup" => {
                hotkey::hide_popup(window.app_handle(), false);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_items,
            commands::get_item,
            commands::list_apps,
            commands::set_pinned,
            commands::delete_item,
            commands::clear_history,
            commands::copy_item,
            commands::paste_item,
            commands::hide_popup,
            commands::open_window,
            commands::get_settings,
            commands::set_settings,
            commands::accent_color,
            commands::has_mica,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
