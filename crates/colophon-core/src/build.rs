//! The full album build, from a folder to `album.json` + `album.pdf`.
//! Kept in the library so the CLI and the app run the exact same pipeline.

use crate::pipeline::Photo;
use crate::releve::Releve;
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
    /// Print the colophon page. On by default, and a recomposition carries
    /// over what the album already said: taking the page away once must not
    /// have to be done again after every recomposition.
    pub colophon: bool,
    /// Print the half-title page at the head of the book. Same default and
    /// same carry-over as the colophon: they are the two pages the machine
    /// writes about the book rather than with its photographs.
    pub garde: bool,
    /// Alternative proposals to compose beside the one asked for. Empty is
    /// the plain path: one album, exactly as before.
    pub variantes: Vec<VarianteSpec>,
    /// Adjustments carried over on recomposition, keyed by source like
    /// `Album::reglages`. A field the recomposition does not carry is a
    /// field it destroys in silence — same rule as `densite` and `cover`.
    /// A fresh build has none.
    pub reglages: std::collections::BTreeMap<String, model::Reglage>,
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
            colophon: true,
            garde: true,
            variantes: Vec::new(),
            reglages: std::collections::BTreeMap::new(),
        }
    }
}

/// One alternative proposal to compose beside the one the caller asked for.
///
/// The composer is deterministic and already parameterised: the same analysed
/// photos, another pace and another spread budget, and it produces a visibly
/// different book for the price of some arithmetic. Nothing here touches the
/// composer's own thresholds, only the two numbers it is called with.
#[derive(Debug, Clone)]
pub struct VarianteSpec {
    /// File suffix and handle: `album.<id>.json`. A bare word, no path.
    pub id: String,
    pub nom: String,
    /// The one sentence that says what makes this proposal different.
    pub about: String,
    pub densite: layout::Densite,
    pub spreads: usize,
}

/// A composed proposal, as the creation screen shows it: what it is called,
/// what makes it different, how long it is, and three spreads to look at.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VarianteResume {
    pub id: String,
    pub nom: String,
    pub about: String,
    pub planches: usize,
    pub photos: usize,
    /// Sources of one photo from three spreads spread across the book, for
    /// the thumbnails beside the sentence. Never a path: the same relative
    /// source every slot carries.
    pub apercu: Vec<String>,
}

impl VarianteResume {
    fn de(spec: &VarianteSpec, album: &model::Album, photos: usize) -> Self {
        // A quarter, a half and three quarters in: three spreads that say
        // what the book looks like, rather than three from its opening.
        let avec_photo: Vec<&model::Spread> =
            album.spreads.iter().filter(|s| !s.slots.is_empty()).collect();
        let apercu = [1, 2, 3]
            .iter()
            .filter_map(|q| avec_photo.get(avec_photo.len() * q / 4))
            .filter_map(|s| s.slots.first().map(|sl| sl.src.clone()))
            .collect();
        Self {
            id: spec.id.clone(),
            nom: spec.nom.clone(),
            about: spec.about.clone(),
            planches: album.spreads.len(),
            photos,
            apercu,
        }
    }
}

pub struct BuildReport {
    pub album: model::Album,
    pub album_json: PathBuf,
    /// The preview PDF, absent when the composition had no thumbnails to
    /// draw it from — a relevé read back from its fiches. An empty path
    /// would read as a file somewhere.
    pub album_pdf: Option<PathBuf>,
    pub photos_scanned: usize,
    pub photos_kept: usize,
    pub chapters: usize,
    /// The alternatives composed beside the album, empty when none were
    /// asked for. Each is on disk as `album.<id>.json`.
    pub variantes: Vec<VarianteResume>,
}

/// The reading half of a composition: scan, thumbnails, metadata, analysis,
/// faces. The only half that touches a pixel, and so the only one that needs
/// the photographs to be there. Everything it measures is what [`composer`]
/// reads, and it serializes whole: see [`crate::releve`].
pub fn releve(photos_dir: &Path, out: &Path, opts: &BuildOptions) -> Result<Releve> {
    let say = &opts.progress;
    let root = photos_dir
        .canonicalize()
        .with_context(|| format!("photos folder {}", photos_dir.display()))?;
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
    // The folder gave nothing to work with: a named refusal, never an album
    // of zero pages presented as a success. Nothing has been written yet.
    if scanned.images.is_empty() {
        let detail = match (scanned.skipped_heic, scanned.skipped_other) {
            (0, 0) => "le dossier ne contient aucune image (JPEG, PNG ou HEIC)".to_string(),
            (h, o) => format!(
                "aucune image lisible : {h} HEIC que cette machine ne décode pas, \
                 {o} fichiers dans des formats non pris en charge"
            ),
        };
        anyhow::bail!("aucune photo exploitable : {detail}");
    }

    // 2. metadata + thumbnails + analysis, in parallel. The longest phase by
    // far, so it reports counts as it goes: a progress bar with nothing to
    // say for ten seconds is a frozen app to the person watching.
    // A file the decoder refuses is not dropped on the floor: it is kept,
    // with its reason, and lands in the curation report like any other
    // set-aside. An unreadable file the user cannot see is how an album
    // quietly loses photos.
    enum Analysed {
        Photo(Box<Photo>),
        Unreadable(PathBuf, String),
    }
    let cache = thumb::ThumbCache::new(out)?;
    let total = scanned.images.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let analysed: Vec<Analysed> = scanned
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
                    // File name only: this line travels to the app's
                    // progress panel and to the file log, and a full path
                    // names people and places.
                    let nom = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    say(&format!("skip {nom}: {e:#}"));
                    return Some(Analysed::Unreadable(path.clone(), format!("{e:#}")));
                }
            };
            let analysis = analyze::analyze(&img);
            let faces = face::face_boxes(det.as_mut(), &img);
            let focal = face::focal_from_boxes(&faces);
            // Original size, oriented. Header read only, no decode. Falls
            // back to the thumbnail size, which understates the pixels and
            // keeps the composer conservative about big cells.
            let orig = crate::heic::oriented_dimensions(path, meta.orientation)
                .unwrap_or((analysis.width, analysis.height));
            Some(Analysed::Photo(Box::new(Photo {
                path: path.clone(),
                meta,
                analysis,
                orig,
                faces,
                focal,
            })))
        })
        .flatten()
        .collect();
    anyhow::ensure!(!cancelled(), "composition annulée");
    let mut photos: Vec<Photo> = Vec::with_capacity(analysed.len());
    let mut unreadable: Vec<(PathBuf, String)> = Vec::new();
    for a in analysed {
        match a {
            Analysed::Photo(p) => photos.push(*p),
            Analysed::Unreadable(p, e) => unreadable.push((p, e)),
        }
    }
    if !unreadable.is_empty() {
        say(&format!(
            "illisibles : {} fichiers refusés par le décodeur",
            unreadable.len()
        ));
    }
    say(&format!("analyze: {} photos", photos.len()));

    // Everything failed to decode: refuse loudly, before any album file
    // exists. The file names are the whole diagnosis, so they are in the
    // message (names only, never full paths).
    if photos.is_empty() {
        let noms: Vec<String> = unreadable
            .iter()
            .take(12)
            .map(|(p, _)| {
                p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
            })
            .collect();
        anyhow::bail!(
            "aucune photo exploitable : les {} fichiers image du dossier sont \
             illisibles ou tronqués ({})",
            unreadable.len(),
            noms.join(", ")
        );
    }

    Ok(Releve {
        version: crate::releve::VERSION,
        racine: root,
        skipped_heic: scanned.skipped_heic,
        skipped_other: scanned.skipped_other,
        // The decoder's message stops here, on the progress line above:
        // nothing downstream reads it, and it would churn the fiches at
        // every bump of the image crate.
        illisibles: unreadable.into_iter().map(|(p, _)| p).collect(),
        photos,
        vignettes: true,
    })
}

/// The composing half: curation, chapters, layout, and the files that come
/// out of them. It touches no pixel until its last two steps, the thumbnail
/// index and the preview PDF, which draw the images the analysis worked on —
/// and a relevé read back from its fiches has none. It stops there, and says
/// so.
pub fn composer(releve: Releve, out: &Path, opts: BuildOptions) -> Result<BuildReport> {
    let say = &opts.progress;
    let root = releve.racine.clone();
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

    let vignettes = releve.vignettes;
    let photos_scanned = releve.photos_scannees();
    // An album composed without the photographs carries the relevé it came
    // from: its thumbnails do not exist, so the relevé is the only thing the
    // linter can measure. Written first, because the fiches are this
    // composition's input and not a by-product of it.
    if !vignettes {
        releve.ecrire(&out.join(crate::releve::FICHIER))?;
    }
    let unreadable = releve.illisibles;
    let photos = releve.photos;

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

    // Unreadable files enter the report first: the sorting view shows the
    // file name without a thumbnail (there is nothing to draw), which is
    // still infinitely better than pretending the file never existed.
    discards.extend(unreadable.iter().map(|p| model::Discard {
        src: rel(p),
        reason: "illisible".into(),
        kept: None,
        focal: model::default_focal(),
    }));

    // An explicit no comes first: a photo rejected in the user's
    // cataloguing app leaves before anything is compared, whatever else the
    // curation would have called it.
    let (photos, rejected) = pipeline::split_rejected(photos);
    if !rejected.is_empty() {
        say(&format!(
            "notes : {} photos rejetées dans votre logiciel photo, écartées",
            rejected.len()
        ));
    }
    discards.extend(rejected.iter().map(|p| model::Discard {
        src: rel(&p.path),
        reason: "rejetee".into(),
        kept: None,
        focal: focal_of(&p.path),
    }));

    let starred = photos.iter().filter(|p| p.meta.rating.is_some_and(|r| r >= 1)).count();
    if starred > 0 {
        say(&format!(
            "notes : {starred} photos étoilées, la curation en tient compte"
        ));
    }

    // A small folder gets a smaller curation. The statistical filters
    // (parasite, same-moment, scene windows) are tuned on hundreds of
    // photos; under the threshold they eat the album instead of trimming
    // it, so only the certain rejects apply and the spread budget follows
    // the photos that are actually there.
    let petit = photos.len() + rejected.len() < pipeline::PETIT_DOSSIER;
    if petit {
        say(&format!(
            "petit dossier ({} photos) : seuls les rejets certains s'appliquent",
            photos.len()
        ));
    }

    let (photos, junk) = if petit {
        (photos, Vec::new())
    } else {
        pipeline::split_junk(photos)
    };
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

    let (kept, dups) = pipeline::dedup(photos, petit);
    say(&format!("dedup: {} kept", kept.len()));
    discards.extend(dups.iter().map(|(lost, won)| model::Discard {
        src: rel(lost),
        reason: "doublon".into(),
        kept: Some(rel(won)),
        focal: focal_of(lost),
    }));

    // A small folder aims the album at the photos it has, not at the 48
    // spreads the caller asked for: three photos are one spread, not a
    // request the composer chases across five attempts.
    let spreads_target = if petit {
        (kept.len() * 10)
            .div_ceil(opts.densite.photos_per_spread_x10())
            .max(1)
    } else {
        opts.spreads.max(8)
    };
    // A chapter costs a dedicated opening page: too many chapters and the
    // album turns into a procession of solos.
    let max_chapters = (spreads_target / 4).clamp(4, 20);

    let chapters = pipeline::chapters(kept);
    let natural = chapters.len();
    let mut base = pipeline::merge_chapters(chapters, max_chapters);
    let twins: Vec<_> = base.iter_mut().flat_map(pipeline::thin_twins).collect();
    // Same-moment capping is a statistical filter: off on small folders,
    // where three frames of one minute may be the whole event.
    let moments: Vec<_> = if petit {
        Vec::new()
    } else {
        base.iter_mut().flat_map(pipeline::cap_moments).collect()
    };
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
    // On a small folder the correction loop is off with the other
    // statistical filters: chasing a spread target by trimming photos is
    // how three photos become one planche. One pass, every photo placed,
    // and the album is as long as it is.
    //
    // That last paragraph is why the app can propose several albums: the
    // pipeline above is what costs, chaptering and layout are arithmetic on
    // data already in memory. Composing three proposals costs three passes
    // of the cheap half and one pass of the expensive one.
    anyhow::ensure!(!cancelled(), "composition annulée");

    let compose_une = |densite: layout::Densite,
                       cible: usize|
     -> (model::Album, Vec<model::Discard>, usize) {
        let mut album = model::Album::new(&title, &root, opts.trim);
        album.cover = opts.cover.clone();
        album.densite = densite;
        album.reglages = opts.reglages.clone();
        let total_kept: usize = base.iter().map(|c| c.photos.len()).sum();
        let mut budget = if petit {
            total_kept
        } else {
            cible * densite.photos_per_spread_x10() / 10
        };
        let attempts = if petit { 1 } else { 5 };
        let mut photos_kept = 0;
        // Keep the attempt closest to the target: on fragmented sets the
        // spread count can refuse to follow the budget, and the last attempt
        // is then the worst one, not the best.
        let mut best: Option<(usize, Vec<model::Spread>, usize)> = None;
        for attempt in 0..attempts {
            let mut trial = base.clone();
            if !petit {
                let caps = pipeline::allocate_budget(&trial, budget);
                for (c, cap) in trial.iter_mut().zip(caps) {
                    pipeline::cap_chapter(c, cap);
                }
            }
            let kept = trial.iter().map(|c| c.photos.len()).sum();

            let mut composer = layout::Composer::avec_densite(&album, densite);
            // Captions are worked out for the run of chapters at once, not one
            // by one: whether a place is worth naming depends on the chapter
            // before it.
            let captions = chapter_captions(&trial);
            let spreads: Vec<model::Spread> = trial
                .iter()
                .zip(captions)
                .flat_map(|(c, caption)| composer.compose(c, caption, &root))
                .collect();

            let got = spreads.len();
            let off = got.abs_diff(cible) * 100 / cible.max(1);
            if best.as_ref().is_none_or(|(b, _, _)| off < *b) {
                best = Some((off, spreads, kept));
            }
            if off <= 6 || attempt == attempts - 1 || got == 0 {
                break;
            }
            // Aim the next budget at the target, damped so it cannot oscillate,
            // and never starved below two photos per requested spread: fewer
            // photos never means fewer spreads once chapters run on minimums.
            let aimed = budget * cible / got;
            budget = ((budget + aimed) / 2).max(cible * 2);
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
        }

        // The colophon page, on by default: the software is called Colophon
        // and did not print one. The facts travel in album.json, the page
        // itself is an ordinary spread at the end of the book, so it counts
        // in the pagination the suppliers sanction without a single special
        // case. The Envoi screen takes it away in one click.
        album.colophon = Some(crate::colophon::faits(
            &base,
            photos_kept,
            photos_scanned,
            chrono::Local::now().date_naive(),
        ));
        if let (true, Some(f)) = (opts.colophon, &album.colophon) {
            album.spreads.push(crate::colophon::spread(
                f,
                opts.trim,
                crate::printer::GRAMMAGE_DEFAUT,
                env!("CARGO_PKG_VERSION"),
            ));
        }

        // And the half-title at the other end, from the same facts: the
        // title, the days, the towns. Inserted after the pinned spreads have
        // found their places, so it opens the book whatever else was kept.
        if let (true, Some(f)) = (opts.garde, &album.colophon) {
            let g = crate::pdf::geometry(&album);
            let titre = crate::cover::titre_du_livre(&album).to_string();
            let garde = crate::garde::spread(&titre, f, crate::garde::place(&g));
            album.spreads.insert(0, garde);
        }

        // Photos that survived curation but not this proposal's own spread
        // budget. Per proposal, because a tighter book sets more of them
        // aside: the sorting view has to tell the truth about the album on
        // screen, not about the one that was not chosen.
        let shown: std::collections::HashSet<String> = album
            .spreads
            .iter()
            .flat_map(|s| s.slots.iter().map(|sl| sl.src.clone()))
            .collect();
        let hors_budget: Vec<model::Discard> = base
            .iter()
            .flat_map(|c| c.photos.iter())
            .filter_map(|photo| {
                let src = rel(&photo.path);
                (!shown.contains(&src)).then(|| model::Discard {
                    src,
                    reason: "hors_budget".into(),
                    kept: None,
                    focal: focal_of(&photo.path),
                })
            })
            .collect();

        (album, hors_budget, photos_kept)
    };

    // The proposal the caller asked for comes first and stays the default;
    // the others are alternatives the creation screen shows beside it. An
    // empty list is the old behaviour, one album, unchanged.
    let specs: Vec<VarianteSpec> = std::iter::once(VarianteSpec {
        id: "demandee".into(),
        nom: "Comme demandé".into(),
        about: String::new(),
        densite: opts.densite,
        spreads: spreads_target,
    })
    .chain(opts.variantes.iter().cloned())
    .collect();

    let mut composees: Vec<(VarianteSpec, model::Album, Vec<model::Discard>, usize)> =
        Vec::with_capacity(specs.len());
    for spec in specs {
        anyhow::ensure!(!cancelled(), "composition annulée");
        let (album, hors_budget, kept) = compose_une(spec.densite, spec.spreads);
        if !composees.is_empty() {
            say(&format!("variante {} : {} planches", spec.id, album.spreads.len()));
        }
        composees.push((spec, album, hors_budget, kept));
    }
    // The first proposal is the one asked for, and the album this build
    // opens. It is written under its own name too: the choice screen has to
    // be able to come back to it after showing another.
    let (album, hors_budget, photos_kept) = {
        let (_, a, h, k) = &composees[0];
        (a.clone(), h.clone(), *k)
    };

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

    // The album came out empty: every photo was set aside. Writing an
    // album.json and a PDF of zero pages here would present a failure as a
    // success, which is the one forbidden outcome. Refuse, with the counts
    // that explain it, and write nothing. Pinned spreads are user work and
    // count as content even without photos (a text page, say).
    if !album.spreads.iter().any(|s| !s.slots.is_empty()) && opts.pinned.is_empty() {
        let mut par_raison: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for d in &discards {
            *par_raison.entry(d.reason.as_str()).or_default() += 1;
        }
        let resume: Vec<String> =
            par_raison.iter().map(|(r, n)| format!("{n} {r}")).collect();
        anyhow::bail!(
            "aucune photo exploitable : les {photos_scanned} images du dossier \
             ont toutes été écartées ({}) ; aucun album n'a été écrit",
            resume.join(", ")
        );
    }
    // 5. album.json, plus the thumbnail index. Cache filenames hash the
    // absolute path and mtime, which no reader can recompute: without this
    // index an album folder is unreadable on another machine.
    //
    // Curation splits in two here: what every proposal set aside for the same
    // reason (junk, panoramas, definition, duplicates, twins), and what each
    // proposal's own spread budget left out. The sorting view has to tell the
    // truth about the album on screen, not about the one nobody chose.
    let commun = discards.clone();
    discards.extend(hors_budget);
    say(&format!("curation: {} photos set aside", discards.len()));

    // The proposals not chosen, written beside the default one. They cost one
    // JSON file each, the thumbnails being shared, and the alternative to
    // keeping them is recomposing the folder from scratch. The first save
    // takes them away: past a hand edit they stop being an offer and become a
    // stale copy of somebody's work.
    let mut variantes: Vec<VarianteResume> = Vec::new();
    for (i, (spec, autre, hors, kept)) in composees.iter().enumerate() {
        let mut curation = commun.clone();
        curation.extend(hors.iter().cloned());
        fs::write(
            out.join(format!("album.{}.json", spec.id)),
            serde_json::to_string_pretty(autre)?,
        )?;
        fs::write(
            out.join(format!("curation.{}.json", spec.id)),
            serde_json::to_string_pretty(&curation)?,
        )?;
        // The one asked for is the album itself, not an alternative beside it.
        if i > 0 {
            variantes.push(VarianteResume::de(spec, autre, *kept));
        }
    }

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

    // The pixels resume here, and only here. A relevé read back from its
    // fiches stops at this line: it has an album, its proposal, its curation
    // and its variants, and no image to draw them with.
    if !vignettes {
        say(&format!(
            "sans les photos : {} écrit à côté de l'album, ni vignettes, ni \
             thumbs.json, ni PDF, ni couverture",
            crate::releve::FICHIER
        ));
        return Ok(BuildReport {
            chapters: base.len(),
            album,
            album_json,
            album_pdf: None,
            photos_scanned,
            photos_kept,
            variantes,
        });
    }
    let cache = thumb::ThumbCache::new(out)?;
    write_thumb_index(&album, &discards, &root, &cache, out)?;

    // 6. render PDF from thumbnails (preview quality in P0)
    anyhow::ensure!(!cancelled(), "composition annulée");
    say("pdf: rendu des planches");
    let mut writer = pdf::PdfWriter::new(&album);
    for (i, spread) in album.spreads.iter().enumerate() {
        let assets: Vec<pdf::JpegAsset> = spread
            .slots
            .iter()
            .filter_map(|slot| {
                let src = root.join(&slot.src);
                let thumb_path = cache.path_for(&src);
                let data = fs::read(&thumb_path).ok()?;
                // A recomposition of an adjusted album comes through here:
                // the cached thumbnail stays untouched on disk, its copy is
                // adjusted on the way into the preview PDF.
                let (data, w, h) = match album.reglages.get(&slot.src) {
                    Some(r) => crate::reglage::regler_jpeg(&data, r, 92).ok()?,
                    None => {
                        let (w, h) = jpeg_dimensions(&data)?;
                        (data, w, h)
                    }
                };
                Some(pdf::JpegAsset { data, width: w, height: h, focal: slot.focal, zoom: slot.zoom })
            })
            .collect();
        // A spread that cannot be drawn is an error, not a page that
        // silently vanishes from the book. Nothing is saved on the way out.
        anyhow::ensure!(
            assets.len() == spread.slots.len(),
            "planche {} : vignette manquante, le PDF n'a pas été écrit ; \
             recomposez l'album",
            i + 1
        );
        writer.add_spread(spread, &assets)?;
    }
    let album_pdf = out.join("album.pdf");
    writer.save(&album_pdf)?;

    Ok(BuildReport {
        chapters: base.len(),
        album,
        album_json,
        album_pdf: Some(album_pdf),
        photos_scanned,
        photos_kept,
        variantes,
    })
}

/// The whole build, from a folder to `album.json` + `album.pdf`: the two
/// halves in a row, and nothing else. Every caller that has the photographs
/// at hand goes through here.
pub fn build_album(photos_dir: &Path, out: &Path, opts: BuildOptions) -> Result<BuildReport> {
    let releve = releve(photos_dir, out, &opts)?;
    composer(releve, out, opts)
}

/// The two proposals shown beside the one the creation screen asked for.
/// One question, three answers: a lighter book at the same length, and the
/// same trip in fewer pages. Both use pace values the linter is green on
/// (`Densite::offertes`), and neither touches a composer threshold.
pub fn variantes_offertes(densite: layout::Densite, spreads: usize) -> Vec<VarianteSpec> {
    let autre = layout::Densite::offertes()
        .iter()
        .copied()
        .find(|d| *d != densite)
        .unwrap_or(densite);
    vec![
        VarianteSpec {
            id: "autre-rythme".into(),
            nom: autre.nom().to_string(),
            about: autre.about().to_string(),
            densite: autre,
            spreads,
        },
        VarianteSpec {
            id: "resserree".into(),
            nom: "Plus court".into(),
            // Says what it does and not what it flatters: a third fewer
            // spreads means a third fewer photographs, and a cheaper book.
            about: "Un tiers de planches en moins, donc moins de photos retenues. \
                    Un livre qui se feuillette d'un trait, et qui coûte moins cher \
                    à imprimer."
                .into(),
            densite,
            // A third shorter, floored so a small album cannot collapse under
            // the eight spreads the composer works from.
            spreads: (spreads * 2 / 3).max(8),
        },
    ]
}

/// Delete the proposals nobody chose. Called at the first save: past a hand
/// edit they stop being an offer and become a stale copy of somebody's work.
/// Missing files are not an error, this runs on every save.
pub fn oublier_variantes(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut n = 0;
    for e in entries.flatten() {
        let nom = e.file_name().to_string_lossy().to_string();
        let variante = (nom.starts_with("album.") || nom.starts_with("curation."))
            && nom.ends_with(".json")
            && nom.matches('.').count() == 2
            && nom != "album.origin.json";
        if variante && fs::remove_file(e.path()).is_ok() {
            n += 1;
        }
    }
    n
}

/// Write `album.json` atomically: temp file then rename, so a crash halfway
/// never leaves a truncated album. The album is the user's work, not a cache.
pub fn write_album_json(dir: &Path, album: &model::Album) -> Result<PathBuf> {
    let target = dir.join("album.json");
    let tmp = dir.join("album.json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(album)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    // One step back, always: the previous version survives as .bak, next to
    // the file and as hand-repairable as it. A save that fails half-way
    // leaves the .bak of the version before, never a torn file. Best
    // effort: a missing or unreadable album.json must not block the save.
    if target.exists() {
        let _ = fs::copy(&target, dir.join("album.json.bak"));
    }
    fs::rename(&tmp, &target).with_context(|| format!("rename onto {}", target.display()))?;
    Ok(target)
}

/// What a migration did, so a bilan can say it instead of staying quiet.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// The schema the folder was written under.
    pub depuis: u32,
    pub slots: usize,
    /// Slots whose photo could not be resolved, and whose `focal` was
    /// therefore left as it stood. Approximate, and named as such.
    pub irresolus: usize,
}

/// Bring an album folder up to [`model::SCHEMA`], in place, and say what it
/// cost. `Ok(None)` when there was nothing to do.
///
/// **One hook, one write.** The migration rewrites `album.json`, so every
/// later reader — the audit, the print pass, the cover, the bench — sees the
/// current schema without knowing a migration exists. Wiring the conversion
/// into each reader instead would mean six places to keep in step, and the
/// first one forgotten would read a point of the image as a fraction of the
/// room without anything failing.
///
/// The aspect ratios come from the cached thumbnails: they are in the folder
/// by construction, `image_dimensions` reads only their header, and their
/// ratio is the original's to within a rounded pixel. A photo with no
/// thumbnail keeps its `focal` and is counted.
pub fn migrate_album_folder(dir: &Path) -> Result<Option<Migration>> {
    let json = dir.join("album.json");
    let mut album: model::Album = serde_json::from_str(
        &fs::read_to_string(&json).with_context(|| format!("read {}", json.display()))?,
    )
    .context("album.json illisible")?;
    if album.version >= model::SCHEMA {
        return Ok(None);
    }
    let depuis = album.version;

    // Un index de vignettes absent ou illisible ne fait pas échouer la
    // migration : il la rend intégralement irrésolue, et c'est le compteur
    // qui le dit. Refuser d'ouvrir un album parce qu'on n'a pas su le migrer
    // serait punir l'utilisateur d'un changement de schéma qui est le nôtre.
    let thumbs: std::collections::BTreeMap<String, String> =
        fs::read_to_string(dir.join("thumbs.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let mut ratios: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut ratio = |src: &str| -> Option<f64> {
        if let Some(r) = ratios.get(src) {
            return Some(*r);
        }
        let name = thumbs.get(src)?;
        let (w, h) = image::image_dimensions(dir.join(".cache").join("thumbs").join(name)).ok()?;
        if w == 0 || h == 0 {
            return None;
        }
        let r = f64::from(w) / f64::from(h);
        ratios.insert(src.to_string(), r);
        Some(r)
    };

    let g = crate::pdf::geometry(&album);
    let (mut slots, mut irresolus) = (0usize, 0usize);
    for spread in &mut album.spreads {
        let rects = crate::pdf::slots_for(&spread.template, spread.slots.len(), &g);
        for (i, slot) in spread.slots.iter_mut().enumerate() {
            slots += 1;
            let (Some(r), Some(rect)) = (ratio(&slot.src), rects.get(i)) else {
                irresolus += 1;
                continue;
            };
            slot.focal = model::point_from_room(rect.w / rect.h, r, slot.focal, slot.zoom);
        }
    }

    // La couverture est un second site : son rect ne vient pas de `slots_for`
    // mais de la géométrie de couverture. Le dos n'y entre pas — `photo_rect`
    // fait `trim.w + fond perdu extérieur` sur `trim.h + haut + bas` — donc
    // seuls les fonds perdus du profil font varier son ratio, de l'ordre du
    // pour cent. N'importe quel profil répond donc au pixel près, et ne pas
    // la migrer du tout serait l'erreur bien plus grosse.
    if let Some(profil) = crate::printer::PrinterProfile::tous().first() {
        let cg = crate::cover::geometry(&album, profil);
        let rect = crate::cover::photo_rect(&cg);
        if let Some(slot) = album.cover.as_mut().and_then(|c| c.photo.as_mut()) {
            slots += 1;
            match ratios.get(&slot.src).copied().or_else(|| {
                let name = thumbs.get(&slot.src)?;
                let (w, h) =
                    image::image_dimensions(dir.join(".cache").join("thumbs").join(name)).ok()?;
                (w > 0 && h > 0).then(|| f64::from(w) / f64::from(h))
            }) {
                Some(r) => {
                    slot.focal =
                        model::point_from_room(rect.w / rect.h, r, slot.focal, slot.zoom)
                }
                None => irresolus += 1,
            }
        }
    }

    // Estampiller même quand tout ne s'est pas résolu : ne pas le faire ferait
    // re-migrer au chargement suivant les slots déjà convertis, et une double
    // migration abîme ce qu'une simple réparait.
    album.version = model::SCHEMA;
    fs::write(&json, serde_json::to_string_pretty(&album)?)
        .with_context(|| format!("write {}", json.display()))?;
    Ok(Some(Migration { depuis, slots, irresolus }))
}

/// Re-render `album.pdf` from `album.json` alone, resolving every photo
/// through `thumbs.json`. No scan, no analysis: this is what the editor calls
/// after a change, and it works even when the original folder has moved.
pub fn render_album_pdf(dir: &Path) -> Result<PathBuf> {
    // Avant toute lecture : un dossier d'avant le schéma 2 porte des `focal`
    // qui ne veulent plus ce qu'ils disent, et le rendre sans migrer le
    // recadrerait en silence.
    migrate_album_folder(dir)?;
    let json = dir.join("album.json");
    let album: model::Album = serde_json::from_str(
        &fs::read_to_string(&json).with_context(|| format!("read {}", json.display()))?,
    )
    .context("album.json illisible")?;
    let thumbs: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&fs::read_to_string(dir.join("thumbs.json"))?)
            .context("thumbs.json illisible")?;
    // An album of zero pages is not a document, whatever wrote it: refuse
    // to render rather than hand back an empty PDF that looks like one.
    anyhow::ensure!(
        !album.spreads.is_empty(),
        "l'album n'a aucune planche : rien à rendre"
    );

    let mut writer = pdf::PdfWriter::new(&album);
    for (i, spread) in album.spreads.iter().enumerate() {
        let assets: Vec<pdf::JpegAsset> = spread
            .slots
            .iter()
            .filter_map(|slot| {
                let name = thumbs.get(&slot.src)?;
                let data = fs::read(dir.join(".cache").join("thumbs").join(name)).ok()?;
                // The faithful preview and the page turn both read this PDF:
                // an adjusted photo shows adjusted there too, the cache file
                // itself staying pixel-for-pixel what the analysis measured.
                let (data, w, h) = match album.reglages.get(&slot.src) {
                    Some(r) => crate::reglage::regler_jpeg(&data, r, 92).ok()?,
                    None => {
                        let (w, h) = jpeg_dimensions(&data)?;
                        (data, w, h)
                    }
                };
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

/// The dates of one chapter, read from the photos whose EXIF is trusted.
/// The others fell back to the file's mtime, which is a copy date, not a
/// shooting date: printing it once produced « 6 décembre – 14 juin 2026 »
/// on an album of 2024. A chapter with no trusted date shows none.
pub fn chapter_dates(c: &pipeline::Chapter) -> Option<String> {
    let mut dates = c
        .photos
        .iter()
        .filter(|p| p.meta.taken_reliable)
        .map(|p| p.meta.taken.date());
    let first = dates.next()?;
    let (s, e) = dates.fold((first, first), |(lo, hi), d| (lo.min(d), hi.max(d)));
    Some(if s == e {
        date_fr(s, true)
    } else {
        format!("{} \u{2013} {}", date_fr(s, false), date_fr(e, true))
    })
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
///
/// A chapter may also have nothing worth printing: no trusted date and no
/// agreed town. Its opening spread then carries no line at all, which is
/// the one honest option left, and the linter's missing-caption counter
/// keeps watching the album's first spread.
pub fn chapter_captions(chapters: &[pipeline::Chapter]) -> Vec<Option<String>> {
    let mut out = Vec::with_capacity(chapters.len());
    let mut previous: Option<&str> = None;
    for c in chapters {
        let dates = chapter_dates(c);
        let place = chapter_place(c);
        out.push(match (place, dates) {
            (Some(name), Some(dates)) if previous != Some(name) => {
                Some(format!("{name}, {dates}"))
            }
            (Some(name), None) if previous != Some(name) => Some(name.to_string()),
            (_, dates) => dates,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway photos folder and its (not yet created) output folder.
    fn dossier_test(name: &str) -> (PathBuf, PathBuf) {
        let base =
            std::env::temp_dir().join(format!("colophon-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let photos = base.join("photos");
        fs::create_dir_all(&photos).unwrap();
        (photos, base.join("out"))
    }

    /// A refused build writes no album file at all. The thumbnail cache may
    /// exist (it is a cache), the album must not.
    fn rien_d_ecrit(out: &Path) {
        for f in [
            "album.json",
            "album.origin.json",
            "album.pdf",
            "curation.json",
            "thumbs.json",
        ] {
            assert!(!out.join(f).exists(), "{f} ne devrait pas avoir été écrit");
        }
    }

    /// A tiny but valid JPEG: decodes fine, prints terribly. Striped so two
    /// of them never read as twins.
    fn petit_jpeg(path: &Path, seed: u32) {
        let img = image::RgbImage::from_fn(80, 80, |x, y| {
            if (x / (3 + seed) + y / (2 + seed)) % 2 == 0 {
                image::Rgb([250 - (seed * 40) as u8, (seed * 60) as u8, 30])
            } else {
                image::Rgb([10, 80, 220])
            }
        });
        img.save(path).unwrap();
    }

    /// La migration du schéma 1 vers le 2 : elle lit le ratio dans la
    /// vignette, prend le rect dans le gabarit, convertit, estampille — et
    /// **ne recommence pas**. La double migration est le seul vrai danger
    /// ici : elle abîmerait ce qu'une simple réparait, en silence.
    #[test]
    fn la_migration_convertit_une_fois_et_pas_deux() {
        let (_photos, out) = dossier_test("migration");
        let thumbs = out.join(".cache").join("thumbs");
        fs::create_dir_all(&thumbs).unwrap();
        // Deux vignettes de ratios francs et différents : 1,5 et 0,5.
        image::RgbImage::from_fn(120, 80, |_, _| image::Rgb([120, 40, 200]))
            .save(thumbs.join("ta.jpg"))
            .unwrap();
        image::RgbImage::from_fn(60, 120, |_, _| image::Rgb([20, 180, 90]))
            .save(thumbs.join("tb.jpg"))
            .unwrap();
        fs::write(
            out.join("thumbs.json"),
            r#"{"a.jpg":"ta.jpg","b.jpg":"tb.jpg"}"#,
        )
        .unwrap();
        let avant = [[0.2_f64, 0.8_f64], [0.9, 0.1]];
        // `root` reste un littéral : la migration ne résout aucune photo par
        // lui (les ratios viennent des vignettes), et y écrire `out.display()`
        // produisait sous Windows un `C:\Users\...` dont les antislashs sont
        // des échappements JSON invalides. Le test tombait là, et nulle part
        // ailleurs, sur le seul runner Windows.
        fs::write(
            out.join("album.json"),
            format!(
                r#"{{"version":1,"title":"t","root":".","trim_mm":{{"w":210.0,"h":210.0}},
                    "bleed_mm":3.0,"spreads":[{{"template":"duo","slots":[
                      {{"src":"a.jpg","focal":[{},{}]}},
                      {{"src":"b.jpg","focal":[{},{}]}}]}}]}}"#,
                avant[0][0], avant[0][1], avant[1][0], avant[1][1],
            ),
        )
        .unwrap();

        let m = migrate_album_folder(&out).unwrap().expect("il y avait à migrer");
        assert_eq!(m.depuis, 1);
        assert_eq!(m.slots, 2);
        assert_eq!(m.irresolus, 0, "les deux vignettes étaient là");

        let lu: model::Album =
            serde_json::from_str(&fs::read_to_string(out.join("album.json")).unwrap()).unwrap();
        assert_eq!(lu.version, model::SCHEMA);

        let g = crate::pdf::geometry(&lu);
        let rects = crate::pdf::slots_for("duo", 2, &g);
        for (i, ratio) in [120.0 / 80.0, 60.0 / 120.0].iter().enumerate() {
            let attendu = model::point_from_room(
                rects[i].w / rects[i].h,
                *ratio,
                avant[i],
                1.0,
            );
            assert!(
                (lu.spreads[0].slots[i].focal[0] - attendu[0]).abs() < 1e-12
                    && (lu.spreads[0].slots[i].focal[1] - attendu[1]).abs() < 1e-12,
                "slot {i} : {:?} attendu {attendu:?}", lu.spreads[0].slots[i].focal
            );
            assert!(
                (lu.spreads[0].slots[i].focal[1] - avant[i][1]).abs() > 1e-6,
                "slot {i} : le focal n'a pas bougé, la migration n'a rien fait"
            );
        }

        // Deuxième passage : plus rien à faire, et surtout rien de touché.
        assert!(migrate_album_folder(&out).unwrap().is_none());
        let relu: model::Album =
            serde_json::from_str(&fs::read_to_string(out.join("album.json")).unwrap()).unwrap();
        for i in 0..2 {
            assert_eq!(
                relu.spreads[0].slots[i].focal, lu.spreads[0].slots[i].focal,
                "slot {i} : migré deux fois"
            );
        }
    }

    /// Une photo sans vignette garde son focal, et se compte. Un album qui
    /// migre à moitié le dit ; il ne se tait pas, et il ne re-migre pas.
    #[test]
    fn une_photo_irresolue_est_comptee_et_lalbum_est_quand_meme_estampille() {
        let (_photos, out) = dossier_test("migration-trou");
        fs::create_dir_all(out.join(".cache").join("thumbs")).unwrap();
        fs::write(out.join("thumbs.json"), r#"{}"#).unwrap();
        // Littéral, pour la même raison que le test au-dessus.
        fs::write(
            out.join("album.json"),
            r#"{"version":1,"title":"t","root":".","trim_mm":{"w":210.0,"h":210.0},
               "bleed_mm":3.0,"spreads":[{"template":"duo","slots":[
                 {"src":"a.jpg","focal":[0.2,0.8]}]}]}"#,
        )
        .unwrap();

        let m = migrate_album_folder(&out).unwrap().expect("il y avait à migrer");
        assert_eq!((m.slots, m.irresolus), (1, 1));
        let lu: model::Album =
            serde_json::from_str(&fs::read_to_string(out.join("album.json")).unwrap()).unwrap();
        assert_eq!(lu.version, model::SCHEMA, "estampillé quand même");
        assert_eq!(lu.spreads[0].slots[0].focal, [0.2, 0.8], "gardé tel quel");
        assert!(migrate_album_folder(&out).unwrap().is_none(), "ne re-migre pas");
    }

    /// The 16/08 case, first form: an empty folder. The old behaviour was a
    /// 0-spread album.pdf and exit 0; the contract is a named error, a
    /// non-zero exit and no file.
    #[test]
    fn un_dossier_vide_ne_produit_aucun_fichier() {
        let (photos, out) = dossier_test("vide");
        let err = build_album(&photos, &out, BuildOptions::default()).err().expect("le build aurait dû refuser");
        let msg = format!("{err:#}");
        assert!(msg.contains("aucune photo exploitable"), "{msg}");
        rien_d_ecrit(&out);
    }

    /// Second form: files with photo extensions the decoder refuses. The
    /// error names the files, because the names are the whole diagnosis.
    #[test]
    fn un_dossier_de_fichiers_corrompus_est_refuse_et_nomme() {
        let (photos, out) = dossier_test("corrompu");
        fs::write(photos.join("cassee-1.jpg"), b"pas un jpeg du tout").unwrap();
        fs::write(photos.join("cassee-2.jpg"), &[0xFF, 0xD8, 0xFF, 0x00]).unwrap();
        let err = build_album(&photos, &out, BuildOptions::default()).err().expect("le build aurait dû refuser");
        let msg = format!("{err:#}");
        assert!(msg.contains("aucune photo exploitable"), "{msg}");
        assert!(msg.contains("cassee-1.jpg"), "les noms font le diagnostic : {msg}");
        rien_d_ecrit(&out);
    }

    /// Third form: a mixed folder whose only readable photo cannot print
    /// (80 px). Everything is set aside, so the build refuses with the
    /// counts, instead of writing an album of zero pages.
    #[test]
    fn un_dossier_mixte_sans_photo_exploitable_est_refuse() {
        let (photos, out) = dossier_test("mixte");
        fs::write(photos.join("cassee.jpg"), b"tronquee").unwrap();
        petit_jpeg(&photos.join("minuscule.jpg"), 1);
        let err = build_album(&photos, &out, BuildOptions::default()).err().expect("le build aurait dû refuser");
        let msg = format!("{err:#}");
        assert!(msg.contains("écartées"), "{msg}");
        assert!(msg.contains("illisible"), "le compte des illisibles manque : {msg}");
        rien_d_ecrit(&out);
    }

    /// A printable JPEG: 2000 px holds even a full page of 21 cm at 260 ppi,
    /// so every template is open to it, and genuinely distinct per seed. The
    /// perceptual hashes see an image as a tiny grid, so the picture IS a
    /// tiny grid: an 8 × 8 board of pseudo-random colour cells. Two boards
    /// from two seeds land ~32 dHash bits apart, beyond every duplicate
    /// threshold including the composer's own 24-bit rule. Anything with
    /// repeated structure (stripes, bands) collapses to the same hash.
    /// Files are stamped an hour apart so they are not one burst.
    fn jpeg_imprimable(path: &Path, seed: u32) {
        let mut rng = seed.wrapping_mul(2654435761).wrapping_add(97);
        let mut cells = [[0u8; 3]; 64];
        for c in cells.iter_mut() {
            for ch in c.iter_mut() {
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                *ch = (rng >> 24) as u8;
            }
        }
        let img = image::RgbImage::from_fn(2000, 2000, |x, y| {
            image::Rgb(cells[((y / 250).min(7) * 8 + (x / 250).min(7)) as usize])
        });
        img.save(path).unwrap();
        let f = fs::File::options().write(true).open(path).unwrap();
        let t = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(1_700_000_000 + u64::from(seed) * 3600);
        f.set_modified(t).unwrap();
    }

    /// Chantier 6, the 16/08 case: three photos used to lose two to the
    /// « parasite » filter and make a one-spread album. A small folder now
    /// keeps only the certain rejects: the three photos are all in the book.
    #[test]
    fn trois_photos_font_un_petit_album_complet() {
        let (photos, out) = dossier_test("trois");
        for i in 0..3 {
            jpeg_imprimable(&photos.join(format!("photo-{i}.jpg")), i);
        }
        let report = build_album(&photos, &out, BuildOptions::default())
            .expect("trois photos font un album, pas un refus");
        let slots: usize =
            report.album.spreads.iter().map(|s| s.slots.len()).sum();
        assert_eq!(slots, 3, "les trois photos sont dans l'album");
        // Les planches de photographies : la garde et le colophon sont deux
        // pages de plus, écrites par la machine sur le livre lui-même.
        let planches = planches_photo(&report.album);
        assert!((1..=3).contains(&planches), "{planches} planches pour 3 photos");
        assert!(out.join("album.pdf").exists());
    }

    /// Combien de planches portent des photographies, la garde et le
    /// colophon mis à part.
    fn planches_photo(album: &model::Album) -> usize {
        album.spreads.iter().filter(|s| !s.slots.is_empty()).count()
    }

    /// The three promises of the adjustments' storage, on a real build.
    /// A recomposition carries the table (dropping the field from
    /// `BuildOptions` makes this fall); the preview render applies it
    /// (neutralising the application in `render_album_pdf` makes this fall);
    /// and two renders of the same adjusted album are byte-identical under
    /// `SOURCE_DATE_EPOCH`, exactly like the unadjusted ones.
    #[test]
    fn la_recomposition_garde_les_reglages_et_lapercu_les_applique() {
        let (photos, out) = dossier_test("reglages");
        for i in 0..3 {
            jpeg_imprimable(&photos.join(format!("photo-{i}.jpg")), i);
        }
        let report = build_album(&photos, &out, BuildOptions::default())
            .expect("trois photos font un album");
        let src = report
            .album
            .spreads
            .iter()
            .flat_map(|s| s.slots.iter())
            .next()
            .expect("au moins une photo")
            .src
            .clone();

        // One clock for every render below: the diffs are about pixels.
        std::env::set_var("SOURCE_DATE_EPOCH", "1700000000");
        let nu = {
            render_album_pdf(&out).unwrap();
            fs::read(out.join("album.pdf")).unwrap()
        };

        // The recomposition path: same folder, the réglage in the options,
        // exactly what `recompose_album` passes.
        let mut opts = BuildOptions::default();
        opts.reglages.insert(
            src.clone(),
            model::Reglage { expo: 1.0, contraste: 0.0, nb: false },
        );
        let repris = build_album(&photos, &out, opts).expect("recomposition");
        assert_eq!(
            repris.album.reglages.get(&src),
            Some(&model::Reglage { expo: 1.0, contraste: 0.0, nb: false }),
            "la table survit à la recomposition"
        );

        let regle = {
            render_album_pdf(&out).unwrap();
            fs::read(out.join("album.pdf")).unwrap()
        };
        assert_ne!(nu, regle, "le réglage doit atteindre l'aperçu rendu");

        let regle_bis = {
            render_album_pdf(&out).unwrap();
            fs::read(out.join("album.pdf")).unwrap()
        };
        assert_eq!(regle, regle_bis, "deux rendus du même album réglé, à l'octet");

        // Take the réglage away and the render is today's again: the
        // identity is the default, not a near miss.
        let mut album = repris.album.clone();
        album.reglages.clear();
        write_album_json(&out, &album).unwrap();
        render_album_pdf(&out).unwrap();
        assert_eq!(nu, fs::read(out.join("album.pdf")).unwrap());
        std::env::remove_var("SOURCE_DATE_EPOCH");
    }

    /// Ten photos: the album is sized on the folder, not on the 48 spreads
    /// asked by default. Every photo lands.
    #[test]
    fn dix_photos_font_un_album_proportionne() {
        let (photos, out) = dossier_test("dix");
        for i in 0..10 {
            jpeg_imprimable(&photos.join(format!("photo-{i}.jpg")), i);
        }
        let report = build_album(&photos, &out, BuildOptions::default())
            .expect("dix photos font un album");
        let slots: usize =
            report.album.spreads.iter().map(|s| s.slots.len()).sum();
        assert_eq!(slots, 10, "les dix photos sont dans l'album");
        let planches = planches_photo(&report.album);
        assert!((2..=6).contains(&planches), "{planches} planches pour 10 photos");
        // Les deux pages de la machine encadrent le livre, dans cet ordre.
        assert_eq!(report.album.spreads[0].template, crate::garde::TEMPLATE);
        assert_eq!(
            report.album.spreads.last().unwrap().template,
            crate::colophon::TEMPLATE
        );
    }

    /// The threshold itself, tested through the refusal messages so it costs
    /// nothing: EXIF-less tiny photos die of « definition » below 25 (the
    /// parasite filter is off) and of « parasite » at 25 (it is back on).
    #[test]
    fn la_bascule_du_petit_dossier_est_a_25_photos() {
        let (photos, out) = dossier_test("bascule24");
        for i in 0..24 {
            petit_jpeg(&photos.join(format!("p-{i}.jpg")), i);
        }
        let err = build_album(&photos, &out, BuildOptions::default())
            .err()
            .expect("tout est trop petit pour imprimer");
        let msg = format!("{err:#}");
        assert!(msg.contains("definition"), "{msg}");
        assert!(!msg.contains("parasite"), "à 24, le filtre parasite est coupé : {msg}");

        let (photos, out) = dossier_test("bascule25");
        for i in 0..25 {
            petit_jpeg(&photos.join(format!("p-{i}.jpg")), i);
        }
        let err = build_album(&photos, &out, BuildOptions::default())
            .err()
            .expect("tout est écarté");
        let msg = format!("{err:#}");
        assert!(msg.contains("parasite"), "à 25, le filtre parasite revient : {msg}");
    }

    /// Every save keeps one step back: album.json.bak carries the previous
    /// version, the atomic rename still rules the file itself, and the very
    /// first write leaves no .bak because there is nothing to keep.
    #[test]
    fn each_save_keeps_the_previous_version_as_bak() {
        let dir = std::env::temp_dir().join(format!("colophon-bak-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut album = crate::model::Album::new(
            "v1",
            Path::new("/photos"),
            crate::model::Size { w: 210.0, h: 210.0 },
        );
        write_album_json(&dir, &album).unwrap();
        assert!(!dir.join("album.json.bak").exists(), "premier enregistrement, rien à garder");

        album.title = "v2".into();
        write_album_json(&dir, &album).unwrap();
        let bak: crate::model::Album =
            serde_json::from_str(&fs::read_to_string(dir.join("album.json.bak")).unwrap())
                .unwrap();
        let cur: crate::model::Album =
            serde_json::from_str(&fs::read_to_string(dir.join("album.json")).unwrap()).unwrap();
        assert_eq!(bak.title, "v1");
        assert_eq!(cur.title, "v2");

        let _ = fs::remove_dir_all(&dir);
    }
}
