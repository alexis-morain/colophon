//! Spread composition: turn each chapter's photo run into a sequence of
//! templated spreads. Deterministic, tuned by simple rules on count,
//! orientation and score.

use crate::model::{Slot, Spread};
use crate::pipeline::{Chapter, Photo};

/// Score above which a landscape photo qualifies for a hero spread,
/// expressed as a quantile of the chapter's scores.
const HERO_QUANTILE: f64 = 0.85;

pub fn compose(chapter: &Chapter, caption: Option<String>, root: &std::path::Path) -> Vec<Spread> {
    let photos = &chapter.photos;
    let mut spreads = Vec::new();
    let hero_threshold = quantile_score(photos, HERO_QUANTILE);

    let mut i = 0;
    let mut first = true;
    while i < photos.len() {
        let remaining = photos.len() - i;
        let p = &photos[i];

        // A strong landscape deserves the full spread, but never two heroes in a row.
        let last_was_hero = spreads
            .last()
            .is_some_and(|s: &Spread| s.template == "hero");
        if !p.analysis.is_portrait()
            && p.meta.taken_reliable
            && p.effective_score() >= hero_threshold
            && !last_was_hero
            && remaining >= 1
        {
            spreads.push(spread("hero", &photos[i..i + 1], root));
            i += 1;
        } else {
            let take = chunk_size(&photos[i..], remaining);
            let template = template_for(&photos[i..i + take]);
            spreads.push(spread(template, &photos[i..i + take], root));
            i += take;
        }

        if first {
            if let Some(c) = &caption {
                spreads.last_mut().unwrap().caption = Some(c.clone());
            }
            first = false;
        }
    }
    spreads
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

/// Pick how many photos the next spread absorbs. Avoid a lonely leftover.
fn chunk_size(rest: &[Photo], remaining: usize) -> usize {
    let base = match remaining {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 3, // 3 + 2 beats 4 + 1
        _ => {
            // vary rhythm: portrait pairs like a duo, otherwise cycle 2/3/4
            let portraits = rest.iter().take(4).filter(|p| p.analysis.is_portrait()).count();
            if portraits >= 2 {
                2
            } else {
                match rest.len() % 3 {
                    0 => 3,
                    1 => 4,
                    _ => 2,
                }
            }
        }
    };
    base.min(remaining)
}

fn template_for(chunk: &[Photo]) -> &'static str {
    match chunk.len() {
        1 => {
            if chunk[0].analysis.is_portrait() {
                "solo" // centered with margins: a portrait must not be butchered to fill a spread
            } else {
                "full2_single"
            }
        }
        2 => "duo",
        3 => "trio",
        _ => "quad",
    }
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
                focal: [0.5, 0.42], // slightly above centre: better default for people
            })
            .collect(),
        caption: None,
    }
}
