//! Tauri commands called from the frontend.

use std::sync::Mutex;

use tauri::State;

use crate::storage::{Item, Storage};

pub type Db = Mutex<Storage>;

#[tauri::command]
pub fn list_items(db: State<Db>, query: String, limit: u32) -> Result<Vec<Item>, String> {
    db.lock().unwrap().list(&query, limit).map_err(|e| e.to_string())
}
