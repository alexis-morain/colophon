//! Thumbnail cache. The whole pipeline works on bounded-size thumbnails;
//! originals are only reopened at render time, one at a time.

use anyhow::{Context, Result};
use image::DynamicImage;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub const THUMB_SIZE: u32 = 1600;

/// The cache file a `thumbs.json` entry names, or `None` when the entry is
/// not a bare file name. `thumbs.json` is data from an album folder, and an
/// album folder is what people share: a value of `../../etc/passwd` used to
/// be joined as-is by six readers, none of which had the closed-set guard
/// the police file and the album id already have. A name is a name — no
/// separator, no `.`, no `..`, nothing empty — or it is nothing.
pub fn chemin(dir: &Path, name: &str) -> Option<PathBuf> {
    let propre = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.contains('\0');
    propre.then(|| dir.join(".cache").join("thumbs").join(name))
}

pub struct ThumbCache {
    dir: PathBuf,
}

impl ThumbCache {
    pub fn new(album_out: &Path) -> Result<Self> {
        let dir = album_out.join(".cache").join("thumbs");
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn key(path: &Path) -> String {
        let mut h = DefaultHasher::new();
        path.hash(&mut h);
        if let Ok(m) = fs::metadata(path) {
            m.len().hash(&mut h);
            if let Ok(t) = m.modified() {
                t.hash(&mut h);
            }
        }
        format!("{:016x}.jpg", h.finish())
    }

    pub fn path_for(&self, src: &Path) -> PathBuf {
        self.dir.join(Self::key(src))
    }

    /// Remove every cached file whose name is not in `gardes`, and say how
    /// many. The key above invalidates a thumbnail the moment its photo is
    /// touched — and never removed the old one: a photo edited, renamed or
    /// deleted orphaned its thumbnail for good, 227 Ko each, with no bound
    /// on age or size. `log.rs` caps its own file « so the log can never
    /// quietly eat the disk the way the thumbnail caches did » — this is
    /// that cache, finally pruned, right after the index that names what
    /// the album still uses has been written.
    pub fn elaguer(&self, gardes: &std::collections::BTreeSet<String>) -> Result<usize> {
        let mut retires = 0;
        for entree in fs::read_dir(&self.dir)? {
            let entree = entree?;
            let nom = entree.file_name().to_string_lossy().to_string();
            if gardes.contains(&nom) || !entree.file_type()?.is_file() {
                continue;
            }
            fs::remove_file(entree.path())?;
            retires += 1;
        }
        Ok(retires)
    }

    /// Returns the cached thumbnail, building it (orientation applied) if needed.
    pub fn get(&self, src: &Path, orientation: u32) -> Result<DynamicImage> {
        let cached = self.path_for(src);
        if cached.exists() {
            if let Ok(img) = image::open(&cached) {
                return Ok(img);
            }
        }
        // A RAW hands over the preview its camera rendered, never the
        // demosaic: everything up to the print reads this cache, and the
        // print asks for the sensor itself only when the preview falls
        // short of the resolution floor (`print.rs`).
        let img = if crate::heic::is_raw(src) {
            crate::heic::apercu_vignette(src, THUMB_SIZE)
        } else {
            crate::heic::open(src)
        }
        .with_context(|| format!("decode {}", src.display()))?;
        let img = apply_orientation(img, orientation);
        // Downscale only: `thumbnail` would upscale a small photo to the
        // box, forging pixels the original never had and hiding its real
        // size from every reader. The thumbnail of a small photo is the
        // photo; a thumbnail under THUMB_SIZE is therefore always the
        // original's exact pixel count, which is what lets the editor warn
        // about resolution without reopening the file.
        let thumb = if img.width().max(img.height()) > THUMB_SIZE {
            img.thumbnail(THUMB_SIZE, THUMB_SIZE)
        } else {
            img
        };
        thumb
            .to_rgb8()
            .save_with_format(&cached, image::ImageFormat::Jpeg)
            .with_context(|| format!("write thumb for {}", src.display()))?;
        // Read it back instead of returning the in-memory version: JPEG is
        // lossy, and every later run analyses the encoded pixels. Without this
        // the first build and the next one produce different albums.
        image::open(&cached).with_context(|| format!("reread thumb for {}", src.display()))
    }
}

pub fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

#[cfg(test)]
mod tests_chemin {
    use super::*;

    /// A `thumbs.json` value is a file name or it is nothing: the six
    /// readers of the cache go through here, and here is the only place
    /// that knows what a name is.
    #[test]
    fn un_nom_de_vignette_est_un_nom_ou_rien() {
        let dir = Path::new("/albums/x");
        assert_eq!(
            chemin(dir, "0123456789abcdef.jpg"),
            Some(dir.join(".cache").join("thumbs").join("0123456789abcdef.jpg"))
        );
        for mauvais in ["../../../../etc/passwd", "a/b.jpg", "a\\b.jpg", "..", ".", "", "/etc/passwd", "a\0b"] {
            assert!(chemin(dir, mauvais).is_none(), "{mauvais:?} a passé");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache keeps what the index names and nothing else: a stray file
    /// goes, a referenced one stays, a subfolder is left alone.
    #[test]
    fn le_cache_ne_garde_que_ce_que_l_index_nomme() {
        let dir = std::env::temp_dir().join(format!("colophon-elague-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cache = ThumbCache::new(&dir).unwrap();
        let racine = dir.join(".cache").join("thumbs");
        fs::write(racine.join("garde.jpg"), b"a").unwrap();
        fs::write(racine.join("orphelin.jpg"), b"b").unwrap();
        fs::write(racine.join("orphelin-2.jpg"), b"c").unwrap();
        fs::create_dir_all(racine.join("sous")).unwrap();
        let gardes = std::collections::BTreeSet::from(["garde.jpg".to_string()]);
        assert_eq!(cache.elaguer(&gardes).unwrap(), 2);
        assert!(racine.join("garde.jpg").is_file());
        assert!(!racine.join("orphelin.jpg").exists());
        assert!(racine.join("sous").is_dir());
        assert_eq!(cache.elaguer(&gardes).unwrap(), 0, "rien à retirer la seconde fois");
        fs::remove_dir_all(&dir).ok();
    }
}
