//! Folder scan: collect supported images, report what is skipped.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct ScanResult {
    pub images: Vec<PathBuf>,
    pub skipped_heic: usize,
    /// RAW files on a platform without a system decoder. Counted and named,
    /// never handed to a second engine (see `heic.rs`).
    pub skipped_raw: usize,
    pub skipped_other: usize,
}

const SUPPORTED: [&str; 3] = ["jpg", "jpeg", "png"];
const HEIC: [&str; 2] = ["heic", "heif"];
const IGNORED: [&str; 12] = [
    "mov", "mp4", "avi", "m4v", "mts", "3gp", "aae", "xmp", "json", "txt", "db", "ds_store",
];

pub fn scan(root: &Path) -> ScanResult {
    let mut images = Vec::new();
    let mut skipped_heic = 0;
    let mut skipped_raw = 0;
    let mut skipped_other = 0;
    // HEIC and RAW join the album when the platform can decode them
    // (ImageIO on macOS, WIC on Windows); elsewhere they are counted and
    // reported, each under its own name.
    let systeme_ok = crate::heic::system().is_some();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let ext = entry
            .path()
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if SUPPORTED.contains(&ext.as_str()) {
            images.push(entry.into_path());
        } else if HEIC.contains(&ext.as_str()) {
            if systeme_ok {
                images.push(entry.into_path());
            } else {
                skipped_heic += 1;
            }
        } else if crate::heic::RAW.contains(&ext.as_str()) {
            if systeme_ok {
                images.push(entry.into_path());
            } else {
                skipped_raw += 1;
            }
        } else if IGNORED.contains(&ext.as_str()) {
            // silently ignored: videos and sidecars are expected in real folders
        } else {
            skipped_other += 1;
        }
    }
    images.sort();
    ScanResult { images, skipped_heic, skipped_raw, skipped_other }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A RAW is admitted where the platform decodes it and counted where
    /// it does not; on neither is it an unknown file, and a sidecar next to
    /// it stays silent. The two branches are the same assertion: every
    /// RAW of the folder is accounted for, once.
    #[test]
    fn un_raw_est_admis_ou_compte_jamais_inconnu() {
        let dir = std::env::temp_dir().join(format!("colophon-scan-raw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for nom in ["a.CR3", "b.nef", "c.orf", "c.orf.xmp", "d.jpg", "e.heic", "f.bmp"] {
            std::fs::write(dir.join(nom), b"pas une image").unwrap();
        }
        let r = scan(&dir);
        let raws_admis = r.images.iter().filter(|p| crate::heic::is_raw(p)).count();
        assert_eq!(raws_admis + r.skipped_raw, 3, "chaque RAW compte une fois");
        assert_eq!(r.skipped_other, 1, "le bmp est le seul inconnu");
        assert!(r.images.iter().any(|p| p.ends_with("d.jpg")));
        if crate::heic::system().is_some() {
            assert_eq!(r.skipped_raw, 0);
            assert_eq!(r.skipped_heic, 0);
        } else {
            assert_eq!(r.skipped_raw, 3);
            assert_eq!(r.skipped_heic, 1);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
