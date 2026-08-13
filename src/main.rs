mod analyze;
mod layout;
mod meta;
mod model;
mod pdf;
mod pipeline;
mod scan;
mod thumb;

use anyhow::{Context, Result};
use clap::Parser;
use pipeline::Photo;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;

/// Colophon: from a folder of photos to a print-ready album.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Folder of photos to build the album from
    photos: PathBuf,

    /// Output directory (album.json, album.pdf, thumbnail cache)
    #[arg(short, long, default_value = "album-out")]
    out: PathBuf,

    /// Album title (defaults to the folder name)
    #[arg(short, long)]
    title: Option<String>,

    /// Target number of spreads (double pages) for the finished album
    #[arg(short, long, default_value_t = 48)]
    spreads: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let t0 = std::time::Instant::now();

    let root = cli
        .photos
        .canonicalize()
        .with_context(|| format!("photos folder {}", cli.photos.display()))?;
    let title = cli.title.clone().unwrap_or_else(|| {
        root.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Album".into())
    });
    fs::create_dir_all(&cli.out)?;

    // 1. scan
    let scanned = scan::scan(&root);
    eprintln!(
        "scan: {} images ({} HEIC skipped, {} unknown skipped)",
        scanned.images.len(),
        scanned.skipped_heic,
        scanned.skipped_other
    );
    if scanned.skipped_heic > 0 {
        eprintln!("note: HEIC decoding is not wired yet; those photos are left out for now");
    }
    anyhow::ensure!(!scanned.images.is_empty(), "no usable images found");

    // 2. metadata + thumbnails + analysis, in parallel
    let cache = thumb::ThumbCache::new(&cli.out)?;
    let photos: Vec<Photo> = scanned
        .images
        .par_iter()
        .filter_map(|path| {
            let meta = meta::read(path);
            let img = match cache.get(path, meta.orientation) {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("skip {}: {e:#}", path.display());
                    return None;
                }
            };
            let analysis = analyze::analyze(&img);
            Some(Photo { path: path.clone(), meta, analysis })
        })
        .collect();
    eprintln!("analyze: {} photos in {:.1?}", photos.len(), t0.elapsed());

    // 3. drop junk, dedup bursts and scenes, chapter, cap
    let (photos, junk) = pipeline::split_junk(photos);
    if junk > 0 {
        eprintln!("junk: {junk} photos without EXIF date or GPS excluded (screenshots, forwards)");
    }
    let kept = pipeline::dedup(photos);
    eprintln!("dedup: {} kept", kept.len());

    // Page budget: ~2.6 photos per spread on average with our templates.
    let spreads_target = cli.spreads.max(8);
    let photo_budget = spreads_target * 26 / 10;
    let max_chapters = (spreads_target / 3).clamp(4, 26);

    let chapters = pipeline::chapters(kept);
    let natural = chapters.len();
    let mut chapters = pipeline::merge_chapters(chapters, max_chapters);
    let caps = pipeline::allocate_budget(&chapters, photo_budget);
    for (c, cap) in chapters.iter_mut().zip(caps) {
        pipeline::cap_chapter(c, cap);
    }
    eprintln!(
        "chapters: {} (from {} natural, {} photos kept for ~{} spreads)",
        chapters.len(),
        natural,
        chapters.iter().map(|c| c.photos.len()).sum::<usize>(),
        spreads_target
    );

    // 4. compose spreads
    let mut album = model::Album::new(&title);
    for c in &chapters {
        let caption = Some(chapter_caption(c));
        album.spreads.extend(layout::compose(c, caption, &root));
    }
    eprintln!("layout: {} spreads", album.spreads.len());

    // 5. album.json
    let json_path = cli.out.join("album.json");
    fs::write(&json_path, serde_json::to_string_pretty(&album)?)?;

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
            eprintln!("spread skipped: missing thumbnails");
        }
    }
    let pdf_path = cli.out.join("album.pdf");
    writer.save(&pdf_path)?;

    eprintln!(
        "done in {:.1?}: {} and {}",
        t0.elapsed(),
        json_path.display(),
        pdf_path.display()
    );
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

fn chapter_caption(c: &pipeline::Chapter) -> String {
    let s = c.start.date();
    let e = c.end.date();
    if s == e {
        date_fr(s, true)
    } else {
        format!("{} \u{2013} {}", date_fr(s, false), date_fr(e, true))
    }
}

/// Minimal JPEG SOF parser: width/height without a full decode.
fn jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
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
