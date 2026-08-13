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
    /// Face-anchored crop focal point, when at least one face was found.
    pub focal: Option<[f64; 2]>,
}

impl Photo {
    /// Selection score. Photos without a reliable EXIF date are usually
    /// screenshots, downloads or forwarded images: heavy penalty so they
    /// never outrank a real photo, without being dropped outright.
    pub fn effective_score(&self) -> f64 {
        let mut score = self.analysis.score();
        if !self.meta.taken_reliable {
            score *= 0.25;
        }
        // People beat scenery in a family album, mildly.
        if self.focal.is_some() {
            score *= 1.15;
        }
        score
    }
}

#[derive(Debug, Clone)]
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
/// Near-identical frames collapse whatever the clock says. Three selfies of
/// the same face, taken an hour apart, are still one photo in an album, and a
/// mosaic of eight makes that glaring.
const TWIN_HAMMING: u32 = 8;
const TWIN_COLOR: u32 = 10;
/// How far back the comparison reaches. Wide enough that an alternating run
/// (portrait, landscape, portrait) still collapses.
const LOOKBACK: usize = 8;
/// A time gap larger than this starts a new chapter.
const CHAPTER_GAP_HOURS: i64 = 8;

/// A photo dropped in favour of another: (loser's path, winner's path).
pub type DropPair = (PathBuf, PathBuf);

/// Winners can lose a later round; follow the chain so every discarded photo
/// points at a photo that actually survived the pass.
fn settle(pairs: Vec<DropPair>) -> Vec<DropPair> {
    let map: std::collections::HashMap<PathBuf, PathBuf> = pairs.iter().cloned().collect();
    pairs
        .into_iter()
        .map(|(dropped, mut winner)| {
            let mut hops = 0;
            while let Some(next) = map.get(&winner) {
                winner = next.clone();
                hops += 1;
                if hops > 64 {
                    break; // defensive: the pass never drops both ways round
                }
            }
            (dropped, winner)
        })
        .collect()
}

/// Screenshots, memes and forwarded images lack a camera fingerprint.
/// A real photo carries an EXIF capture date; failing that, it needs both
/// GPS and a camera model (iOS can stamp GPS onto saved images, so GPS
/// alone proves nothing). Returns the junk itself: the sorting view shows it.
pub fn split_junk(photos: Vec<Photo>) -> (Vec<Photo>, Vec<Photo>) {
    photos.into_iter().partition(|p| {
        p.meta.taken_reliable || (p.meta.gps.is_some() && p.meta.model.is_some())
    })
}

/// Collapse bursts and near-duplicates, keeping the best-scored photo of
/// each run. Looks back over the last few kept photos so that an
/// alternating burst (dark/bright/dark) still collapses.
/// Also returns who lost against whom, for `curation.json`.
pub fn dedup(mut photos: Vec<Photo>) -> (Vec<Photo>, Vec<DropPair>) {
    photos.sort_by_key(|p| p.meta.taken);
    let mut out: Vec<Photo> = Vec::with_capacity(photos.len());
    let mut drops: Vec<DropPair> = Vec::new();
    for p in photos {
        let lookback = out.len().saturating_sub(LOOKBACK);
        let dup_of = (lookback..out.len()).find(|&i| {
            let prev = &out[i];
            let dt = (p.meta.taken - prev.meta.taken).num_seconds().abs();
            let dist = hamming(p.analysis.dhash, prev.analysis.dhash);
            let cdist = color_distance(&p.analysis.colorsig, &prev.analysis.colorsig);
            (dt <= BURST_GAP_SECONDS && dist <= BURST_HAMMING)
                || (dt <= SCENE_GAP_SECONDS && dist <= SCENE_HAMMING)
                || (dt <= SCENE_GAP_SECONDS && dist <= 22 && cdist <= 12)
                || (dist <= TWIN_HAMMING && cdist <= TWIN_COLOR)
        });
        match dup_of {
            Some(i) => {
                if p.effective_score() > out[i].effective_score() {
                    drops.push((out[i].path.clone(), p.path.clone()));
                    out[i] = p;
                } else {
                    drops.push((p.path.clone(), out[i].path.clone()));
                }
            }
            None => out.push(p),
        }
    }
    (out, settle(drops))
}

/// Photos this close together are one moment. Three selfies inside a minute
/// are one memory even when they look nothing alike to a perceptual hash:
/// change the sky behind a face and the hash moves, the moment does not.
const MOMENT_GAP_SECONDS: i64 = 60;
/// How many frames a single moment is worth in a finished album.
const MOMENT_KEEP: usize = 2;

/// Keep the best few frames of each moment, in chronological order.
/// Every dropped frame points at the best frame of its moment.
pub fn cap_moments(chapter: &mut Chapter) -> Vec<DropPair> {
    let before = chapter.photos.len();
    let mut keep: Vec<usize> = Vec::with_capacity(before);
    let mut moment: Vec<usize> = Vec::new();
    let mut drops: Vec<DropPair> = Vec::new();

    let close = |a: &Photo, b: &Photo| {
        (b.meta.taken - a.meta.taken).num_seconds().abs() <= MOMENT_GAP_SECONDS
    };
    let flush = |moment: &mut Vec<usize>,
                 keep: &mut Vec<usize>,
                 drops: &mut Vec<DropPair>,
                 photos: &[Photo]| {
        if moment.len() > MOMENT_KEEP {
            moment.sort_by(|&a, &b| {
                photos[b].effective_score().partial_cmp(&photos[a].effective_score()).unwrap()
            });
            let best = photos[moment[0]].path.clone();
            for &i in moment.iter().skip(MOMENT_KEEP) {
                drops.push((photos[i].path.clone(), best.clone()));
            }
            moment.truncate(MOMENT_KEEP);
            moment.sort();
        }
        keep.append(moment);
    };

    for i in 0..before {
        let same = moment
            .last()
            .is_some_and(|&last| close(&chapter.photos[last], &chapter.photos[i]));
        if !same {
            flush(&mut moment, &mut keep, &mut drops, &chapter.photos);
        }
        moment.push(i);
    }
    flush(&mut moment, &mut keep, &mut drops, &chapter.photos);

    keep.sort();
    chapter.photos = keep.into_iter().map(|i| chapter.photos[i].clone()).collect();
    drops
}

/// Second pass, once chapters exist: drop near-identical frames whatever
/// their distance in time or in the sequence. `dedup` only looks a few photos
/// back, which covers bursts but lets the same selfie come round three times
/// in an afternoon; a mosaic of eight then prints them side by side.
/// Chapters hold a few dozen photos, so comparing all pairs is free.
/// Returns who lost against whom, for `curation.json`.
pub fn thin_twins(chapter: &mut Chapter) -> Vec<DropPair> {
    let n = chapter.photos.len();
    let mut dropped = vec![false; n];
    let mut drops: Vec<DropPair> = Vec::new();
    for i in 0..n {
        if dropped[i] {
            continue;
        }
        for j in i + 1..n {
            if dropped[j] {
                continue;
            }
            let (a, b) = (&chapter.photos[i], &chapter.photos[j]);
            let dist = hamming(a.analysis.dhash, b.analysis.dhash);
            let cdist = color_distance(&a.analysis.colorsig, &b.analysis.colorsig);
            if dist > TWIN_HAMMING || cdist > TWIN_COLOR {
                continue;
            }
            // Keep the better frame of the pair, drop the other.
            if b.effective_score() > a.effective_score() {
                dropped[i] = true;
                drops.push((a.path.clone(), b.path.clone()));
                break;
            }
            dropped[j] = true;
            drops.push((b.path.clone(), a.path.clone()));
        }
    }
    let mut k = 0;
    chapter.photos.retain(|_| {
        let keep = !dropped[k];
        k += 1;
        keep
    });
    settle(drops)
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

/// Merge adjacent chapters until at most `max` remain, always closing the
/// smallest time gap first so the strongest natural boundaries survive.
/// A year of scattered photos collapses into month-ish runs; a one-week
/// trip with clear day gaps is left untouched.
pub fn merge_chapters(mut chapters: Vec<Chapter>, max: usize) -> Vec<Chapter> {
    while chapters.len() > max.max(1) {
        let mut best = 1usize;
        let mut best_gap = i64::MAX;
        for i in 1..chapters.len() {
            let gap = (chapters[i].start - chapters[i - 1].end).num_seconds();
            if gap < best_gap {
                best_gap = gap;
                best = i;
            }
        }
        let absorbed = chapters.remove(best);
        let prev = &mut chapters[best - 1];
        prev.end = absorbed.end;
        prev.photos.extend(absorbed.photos);
    }
    chapters
}

/// Split a total photo budget across chapters, proportional to the square
/// root of their size: big events get more room, small ones still exist.
pub fn allocate_budget(chapters: &[Chapter], total: usize) -> Vec<usize> {
    let weights: Vec<f64> = chapters.iter().map(|c| (c.photos.len() as f64).sqrt()).collect();
    let sum: f64 = weights.iter().sum();
    chapters
        .iter()
        .zip(&weights)
        .map(|(c, w)| {
            let share = (total as f64 * w / sum).round() as usize;
            share.clamp(2, c.photos.len().max(2))
        })
        .collect()
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
