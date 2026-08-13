//! Desktop shell. The window is a thin viewer over `album.json`: the engine
//! stays in `colophon-core`, the app only opens albums and serves the
//! thumbnails the book view needs.

use colophon_core::model::Album;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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
}

#[derive(Serialize)]
struct OpenedAlbum {
    album: Album,
    dir: String,
    /// False when the original photo folder has moved: the preview still works
    /// off the thumbnail cache, a full-resolution export would not.
    root_present: bool,
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
    *state.open.lock().unwrap() = Some(Opened { dir: dir.clone(), thumbs });
    Ok(OpenedAlbum {
        album,
        dir: dir.to_string_lossy().to_string(),
        root_present,
    })
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<OpenedAlbum, String> {
    let trim = colophon_core::format::parse(&format).map_err(|e| e.to_string())?;
    let photos = PathBuf::from(&photos_dir);
    let out = album_out_dir(&app, &photos)?;

    let emitter = app.clone();
    let build_out = out.clone();
    tauri::async_runtime::spawn_blocking(move || {
        colophon_core::build_album(
            &photos,
            &build_out,
            colophon_core::BuildOptions {
                title: None,
                spreads: spreads.clamp(8, 200),
                trim,
                progress: Box::new(move |line| {
                    let _ = emitter.emit("build:progress", line);
                }),
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))?;

    open_album(out.to_string_lossy().to_string(), state)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_album,
            thumb,
            save_album,
            render_pdf,
            list_formats,
            build_album_from_folder
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

    #[test]
    fn album_json_path_works_too() {
        let Some(dir) = sample() else { return };
        let (from_json, _, _) = load_album(&dir.join("album.json")).expect("album.json direct");
        assert_eq!(from_json, dir);
    }
}
