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
    /// Which kinds of copies to save.
    pub record_text: bool,
    pub record_images: bool,
    pub record_files: bool,
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
            record_text: true,
            record_images: true,
            record_files: true,
        }
    }
}

/// Choices from the installer's "What to save" page, written to
/// `install-options.json` (see `src-tauri/windows/installer-hooks.nsh`).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallOptions {
    record_text: Option<bool>,
    record_images: Option<bool>,
    record_files: Option<bool>,
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

    /// Applies and deletes the installer's choices, if a (re)install left any.
    /// Returns true if anything was applied.
    pub fn take_install_options(&mut self, path: &Path) -> bool {
        let Ok(bytes) = fs::read(path) else {
            return false;
        };
        let _ = fs::remove_file(path);
        let Ok(opts) = serde_json::from_slice::<InstallOptions>(&bytes) else {
            return false;
        };
        for (setting, choice) in [
            (&mut self.record_text, opts.record_text),
            (&mut self.record_images, opts.record_images),
            (&mut self.record_files, opts.record_files),
        ] {
            if let Some(v) = choice {
                *setting = v;
            }
        }
        true
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

    #[test]
    fn install_options_are_applied_once_then_deleted() {
        let path = std::env::temp_dir().join(format!("recall-install-{}.json", std::process::id()));
        fs::write(
            &path,
            r#"{"recordText": true, "recordImages": false, "recordFiles": false}"#,
        )
        .unwrap();

        let mut s = Settings::default();
        assert!(s.take_install_options(&path));
        assert!(s.record_text && !s.record_images && !s.record_files);
        assert!(!path.exists());
        assert!(!s.take_install_options(&path), "nothing left to apply");
    }

    #[test]
    fn broken_install_options_change_nothing() {
        let path =
            std::env::temp_dir().join(format!("recall-install-bad-{}.json", std::process::id()));
        fs::write(&path, "not json").unwrap();
        let mut s = Settings::default();
        assert!(!s.take_install_options(&path));
        assert_eq!(s, Settings::default());
        assert!(!path.exists());
    }
}
