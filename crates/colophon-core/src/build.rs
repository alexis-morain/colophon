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
    /// Target number of spreads for the finished album, pinned ones excluded.
    pub spreads: usize,
    /// Trim size of a single page, in millimetres. See `format`.
    pub trim: model::Size,
    /// Called with human-readable progress lines.
    pub progress: Box<dyn Fn(&str) + Send + Sync>,
    /// Returns true when the caller wants the build abandoned. Checked
    /// between stages and between photos; a cancelled build writes nothing.
    pub cancel: Box<dyn Fn() -> bool + Send + Sync>,
    /// Spreads a recomposition must preserve verbatim (edited or locked),
    /// each with the capture time it should be re-inserted at. Their photos
    /// are withdrawn from the pipeline so nothing places them twice.
    pub pinned: Vec<(model::Spread, Option<chrono::NaiveDateTime>)>,
    /// Cover carried over on recomposition; a fresh build has none yet.
    pub cover: Option<model::Cover>,
    /// How much the composer puts on a spread. Chosen at the first build and
    /// kept by every recomposition: changing pace halfway through would
    /// rebuild the album around the spreads the user had already pinned.
    pub densite: layout::Densite,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            title: None,
            spreads: 48,
            trim: model::Size { w: 210.0, h: 210.0 },
            progress: Box::new(|_| {}),
            cancel: Box::new(|| false),
            pinned: Vec::new(),
            cover: None,
            densite: layout::Densite::default(),
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

    let cancelled = || (opts.cancel)();
    let rel = |p: &Path| {
        p.strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string()
    };

    // 1. scan. Photos held by pinned spreads leave the pipeline here: a
    // recomposition must never place them a second time.
    let mut scanned = scan::scan(&root);
    let pinned_srcs: std::collections::HashSet<String> = opts
        .pinned
        .iter()
        .flat_map(|(s, _)| s.slots.iter().map(|sl| sl.src.clone()))
        .collect();
    if !pinned_srcs.is_empty() {
        scanned.images.retain(|p| !pinned_srcs.contains(&rel(p)));
    }
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

    // 2. metadata + thumbnails + analysis, in parallel. The longest phase by
    // far, so it reports counts as it goes: a progress bar with nothing to
    // say for ten seconds is a frozen app to the person watching.
    let cache = thumb::ThumbCache::new(out)?;
    let total = scanned.images.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let photos: Vec<Photo> = scanned
        .images
        .par_iter()
        .map_init(face::new_detector, |det, path| {
            if cancelled() {
                return None; // drain the queue fast, the check below bails
            }
            let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if n % 20 == 0 || n == total {
                say(&format!("analyze: {n}/{total}"));
            }
            let meta = meta::read(path);
            let img = match cache.get(path, meta.orientation) {
                Ok(i) => i,
                Err(e) => {
                    say(&format!("skip {}: {e:#}", path.display()));
                    return None;
                }
            };
            let analysis = analyze::analyze(&img);
            let faces = face::face_boxes(det.as_mut(), &img);
            let focal = face::focal_from_boxes(&faces);
            // Original size, oriented. Header read only, no decode. Falls
            // back to the thumbnail size, which understates the pixels and
            // keeps the composer conservative about big cells.
            let orig = crate::heic::dimensions(path)
                .map(|(w, h)| {
                    if (5..=8).contains(&meta.orientation) { (h, w) } else { (w, h) }
                })
                .unwrap_or((analysis.width, analysis.height));
            Some(Photo { path: path.clone(), meta, analysis, orig, faces, focal })
        })
        .flatten()
        .collect();
    anyhow::ensure!(!cancelled(), "composition annulée");
    say(&format!("analyze: {} photos", photos.len()));

    // Capture times, for re-inserting pinned spreads chronologically.
    let times: std::collections::HashMap<String, chrono::NaiveDateTime> =
        photos.iter().map(|p| (rel(&p.path), p.meta.taken)).collect();

    // 3. drop junk, dedup bursts and scenes, chapter, cap. Every photo set
    // aside is recorded with its reason: curation.json feeds the sorting view.
    // Face anchors survive into curation.json: a rescued photo is cropped
    // like any other. Keyed by path because the passes only return paths.
    let focals: std::collections::HashMap<PathBuf, [f64; 2]> = photos
        .iter()
        .map(|p| (p.path.clone(), p.focal.unwrap_or_else(model::default_focal)))
        .collect();
    let focal_of =
        |p: &Path| focals.get(p).copied().unwrap_or_else(model::default_focal);
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
        focal: focal_of(&p.path),
    }));

    let (photos, panos) = pipeline::split_unprintable(photos);
    if !panos.is_empty() {
        say(&format!(
            "panoramas: {} photos trop larges ou trop étroites pour une page",
            panos.len()
        ));
    }
    discards.extend(panos.iter().map(|p| model::Discard {
        src: rel(&p.path),
        reason: "panorama".into(),
        kept: None,
        focal: focal_of(&p.path),
    }));

    // Photos too small to print even in the smallest cell of THIS format
    // (250 ppi floor). A 1 000 px frame holds a mosaic cell at 21 cm and
    // nothing at all at 30 cm: the split depends on the page size.
    let scratch = model::Album::new(&title, &root, opts.trim);
    let g = pdf::geometry(&scratch);
    let min_cell = pdf::slots_for("octo", 8, &g)
        .into_iter()
        .min_by(|a, b| (a.w * a.h).partial_cmp(&(b.w * b.h)).unwrap())
        .expect("octo a des cases");
    let (photos, lowres): (Vec<_>, Vec<_>) = photos.into_iter().partition(|p| {
        crate::print::PRINT_DPI / crate::print::print_scale(&min_cell, p.orig.0, p.orig.1)
            >= crate::audit::MIN_EFFECTIVE_PPI
    });
    if !lowres.is_empty() {
        say(&format!(
            "définition: {} photos trop petites pour ce format",
            lowres.len()
        ));
    }
    discards.extend(lowres.iter().map(|p| model::Discard {
        src: rel(&p.path),
        reason: "definition".into(),
        kept: None,
        focal: focal_of(&p.path),
    }));

    let (kept, dups) = pipeline::dedup(photos);
    say(&format!("dedup: {} kept", kept.len()));
    discards.extend(dups.iter().map(|(lost, won)| model::Discard {
        src: rel(lost),
        reason: "doublon".into(),
        kept: Some(rel(won)),
        focal: focal_of(lost),
    }));

    let spreads_target = opts.spreads.max(8);
    // A chapter costs a dedicated opening page: too many chapters and the
    // album turns into a procession of solos.
    let max_chapters = (spreads_target / 4).clamp(4, 20);

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
        focal: focal_of(lost),
    }));
    discards.extend(moments.iter().map(|(lost, won)| model::Discard {
        src: rel(lost),
        reason: "meme_moment".into(),
        kept: Some(rel(won)),
        focal: focal_of(lost),
    }));

    // 4. compose spreads. How many photos a spread holds depends on their
    // orientation and their scores, so the budget is aimed rather than
    // computed: compose, measure, correct. Composition costs nothing, no
    // image is touched here.
    anyhow::ensure!(!cancelled(), "composition annulée");
    let mut album = model::Album::new(&title, &root, opts.trim);
    album.cover = opts.cover.clone();
    album.densite = opts.densite;
    let mut budget = spreads_target * opts.densite.photos_per_spread_x10() / 10;
    let mut photos_kept = 0;
    // Keep the attempt closest to the target: on fragmented sets the
    // spread count can refuse to follow the budget, and the last attempt
    // is then the worst one, not the best.
    let mut best: Option<(usize, Vec<model::Spread>, usize)> = None;
    for attempt in 0..5 {
        let mut trial = base.clone();
        let caps = pipeline::allocate_budget(&trial, budget);
        for (c, cap) in trial.iter_mut().zip(caps) {
            pipeline::cap_chapter(c, cap);
        }
        let kept = trial.iter().map(|c| c.photos.len()).sum();

        let mut composer = layout::Composer::avec_densite(&album, opts.densite);
        // Captions are worked out for the run of chapters at once, not one
        // by one: whether a place is worth naming depends on the chapter
        // before it.
        let captions = chapter_captions(&trial);
        let spreads: Vec<model::Spread> = trial
            .iter()
            .zip(captions)
            .flat_map(|(c, caption)| composer.compose(c, Some(caption), &root))
            .collect();

        let got = spreads.len();
        let off = got.abs_diff(spreads_target) * 100 / spreads_target.max(1);
        if best.as_ref().is_none_or(|(b, _, _)| off < *b) {
            best = Some((off, spreads, kept));
        }
        if off <= 6 || attempt == 4 || got == 0 {
            break;
        }
        // Aim the next budget at the target, damped so it cannot oscillate,
        // and never starved below two photos per requested spread: fewer
        // photos never means fewer spreads once chapters run on minimums.
        let aimed = budget * spreads_target / got;
        budget = ((budget + aimed) / 2).max(spreads_target * 2);
    }
    if let Some((_, spreads, kept)) = best {
        album.spreads = spreads;
        photos_kept = kept;
    }

    // Re-insert the pinned spreads where their photos belong in time. A
    // pinned spread's own photos are unknown to `times` (withdrawn above),
    // so already-inserted pinned spreads are transparent to the scan and
    // the original order between them holds.
    if !opts.pinned.is_empty() {
        let time_of = |s: &model::Spread| {
            s.slots.first().and_then(|sl| times.get(&sl.src)).copied()
        };
        let mut last_at: Option<usize> = None;
        for (spread, anchor) in &opts.pinned {
            let at = match anchor {
                Some(t) => album
                    .spreads
                    .iter()
                    .position(|s| time_of(s).is_some_and(|st| st > *t))
                    .unwrap_or(album.spreads.len()),
                // No time at all (a text page opening the album): right
                // after the previous pinned spread, else at the front.
                None => last_at.map(|i| i + 1).unwrap_or(0),
            };
            album.spreads.insert(at, spread.clone());
            last_at = Some(at);
        }
        say(&format!("pinned: {} planches conservées", opts.pinned.len()));
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
                discards.push(model::Discard {
                    src,
                    reason: "hors_budget".into(),
                    kept: None,
                    focal: focal_of(&photo.path),
                });
            }
        }
    }
    say(&format!("curation: {} photos set aside", discards.len()));

    let album_json = write_album_json(out, &album)?;
    // The composer's proposal, kept aside as the reference `--reprise`
    // measures against. Written once and never rewritten: a recomposition is
    // a new proposal, but by then the album already carries hand corrections,
    // and folding those into the reference would hide what we are measuring.
    let origine = out.join("album.origin.json");
    if !origine.exists() {
        fs::write(&origine, serde_json::to_string_pretty(&album)?)
            .with_context(|| format!("write {}", origine.display()))?;
    }
    fs::write(
        out.join("curation.json"),
        serde_json::to_string_pretty(&discards)?,
    )?;
    write_thumb_index(&album, &discards, &root, &cache, out)?;

    // 6. render PDF from thumbnails (preview quality in P0)
    anyhow::ensure!(!cancelled(), "composition annulée");
    say("pdf: rendu des planches");
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
                Some(pdf::JpegAsset { data, width: w, height: h, focal: slot.focal, zoom: slot.zoom })
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
                Some(pdf::JpegAsset { data, width: w, height: h, focal: slot.focal, zoom: slot.zoom })
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

pub fn date_fr(d: chrono::NaiveDate, with_year: bool) -> String {
    use chrono::Datelike;
    let m = MONTHS_FR[d.month0() as usize];
    if with_year {
        format!("{} {} {}", d.day(), m, d.year())
    } else {
        format!("{} {}", d.day(), m)
    }
}

/// The dates of one chapter, the line every chapter has carried so far.
pub fn chapter_dates(c: &pipeline::Chapter) -> String {
    let s = c.start.date();
    let e = c.end.date();
    if s == e {
        date_fr(s, true)
    } else {
        format!("{} \u{2013} {}", date_fr(s, false), date_fr(e, true))
    }
}

/// The town a chapter was shot in, when its photos agree on one.
pub fn chapter_place(c: &pipeline::Chapter) -> Option<&'static str> {
    let points: Vec<(f64, f64)> = c.photos.iter().filter_map(|p| p.meta.gps).collect();
    crate::places::place_of(&points).map(|city| city.name)
}

/// The line printed on each chapter's opening spread: where, then when.
///
/// The place comes from the GPS the cameras wrote, and only when a chapter's
/// photos agree on one town (see [`crate::places`]). Two things keep it
/// quiet. A chapter whose photos disagree, or carry no coordinates at all,
/// shows its dates alone, exactly as before. And a chapter in the same town
/// as the one before it drops the name: a week in Calvi is one place, not
/// eight chapters shouting « Calvi », and repeating it teaches the reader
/// to stop reading the line.
pub fn chapter_captions(chapters: &[pipeline::Chapter]) -> Vec<String> {
    let mut out = Vec::with_capacity(chapters.len());
    let mut previous: Option<&str> = None;
    for c in chapters {
        let dates = chapter_dates(c);
        let place = chapter_place(c);
        out.push(match place {
            Some(name) if previous != Some(name) => format!("{name}, {dates}"),
            _ => dates,
        });
        // A chapter that named nowhere does not reset the run: crossing an
        // unlocated day and coming back is still the same stay.
        if place.is_some() {
            previous = place;
        }
    }
    out
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
