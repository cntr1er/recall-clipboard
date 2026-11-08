//! Clipboard change notifications and reading.
//!
//! Windows: a hidden message-only window registered with
//! `AddClipboardFormatListener`, so the OS tells us about every change and we
//! never poll.

#[cfg(windows)]
pub use imp::{read_text, watch};

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;
    use std::thread;
    use std::time::Duration;

    use windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
        RegisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WNDCLASSW,
    };
    use windows::core::w;

    static ON_CHANGE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

    /// Calls `on_change` on a background thread after every clipboard change.
    /// Only the first call has an effect.
    pub fn watch(on_change: impl Fn() + Send + Sync + 'static) {
        if ON_CHANGE.set(Box::new(on_change)).is_err() {
            return;
        }
        thread::spawn(|| {
            if let Err(e) = unsafe { message_loop() } {
                eprintln!("clipboard watcher stopped: {e}");
            }
        });
    }

    unsafe fn message_loop() -> windows::core::Result<()> {
        unsafe {
            let instance = GetModuleHandleW(None)?.into();
            let class = w!("RecallClipboardListener");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                return Err(windows::core::Error::from_thread());
            }
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance),
                None,
            )?;
            AddClipboardFormatListener(hwnd)?;

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
            Ok(())
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == WM_CLIPBOARDUPDATE {
            if let Some(f) = ON_CHANGE.get() {
                f();
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }

    /// Holds the clipboard open; closes it on drop.
    struct Opened;

    impl Opened {
        fn new() -> Option<Self> {
            // The app that just wrote the clipboard may still hold it briefly.
            for _ in 0..10 {
                if unsafe { OpenClipboard(None) }.is_ok() {
                    return Some(Opened);
                }
                thread::sleep(Duration::from_millis(20));
            }
            None
        }
    }

    impl Drop for Opened {
        fn drop(&mut self) {
            let _ = unsafe { CloseClipboard() };
        }
    }

    /// The clipboard's Unicode text, if it has any.
    pub fn read_text() -> Option<String> {
        let _open = Opened::new()?;
        unsafe {
            let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
            let global = HGLOBAL(handle.0);
            let ptr = GlobalLock(global) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let units = std::slice::from_raw_parts(ptr, GlobalSize(global) / 2);
            let len = units.iter().position(|&c| c == 0).unwrap_or(units.len());
            let text = String::from_utf16_lossy(&units[..len]);
            let _ = GlobalUnlock(global);
            Some(text)
        }
    }
}

// shortcut: capture is Windows-only; add a per-OS watcher (or polling) to support macOS/Linux.
#[cfg(not(windows))]
pub fn watch(_on_change: impl Fn() + Send + Sync + 'static) {}

#[cfg(not(windows))]
pub fn read_text() -> Option<String> {
    None
}
