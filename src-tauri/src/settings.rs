//! User settings, persisted as JSON next to the database.

use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Global shortcut for the popup, in `global-hotkey` syntax, e.g. `Ctrl+Shift+V`.
    pub hotkey: String,
    /// Unpinned items older than this are deleted.
    pub retention_days: u32,
    /// Most unpinned items to keep; pinned items don't count.
    pub max_items: u32,
    pub paused: bool,
    /// Executable names (lowercase, e.g. `keepass.exe`) whose copies are never recorded.
    pub excluded_apps: Vec<String>,
    /// The first-run welcome window has been dismissed.
    pub welcomed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Shift+V".into(),
            retention_days: 30,
            max_items: 1000,
            paused: false,
            excluded_apps: Vec::new(),
            welcomed: false,
        }
    }
}

impl Settings {
    /// Loads settings, falling back to defaults if the file is missing or unreadable.
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        // Write then rename, so a crash mid-write can't leave a truncated file.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(tmp, path)
    }

    /// Clamps values coming from the UI into a safe range.
    pub fn sanitized(mut self) -> Self {
        self.retention_days = self.retention_days.clamp(1, 3650);
        self.max_items = self.max_items.clamp(10, 100_000);
        let mut apps: Vec<String> = self
            .excluded_apps
            .iter()
            .map(|a| a.trim().to_lowercase())
            .filter(|a| !a.is_empty())
            .collect();
        apps.sort();
        apps.dedup();
        self.excluded_apps = apps;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"retentionDays": 7}"#).unwrap();
        assert_eq!(s.retention_days, 7);
        assert_eq!(s.hotkey, "Ctrl+Shift+V");
    }

    #[test]
    fn sanitized_clamps_and_normalizes() {
        let s = Settings {
            retention_days: 0,
            max_items: 1,
            excluded_apps: vec![" KeePass.exe ".into(), "keepass.exe".into(), "".into()],
            ..Default::default()
        }
        .sanitized();
        assert_eq!(s.retention_days, 1);
        assert_eq!(s.max_items, 10);
        assert_eq!(s.excluded_apps, ["keepass.exe"]);
    }
}
