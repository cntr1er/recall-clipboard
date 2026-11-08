//! The global hotkey, the popup window, and pasting into the previous app.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

const POPUP: &str = "popup";
const POPUP_SIZE: (f64, f64) = (400.0, 460.0);

/// Replaces the registered global shortcut. On failure the old one stays active.
pub fn register(app: &AppHandle, new: &str, old: Option<&str>) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    gs.register(new).map_err(|e| {
        if let Some(old) = old {
            let _ = gs.register(old);
        }
        format!("Couldn't use {new}: {e}")
    })
}

pub fn toggle_popup(app: &AppHandle) {
    let Some(popup) = app.get_webview_window(POPUP) else {
        return;
    };
    if popup.is_visible().unwrap_or(false) {
        hide_popup(app, true);
        return;
    }
    imp::remember_foreground();

    // Open at the cursor, nudged so the whole window fits in the monitor's work area.
    if let Ok(cursor) = app.cursor_position()
        && let Ok(Some(monitor)) = app.monitor_from_point(cursor.x, cursor.y)
    {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let (w, h) = ((POPUP_SIZE.0 * scale) as i32, (POPUP_SIZE.1 * scale) as i32);
        let clamp = |v: f64, start: i32, len: u32, size: i32| {
            (v as i32).min(start + len as i32 - size).max(start)
        };
        let x = clamp(cursor.x, area.position.x, area.size.width, w);
        let y = clamp(cursor.y, area.position.y, area.size.height, h);
        let _ = popup.set_position(PhysicalPosition::new(x, y));
    }
    let _ = popup.show();
    let _ = popup.set_focus();
    let _ = app.emit_to(POPUP, "popup-opened", ());
}

/// Hides the popup; optionally gives focus back to the window that had it before.
pub fn hide_popup(app: &AppHandle, restore_focus: bool) {
    if let Some(popup) = app.get_webview_window(POPUP) {
        let _ = popup.hide();
    }
    if restore_focus {
        imp::restore_foreground();
    }
}

/// Hides the popup, refocuses the previous app and presses Ctrl+V in it.
/// The clipboard must already hold the item.
pub fn paste_into_previous(app: &AppHandle) {
    hide_popup(app, true);
    // Give the target window a moment to become active before it receives keys.
    std::thread::sleep(Duration::from_millis(80));
    imp::send_ctrl_v();
}

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicPtr, Ordering};

    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
        KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        VK_V,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

    static PREVIOUS: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

    pub fn remember_foreground() {
        PREVIOUS.store(unsafe { GetForegroundWindow() }.0, Ordering::Relaxed);
    }

    pub fn restore_foreground() {
        let hwnd = HWND(PREVIOUS.load(Ordering::Relaxed));
        if !hwnd.is_invalid() {
            let _ = unsafe { SetForegroundWindow(hwnd) };
        }
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    ..Default::default()
                },
            },
        }
    }

    pub fn send_ctrl_v() {
        // Release modifiers the user may still hold (e.g. Shift from the hotkey),
        // or the target app would see Ctrl+Shift+V instead of Ctrl+V.
        let mut inputs: Vec<INPUT> = [VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            .filter(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0)
            .map(|vk| key(vk, true))
            .collect();
        inputs.extend([
            key(VK_CONTROL, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ]);
        unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    }
}

// shortcut: refocus + keystroke injection is Windows-only; on other OSes the item is
// only copied to the clipboard.
#[cfg(not(windows))]
mod imp {
    pub fn remember_foreground() {}
    pub fn restore_foreground() {}
    pub fn send_ctrl_v() {}
}
