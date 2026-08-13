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

    // 3. drop junk, dedup bursts and scenes, chapter, cap. Every photo set
    // aside is recorded with its reason: curation.json feeds the sorting view.
    let rel = |p: &Path| {
        p.strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string()
    };
    let mut discards: Vec<model::Discard> = Vec::new();

    let (photos, junk) = pipeline::split_junk(photos);
    if !junk.is_empty() {
        say(&format!(
            "junk: {} photos without EXIF date or GPS excluded (screenshots, forwards)",
            junk.len()
        ));
    }
    discards.extend(junk.iter().map(|p| model::Discard {
        src: rel(&p.path),
        reason: "parasite".into(),
        kept: None,
    }));

    let (kept, dups) = pipeline::dedup(photos);
    say(&format!("dedup: {} kept", kept.len()));
    discards.extend(dups.iter().map(|(lost, won)| model::Discard {
        src: rel(lost),
        reason: "doublon".into(),
        kept: Some(rel(won)),
    }));

    let spreads_target = opts.spreads.max(8);
    let max_chapters = (spreads_target / 3).clamp(4, 26);

    let chapters = pipeline::chapters(kept);
    let natural = chapters.len();
    let mut base = pipeline::merge_chapters(chapters, max_chapters);
    let twins: Vec<_> = base.iter_mut().flat_map(pipeline::thin_twins).collect();
    let moments: Vec<_> = base.iter_mut().flat_map(pipeline::cap_moments).collect();
    if !twins.is_empty() || !moments.is_empty() {
        say(&format!(
            "thinning: {} near-identical frames, {} extra frames of the same moment",
            twins.len(),
            moments.len()
        ));
    }
    discards.extend(twins.iter().map(|(lost, won)| model::Discard {
        src: rel(lost),
        reason: "jumeau".into(),
        kept: Some(rel(won)),
    }));
    discards.extend(moments.iter().map(|(lost, won)| model::Discard {
        src: rel(lost),
        reason: "meme_moment".into(),
        kept: Some(rel(won)),
    }));

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
    // Photos that survived curation but not the spread budget.
    let shown: std::collections::HashSet<String> = album
        .spreads
        .iter()
        .flat_map(|s| s.slots.iter().map(|sl| sl.src.clone()))
        .collect();
    for chapter in &base {
        for photo in &chapter.photos {
            let src = rel(&photo.path);
            if !shown.contains(&src) {
                discards.push(model::Discard { src, reason: "hors_budget".into(), kept: None });
            }
        }
    }
    say(&format!("curation: {} photos set aside", discards.len()));

    let album_json = write_album_json(out, &album)?;
    fs::write(
        out.join("curation.json"),
        serde_json::to_string_pretty(&discards)?,
    )?;
    write_thumb_index(&album, &discards, &root, &cache, out)?;

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

/// Write `album.json` atomically: temp file then rename, so a crash halfway
/// never leaves a truncated album. The album is the user's work, not a cache.
pub fn write_album_json(dir: &Path, album: &model::Album) -> Result<PathBuf> {
    let target = dir.join("album.json");
    let tmp = dir.join("album.json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(album)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, &target).with_context(|| format!("rename onto {}", target.display()))?;
    Ok(target)
}

/// Re-render `album.pdf` from `album.json` alone, resolving every photo
/// through `thumbs.json`. No scan, no analysis: this is what the editor calls
/// after a change, and it works even when the original folder has moved.
pub fn render_album_pdf(dir: &Path) -> Result<PathBuf> {
    let json = dir.join("album.json");
    let album: model::Album = serde_json::from_str(
        &fs::read_to_string(&json).with_context(|| format!("read {}", json.display()))?,
    )
    .context("album.json illisible")?;
    let thumbs: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&fs::read_to_string(dir.join("thumbs.json"))?)
            .context("thumbs.json illisible")?;

    let mut writer = pdf::PdfWriter::new(&album);
    for (i, spread) in album.spreads.iter().enumerate() {
        let assets: Vec<pdf::JpegAsset> = spread
            .slots
            .iter()
            .filter_map(|slot| {
                let name = thumbs.get(&slot.src)?;
                let data = fs::read(dir.join(".cache").join("thumbs").join(name)).ok()?;
                let (w, h) = jpeg_dimensions(&data)?;
                Some(pdf::JpegAsset { data, width: w, height: h, focal: slot.focal })
            })
            .collect();
        anyhow::ensure!(
            assets.len() == spread.slots.len(),
            "planche {}: vignette manquante, régénérez l'album avec la commande colophon",
            i + 1
        );
        writer.add_spread(spread, &assets)?;
    }
    let pdf_path = dir.join("album.pdf");
    writer.save(&pdf_path)?;
    Ok(pdf_path)
}

/// `thumbs.json`: slot source to cached thumbnail filename, relative to
/// `.cache/thumbs`. Written next to album.json so the folder travels whole.
/// Discarded photos are indexed too: the sorting view shows what was set
/// aside, and their thumbnails already exist from the analysis pass.
fn write_thumb_index(
    album: &model::Album,
    discards: &[model::Discard],
    root: &Path,
    cache: &thumb::ThumbCache,
    out: &Path,
) -> Result<()> {
    let mut index = std::collections::BTreeMap::new();
    let mut add = |src: &str| {
        let cached = cache.path_for(&root.join(src));
        if let Some(name) = cached.file_name() {
            index.insert(src.to_string(), name.to_string_lossy().to_string());
        }
    };
    for spread in &album.spreads {
        for slot in &spread.slots {
            add(&slot.src);
        }
    }
    for d in discards {
        add(&d.src);
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
