//! Thumbnail cache. The whole pipeline works on bounded-size thumbnails;
//! originals are only reopened at render time, one at a time.

use anyhow::{Context, Result};
use image::DynamicImage;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub const THUMB_SIZE: u32 = 1600;

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

    /// Returns the cached thumbnail, building it (orientation applied) if needed.
    pub fn get(&self, src: &Path, orientation: u32) -> Result<DynamicImage> {
        let cached = self.path_for(src);
        if cached.exists() {
            if let Ok(img) = image::open(&cached) {
                return Ok(img);
            }
        }
        let img = crate::heic::open(src).with_context(|| format!("decode {}", src.display()))?;
        let img = apply_orientation(img, orientation);
        let thumb = img.thumbnail(THUMB_SIZE, THUMB_SIZE);
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
