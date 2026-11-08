use tauri::{
    AppHandle, Manager, Wry,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{show_window, update_settings};

const TRAY_ID: &str = "main";

/// The Pause/Resume menu item, kept so its label can follow the setting.
struct PauseItem(MenuItem<Wry>);

pub fn create(app: &AppHandle, paused: bool) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", pause_label(paused), true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &pause,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    app.manage(PauseItem(pause));

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(
            app.default_window_icon()
                .expect("bundle has an icon")
                .clone(),
        )
        .tooltip(tooltip(paused))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app, "main"),
            "settings" => show_window(app, "settings"),
            "pause" => {
                if let Err(e) = update_settings(app, |s| s.paused = !s.paused) {
                    eprintln!("couldn't toggle pause: {e}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle(), "main");
            }
        })
        .build(app)?;
    Ok(())
}

/// Updates the menu label and tooltip after the pause setting changed.
pub fn set_paused(app: &AppHandle, paused: bool) {
    let _ = app.state::<PauseItem>().0.set_text(pause_label(paused));
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip(paused)));
    }
}

fn pause_label(paused: bool) -> &'static str {
    if paused {
        "Resume recording"
    } else {
        "Pause recording"
    }
}

fn tooltip(paused: bool) -> &'static str {
    if paused {
        "Recall Clipboard (paused)"
    } else {
        "Recall Clipboard"
    }
}
