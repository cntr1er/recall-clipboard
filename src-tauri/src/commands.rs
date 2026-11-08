//! Tauri commands called from the frontend.

use tauri::{AppHandle, Emitter, Manager, State};

use crate::settings::Settings;
use crate::storage::{App, Filter, Item};
use crate::theme::{self, Accent};
use crate::{Db, Mica, Paths, SettingsState, clipboard, hotkey, images, now_ms};

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn history_changed(app: &AppHandle) {
    let _ = app.emit("history-changed", ());
}

#[tauri::command]
pub fn list_items(db: State<Db>, filter: Filter) -> Res<Vec<Item>> {
    db.lock().unwrap().list(&filter).map_err(err)
}

#[tauri::command]
pub fn get_item(db: State<Db>, id: i64) -> Res<Option<Item>> {
    db.lock().unwrap().get(id).map_err(err)
}

#[tauri::command]
pub fn list_apps(db: State<Db>) -> Res<Vec<App>> {
    db.lock().unwrap().apps().map_err(err)
}

#[tauri::command]
pub fn set_pinned(app: AppHandle, db: State<Db>, id: i64, pinned: bool) -> Res<()> {
    db.lock().unwrap().set_pinned(id, pinned).map_err(err)?;
    history_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_item(app: AppHandle, db: State<Db>, paths: State<Paths>, id: i64) -> Res<()> {
    if let Some(hash) = db.lock().unwrap().delete(id).map_err(err)? {
        images::remove(&paths.images, &hash);
    }
    history_changed(&app);
    Ok(())
}

/// Deletes every unpinned item.
#[tauri::command]
pub fn clear_history(app: AppHandle, db: State<Db>, paths: State<Paths>) -> Res<()> {
    for hash in db.lock().unwrap().clear_unpinned().map_err(err)? {
        images::remove(&paths.images, &hash);
    }
    history_changed(&app);
    Ok(())
}

/// Puts an item on the clipboard and moves it to the top of the history.
fn put_on_clipboard(app: &AppHandle, id: i64) -> Res<()> {
    let db = app.state::<Db>();
    let item = db
        .lock()
        .unwrap()
        .get(id)
        .map_err(err)?
        .ok_or("That item no longer exists.")?;
    let written = match item.kind.as_str() {
        "image" => {
            let png = std::fs::read(images::file(&app.state::<Paths>().images, &item.content))
                .map_err(err)?;
            let img = images::decode_png(&png).ok_or("The image file is damaged.")?;
            clipboard::write_image(&png, &images::encode_dib(&img))
        }
        "files" => {
            // Files may have been moved or deleted since they were copied.
            let paths: Vec<String> = item
                .content
                .lines()
                .filter(|p| std::path::Path::new(p).exists())
                .map(String::from)
                .collect();
            if paths.is_empty() {
                return Err("These files no longer exist.".into());
            }
            clipboard::write_files(&paths)
        }
        _ => clipboard::write_text(&item.content),
    };
    if !written {
        return Err("Couldn't access the clipboard. Try again.".into());
    }
    db.lock().unwrap().touch(id, now_ms()).map_err(err)?;
    history_changed(app);
    Ok(())
}

#[tauri::command]
pub fn copy_item(app: AppHandle, id: i64) -> Res<()> {
    put_on_clipboard(&app, id)
}

/// Copies the item, then pastes it into the app that was focused before the popup.
#[tauri::command]
pub async fn paste_item(app: AppHandle, id: i64) -> Res<()> {
    put_on_clipboard(&app, id)?;
    hotkey::paste_into_previous(&app);
    Ok(())
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    hotkey::hide_popup(&app, true);
}

#[tauri::command]
pub fn open_window(app: AppHandle, label: String) -> Res<()> {
    if !["main", "settings"].contains(&label.as_str()) {
        return Err(format!("unknown window {label}"));
    }
    crate::show_window(&app, &label);
    Ok(())
}

#[tauri::command]
pub fn get_settings(settings: State<SettingsState>) -> Settings {
    settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, settings: Settings) -> Res<Settings> {
    crate::update_settings(&app, |s| *s = settings)
}

#[tauri::command]
pub fn accent_color() -> Option<Accent> {
    theme::accent()
}

#[tauri::command]
pub fn has_mica(mica: State<Mica>) -> bool {
    mica.0.load(std::sync::atomic::Ordering::Relaxed)
}
