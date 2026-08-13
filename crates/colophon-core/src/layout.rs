//! Spread composition: turn each chapter's photo run into a sequence of
//! templated spreads. Deterministic, tuned by simple rules on count,
//! orientation and score.
//!
//! One hard rule: no image ever crosses the fold. A photo printed across the
//! double page loses its middle to the binding, so the widest an image gets
//! is one full page.

use crate::model::{Slot, Spread};
use crate::pipeline::{Chapter, Photo};

/// Score above which a photo qualifies for a page of its own,
/// expressed as a quantile of the chapter's scores.
const FEATURE_QUANTILE: f64 = 0.85;

/// A cover-crop that throws away more than this much of the frame is a
/// butchered photo: give it margins instead of a full page.
const MIN_VISIBLE: f64 = 0.72;

/// Photos per spread, cycled through so the book keeps changing pace.
/// Mosaics of six and eight land often enough to be a rhythm, rarely
/// enough to stay a surprise.
const RHYTHM: [usize; 10] = [2, 3, 4, 2, 6, 3, 2, 8, 4, 3];

/// Average photos per spread this rhythm produces, times ten. The photo
/// budget in `build` leans on it to hit the requested number of spreads.
pub const PHOTOS_PER_SPREAD_X10: usize = 32;

pub struct Composer {
    /// Runs across the whole album so facing pages keep alternating between
    /// chapters, not just inside one.
    beat: usize,
    page_aspect: f64,
}

impl Composer {
    pub fn new(page_aspect: f64) -> Self {
        Self { beat: 0, page_aspect }
    }

    pub fn compose(
        &mut self,
        chapter: &Chapter,
        caption: Option<String>,
        root: &std::path::Path,
    ) -> Vec<Spread> {
        let photos = &chapter.photos;
        let mut spreads = Vec::new();
        let feature_threshold = quantile_score(photos, FEATURE_QUANTILE);

        let mut i = 0;
        let mut first = true;
        while i < photos.len() {
            let p = &photos[i];
            let remaining = photos.len() - i;

            // A standout photo earns a page to itself, but never two in a row.
            let last_was_feature = spreads
                .last()
                .is_some_and(|s: &Spread| s.slots.len() == 1);
            let take = if p.meta.taken_reliable
                && p.effective_score() >= feature_threshold
                && !last_was_feature
                && remaining != 2
            {
                1
            } else {
                chunk_size(&photos[i..], self.beat)
            };

            let template = self.template_for(&photos[i..i + take]);
            spreads.push(spread(&template, &photos[i..i + take], root));
            self.beat += 1;
            i += take;

            if first {
                if let Some(c) = &caption {
                    spreads.last_mut().unwrap().caption = Some(c.clone());
                }
                first = false;
            }
        }
        spreads
    }

    /// Template name for a chunk. The `_verso` variants flip the layout onto
    /// the other page; they alternate with the beat.
    fn template_for(&self, chunk: &[Photo]) -> String {
        let flip = self.beat % 2 == 1;
        let base = match chunk.len() {
            1 => {
                let a = chunk[0].analysis.aspect();
                let visible = (a / self.page_aspect).min(self.page_aspect / a);
                if visible >= MIN_VISIBLE {
                    // fills a page without losing much: print it edge to edge
                    "full1"
                } else if chunk[0].analysis.is_portrait() {
                    "solo"
                } else {
                    "solo_paysage"
                }
            }
            2 => "duo",
            3 => "trio",
            4 => "quad",
            5 | 6 => "six",
            _ => "octo",
        };
        // duo, quad and octo are symmetric: flipping them changes nothing.
        if flip && matches!(base, "full1" | "solo" | "solo_paysage" | "trio" | "six") {
            format!("{base}_verso")
        } else {
            base.to_string()
        }
    }
}

fn quantile_score(photos: &[Photo], q: f64) -> f64 {
    let mut scores: Vec<f64> = photos.iter().map(|p| p.effective_score()).collect();
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if scores.is_empty() {
        return f64::MAX;
    }
    let idx = ((scores.len() as f64 - 1.0) * q).round() as usize;
    scores[idx]
}

/// How many photos the next spread absorbs. Only counts a template can lay
/// out come back, and a lonely leftover is never created.
fn chunk_size(rest: &[Photo], beat: usize) -> usize {
    let remaining = rest.len();
    match remaining {
        0..=4 => return remaining.max(1),
        5 => return 3, // 3 + 2 beats 4 + 1
        7 => return 4, // 4 + 3
        _ => {}
    }

    // A run of portraits reads best two by two, facing each other.
    let portraits = rest.iter().take(4).filter(|p| p.analysis.is_portrait()).count();
    if portraits >= 3 {
        return 2;
    }

    let mut take = RHYTHM[beat % RHYTHM.len()].min(remaining);
    if remaining - take == 1 {
        take -= 1;
    }
    while !matches!(take, 1 | 2 | 3 | 4 | 6 | 8) {
        take -= 1;
    }
    take
}

fn spread(template: &str, photos: &[Photo], root: &std::path::Path) -> Spread {
    Spread {
        template: template.to_string(),
        slots: photos
            .iter()
            .map(|p| Slot {
                src: p
                    .path
                    .strip_prefix(root)
                    .unwrap_or(&p.path)
                    .to_string_lossy()
                    .to_string(),
                // Face anchor when we have one, otherwise slightly above centre.
                focal: p.focal.unwrap_or([0.5, 0.42]),
            })
            .collect(),
        caption: None,
    }
}
