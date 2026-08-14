//! Print-quality PDF render. The preview pipeline works on 1600 px
//! thumbnails; this one reopens every original, one at a time, and embeds it
//! at the resolution its slot actually needs at 300 dpi. Never more (a
//! 45 Mpx frame in an octo cell would bloat the file for nothing), never
//! upscaled (missing pixels cannot be invented).

use crate::model::Album;
use crate::{meta, pdf, thumb};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Target output resolution. 300 dpi is what both Cloudprinter and Prodigi
/// print at; above it the extra pixels are discarded by the RIP.
pub const PRINT_DPI: f64 = 300.0;

/// JPEG re-encode quality for downscaled images. 92 is visually lossless on
/// photographic content and keeps a 96-page album under control.
const JPEG_QUALITY: u8 = 92;

/// Downscale factor for one slot: 1.0 means the source, cover-cropped into
/// the slot, sits exactly at [`PRINT_DPI`]. Above 1.0 the image is already at
/// or below print resolution and must not be touched.
pub fn print_scale(rect: &pdf::Rect, iw: u32, ih: u32) -> f64 {
    // Same cover-crop as the renderer: mm per source pixel once scaled to fill.
    let s = (rect.w / iw as f64).max(rect.h / ih as f64);
    s * PRINT_DPI / 25.4
}

/// Width, height and component count from a JPEG's SOF marker, without
/// decoding. Component count decides passthrough: only plain 3-component
/// (YCbCr/RGB) files can be embedded as-is under DeviceRGB.
fn jpeg_sof(data: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2usize;
    while i + 10 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            let h = u32::from(data[i + 5]) << 8 | u32::from(data[i + 6]);
            let w = u32::from(data[i + 7]) << 8 | u32::from(data[i + 8]);
            return Some((w, h, data[i + 9]));
        }
        let len = usize::from(data[i + 2]) << 8 | usize::from(data[i + 3]);
        i += 2 + len;
    }
    None
}

/// One original resolved for print: passthrough when the file is a plain
/// upright JPEG already at or below the slot's need, decode + orient +
/// downscale + re-encode otherwise.
fn print_asset(
    src: &Path,
    orientation: u32,
    rect: &pdf::Rect,
    focal: [f64; 2],
    zoom: f64,
) -> Result<pdf::JpegAsset> {
    let zoom = zoom.max(1.0);
    let is_jpeg = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
        .unwrap_or(false);

    if is_jpeg && orientation == 1 {
        let data = fs::read(src).with_context(|| format!("lecture de {}", src.display()))?;
        if let Some((w, h, 3)) = jpeg_sof(&data) {
            // A zoomed slot shows fewer source pixels, so it needs more of
            // them: the passthrough bar rises with the zoom.
            if print_scale(rect, w, h) * zoom >= 1.0 {
                return Ok(pdf::JpegAsset { data, width: w, height: h, focal, zoom });
            }
        }
    }

    let img = crate::heic::open(src).with_context(|| format!("décodage de {}", src.display()))?;
    let img = thumb::apply_orientation(img, orientation);
    let f = print_scale(rect, img.width(), img.height()) * zoom;
    let img = if f < 1.0 {
        let w = ((img.width() as f64 * f).round() as u32).max(1);
        let h = ((img.height() as f64 * f).round() as u32).max(1);
        img.resize_exact(w, h, image::imageops::FilterType::CatmullRom)
    } else {
        img
    };
    let rgb = img.to_rgb8();
    let mut data = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, JPEG_QUALITY)
        .encode_image(&rgb)
        .with_context(|| format!("réencodage de {}", src.display()))?;
    Ok(pdf::JpegAsset { data, width: rgb.width(), height: rgb.height(), focal, zoom })
}

/// The caller decided to abandon a long render. Checked between photos, so
/// cancelling answers within a photo's decode, never mid-file: the atomic
/// temp + rename below guarantees no half-written PDF survives either path.
pub type CancelFlag<'a> = &'a (dyn Fn() -> bool + Sync);

/// Render a print-resolution PDF from `album.json`, resolving every photo
/// through the album root. Sequential on purpose: one 45 Mpx original in
/// memory at a time, five hundred at once would sink the process.
///
/// Unlike the preview render this fails loudly on any missing or unreadable
/// original: a print file with a silently skipped page costs a reprint.
pub fn render_print_pdf(
    dir: &Path,
    out: &Path,
    progress: &dyn Fn(&str),
    cancel: CancelFlag,
) -> Result<PathBuf> {
    let json = dir.join("album.json");
    let album: Album = serde_json::from_str(
        &fs::read_to_string(&json).with_context(|| format!("lecture de {}", json.display()))?,
    )
    .context("album.json illisible")?;
    anyhow::ensure!(
        !album.root.is_empty(),
        "album.json ne connaît pas son dossier de photos : recomposez l'album"
    );
    let root = PathBuf::from(&album.root);
    anyhow::ensure!(
        root.is_dir(),
        "dossier de photos introuvable : {} (déplacé ou disque absent ?)",
        root.display()
    );

    let g = pdf::geometry(&album);
    let total: usize = album.spreads.iter().map(|s| s.slots.len()).sum();
    let mut done = 0usize;
    let mut writer = pdf::PdfWriter::new(&album);
    for (i, spread) in album.spreads.iter().enumerate() {
        let rects = pdf::slots_for(&spread.template, spread.slots.len(), &g);
        let mut assets = Vec::with_capacity(spread.slots.len());
        for (slot, rect) in spread.slots.iter().zip(rects.iter()) {
            anyhow::ensure!(!cancel(), "export annulé");
            let src = root.join(&slot.src);
            let orientation = meta::read(&src).orientation;
            let asset = print_asset(&src, orientation, rect, slot.focal, slot.zoom)
                .with_context(|| format!("planche {} : {}", i + 1, slot.src))?;
            assets.push(asset);
            done += 1;
            progress(&format!("render: {done}/{total}"));
        }
        writer.add_spread(spread, &assets)?;
    }
    anyhow::ensure!(!cancel(), "export annulé");

    // Temp file then rename: a crash mid-write never leaves a truncated PDF
    // where the caller will look for a finished one.
    let tmp = out.with_extension("pdf.part");
    let saved = writer.save(&tmp);
    if saved.is_err() {
        let _ = fs::remove_file(&tmp);
        saved?;
    }
    fs::rename(&tmp, out)
        .with_context(|| format!("renommage vers {}", out.display()))?;
    Ok(out.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> pdf::Rect {
        pdf::Rect { x: 0.0, y: 0.0, w, h }
    }

    #[test]
    fn scale_downsamples_oversized_and_never_upscales() {
        // 100 mm slot at 300 dpi needs ~1181 px on the covered side.
        let r = rect(100.0, 100.0);
        assert!(print_scale(&r, 3000, 2000) < 1.0); // 45 Mpx-class: downscale
        assert!(print_scale(&r, 1000, 1000) > 1.0); // small original: keep
        // Cover-crop picks the tighter side: a wide slot over a tall image
        // is governed by the width.
        let wide = rect(200.0, 50.0);
        let f = print_scale(&wide, 2000, 4000);
        assert!((f - (200.0 / 2000.0) * PRINT_DPI / 25.4).abs() < 1e-9);
    }

    /// A cancelled export writes nothing: no destination file, no leftover
    /// .part. The DoD's « annuler un export en cours sans corruption ».
    #[test]
    fn cancelled_export_leaves_no_file() {
        let dir = std::env::temp_dir().join(format!("colophon-cancel-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // Minimal album folder: one spread, one photo.
        let src_dir = dir.join("photos");
        fs::create_dir_all(&src_dir).unwrap();
        let img = image::RgbImage::from_pixel(320, 240, image::Rgb([90, 120, 150]));
        image::DynamicImage::ImageRgb8(img)
            .save_with_format(src_dir.join("a.jpg"), image::ImageFormat::Jpeg)
            .unwrap();
        let mut album = Album::new("t", &src_dir, crate::model::Size { w: 210.0, h: 210.0 });
        album.spreads.push(crate::model::Spread {
            template: "solo".into(),
            slots: vec![crate::model::Slot::new("a.jpg".into(), [0.5, 0.5])],
            caption: None,
            text: None,
            edited: false,
            locked: false,
        });
        fs::write(dir.join("album.json"), serde_json::to_string(&album).unwrap()).unwrap();

        let out = dir.join("out.pdf");
        let err = render_print_pdf(&dir, &out, &|_| {}, &|| true).unwrap_err();
        assert!(err.to_string().contains("annulé"), "{err}");
        assert!(!out.exists(), "un export annulé ne doit rien écrire");
        assert!(!out.with_extension("pdf.part").exists(), "pas de .part orphelin");

        // Same folder, no cancellation: the file lands, atomically.
        render_print_pdf(&dir, &out, &|_| {}, &|| false).unwrap();
        assert!(out.exists());
        assert!(!out.with_extension("pdf.part").exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sof_parser_reads_size_and_components() {
        let img = image::RgbImage::from_pixel(10, 8, image::Rgb([120, 90, 60]));
        let mut data = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, 90)
            .encode_image(&img)
            .unwrap();
        assert_eq!(jpeg_sof(&data), Some((10, 8, 3)));
    }

    #[test]
    fn asset_passthrough_and_downscale() {
        let dir = std::env::temp_dir().join(format!("colophon-print-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("t.jpg");
        let img = image::RgbImage::from_fn(400, 300, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        image::DynamicImage::ImageRgb8(img)
            .save_with_format(&src, image::ImageFormat::Jpeg)
            .unwrap();

        // Big slot: the 400 px original is far below print need, passthrough.
        let a = print_asset(&src, 1, &rect(200.0, 150.0), [0.5, 0.5], 1.0).unwrap();
        assert_eq!((a.width, a.height), (400, 300));
        assert_eq!(a.data, fs::read(&src).unwrap());

        // Tiny slot: 10 mm at 300 dpi is 118 px, the original is downscaled.
        let a = print_asset(&src, 1, &rect(10.0, 10.0), [0.5, 0.5], 1.0).unwrap();
        assert_eq!(a.height, 118);
        assert!(a.width < 400);

        // Same tiny slot zoomed ×2: the crop shows half the frame, so twice
        // the pixels are kept for the same printed millimetres.
        let a = print_asset(&src, 1, &rect(10.0, 10.0), [0.5, 0.5], 2.0).unwrap();
        assert_eq!(a.height, 236);

        // Rotated EXIF: no passthrough, the pixels come out oriented.
        let a = print_asset(&src, 6, &rect(200.0, 150.0), [0.5, 0.5], 1.0).unwrap();
        assert_eq!((a.width, a.height), (300, 400));

        fs::remove_dir_all(&dir).ok();
    }
}
