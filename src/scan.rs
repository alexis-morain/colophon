//! Folder scan: collect supported images, report what is skipped.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct ScanResult {
    pub images: Vec<PathBuf>,
    pub skipped_heic: usize,
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
    let mut skipped_other = 0;

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
            skipped_heic += 1;
        } else if IGNORED.contains(&ext.as_str()) {
            // silently ignored: videos and sidecars are expected in real folders
        } else {
            skipped_other += 1;
        }
    }
    images.sort();
    ScanResult { images, skipped_heic, skipped_other }
}
