//! Finds which app a copy came from: the foreground window's process, its
//! friendly name (from the exe's version info) and its icon.

use std::path::PathBuf;

pub struct SourceApp {
    /// Lowercase file name, e.g. `chrome.exe`. Used as the app's key everywhere.
    pub exe: String,
    pub path: PathBuf,
}

impl SourceApp {
    fn from_path(path: PathBuf) -> Option<Self> {
        let exe = path.file_name()?.to_string_lossy().to_lowercase();
        Some(Self { exe, path })
    }

    /// Friendly name, falling back to the file stem (`chrome`).
    pub fn display_name(&self) -> String {
        imp::file_description(&self.path).unwrap_or_else(|| {
            self.path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
    }

    pub fn icon(&self) -> Option<image::RgbaImage> {
        imp::icon(&self.path)
    }
}

// shortcut: UWP apps report their host (ApplicationFrameHost.exe) as the source;
// resolve the hosted app's package if that ever matters.
pub fn foreground() -> Option<SourceApp> {
    SourceApp::from_path(imp::foreground_exe()?)
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};

    use image::RgbaImage;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, GetObjectW, HBITMAP,
    };
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::Shell::ExtractIconExW;
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, GetForegroundWindow, GetIconInfo, GetWindowThreadProcessId, HICON, ICONINFO,
    };
    use windows::core::{HSTRING, PWSTR, w};

    pub fn foreground_exe() -> Option<PathBuf> {
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            // Fails for elevated processes when we're not elevated; that's fine.
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(process);
            ok.ok()?;
            Some(PathBuf::from(String::from_utf16_lossy(
                &buf[..len as usize],
            )))
        }
    }

    /// The exe's `FileDescription`, e.g. "Google Chrome".
    pub fn file_description(path: &Path) -> Option<String> {
        let file = HSTRING::from(path.as_os_str());
        unsafe {
            let size = GetFileVersionInfoSizeW(&file, None);
            if size == 0 {
                return None;
            }
            let mut info = vec![0u8; size as usize];
            GetFileVersionInfoW(&file, None, size, info.as_mut_ptr().cast()).ok()?;

            let mut ptr: *mut c_void = std::ptr::null_mut();
            let mut len = 0u32;
            let (lang, codepage) = if VerQueryValueW(
                info.as_ptr().cast(),
                w!("\\VarFileInfo\\Translation"),
                &mut ptr,
                &mut len,
            )
            .as_bool()
                && len >= 4
            {
                let pair = ptr as *const u16;
                (*pair, *pair.add(1))
            } else {
                (0x0409, 0x04b0) // en-US, Unicode
            };

            let key = HSTRING::from(format!(
                "\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription"
            ));
            if !VerQueryValueW(info.as_ptr().cast(), &key, &mut ptr, &mut len).as_bool() || len == 0
            {
                return None;
            }
            let chars = std::slice::from_raw_parts(ptr as *const u16, len as usize);
            let name = String::from_utf16_lossy(chars)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            (!name.is_empty()).then_some(name)
        }
    }

    /// The exe's large (32 px) icon as RGBA.
    pub fn icon(path: &Path) -> Option<RgbaImage> {
        unsafe {
            let mut hicon = HICON::default();
            ExtractIconExW(
                &HSTRING::from(path.as_os_str()),
                0,
                Some(&mut hicon),
                None,
                1,
            );
            if hicon.is_invalid() {
                return None;
            }
            let mut info = ICONINFO::default();
            let result = GetIconInfo(hicon, &mut info).ok().and_then(|_| {
                let color = bitmap_bgra(info.hbmColor)?;
                let mask = bitmap_bgra(info.hbmMask);
                Some(to_rgba(color, mask))
            });
            let _ = DeleteObject(info.hbmColor.into());
            let _ = DeleteObject(info.hbmMask.into());
            let _ = DestroyIcon(hicon);
            result
        }
    }

    /// (width, height, top-down BGRA pixels)
    unsafe fn bitmap_bgra(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
        unsafe {
            if bitmap.is_invalid() {
                return None;
            }
            let mut bm = BITMAP::default();
            let size = size_of::<BITMAP>() as i32;
            if GetObjectW(bitmap.into(), size, Some((&mut bm as *mut BITMAP).cast())) == 0 {
                return None;
            }
            let (w, h) = (bm.bmWidth, bm.bmHeight);
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // negative: top-down rows
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = vec![0u8; (w * h * 4) as usize];
            let dc = CreateCompatibleDC(None);
            let lines = GetDIBits(
                dc,
                bitmap,
                0,
                h as u32,
                Some(pixels.as_mut_ptr().cast()),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(dc);
            (lines > 0).then_some((w as u32, h as u32, pixels))
        }
    }

    fn to_rgba((w, h, bgra): (u32, u32, Vec<u8>), mask: Option<(u32, u32, Vec<u8>)>) -> RgbaImage {
        let has_alpha = bgra.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
        let mut img = RgbaImage::new(w, h);
        for (i, px) in img.pixels_mut().enumerate() {
            let p = &bgra[i * 4..i * 4 + 4];
            // Old icons have no alpha channel; their AND mask is white where transparent.
            let alpha = if has_alpha {
                p[3]
            } else {
                match &mask {
                    Some((_, _, m)) if m.get(i * 4) == Some(&255) => 0,
                    _ => 255,
                }
            };
            *px = image::Rgba([p[2], p[1], p[0], alpha]);
        }
        img
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::{Path, PathBuf};
    pub fn foreground_exe() -> Option<PathBuf> {
        None
    }
    pub fn file_description(_: &Path) -> Option<String> {
        None
    }
    pub fn icon(_: &Path) -> Option<image::RgbaImage> {
        None
    }
}
