//! Editorial pipeline: burst grouping, best-of-burst selection, chaptering.

use crate::analyze::{color_distance, hamming, Analysis};
use crate::meta::PhotoMeta;
use chrono::NaiveDateTime;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Photo {
    pub path: PathBuf,
    pub meta: PhotoMeta,
    pub analysis: Analysis,
}

impl Photo {
    /// Selection score. Photos without a reliable EXIF date are usually
    /// screenshots, downloads or forwarded images: heavy penalty so they
    /// never outrank a real photo, without being dropped outright.
    pub fn effective_score(&self) -> f64 {
        let base = self.analysis.score();
        if self.meta.taken_reliable {
            base
        } else {
            base * 0.25
        }
    }
}

#[derive(Debug)]
pub struct Chapter {
    pub photos: Vec<Photo>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
}

/// Bursts: photos closer than this in time AND visually similar collapse to one.
const BURST_GAP_SECONDS: i64 = 4;
const BURST_HAMMING: u32 = 12;
/// Same-scene collapse: within this window, visually close photos merge too.
/// Catches the "five shots of the same coastline minutes apart" pattern.
const SCENE_GAP_SECONDS: i64 = 15 * 60;
const SCENE_HAMMING: u32 = 11;
/// A time gap larger than this starts a new chapter.
const CHAPTER_GAP_HOURS: i64 = 8;

/// Screenshots, memes and forwarded images lack a camera fingerprint.
/// A real photo carries an EXIF capture date; failing that, it needs both
/// GPS and a camera model (iOS can stamp GPS onto saved images, so GPS
/// alone proves nothing).
pub fn split_junk(photos: Vec<Photo>) -> (Vec<Photo>, usize) {
    let (keep, junk): (Vec<_>, Vec<_>) = photos.into_iter().partition(|p| {
        p.meta.taken_reliable || (p.meta.gps.is_some() && p.meta.model.is_some())
    });
    let n = junk.len();
    (keep, n)
}

/// Collapse bursts and near-duplicates, keeping the best-scored photo of
/// each run. Looks back over the last few kept photos so that an
/// alternating burst (dark/bright/dark) still collapses.
pub fn dedup(mut photos: Vec<Photo>) -> Vec<Photo> {
    photos.sort_by_key(|p| p.meta.taken);
    let mut out: Vec<Photo> = Vec::with_capacity(photos.len());
    for p in photos {
        let lookback = out.len().saturating_sub(3);
        let dup_of = (lookback..out.len()).find(|&i| {
            let prev = &out[i];
            let dt = (p.meta.taken - prev.meta.taken).num_seconds().abs();
            let dist = hamming(p.analysis.dhash, prev.analysis.dhash);
            let cdist = color_distance(&p.analysis.colorsig, &prev.analysis.colorsig);
            (dt <= BURST_GAP_SECONDS && dist <= BURST_HAMMING)
                || (dt <= SCENE_GAP_SECONDS && dist <= SCENE_HAMMING)
                || (dt <= SCENE_GAP_SECONDS && dist <= 22 && cdist <= 12)
                || dist <= 4
        });
        match dup_of {
            Some(i) => {
                if p.effective_score() > out[i].effective_score() {
                    out[i] = p;
                }
            }
            None => out.push(p),
        }
    }
    out
}

/// Split the (time-sorted) photos into chapters on large time gaps.
pub fn chapters(photos: Vec<Photo>) -> Vec<Chapter> {
    let mut chapters: Vec<Chapter> = Vec::new();
    for p in photos {
        let new_chapter = chapters.last().is_none_or(|c| {
            (p.meta.taken - c.end).num_hours() >= CHAPTER_GAP_HOURS
        });
        if new_chapter {
            chapters.push(Chapter { start: p.meta.taken, end: p.meta.taken, photos: vec![p] });
        } else {
            let c = chapters.last_mut().unwrap();
            c.end = p.meta.taken;
            c.photos.push(p);
        }
    }
    chapters
}

/// Cap each chapter to its strongest photos, keeping chronological order.
pub fn cap_chapter(chapter: &mut Chapter, max: usize) {
    if chapter.photos.len() <= max {
        return;
    }
    let mut idx: Vec<usize> = (0..chapter.photos.len()).collect();
    idx.sort_by(|&a, &b| {
        chapter.photos[b]
            .effective_score()
            .partial_cmp(&chapter.photos[a].effective_score())
            .unwrap()
    });
    let mut keep: Vec<usize> = idx.into_iter().take(max).collect();
    keep.sort();
    chapter.photos = keep.into_iter().map(|i| chapter.photos[i].clone()).collect();
}
