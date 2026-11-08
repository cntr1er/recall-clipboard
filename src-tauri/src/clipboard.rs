//! Clipboard change notifications, reading and writing.
//!
//! Windows: a hidden message-only window registered with
//! `AddClipboardFormatListener`, so the OS tells us about every change and we
//! never poll. The same window owns the clipboard when we write to it, which is
//! how we recognize (and skip) our own changes.

/// What a clipboard change contained, in order of preference.
pub enum Clip {
    /// Full paths of files copied in Explorer (`CF_HDROP`).
    Files(Vec<String>),
    Text(String),
    /// Bytes of the registered "PNG" format (keeps transparency).
    Png(Vec<u8>),
    /// Bytes of `CF_DIB`.
    Dib(Vec<u8>),
}

/// Which kinds of content to read. Disabled kinds are never read at all.
#[derive(Clone, Copy)]
pub struct Wanted {
    pub text: bool,
    pub images: bool,
    pub files: bool,
}

/// `DROPFILES` header size: pFiles (u32), pt (2 × i32), fNC (i32), fWide (i32).
const DROPFILES_SIZE: usize = 20;

/// Encodes paths as a `CF_HDROP` payload: a `DROPFILES` header followed by
/// NUL-terminated UTF-16 paths and a final NUL.
pub fn encode_drop_files(paths: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [DROPFILES_SIZE as u32, 0, 0, 0, 1] {
        out.extend_from_slice(&v.to_le_bytes()); // fWide = 1: paths are UTF-16
    }
    for unit in paths
        .iter()
        .flat_map(|p| p.encode_utf16().chain([0]))
        .chain([0])
    {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

/// Decodes a `CF_HDROP` payload. Returns `None` if it's malformed or empty.
pub fn decode_drop_files(data: &[u8]) -> Option<Vec<String>> {
    let offset = u32::from_le_bytes(data.get(..4)?.try_into().ok()?) as usize;
    let wide = u32::from_le_bytes(data.get(16..20)?.try_into().ok()?) != 0;
    let list = data.get(offset..)?;
    let names: Vec<String> = if wide {
        let units: Vec<u16> = list
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        units
            .split(|&u| u == 0)
            .take_while(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    } else {
        // shortcut: ANSI file lists are decoded as Latin-1; real ANSI code pages are rare here.
        list.split(|&b| b == 0)
            .take_while(|s| !s.is_empty())
            .map(|s| s.iter().map(|&b| b as char).collect())
            .collect()
    };
    (!names.is_empty()).then_some(names)
}

#[cfg(windows)]
pub use imp::{changed_by_us, read, watch, write_files, write_image, write_text};

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::thread;
    use std::time::Duration;

    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
        GetClipboardOwner, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
        SetClipboardData,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
    };
    use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
        RegisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WNDCLASSW,
    };
    use windows::core::{HSTRING, w};

    use super::{Clip, Wanted};
    use crate::privacy::{self, ClipFlags};

    static ON_CHANGE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
    static LISTENER: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

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
            LISTENER.store(hwnd.0, Ordering::Relaxed);
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

    fn listener() -> HWND {
        HWND(LISTENER.load(Ordering::Relaxed))
    }

    /// True if the current clipboard contents were written by this app.
    pub fn changed_by_us() -> bool {
        let ours = listener();
        !ours.is_invalid() && unsafe { GetClipboardOwner() }.is_ok_and(|o| o == ours)
    }

    fn format(name: &str) -> u32 {
        unsafe { RegisterClipboardFormatW(&HSTRING::from(name)) }
    }

    fn available(format: u32) -> bool {
        unsafe { IsClipboardFormatAvailable(format) }.is_ok()
    }

    /// Holds the clipboard open; closes it on drop.
    struct Opened;

    impl Opened {
        fn new(owner: Option<HWND>) -> Option<Self> {
            // The app that just wrote the clipboard may still hold it briefly.
            for _ in 0..10 {
                if unsafe { OpenClipboard(owner) }.is_ok() {
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

    /// Copies one format's bytes out. The clipboard must be open.
    fn bytes(format: u32) -> Option<Vec<u8>> {
        if !available(format) {
            return None;
        }
        unsafe {
            let global = HGLOBAL(GetClipboardData(format).ok()?.0);
            let ptr = GlobalLock(global) as *const u8;
            if ptr.is_null() {
                return None;
            }
            let data = std::slice::from_raw_parts(ptr, GlobalSize(global)).to_vec();
            let _ = GlobalUnlock(global);
            Some(data)
        }
    }

    fn utf16_text(bytes: &[u8]) -> String {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        let len = units.iter().position(|&c| c == 0).unwrap_or(units.len());
        String::from_utf16_lossy(&units[..len])
    }

    /// Reads the clipboard, or returns `None` if it's empty, unreadable, of an
    /// unwanted kind, or marked private by its source (in which case the content
    /// is never read).
    pub fn read(wanted: Wanted) -> Option<Clip> {
        let _open = Opened::new(None)?;
        let flags = ClipFlags {
            exclude_from_monitoring: available(format(
                "ExcludeClipboardContentFromMonitorProcessing",
            )),
            can_include_in_history: bytes(format("CanIncludeInClipboardHistory"))
                .and_then(|b| Some(u32::from_le_bytes(b.get(..4)?.try_into().ok()?))),
            viewer_ignore: available(format("Clipboard Viewer Ignore")),
        };
        if privacy::is_private(&flags) {
            return None;
        }
        let files = || {
            let data = bytes(CF_HDROP.0 as u32)?;
            super::decode_drop_files(&data).map(Clip::Files)
        };
        let text = || {
            let t = utf16_text(&bytes(CF_UNICODETEXT.0 as u32)?);
            (!t.trim().is_empty()).then_some(Clip::Text(t))
        };
        let image = || {
            bytes(format("PNG"))
                .map(Clip::Png)
                .or_else(|| bytes(CF_DIB.0 as u32).map(Clip::Dib))
        };
        // Files first: Explorer copies may also carry text or a bitmap preview.
        wanted
            .files
            .then(files)
            .flatten()
            .or_else(|| wanted.text.then(text).flatten())
            .or_else(|| wanted.images.then(image).flatten())
    }

    fn write(formats: &[(u32, &[u8])]) -> bool {
        let Some(_open) = Opened::new(Some(listener())) else {
            return false;
        };
        unsafe {
            if EmptyClipboard().is_err() {
                return false;
            }
            for &(format, data) in formats {
                let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, data.len()) else {
                    return false;
                };
                let ptr = GlobalLock(global) as *mut u8;
                if ptr.is_null() {
                    let _ = GlobalFree(Some(global));
                    return false;
                }
                std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
                let _ = GlobalUnlock(global);
                // On success the clipboard owns the memory; on failure we still do.
                if SetClipboardData(format, Some(HANDLE(global.0))).is_err() {
                    let _ = GlobalFree(Some(global));
                    return false;
                }
            }
        }
        true
    }

    pub fn write_text(text: &str) -> bool {
        let bytes: Vec<u8> = text
            .encode_utf16()
            .chain([0])
            .flat_map(|u| u.to_le_bytes())
            .collect();
        write(&[(CF_UNICODETEXT.0 as u32, &bytes)])
    }

    /// Writes both PNG (keeps transparency) and DIB (understood by every app).
    pub fn write_image(png: &[u8], dib: &[u8]) -> bool {
        write(&[(format("PNG"), png), (CF_DIB.0 as u32, dib)])
    }

    /// Writes a file list that Explorer pastes as a copy (not a move).
    pub fn write_files(paths: &[String]) -> bool {
        const DROPEFFECT_COPY: u32 = 1;
        write(&[
            (CF_HDROP.0 as u32, &super::encode_drop_files(paths)),
            (
                format("Preferred DropEffect"),
                &DROPEFFECT_COPY.to_le_bytes(),
            ),
        ])
    }
}

// shortcut: the clipboard is Windows-only; add per-OS backends to support macOS/Linux.
#[cfg(not(windows))]
mod stub {
    use super::{Clip, Wanted};
    pub fn watch(_on_change: impl Fn() + Send + Sync + 'static) {}
    pub fn changed_by_us() -> bool {
        false
    }
    pub fn read(_wanted: Wanted) -> Option<Clip> {
        None
    }
    pub fn write_files(_paths: &[String]) -> bool {
        false
    }
    pub fn write_text(_text: &str) -> bool {
        false
    }
    pub fn write_image(_png: &[u8], _dib: &[u8]) -> bool {
        false
    }
}

#[cfg(not(windows))]
pub use stub::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_files_round_trip() {
        let paths = vec![
            r"C:\Users\me\Desktop\report.pdf".to_string(),
            r"D:\Fotos\Ünïcødé 📷.png".to_string(),
        ];
        assert_eq!(
            decode_drop_files(&encode_drop_files(&paths)).unwrap(),
            paths
        );
    }

    #[test]
    fn drop_files_decodes_ansi_lists() {
        let mut data = vec![0u8; DROPFILES_SIZE];
        data[0] = DROPFILES_SIZE as u8; // pFiles; fWide stays 0
        data.extend_from_slice(b"C:\\a.txt\0C:\\b.txt\0\0");
        assert_eq!(
            decode_drop_files(&data).unwrap(),
            [r"C:\a.txt", r"C:\b.txt"]
        );
    }

    #[test]
    fn malformed_drop_files_are_rejected() {
        assert!(decode_drop_files(&[1, 2, 3]).is_none());
        assert!(
            decode_drop_files(&encode_drop_files(&[])).is_none(),
            "empty list"
        );
        let mut bad_offset = encode_drop_files(&["x".into()]);
        bad_offset[0] = 255;
        assert!(decode_drop_files(&bad_offset).is_none());
    }
}
