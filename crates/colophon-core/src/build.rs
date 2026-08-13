//! The full album build, from a folder to `album.json` + `album.pdf`.
//! Kept in the library so the CLI and the app run the exact same pipeline.

use crate::pipeline::Photo;
use crate::{analyze, face, layout, meta, model, pdf, pipeline, scan, thumb};
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

pub struct BuildOptions {
    /// Album title. Defaults to the folder name when empty.
    pub title: Option<String>,
    /// Target number of spreads for the finished album.
    pub spreads: usize,
    /// Trim size of a single page, in millimetres. See `format`.
    pub trim: model::Size,
    /// Called with human-readable progress lines.
    pub progress: Box<dyn Fn(&str) + Send + Sync>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            title: None,
            spreads: 48,
            trim: model::Size { w: 210.0, h: 210.0 },
            progress: Box::new(|_| {}),
        }
    }
}

pub struct BuildReport {
    pub album: model::Album,
    pub album_json: PathBuf,
    pub album_pdf: PathBuf,
    pub photos_scanned: usize,
    pub photos_kept: usize,
    pub chapters: usize,
}

pub fn build_album(photos_dir: &Path, out: &Path, opts: BuildOptions) -> Result<BuildReport> {
    let say = &opts.progress;
    let root = photos_dir
        .canonicalize()
        .with_context(|| format!("photos folder {}", photos_dir.display()))?;
    let title = opts.title.clone().unwrap_or_else(|| {
        root.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Album".into())
    });
    fs::create_dir_all(out)?;

    // 1. scan
    let scanned = scan::scan(&root);
    say(&format!(
        "scan: {} images ({} HEIC skipped, {} unknown skipped)",
        scanned.images.len(),
        scanned.skipped_heic,
        scanned.skipped_other
    ));
    if scanned.skipped_heic > 0 {
        say("note: HEIC decoding is not wired yet; those photos are left out for now");
    }
    anyhow::ensure!(!scanned.images.is_empty(), "no usable images found");
    let photos_scanned = scanned.images.len();

    // 2. metadata + thumbnails + analysis, in parallel
    let cache = thumb::ThumbCache::new(out)?;
    let photos: Vec<Photo> = scanned
        .images
        .par_iter()
        .map_init(face::new_detector, |det, path| {
            let meta = meta::read(path);
            let img = match cache.get(path, meta.orientation) {
                Ok(i) => i,
                Err(e) => {
                    say(&format!("skip {}: {e:#}", path.display()));
                    return None;
                }
            };
            let analysis = analyze::analyze(&img);
            let focal = face::focal_point(det.as_mut(), &img);
            Some(Photo { path: path.clone(), meta, analysis, focal })
        })
        .flatten()
        .collect();
    say(&format!("analyze: {} photos", photos.len()));

    // 3. drop junk, dedup bursts and scenes, chapter, cap
    let (photos, junk) = pipeline::split_junk(photos);
    if junk > 0 {
        say(&format!(
            "junk: {junk} photos without EXIF date or GPS excluded (screenshots, forwards)"
        ));
    }
    let kept = pipeline::dedup(photos);
    say(&format!("dedup: {} kept", kept.len()));

    let spreads_target = opts.spreads.max(8);
    let max_chapters = (spreads_target / 3).clamp(4, 26);

    let chapters = pipeline::chapters(kept);
    let natural = chapters.len();
    let mut base = pipeline::merge_chapters(chapters, max_chapters);
    let twins: usize = base.iter_mut().map(pipeline::thin_twins).sum();
    let moments: usize = base.iter_mut().map(pipeline::cap_moments).sum();
    if twins + moments > 0 {
        say(&format!(
            "thinning: {twins} near-identical frames, {moments} extra frames of the same moment"
        ));
    }

    // 4. compose spreads. How many photos a spread holds depends on their
    // orientation and their scores, so the budget is aimed rather than
    // computed: compose, measure, correct. Composition costs nothing, no
    // image is touched here.
    let mut album = model::Album::new(&title, &root, opts.trim);
    let mut budget = spreads_target * layout::PHOTOS_PER_SPREAD_X10 / 10;
    let mut photos_kept = 0;
    for attempt in 0..5 {
        let mut trial = base.clone();
        let caps = pipeline::allocate_budget(&trial, budget);
        for (c, cap) in trial.iter_mut().zip(caps) {
            pipeline::cap_chapter(c, cap);
        }
        photos_kept = trial.iter().map(|c| c.photos.len()).sum();

        let mut composer = layout::Composer::new(album.page_aspect());
        album.spreads = trial
            .iter()
            .flat_map(|c| composer.compose(c, Some(chapter_caption(c)), &root))
            .collect();

        let got = album.spreads.len();
        let off = got.abs_diff(spreads_target) * 100 / spreads_target.max(1);
        if off <= 6 || attempt == 4 || got == 0 {
            break;
        }
        // Aim the next budget at the target, damped so it cannot oscillate.
        let aimed = budget * spreads_target / got;
        budget = (budget + aimed) / 2;
    }

    say(&format!(
        "chapters: {} (from {natural} natural, {photos_kept} photos kept)",
        base.len()
    ));
    say(&format!(
        "layout: {} spreads for ~{spreads_target} asked, pages de {:.0} × {:.0} mm",
        album.spreads.len(),
        opts.trim.w,
        opts.trim.h
    ));

    // 5. album.json, plus the thumbnail index. Cache filenames hash the
    // absolute path and mtime, which no reader can recompute: without this
    // index an album folder is unreadable on another machine.
    let album_json = out.join("album.json");
    fs::write(&album_json, serde_json::to_string_pretty(&album)?)?;
    write_thumb_index(&album, &root, &cache, out)?;

    // 6. render PDF from thumbnails (preview quality in P0)
    let mut writer = pdf::PdfWriter::new(&album);
    for spread in &album.spreads {
        let assets: Vec<pdf::JpegAsset> = spread
            .slots
            .iter()
            .filter_map(|slot| {
                let src = root.join(&slot.src);
                let thumb_path = cache.path_for(&src);
                let data = fs::read(&thumb_path).ok()?;
                let (w, h) = jpeg_dimensions(&data)?;
                Some(pdf::JpegAsset { data, width: w, height: h, focal: slot.focal })
            })
            .collect();
        if assets.len() == spread.slots.len() {
            writer.add_spread(spread, &assets)?;
        } else {
            say("spread skipped: missing thumbnails");
        }
    }
    let album_pdf = out.join("album.pdf");
    writer.save(&album_pdf)?;

    Ok(BuildReport {
        chapters: base.len(),
        album,
        album_json,
        album_pdf,
        photos_scanned,
        photos_kept,
    })
}

/// `thumbs.json`: slot source to cached thumbnail filename, relative to
/// `.cache/thumbs`. Written next to album.json so the folder travels whole.
fn write_thumb_index(
    album: &model::Album,
    root: &Path,
    cache: &thumb::ThumbCache,
    out: &Path,
) -> Result<()> {
    let mut index = std::collections::BTreeMap::new();
    for spread in &album.spreads {
        for slot in &spread.slots {
            let cached = cache.path_for(&root.join(&slot.src));
            if let Some(name) = cached.file_name() {
                index.insert(slot.src.clone(), name.to_string_lossy().to_string());
            }
        }
    }
    fs::write(out.join("thumbs.json"), serde_json::to_string_pretty(&index)?)?;
    Ok(())
}

const MONTHS_FR: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août",
    "septembre", "octobre", "novembre", "décembre",
];

fn date_fr(d: chrono::NaiveDate, with_year: bool) -> String {
    use chrono::Datelike;
    let m = MONTHS_FR[d.month0() as usize];
    if with_year {
        format!("{} {} {}", d.day(), m, d.year())
    } else {
        format!("{} {}", d.day(), m)
    }
}

pub fn chapter_caption(c: &pipeline::Chapter) -> String {
    let s = c.start.date();
    let e = c.end.date();
    if s == e {
        date_fr(s, true)
    } else {
        format!("{} \u{2013} {}", date_fr(s, false), date_fr(e, true))
    }
}

/// Minimal JPEG SOF parser: width/height without a full decode.
pub fn jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2usize;
    while i + 9 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            let h = u32::from(data[i + 5]) << 8 | u32::from(data[i + 6]);
            let w = u32::from(data[i + 7]) << 8 | u32::from(data[i + 8]);
            return Some((w, h));
        }
        let len = usize::from(data[i + 2]) << 8 | usize::from(data[i + 3]);
        i += 2 + len;
    }
    None
}
