//! Desktop shell. The window is a thin viewer over `album.json`: the engine
//! stays in `colophon-core`, the app only opens albums and serves the
//! thumbnails the book view needs.

use colophon_core::model::Album;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};

/// Slot source to cached thumbnail filename, as written in thumbs.json.
type ThumbIndex = BTreeMap<String, String>;

/// The album currently open, with everything needed to resolve its photos.
struct Opened {
    /// Directory holding album.json, thumbs.json and `.cache/thumbs`.
    dir: PathBuf,
    thumbs: ThumbIndex,
}

#[derive(Default)]
struct AppState {
    open: Mutex<Option<Opened>>,
    /// Raised by the Annuler buttons; the running build or export polls it.
    cancel_build: Arc<AtomicBool>,
    cancel_export: Arc<AtomicBool>,
}

#[derive(Serialize)]
struct OpenedAlbum {
    album: Album,
    dir: String,
    /// False when the original photo folder has moved: the preview still works
    /// off the thumbnail cache, a full-resolution export would not.
    root_present: bool,
    /// Every source with a cached thumbnail: shown photos plus discarded
    /// ones. The sorting view derives "removed by hand" from what is here
    /// but neither in the album nor in curation.json.
    thumb_srcs: Vec<String>,
}

/// What the composition just did, told in numbers: the front shows it once,
/// when the build ends, before the book opens. The discard detail comes from
/// curation.json, already served; these three counts exist nowhere else.
#[derive(Serialize)]
struct BuildBilan {
    photos_scanned: usize,
    photos_kept: usize,
    chapters: usize,
}

#[derive(Serialize)]
struct BuiltAlbum {
    opened: OpenedAlbum,
    bilan: BuildBilan,
    /// The proposals composed beside the one that is open, each on disk.
    /// The creation screen shows them side by side; picking one swaps it in.
    variantes: Vec<colophon_core::build::VarianteResume>,
}

/// Read an album folder (or its album.json) into the album and its thumb index.
/// Kept free of Tauri types so it can be tested on a real album folder.
fn load_album(path: &Path) -> Result<(PathBuf, Album, ThumbIndex), String> {
    let (dir, json) = if path.is_dir() {
        (path.to_path_buf(), path.join("album.json"))
    } else {
        (
            path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            path.to_path_buf(),
        )
    };

    let text = std::fs::read_to_string(&json)
        .map_err(|e| format!("lecture de {} : {e}", json.display()))?;
    let album: Album =
        serde_json::from_str(&text).map_err(|e| format!("album.json illisible : {e}"))?;

    let thumbs: ThumbIndex = std::fs::read_to_string(dir.join("thumbs.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if thumbs.is_empty() {
        return Err(format!(
            "thumbs.json absent ou vide dans {}. Régénérez l'album avec la commande colophon.",
            dir.display()
        ));
    }
    Ok((dir, album, thumbs))
}

fn read_thumb(dir: &Path, thumbs: &ThumbIndex, src: &str) -> Result<Vec<u8>, String> {
    let name = thumbs
        .get(src)
        .ok_or_else(|| format!("{src} absent de thumbs.json"))?;
    let file = dir.join(".cache").join("thumbs").join(name);
    std::fs::read(&file).map_err(|e| format!("vignette {} illisible : {e}", file.display()))
}

/// Open an album from either its folder or its `album.json` directly.
#[tauri::command]
fn open_album(path: String, state: State<'_, AppState>) -> Result<OpenedAlbum, String> {
    let (dir, album, thumbs) = load_album(Path::new(&path))?;
    let root_present = Path::new(&album.root).is_dir();
    let thumb_srcs = thumbs.keys().cloned().collect();
    *state.open.lock().unwrap() = Some(Opened { dir: dir.clone(), thumbs });
    Ok(OpenedAlbum {
        album,
        dir: dir.to_string_lossy().to_string(),
        root_present,
        thumb_srcs,
    })
}

/// The face-anchored focal point of a photo, recomputed on its cached
/// thumbnail. The crop editor's double-click recentres on it: the detector
/// value is not persisted anywhere, and one thumbnail pass costs nothing.
#[tauri::command]
fn detected_focal(src: String, state: State<'_, AppState>) -> Result<[f64; 2], String> {
    let data = {
        let guard = state.open.lock().unwrap();
        let opened = guard.as_ref().ok_or("aucun album ouvert")?;
        read_thumb(&opened.dir, &opened.thumbs, &src)?
    };
    let img = colophon_core::image::load_from_memory(&data)
        .map_err(|e| format!("vignette illisible : {e}"))?;
    let mut det = colophon_core::face::new_detector();
    let faces = colophon_core::face::face_boxes(det.as_mut(), &img);
    Ok(colophon_core::face::focal_from_boxes(&faces).unwrap_or([0.5, 0.42]))
}

/// The EXIF capture date of a photo, formatted the way chapter captions
/// are. The caption editor proposes it, never imposes it. None when the
/// date is unreliable (screenshots, forwards) or the original is gone.
#[tauri::command]
fn caption_suggestion(src: String, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let dir = {
        let guard = state.open.lock().unwrap();
        guard.as_ref().ok_or("aucun album ouvert")?.dir.clone()
    };
    let text = std::fs::read_to_string(dir.join("album.json"))
        .map_err(|e| format!("lecture de album.json : {e}"))?;
    let album: Album =
        serde_json::from_str(&text).map_err(|e| format!("album.json illisible : {e}"))?;
    let path = PathBuf::from(&album.root).join(&src);
    if !path.is_file() {
        return Ok(None);
    }
    let meta = colophon_core::meta::read(&path);
    Ok(meta
        .taken_reliable
        .then(|| colophon_core::build::date_fr(meta.taken.date(), true)))
}

/// The photos curation set aside, with reasons, from curation.json.
/// Empty when the album predates the export: the sorting view just shows
/// the hand-removed photos then.
#[tauri::command]
fn curation(state: State<'_, AppState>) -> Result<Vec<colophon_core::model::Discard>, String> {
    let guard = state.open.lock().unwrap();
    let opened = guard.as_ref().ok_or("aucun album ouvert")?;
    let path = opened.dir.join("curation.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("lecture de curation.json : {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("curation.json illisible : {e}"))
}

/// Raw JPEG bytes of one slot's thumbnail. Returned as a binary IPC response,
/// so the front end can turn it into a blob URL without a base64 round trip.
#[tauri::command]
fn thumb(src: String, state: State<'_, AppState>) -> Result<tauri::ipc::Response, String> {
    let guard = state.open.lock().unwrap();
    let opened = guard.as_ref().ok_or("aucun album ouvert")?;
    let data = read_thumb(&opened.dir, &opened.thumbs, &src)?;
    Ok(tauri::ipc::Response::new(data))
}

/// Persist the edited album over album.json, atomically: the album is the
/// user's work, a crash mid-write must not cost it.
#[tauri::command]
fn save_album(album: Album, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.open.lock().unwrap();
    let opened = guard.as_ref().ok_or("aucun album ouvert")?;
    colophon_core::write_album_json(&opened.dir, &album)
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))?;
    // The proposals nobody chose go here: past a hand edit they stop being an
    // offer and become a stale copy of somebody's work.
    colophon_core::build::oublier_variantes(&opened.dir);
    Ok(())
}

#[derive(Serialize)]
struct FormatPreset {
    name: String,
    w: f64,
    h: f64,
    about: String,
}

/// The page format presets, for the creation screen's picker.
#[tauri::command]
fn list_formats() -> Vec<FormatPreset> {
    colophon_core::format::FORMATS
        .iter()
        .map(|(name, w, h, about)| FormatPreset {
            name: name.to_string(),
            w: *w,
            h: *h,
            about: about.to_string(),
        })
        .collect()
}

/// One album folder per source folder, keyed by its absolute path so two
/// folders sharing a name never collide, inside the app's own data dir.
fn album_out_dir(app: &tauri::AppHandle, photos: &Path) -> Result<PathBuf, String> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    photos.hash(&mut h);
    let name = photos
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "album".into());
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("dossier de données introuvable : {e}"))?
        .join("albums");
    Ok(base.join(format!("{name}-{:08x}", h.finish() as u32)))
}

/// Build an album from a folder of photos, then open it. Progress lines
/// stream to the front as `build:progress` events; the build itself runs on
/// a blocking thread, it takes seconds. Rebuilding the same folder reuses
/// its thumbnail cache, so a second pass is fast.
#[tauri::command]
async fn build_album_from_folder(
    photos_dir: String,
    format: String,
    spreads: usize,
    densite: String,
    title: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<BuiltAlbum, String> {
    let trim = colophon_core::format::parse(&format).map_err(|e| e.to_string())?;
    let densite = colophon_core::layout::Densite::par_id(&densite)
        .ok_or_else(|| format!("densité inconnue : {densite}"))?;
    let photos = PathBuf::from(&photos_dir);
    let out = album_out_dir(&app, &photos)?;
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());

    let emitter = app.clone();
    let build_out = out.clone();
    state.cancel_build.store(false, Ordering::Relaxed);
    let flag = state.cancel_build.clone();
    let report = tauri::async_runtime::spawn_blocking(move || {
        colophon_core::build_album(
            &photos,
            &build_out,
            colophon_core::BuildOptions {
                title,
                spreads: spreads.clamp(8, 200),
                trim,
                progress: Box::new(move |line| {
                    colophon_core::log::line(line);
                    let _ = emitter.emit("build:progress", line);
                }),
                cancel: Box::new(move || flag.load(Ordering::Relaxed)),
                densite,
                // One question, three answers. The analysis is what costs;
                // composing two more proposals from the same photos is
                // arithmetic, and it turns a wait into a choice.
                variantes: colophon_core::build::variantes_offertes(densite, spreads.clamp(8, 200)),
                ..Default::default()
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| {
        colophon_core::log::line(&format!("composition en échec : {e:#}"));
        format!("{e:#}")
    })?;

    let opened = open_album(out.to_string_lossy().to_string(), state)?;
    Ok(BuiltAlbum {
        opened,
        bilan: BuildBilan {
            photos_scanned: report.photos_scanned,
            photos_kept: report.photos_kept,
            chapters: report.chapters,
        },
        variantes: report.variantes,
    })
}

/// Swap in one of the proposals composed beside the album, by its id. The
/// file becomes `album.json` and its own curation becomes `curation.json`;
/// the proposal being replaced stays on disk under its own name, so the
/// choice is reversible until the first save.
///
/// The id is a bare word from a `VarianteSpec`, and it is checked as one:
/// this joins a file name onto the open album's folder.
#[tauri::command]
fn choose_variante(id: String, state: State<'_, AppState>) -> Result<OpenedAlbum, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("identifiant de variante invalide : {id}"));
    }
    let dir = {
        let guard = state.open.lock().unwrap();
        guard.as_ref().ok_or("aucun album ouvert")?.dir.clone()
    };
    let album = dir.join(format!("album.{id}.json"));
    if !album.is_file() {
        return Err(format!(
            "cette proposition n'est plus sur le disque : elle a été effacée au premier enregistrement"
        ));
    }
    let text = std::fs::read_to_string(&album).map_err(|e| format!("lecture : {e}"))?;
    let parsed: Album =
        serde_json::from_str(&text).map_err(|e| format!("proposition illisible : {e}"))?;
    colophon_core::write_album_json(&dir, &parsed).map_err(|e| format!("{e:#}"))?;
    // Curation follows the album: a tighter proposal sets more photos aside,
    // and the sorting view must describe the book on screen.
    let curation = dir.join(format!("curation.{id}.json"));
    if curation.is_file() {
        std::fs::copy(&curation, dir.join("curation.json"))
            .map_err(|e| format!("curation : {e}"))?;
    }
    colophon_core::log::line(&format!("proposition {id} retenue"));
    open_album(dir.to_string_lossy().to_string(), state)
}

/// Recompose the open album from its photo folder, preserving every spread
/// edited by hand or locked. Their photos are withdrawn from the pipeline
/// and the spreads re-inserted at their place in time: recomposing is safe.
#[tauri::command]
async fn recompose_album(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<OpenedAlbum, String> {
    let dir = {
        let guard = state.open.lock().unwrap();
        guard.as_ref().ok_or("aucun album ouvert")?.dir.clone()
    };
    let json = dir.join("album.json");
    let text = std::fs::read_to_string(&json)
        .map_err(|e| format!("lecture de {} : {e}", json.display()))?;
    let album: Album =
        serde_json::from_str(&text).map_err(|e| format!("album.json illisible : {e}"))?;
    let root = PathBuf::from(&album.root);
    if !root.is_dir() {
        return Err(format!(
            "dossier de photos introuvable ({}) : impossible de recomposer",
            root.display()
        ));
    }

    // Pinned spreads keep their place through a capture-time anchor. A
    // photo-less spread (texte, vide) inherits the time of the spread
    // before it, so it stays glued to its chapter.
    let mut pinned = Vec::new();
    let mut last: Option<colophon_core::chrono::NaiveDateTime> = None;
    for spread in &album.spreads {
        let t = spread
            .slots
            .first()
            .map(|sl| colophon_core::meta::read(&root.join(&sl.src)).taken)
            .or(last);
        if spread.pinned() {
            pinned.push((spread.clone(), t));
        }
        last = t;
    }
    let target = album.spreads.len().saturating_sub(pinned.len()).max(4);

    state.cancel_build.store(false, Ordering::Relaxed);
    let flag = state.cancel_build.clone();
    let emitter = app.clone();
    let build_out = dir.clone();
    let opts = colophon_core::BuildOptions {
        title: Some(album.title.clone()),
        spreads: target,
        trim: album.trim_mm,
        progress: Box::new(move |line| {
            colophon_core::log::line(line);
            let _ = emitter.emit("build:progress", line);
        }),
        cancel: Box::new(move || flag.load(Ordering::Relaxed)),
        pinned,
        cover: album.cover.clone(),
        // The pace the album was built at, read back from the file: a
        // recomposition keeps it rather than quietly reverting to the
        // default one.
        densite: album.densite,
        // Same reasoning for the colophon page: somebody who took it away
        // must not have to take it away again after every recomposition.
        colophon: album
            .spreads
            .iter()
            .any(|s| s.template == colophon_core::colophon::TEMPLATE),
        // A recomposition is not a choice of album: the user already made
        // that one, and a second offer would throw away every hand edit that
        // the pinned spreads were kept for.
        variantes: Vec::new(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        colophon_core::build_album(&root, &build_out, opts)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| {
        colophon_core::log::line(&format!("recomposition en échec : {e:#}"));
        format!("{e:#}")
    })?;

    open_album(dir.to_string_lossy().to_string(), state)
}

/// Abandon the composition in flight. The build polls the flag between
/// photos and stages, and a cancelled build writes no album.
#[tauri::command]
fn cancel_build(state: State<'_, AppState>) {
    state.cancel_build.store(true, Ordering::Relaxed);
}

/// Abandon the print render in flight. The atomic temp + rename in core
/// means no half-written PDF can survive this.
#[tauri::command]
fn cancel_export(state: State<'_, AppState>) {
    state.cancel_export.store(true, Ordering::Relaxed);
}

/// Re-render album.pdf from the saved album.json, off the main thread: fifty
/// spreads of JPEG passthrough take a second or two.
#[tauri::command]
async fn render_pdf(state: State<'_, AppState>) -> Result<String, String> {
    let dir = {
        let guard = state.open.lock().unwrap();
        guard.as_ref().ok_or("aucun album ouvert")?.dir.clone()
    };
    tauri::async_runtime::spawn_blocking(move || colophon_core::render_album_pdf(&dir))
        .await
        .map_err(|e| e.to_string())?
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("{e:#}"))
}

/// Render the print-resolution PDF straight to where the user chose to keep
/// it. Reopens every original at 300 dpi, so this takes minutes, not
/// seconds: progress goes out as `export:progress` events (`render: i/n`).
///
/// The cover follows the profile, never a habit: a supplier who wants two
/// files gets the flat cover sheet written beside the interior, one who
/// builds its own gets nothing extra. Both paths come back so the window can
/// name what it wrote.
#[tauri::command]
async fn export_pdf(
    app: tauri::AppHandle,
    dest: String,
    profil: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let dir = {
        let guard = state.open.lock().unwrap();
        guard.as_ref().ok_or("aucun album ouvert")?.dir.clone()
    };
    let profil = printer_profile(&profil)?;
    state.cancel_export.store(false, Ordering::Relaxed);
    let flag = state.cancel_export.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<String>, String> {
        let interior = Path::new(&dest);
        colophon_core::log::line(&format!("export 300 dpi, profil {}", profil.id));
        colophon_core::render_print_pdf(
            &dir,
            profil,
            interior,
            &|line| {
                let _ = app.emit("export:progress", line);
            },
            &move || flag.load(Ordering::Relaxed),
        )
        .map_err(|e| {
            colophon_core::log::line(&format!("export en échec : {e:#}"));
            format!("{e:#}")
        })?;
        let mut written = vec![dest.clone()];

        if profil.fichiers == colophon_core::printer::Fichiers::Deux {
            let _ = app.emit("export:progress", "cover: couverture");
            let stem = interior
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "album".into());
            let cover = interior.with_file_name(format!("{stem}-couverture.pdf"));
            colophon_core::cover::render_cover_pdf(&dir, profil, &cover)
                .map_err(|e| {
                    colophon_core::log::line(&format!("couverture en échec : {e:#}"));
                    format!("couverture : {e:#}")
                })?;
            written.push(cover.to_string_lossy().to_string());
        }
        colophon_core::log::line("export terminé");
        Ok(written)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The composition paces the creation screen offers, each with the sentence
/// that describes it. From the engine, like the formats beside them.
#[derive(Serialize)]
struct DensitePreset {
    id: &'static str,
    nom: &'static str,
    about: &'static str,
    /// Photos per spread on average, for the estimate the screen shows.
    photos_par_planche: f64,
}

#[tauri::command]
fn list_densities() -> Vec<DensitePreset> {
    colophon_core::layout::Densite::offertes()
        .iter()
        .copied()
        .map(|d| DensitePreset {
            id: d.id(),
            nom: d.nom(),
            about: d.about(),
            photos_par_planche: d.photos_per_spread_x10() as f64 / 10.0,
        })
        .collect()
}

/// The printer profiles, as data, for the destination screen. Straight from
/// the engine: the window never carries a second copy of a supplier's specs.
#[tauri::command]
fn list_printers() -> &'static [colophon_core::printer::PrinterProfile] {
    colophon_core::printer::PrinterProfile::tous()
}

fn printer_profile(
    id: &str,
) -> Result<&'static colophon_core::printer::PrinterProfile, String> {
    colophon_core::printer::PrinterProfile::par_id(id)
        .ok_or_else(|| format!("profil imprimeur inconnu : {id}"))
}

/// What the report panel cannot know by itself: version, platform, the
/// scrubbed log tail and the audit counters of the open album. Everything is
/// gathered here and shown in full before the user sends anything; nothing
/// leaves the machine from this command.
#[derive(Serialize)]
struct ReportData {
    version: String,
    os: String,
    /// Last log lines, paths already reduced to file names at write time.
    log: String,
    /// None without an album, or when the audit itself fails: the report
    /// says so instead of blocking the channel.
    audit: Option<colophon_core::audit::AuditReport>,
}

/// Gather the raw material of a problem report. The audit reopens every
/// original to measure resolution, seconds on a big album: off the main
/// thread, like the renders.
#[tauri::command]
async fn report_data(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ReportData, String> {
    let dir = state.open.lock().unwrap().as_ref().map(|o| o.dir.clone());
    let audit = match dir {
        Some(d) => tauri::async_runtime::spawn_blocking(move || {
            colophon_core::audit::audit(&d)
                .map_err(|e| colophon_core::log::line(&format!("audit du rapport en échec : {e:#}")))
                .ok()
        })
        .await
        .map_err(|e| e.to_string())?,
        None => None,
    };
    Ok(ReportData {
        version: app.package_info().version.to_string(),
        os: format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH),
        log: colophon_core::log::extrait(30),
        audit,
    })
}

/// Open the pre-filled issue form in the user's browser. The one URL this
/// can ever open is the repo's issue page: the report channel must not be
/// able to become a generic link-opener.
#[tauri::command]
fn open_report_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://github.com/alexis-morain/colophon/issues/new") {
        return Err(format!("URL hors du dépôt : {url}"));
    }
    #[cfg(target_os = "macos")]
    let run = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(target_os = "windows")]
    let run = std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let run = std::process::Command::new("xdg-open").arg(&url).spawn();
    run.map(|_| ())
        .map_err(|e| format!("ouverture du navigateur : {e}"))
}

/// The colophon page, built from the facts the album carries. `Ok(None)` on
/// an album composed before the page existed: it holds no facts, and the
/// Envoi screen offers nothing rather than inventing a page.
///
/// The version printed is the app's own, not the engine crate's: what the
/// reader of a book wants is the name on the About screen.
#[tauri::command]
fn colophon_spread(
    album: Album,
    app: tauri::AppHandle,
) -> Result<Option<colophon_core::model::Spread>, String> {
    Ok(album.colophon.as_ref().map(|f| {
        colophon_core::colophon::spread(
            f,
            album.trim_mm,
            colophon_core::printer::GRAMMAGE_DEFAUT,
            &app.package_info().version.to_string(),
        )
    }))
}

/// The composer's own version of one spread, for « rendre à l'automatique ».
/// The lock has a way in and needed a way out: this is it. `Ok(None)` means
/// the spread was inserted by hand, nothing automatic ever proposed it; an
/// `Err` means the album predates `album.origin.json` and the front says so.
///
/// Nothing is written here. The front applies the returned spread through the
/// undo stack like any other edit, so the command is one ⌘Z away from undone.
/// The album travels from the front rather than being reread from disk: the
/// index the user clicked belongs to the album on screen, unsaved edits and
/// all, and matching against a stale file would give back the wrong spread.
#[tauri::command]
fn origin_spread(
    album: Album,
    index: usize,
    state: State<'_, AppState>,
) -> Result<Option<colophon_core::model::Spread>, String> {
    let dir = {
        let guard = state.open.lock().unwrap();
        guard.as_ref().ok_or("aucun album ouvert")?.dir.clone()
    };
    let origine = colophon_core::reprise::origine(&dir).map_err(|e| format!("{e:#}"))?;
    Ok(colophon_core::reprise::spread_origine(&origine, &album, index))
}

/// One album folder as the storage panel shows it. The three weights are
/// separated because they are not equally expensive to lose: the cache
/// rebuilds itself on the next open, the preview PDF re-renders in seconds,
/// `album.json` is the user's work and nothing rebuilds it.
#[derive(Serialize)]
struct AlbumEntry {
    /// Folder name inside `albums`, the only handle the front ever sends back.
    id: String,
    title: String,
    /// Page format in millimetres, as the album carries it.
    format: Option<[f64; 2]>,
    spreads: Option<usize>,
    /// Last modification of the folder, seconds since the epoch.
    modified: Option<u64>,
    bytes_total: u64,
    bytes_thumbs: u64,
    bytes_pdf: u64,
    /// Set when `album.json` could not be read. The row still shows, with its
    /// weight and its deletion button: an unreadable album is precisely the
    /// one a user wants to get rid of.
    probleme: Option<String>,
}

#[derive(Serialize)]
struct StorageReport {
    /// The data directory itself, shown in full: it is the one path the user
    /// may need to type or reveal.
    dir: String,
    /// Everything under it, the log included, so the figure matches `du`.
    total: u64,
    albums: Vec<AlbumEntry>,
}

/// Bytes held by a directory tree. An unreadable entry counts as zero rather
/// than failing the whole walk: a storage panel that shows nothing because of
/// one bad permission would be worse than a figure slightly short.
fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

fn albums_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("dossier de données introuvable : {e}"))?
        .join("albums"))
}

fn modified_secs(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

fn album_entry(dir: &Path) -> AlbumEntry {
    let id = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    let bytes_thumbs = dir_size(&dir.join(".cache").join("thumbs"));
    let bytes_pdf = ["album.pdf", "album-print.pdf"]
        .iter()
        .filter_map(|n| std::fs::metadata(dir.join(n)).ok().map(|m| m.len()))
        .sum();
    let mut entry = AlbumEntry {
        title: id.clone(),
        id,
        format: None,
        spreads: None,
        modified: modified_secs(&dir.join("album.json")).or_else(|| modified_secs(dir)),
        bytes_total: dir_size(dir),
        bytes_thumbs,
        bytes_pdf,
        probleme: None,
    };
    match std::fs::read_to_string(dir.join("album.json"))
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str::<Album>(&t).map_err(|e| e.to_string()))
    {
        Ok(album) => {
            if !album.title.trim().is_empty() {
                entry.title = album.title.clone();
            }
            entry.format = Some([album.trim_mm.w, album.trim_mm.h]);
            entry.spreads = Some(album.spreads.len());
        }
        Err(e) => entry.probleme = Some(format!("album.json illisible : {e}")),
    }
    entry
}

/// Every album the app has composed, heaviest first, with the weight of the
/// data directory as a whole. Nothing here ever looks at a photo folder: the
/// panel accounts for what the app itself wrote, and nothing else.
#[tauri::command]
async fn list_albums(app: tauri::AppHandle) -> Result<StorageReport, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("dossier de données introuvable : {e}"))?;
    tauri::async_runtime::spawn_blocking(move || {
        let albums_root = data.join("albums");
        let mut albums: Vec<AlbumEntry> = std::fs::read_dir(&albums_root)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .map(|e| album_entry(&e.path()))
                    .collect()
            })
            .unwrap_or_default();
        albums.sort_by(|a, b| b.bytes_total.cmp(&a.bytes_total));
        StorageReport {
            dir: data.to_string_lossy().to_string(),
            total: dir_size(&data),
            albums,
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Resolve an album id against its root, or refuse. Two guards here, the
/// third (the album is not the one open) belongs to the caller: the id is a
/// bare folder name, and the resolved path really is a direct child of
/// `albums`. A photo folder is never reachable from here, which is the guard
/// that matters most: everything below deletes, and the photos are not ours
/// to delete. Free of Tauri types so all of it can be tested on real folders.
fn resolve_album_dir(root: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.starts_with('.')
        || id.contains('/')
        || id.contains('\\')
        || id.contains("..")
        || Path::new(id).components().count() != 1
    {
        return Err(format!("identifiant d’album invalide : {id}"));
    }
    let dir = root.join(id);
    if !dir.is_dir() {
        return Err(format!("album introuvable : {id}"));
    }
    // Canonicalised on both sides: a symlink planted in `albums` must not
    // make the deletion escape the data directory.
    let (root, dir) = (
        root.canonicalize().map_err(|e| e.to_string())?,
        dir.canonicalize().map_err(|e| e.to_string())?,
    );
    if dir.parent() != Some(root.as_path()) {
        return Err(format!("album hors du dossier de données : {id}"));
    }
    Ok(dir)
}

/// Delete one album folder: the composition, its preview PDF and its cache.
/// The photos it was composed from are not touched and cannot be, the folder
/// this removes is the app's own.
#[tauri::command]
fn delete_album(app: tauri::AppHandle, id: String, state: State<'_, AppState>) -> Result<u64, String> {
    let dir = resolve_album_dir(&albums_dir(&app)?, &id)?;
    {
        let guard = state.open.lock().unwrap();
        if let Some(open) = guard.as_ref() {
            let same = open
                .dir
                .canonicalize()
                .map(|d| d == dir)
                .unwrap_or(false);
            if same {
                return Err(
                    "Cet album est ouvert. Fermez-le (Fichier → Fermer l’album) avant de le supprimer."
                        .into(),
                );
            }
        }
    }
    let freed = dir_size(&dir);
    std::fs::remove_dir_all(&dir).map_err(|e| format!("suppression de {id} : {e}"))?;
    colophon_core::log::line(&format!("album supprimé, {freed} octets libérés"));
    Ok(freed)
}

/// Empty every thumbnail cache, the open album's included: the caches rebuild
/// themselves at the next open, they are the only thing here that does.
#[tauri::command]
async fn purge_thumb_caches(app: tauri::AppHandle) -> Result<u64, String> {
    let root = albums_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut freed = 0u64;
        let Ok(entries) = std::fs::read_dir(&root) else {
            return freed;
        };
        for e in entries.flatten() {
            if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let cache = e.path().join(".cache").join("thumbs");
            if !cache.is_dir() {
                continue;
            }
            let size = dir_size(&cache);
            if std::fs::remove_dir_all(&cache).is_ok() {
                freed += size;
            }
        }
        colophon_core::log::line(&format!("caches de vignettes purgés, {freed} octets"));
        freed
    })
    .await
    .map_err(|e| e.to_string())
}

/// Show the data directory in the system file manager. The path is the app's
/// own, never one the front sends: like the report channel, this must not be
/// able to become a generic opener.
#[tauri::command]
fn reveal_data_dir(app: tauri::AppHandle) -> Result<(), String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("dossier de données introuvable : {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("création du dossier : {e}"))?;
    #[cfg(target_os = "macos")]
    let run = std::process::Command::new("open").arg(&dir).spawn();
    #[cfg(target_os = "windows")]
    let run = std::process::Command::new("explorer").arg(&dir).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let run = std::process::Command::new("xdg-open").arg(&dir).spawn();
    run.map(|_| ())
        .map_err(|e| format!("ouverture du dossier : {e}"))
}

/// Preflight the album as it stands on disk against one profile. Reads every
/// original's dimensions, so it is seconds on a big album: off the main
/// thread, like the renders.
#[tauri::command]
async fn preflight(
    profil: String,
    state: State<'_, AppState>,
) -> Result<colophon_core::prevol::PrevolReport, String> {
    let dir = {
        let guard = state.open.lock().unwrap();
        guard.as_ref().ok_or("aucun album ouvert")?.dir.clone()
    };
    let profil = printer_profile(&profil)?;
    tauri::async_runtime::spawn_blocking(move || colophon_core::prevol::prevol(&dir, profil))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState::default());
            // The file log the report channel will quote: errors and long
            // stages, rotating, path-scrubbed. Failing to open it must not
            // stop the app, an unlogged session beats no session.
            if let Ok(dir) = app.path().app_data_dir() {
                if let Err(e) = colophon_core::log::init(&dir) {
                    eprintln!("log indisponible : {e}");
                } else {
                    colophon_core::log::line(&format!(
                        "démarrage, version {}",
                        app.package_info().version
                    ));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_album,
            thumb,
            save_album,
            render_pdf,
            export_pdf,
            list_formats,
            build_album_from_folder,
            recompose_album,
            cancel_build,
            cancel_export,
            caption_suggestion,
            detected_focal,
            curation,
            list_printers,
            preflight,
            list_densities,
            report_data,
            open_report_url,
            origin_spread,
            choose_variante,
            colophon_spread,
            list_albums,
            delete_album,
            purge_thumb_caches,
            reveal_data_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Albums built by the CLI live outside the repo; the test is a no-op when
    /// they have not been generated yet.
    fn sample() -> Option<PathBuf> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../.albums/corse-2013")
            .canonicalize()
            .ok()?;
        dir.join("album.json").is_file().then_some(dir)
    }

    #[test]
    fn every_slot_resolves_to_a_readable_thumbnail() {
        let Some(dir) = sample() else { return };
        let (dir, album, thumbs) = load_album(&dir).expect("album lisible");
        assert!(!album.spreads.is_empty());
        for spread in &album.spreads {
            for slot in &spread.slots {
                let data = read_thumb(&dir, &thumbs, &slot.src)
                    .unwrap_or_else(|e| panic!("{}: {e}", slot.src));
                assert_eq!(&data[..2], &[0xFF, 0xD8], "{} n'est pas un JPEG", slot.src);
            }
        }
    }

    /// The whole editing loop below the UI: open, remove a photo, let the
    /// template fall back, save atomically, reopen, find the same album.
    #[test]
    fn edited_album_survives_a_save_and_reopen() {
        let Some(dir) = sample() else { return };
        let (_, mut album, _) = load_album(&dir).expect("album lisible");

        let idx = album
            .spreads
            .iter()
            .position(|s| s.slots.len() >= 2)
            .expect("au moins une planche à deux photos");
        let spread = &mut album.spreads[idx];
        spread.slots.remove(0);
        let (template, cap) =
            colophon_core::pdf::fallback_template(&spread.template, spread.slots.len())
                .expect("un gabarit de repli existe");
        spread.template = template.clone();
        spread.slots.truncate(cap);

        let tmp = std::env::temp_dir().join(format!("colophon-save-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        colophon_core::write_album_json(&tmp, &album).expect("écriture atomique");
        let reread: Album =
            serde_json::from_str(&std::fs::read_to_string(tmp.join("album.json")).unwrap())
                .expect("album relisible");
        assert_eq!(reread.spreads[idx].template, template);
        assert_eq!(reread.spreads[idx].slots.len(), cap);
        assert!(!tmp.join("album.json.tmp").exists(), "pas de fichier temporaire orphelin");
        std::fs::remove_dir_all(&tmp).ok();
    }

    /// The editor's PDF button: re-render from album.json + thumbs.json only.
    #[test]
    fn render_pdf_from_saved_album_alone() {
        let Some(dir) = sample() else { return };
        let pdf = colophon_core::render_album_pdf(&dir).expect("rendu PDF");
        let bytes = std::fs::read(&pdf).unwrap();
        assert_eq!(&bytes[..5], b"%PDF-", "album.pdf n'est pas un PDF");
    }

    /// The report channel opens exactly one page: the repo's issue form.
    /// Anything else, scheme included, is refused before any process spawns.
    #[test]
    fn report_url_guard_refuses_anything_but_the_issue_form() {
        assert!(open_report_url("https://example.com/x".into()).is_err());
        assert!(open_report_url(
            "http://github.com/alexis-morain/colophon/issues/new?template=1-bug.yml".into()
        )
        .is_err());
        assert!(open_report_url(
            "https://github.com/autre/depot/issues/new".into()
        )
        .is_err());
    }

    /// A scratch `albums` root with one album folder inside, plus a decoy
    /// sibling standing in for the photo folder the deletion must never see.
    fn albums_root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("colophon-albums-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(base.join("albums").join("corse-2013-abcd1234")).unwrap();
        std::fs::create_dir_all(base.join("photos-de-la-famille")).unwrap();
        base
    }

    /// The id is a folder name, nothing else. Everything that could climb out
    /// of `albums` is refused before a single byte is read.
    #[test]
    fn an_album_id_can_never_be_a_path() {
        let base = albums_root("id");
        let root = base.join("albums");
        for bad in [
            "",
            "..",
            ".",
            ".cache",
            "../photos-de-la-famille",
            "..\\photos-de-la-famille",
            "corse/../..",
            "/etc",
            "sous/dossier",
        ] {
            assert!(
                resolve_album_dir(&root, bad).is_err(),
                "{bad:?} aurait dû être refusé"
            );
        }
        assert!(resolve_album_dir(&root, "corse-2013-abcd1234").is_ok());
        std::fs::remove_dir_all(&base).ok();
    }

    /// The resolved folder must be a direct child of `albums`. A symlink
    /// planted there points at the photo folder: following it would delete
    /// the user's photos, so the guard resolves both sides and compares.
    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_albums_folder_is_refused() {
        let base = albums_root("lien");
        let root = base.join("albums");
        std::os::unix::fs::symlink(base.join("photos-de-la-famille"), root.join("piege")).unwrap();
        let err = resolve_album_dir(&root, "piege").expect_err("le lien doit être refusé");
        assert!(err.contains("hors du dossier de données"), "{err}");
        assert!(base.join("photos-de-la-famille").is_dir());
        std::fs::remove_dir_all(&base).ok();
    }

    /// A deletion frees exactly what the panel announced, and the album's
    /// own root folder is the only thing that goes.
    #[test]
    fn deleting_an_album_frees_what_the_entry_announced() {
        let base = albums_root("poids");
        let dir = base.join("albums").join("corse-2013-abcd1234");
        std::fs::create_dir_all(dir.join(".cache").join("thumbs")).unwrap();
        std::fs::write(dir.join(".cache").join("thumbs").join("a.jpg"), vec![7u8; 4096]).unwrap();
        std::fs::write(dir.join("album.pdf"), vec![0u8; 2048]).unwrap();
        std::fs::write(dir.join("album.json"), "pas du json").unwrap();

        let entry = album_entry(&dir);
        assert_eq!(entry.bytes_thumbs, 4096);
        assert_eq!(entry.bytes_pdf, 2048);
        assert_eq!(entry.bytes_total, 4096 + 2048 + 11);
        // An unreadable album.json still yields a row, weight included.
        assert!(entry.probleme.is_some());
        assert_eq!(entry.title, "corse-2013-abcd1234");

        let resolved = resolve_album_dir(&base.join("albums"), &entry.id).unwrap();
        let freed = dir_size(&resolved);
        std::fs::remove_dir_all(&resolved).unwrap();
        assert_eq!(freed, entry.bytes_total);
        assert!(base.join("photos-de-la-famille").is_dir());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn album_json_path_works_too() {
        let Some(dir) = sample() else { return };
        let (from_json, _, _) = load_album(&dir.join("album.json")).expect("album.json direct");
        assert_eq!(from_json, dir);
    }
}
