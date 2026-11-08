//! Image clips: decoding clipboard formats, and storing them on disk.
//!
//! Images live as `images/<hash>.png` plus `images/<hash>.thumb.png`; the
//! database row only holds the hash. Keeping multi-megabyte blobs out of SQLite
//! keeps the database small and fast, and the webview can load the files directly.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ImageFormat, ImageResult, RgbaImage};

const THUMB_MAX: u32 = 160;

pub fn file(dir: &Path, hash: &str) -> PathBuf {
    dir.join(format!("{hash}.png"))
}

pub fn thumb(dir: &Path, hash: &str) -> PathBuf {
    dir.join(format!("{hash}.thumb.png"))
}

pub fn remove(dir: &Path, hash: &str) {
    let _ = std::fs::remove_file(file(dir, hash));
    let _ = std::fs::remove_file(thumb(dir, hash));
}

pub fn decode_png(bytes: &[u8]) -> Option<RgbaImage> {
    Some(
        image::load_from_memory_with_format(bytes, ImageFormat::Png)
            .ok()?
            .to_rgba8(),
    )
}

/// Decodes a `CF_DIB` clipboard payload (a BMP file without its 14-byte file header).
pub fn decode_dib(dib: &[u8]) -> Option<RgbaImage> {
    let bmp = dib_to_bmp(dib)?;
    let mut img = image::load_from_memory_with_format(&bmp, ImageFormat::Bmp)
        .ok()?
        .to_rgba8();
    // Many apps leave the alpha byte of 32-bit DIBs at zero; treat those as opaque.
    if img.pixels().all(|p| p[3] == 0) {
        img.pixels_mut().for_each(|p| p[3] = 255);
    }
    Some(img)
}

fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(dib.get(o..o + 4)?.try_into().ok()?));
    let u16_at = |o: usize| Some(u16::from_le_bytes(dib.get(o..o + 2)?.try_into().ok()?));
    let header_size = u32_at(0)?;
    let bit_count = u16_at(14)?;
    let compression = u32_at(16)?;
    let colors_used = u32_at(32)?;
    let palette = if colors_used > 0 {
        colors_used
    } else if bit_count <= 8 {
        1 << bit_count
    } else {
        0
    };
    // BI_BITFIELDS with a plain BITMAPINFOHEADER keeps its three masks after the header.
    let masks = if compression == 3 && header_size == 40 {
        12
    } else {
        0
    };
    let pixel_offset = 14 + header_size + palette * 4 + masks;

    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(14 + dib.len() as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&pixel_offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

/// Encodes an image as a 32-bit bottom-up `CF_DIB` payload for writing to the clipboard.
pub fn encode_dib(img: &RgbaImage) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mut dib = Vec::with_capacity(40 + (w * h * 4) as usize);
    for v in [40u32, w, h] {
        dib.extend_from_slice(&v.to_le_bytes());
    }
    dib.extend_from_slice(&1u16.to_le_bytes()); // planes
    dib.extend_from_slice(&32u16.to_le_bytes()); // bit count
    dib.extend_from_slice(&[0; 24]); // BI_RGB, size, resolution, palette: all zero
    for row in img.rows().rev() {
        for p in row {
            dib.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    dib
}

fn encode_png(img: &RgbaImage) -> ImageResult<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    img.write_with_encoder(PngEncoder::new_with_quality(
        &mut out,
        CompressionType::Fast,
        FilterType::Adaptive,
    ))?;
    Ok(out.into_inner())
}

/// Encodes any RGBA image (e.g. an app icon) to PNG on disk.
pub fn save_png(img: &RgbaImage, path: &Path) -> ImageResult<()> {
    std::fs::write(path, encode_png(img)?)?;
    Ok(())
}

/// Saves the image and its thumbnail (unless already stored). Returns its content hash.
pub fn store(dir: &Path, img: &RgbaImage) -> ImageResult<String> {
    // shortcut: std's hasher isn't guaranteed stable across Rust releases, so after an
    // upgrade an old image may be stored once more instead of deduped. Use a fixed
    // hash (e.g. blake3) if that ever matters.
    let mut h = DefaultHasher::new();
    img.dimensions().hash(&mut h);
    img.as_raw().hash(&mut h);
    let hash = format!("{:016x}", h.finish());

    let path = file(dir, &hash);
    if !path.exists() {
        std::fs::write(&path, encode_png(img)?)?;
        let (w, h) = img.dimensions();
        let scale = (THUMB_MAX as f32 / w.max(h) as f32).min(1.0);
        let small = image::imageops::thumbnail(
            img,
            ((w as f32 * scale) as u32).max(1),
            ((h as f32 * scale) as u32).max(1),
        );
        std::fs::write(thumb(dir, &hash), encode_png(&small)?)?;
    }
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RgbaImage {
        RgbaImage::from_fn(3, 2, |x, y| {
            image::Rgba([x as u8 * 50, y as u8 * 90, 7, 255])
        })
    }

    #[test]
    fn dib_round_trip_preserves_pixels_and_orientation() {
        let img = sample();
        assert_eq!(decode_dib(&encode_dib(&img)).unwrap(), img);
    }

    #[test]
    fn zero_alpha_dib_is_treated_as_opaque() {
        let mut img = sample();
        img.pixels_mut().for_each(|p| p[3] = 0);
        let decoded = decode_dib(&encode_dib(&img)).unwrap();
        assert!(decoded.pixels().all(|p| p[3] == 255));
    }

    #[test]
    fn truncated_dib_is_rejected() {
        assert!(decode_dib(&[40, 0, 0]).is_none());
    }

    #[test]
    fn store_dedupes_and_writes_thumbnail() {
        let dir = std::env::temp_dir().join(format!("recall-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = store(&dir, &sample()).unwrap();
        let b = store(&dir, &sample()).unwrap();
        assert_eq!(a, b);
        assert!(file(&dir, &a).exists() && thumb(&dir, &a).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
