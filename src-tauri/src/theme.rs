//! Windows 11 look: Mica backdrop and the system accent color.

use serde::Serialize;
use tauri::WebviewWindow;

/// Applies Mica (Windows 11 only). Returns false where unsupported, so the UI
/// can fall back to an opaque background.
pub fn apply_mica(window: &WebviewWindow) -> bool {
    window_vibrancy::apply_mica(window, None).is_ok()
}

/// The accent color in the shades Windows 11 itself uses on light and dark backgrounds.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accent {
    pub light_theme: String,
    pub dark_theme: String,
}

#[cfg(windows)]
pub fn accent() -> Option<Accent> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};

    let settings = UISettings::new().ok()?;
    let hex = |t| {
        let c = settings.GetColorValue(t).ok()?;
        Some(format!("#{:02x}{:02x}{:02x}", c.R, c.G, c.B))
    };
    Some(Accent {
        light_theme: hex(UIColorType::AccentDark1)?,
        dark_theme: hex(UIColorType::AccentLight2)?,
    })
}

#[cfg(not(windows))]
pub fn accent() -> Option<Accent> {
    None
}
