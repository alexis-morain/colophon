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
        .map_err(|e| format!("{e:#}"))
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
    })
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
            open_report_url
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

    #[test]
    fn album_json_path_works_too() {
        let Some(dir) = sample() else { return };
        let (from_json, _, _) = load_album(&dir.join("album.json")).expect("album.json direct");
        assert_eq!(from_json, dir);
    }
}
