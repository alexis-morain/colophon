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
    /// Original pixel size, EXIF orientation applied. The composer uses it
    /// to keep every cell at a printable effective resolution.
    pub orig: (u32, u32),
    /// Detected face boxes, normalized [x, y, w, h], top-left origin.
    pub faces: Vec<[f64; 4]>,
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

/// Aspect beyond which no template can hold the photo without betraying its
/// orientation: wider than a pano cell can absorb, narrower than an étroit
/// cell can. Stitched panoramas land here; the sorting view shows them.
pub const PRINTABLE_ASPECT_MAX: f64 = 2.8;

/// Photos no page can hold without butchering them, set aside as
/// `panorama` for the sorting view. Rescuing one remains possible: the
/// editor crops it like any other photo, but that is the user's choice.
pub fn split_unprintable(photos: Vec<Photo>) -> (Vec<Photo>, Vec<Photo>) {
    photos.into_iter().partition(|p| {
        let a = p.analysis.aspect();
        (1.0 / PRINTABLE_ASPECT_MAX..=PRINTABLE_ASPECT_MAX).contains(&a)
    })
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

/// Photos this close together are one moment. Inside that window the visual
/// gate loosens: change the sky behind a face and the hash moves a lot, so
/// same-minute frames get compared with a wider tolerance than twins do.
const MOMENT_GAP_SECONDS: i64 = 60;
const MOMENT_HAMMING: u32 = 24;

/// Drop near-duplicates inside each one-minute window, keeping the
/// best-scored frame of every run. Both gates must agree: the clock alone
/// never drops anything, so three genuinely different photos taken in one
/// minute all survive, while three takes of the same one collapse. What
/// exceeds the album budget later is trimmed by score, not by timestamp.
pub fn cap_moments(chapter: &mut Chapter) -> Vec<DropPair> {
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
            // photos are time-sorted: past the window, stay past it
            if (b.meta.taken - a.meta.taken).num_seconds().abs() > MOMENT_GAP_SECONDS {
                break;
            }
            if hamming(a.analysis.dhash, b.analysis.dhash) > MOMENT_HAMMING {
                continue;
            }
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

/// A chapter below this many photos cannot open properly nor fill two
/// spreads: it gets absorbed into a neighbour.
pub const MIN_CHAPTER_PHOTOS: usize = 4;

/// Merge adjacent chapters until at most `max` remain and none is starving,
/// always closing the smallest time gap first so the strongest natural
/// boundaries survive. A year of scattered photos collapses into month-ish
/// runs; a one-week trip with clear day gaps is left untouched.
pub fn merge_chapters(mut chapters: Vec<Chapter>, max: usize) -> Vec<Chapter> {
    loop {
        if chapters.len() <= 1 {
            break;
        }
        let too_many = chapters.len() > max.max(1);
        let starving = chapters
            .iter()
            .position(|c| c.photos.len() < MIN_CHAPTER_PHOTOS);
        if !too_many && starving.is_none() {
            break;
        }
        // Boundaries eligible for closing: all of them when over the cap,
        // otherwise only the ones around the first starving chapter.
        let gap = |i: usize| (chapters[i].start - chapters[i - 1].end).num_seconds();
        let best = if too_many {
            (1..chapters.len()).min_by_key(|&i| gap(i)).unwrap()
        } else {
            let s = starving.unwrap();
            let candidates = [s, s + 1];
            candidates
                .into_iter()
                .filter(|&i| i >= 1 && i < chapters.len())
                .min_by_key(|&i| gap(i))
                .unwrap()
        };
        let absorbed = chapters.remove(best);
        let prev = &mut chapters[best - 1];
        prev.end = prev.end.max(absorbed.end);
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
            // At least four photos: enough to open on a strong image and
            // still fill a second spread, so no chapter is a single planche.
            share.clamp(MIN_CHAPTER_PHOTOS, c.photos.len().max(MIN_CHAPTER_PHOTOS))
        })
        .collect()
}

/// Cap each chapter to its strongest photos, keeping chronological order.
/// Diversity-aware: a weaker but different photo beats a stronger
/// near-twin of something already kept, otherwise the shown set fills up
/// with the same postcard and the composer spends the album keeping the
/// copies apart.
pub fn cap_chapter(chapter: &mut Chapter, max: usize) {
    use crate::analyze::hamming;
    use crate::audit::{DUP_HAMMING, DUP_PHASH};
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
    // True twins never refill an empty seat: a chapter short on diversity
    // shows fewer photos, that is what curation is for.
    const REFILL_HAMMING: u32 = 16;
    const REFILL_PHASH: u32 = 8;
    let mut keep: Vec<usize> = Vec::with_capacity(max);
    for pass in 0..2 {
        for &i in &idx {
            if keep.len() >= max {
                break;
            }
            if keep.contains(&i) {
                continue;
            }
            let (d_max, p_max) = if pass == 0 {
                (DUP_HAMMING, DUP_PHASH)
            } else {
                (REFILL_HAMMING, REFILL_PHASH)
            };
            let twin = keep.iter().any(|&k| {
                let (a, b) = (&chapter.photos[i].analysis, &chapter.photos[k].analysis);
                hamming(a.dhash, b.dhash) <= d_max || hamming(a.phash, b.phash) <= p_max
            });
            if !twin {
                keep.push(i);
            }
        }
    }
    keep.sort();
    chapter.photos = keep.into_iter().map(|i| chapter.photos[i].clone()).collect();
}
