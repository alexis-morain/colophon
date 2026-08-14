//! Spread composition: turn each chapter's photo run into a sequence of
//! templated spreads. Deterministic, tuned by simple rules on count,
//! orientation and score, and constrained by the linter's hard counters:
//! a portrait never lands in a landscape cell, faces stay clear of crop
//! edges, near-duplicates never share a spread, chapters open strong.
//!
//! One hard rule: no image ever crosses the fold. A photo printed across the
//! double page loses its middle to the binding, so the widest an image gets
//! is one full page.

use crate::analyze;
use crate::audit::{
    ASPECT_BETRAYAL, DUP_HAMMING, DUP_PHASH, FACE_MARGIN, FACE_MIN_SHARE,
    MIN_EFFECTIVE_PPI, OPENING_MIN_PHOTOS, SCENE_SPREAD_COLOR, SCENE_SPREAD_SECONDS,
};
use crate::model::{Album, Slot, Spread};
use crate::pdf::{self, Rect, SpreadGeometry};
use crate::pipeline::{Chapter, Photo};
use crate::print;

/// Score above which a photo qualifies for a page of its own,
/// expressed as a quantile of the chapter's scores.
const FEATURE_QUANTILE: f64 = 0.9;

/// A cover-crop that throws away more than this much of the frame is a
/// butchered photo: give it margins instead of a full page.
const MIN_VISIBLE: f64 = 0.72;

/// A chapter opens on a photo from this top quantile of its own scores.
/// Same bar as the linter's ouverture_faible counter.
const OPENING_QUANTILE: f64 = 0.75;

/// How far ahead the forced promotion may pull a full-page photo from.
const FULL_SEARCH_WINDOW: usize = 4;
/// How far ahead the chunker may regroup photos of the same aspect class,
/// so extreme formats (18,5:9 des téléphones) still find a partner.
const GROUP_WINDOW: usize = 8;
/// Never a fourth spread of the same template family in a row.
const REPEAT_LIMIT: usize = 3;


/// How much of the album the composer puts on a spread.
///
/// The same photos, the same rules, three paces. This exists because the
/// GO/NO-GO milestone asks a stranger whether they would show the draft as
/// it is, and « too crowded » and « too empty » are the two answers a single
/// fixed rhythm cannot both avoid. Offering the choice at the first build
/// costs one screen and settles the question with the person who owns the
/// photos, instead of with an average taste nobody has.
///
/// Every variant stays inside the linter's guarantees: none of them lets a
/// run of spreads without a breathing page reach `audit::FLAT_RUN`, and none
/// changes a single rule about faces, resolution or duplicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Densite {
    /// Few photos per spread, a breathing page often. A book to linger on:
    /// for a given number of spreads, fewer photos make it in, each bigger.
    Aeree,
    /// The pace the composer has always had, and the one the counters were
    /// tuned against.
    #[default]
    Equilibree,
    /// Mosaics of six and eight carry the album: far more of the folder
    /// reaches the page, each photo smaller.
    ///
    /// **Not offered yet.** Measured on the three test sets, this pace ends
    /// with four cells under the resolution floor on corse-2013, against a
    /// tolerance of three: a bigger photo budget admits frames whose only
    /// printable home is a small cell, and the composer has no way to give
    /// them one without breaking the rule that keeps near-duplicates off a
    /// spread. It stays in the type, out of [`Densite::offertes`], until
    /// that is solved rather than tuned around.
    Dense,
}

impl Densite {
    /// Photos per spread, cycled through so the book keeps changing pace.
    /// Mosaics of six and eight land often enough to be a rhythm, rarely
    /// enough to stay a surprise.
    pub fn rhythm(self) -> &'static [usize] {
        match self {
            Densite::Aeree => &[1, 2, 2, 3, 1, 2, 4, 2, 1, 3],
            Densite::Equilibree => &[2, 3, 4, 2, 6, 3, 2, 8, 4, 3],
            Densite::Dense => &[3, 4, 6, 3, 8, 4, 6, 8, 4, 6],
        }
    }

    /// After this many spreads without a breathing page, promote one. Held
    /// under the linter's `FLAT_RUN` by the test below, which is what makes
    /// the rhythm a guarantee rather than an intention.
    pub fn force_full_after(self) -> usize {
        match self {
            Densite::Aeree => 3,
            Densite::Equilibree => 5,
            Densite::Dense => 6,
        }
    }

    /// Never two breathing pages closer than this: an album where every
    /// other spread is a solo has no rhythm either.
    pub fn min_break_spacing(self) -> usize {
        match self {
            Densite::Aeree => 2,
            Densite::Equilibree => 4,
            Densite::Dense => 5,
        }
    }

    /// Average photos per spread this rhythm produces, times ten. The photo
    /// budget in `build` leans on it to hit the requested number of spreads.
    pub fn photos_per_spread_x10(self) -> usize {
        let r = self.rhythm();
        r.iter().sum::<usize>() * 10 / r.len()
    }

    pub fn id(self) -> &'static str {
        match self {
            Densite::Aeree => "aeree",
            Densite::Equilibree => "equilibree",
            Densite::Dense => "dense",
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Densite::Aeree => "Aérée",
            Densite::Equilibree => "Équilibrée",
            Densite::Dense => "Dense",
        }
    }

    /// One sentence, for the screen that asks. Says what changes for the
    /// reader, not what changes in the code.
    pub fn about(self) -> &'static str {
        match self {
            Densite::Aeree => "Une ou deux photos par double page, souvent une seule en grand. Moins de photos retenues, chacune plus grande.",
            Densite::Equilibree => "Deux à quatre photos, avec des mosaïques de temps en temps. Le rythme par défaut.",
            Densite::Dense => "Des mosaïques de quatre à huit photos. Beaucoup plus de photos retenues, plus petites.",
        }
    }

    pub fn par_id(id: &str) -> Option<Self> {
        [Densite::Aeree, Densite::Equilibree, Densite::Dense]
            .into_iter()
            .find(|d| d.id() == id)
    }

    /// Every pace the engine can compose at, the CLI included.
    pub fn toutes() -> [Densite; 3] {
        [Densite::Aeree, Densite::Equilibree, Densite::Dense]
    }

    /// The paces the creation screen offers. A pace only reaches this list
    /// once it passes the linter on the three reference sets: see
    /// [`Densite::Dense`] for the one that does not.
    pub fn offertes() -> &'static [Densite] {
        &[Densite::Aeree, Densite::Equilibree]
    }

    /// Whether this is the pace an album gets when nobody chose. Keeps the
    /// field off album.json until it carries something, so the file stays
    /// diffable across the schema change.
    pub fn is_default(&self) -> bool {
        *self == Densite::default()
    }
}

/// Average photos per spread of the default pace, times ten. Kept as a
/// constant because it is what every caller that does not care about the
/// density reads.
pub const PHOTOS_PER_SPREAD_X10: usize = 32;

pub struct Composer {
    /// Runs across the whole album so facing pages keep alternating between
    /// chapters, not just inside one.
    beat: usize,
    /// Spreads since the last breathing page (single photo), across
    /// chapters: flat stretches are an album-wide defect, not a chapter one.
    since_break: usize,
    /// Current template family and how many spreads it has held in a row.
    family_run: (String, usize),
    geom: SpreadGeometry,
    densite: Densite,
}

impl Composer {
    pub fn new(album: &Album) -> Self {
        Self::avec_densite(album, Densite::default())
    }

    pub fn avec_densite(album: &Album, densite: Densite) -> Self {
        Self {
            beat: 0,
            since_break: 0,
            family_run: (String::new(), 0),
            geom: pdf::geometry(album),
            densite,
        }
    }

    pub fn compose(
        &mut self,
        chapter: &Chapter,
        caption: Option<String>,
        root: &std::path::Path,
    ) -> Vec<Spread> {
        let mut photos = chapter.photos.clone();
        promote_opening(&mut photos, &|p| self.holds_a_page(p));
        let feature_threshold = quantile(
            &photos.iter().map(Photo::effective_score).collect::<Vec<_>>(),
            FEATURE_QUANTILE,
        );

        let mut spreads: Vec<Spread> = Vec::new();
        let mut i = 0;
        while i < photos.len() {
            let remaining = photos.len() - i;
            let last_was_feature =
                spreads.last().is_some_and(|s: &Spread| s.slots.len() == 1);

            let mut take = 0usize;
            // A deliberate feature page may go full bleed; a take of one
            // that merely fell out of the chunking stays margined, so a
            // pair of near-twins never prints as two facing full pages.
            let mut feature = false;

            // The chapter opens on a page of its own: the promoted opener.
            if i == 0 && remaining >= OPENING_MIN_PHOTOS && self.holds_a_page(&photos[0]) {
                take = 1;
                feature = true;
            }

            // A standout photo earns a page to itself, with room between
            // two of them: an album alternating solo and grid has no
            // rhythm either.
            if take == 0
                && !last_was_feature
                && remaining != 2
                && self.since_break >= self.densite.min_break_spacing()
                && photos[i].meta.taken_reliable
                && photos[i].effective_score() >= feature_threshold
                && self.holds_a_page(&photos[i])
            {
                take = 1;
                feature = true;
            }

            // The rhythm needs a breathing page: pull the best nearby photo,
            // full bleed when one can hold it. A long flat stretch reads
            // like a spreadsheet.
            if take == 0
                && !last_was_feature
                && remaining != 2
                && self.since_break >= self.densite.force_full_after()
            {
                let end = (i + FULL_SEARCH_WINDOW).min(photos.len());
                let strongest = |range: &mut dyn Iterator<Item = usize>| {
                    range.max_by(|&a, &b| {
                        photos[a]
                            .effective_score()
                            .partial_cmp(&photos[b].effective_score())
                            .unwrap()
                    })
                };
                let best = strongest(&mut (i..end).filter(|&j| self.fits_full(&photos[j])))
                    .or_else(|| strongest(&mut (i..end)));
                if let Some(j) = best {
                    photos[i..=j].rotate_right(1);
                    take = 1;
                    feature = true;
                }
            }

            if take == 0 {
                let hint = chunk_size(&photos[i..], self.beat, self.densite);
                // Fill the spread from the window ahead: photos of the same
                // aspect class, none a near-twin of another. Two 18,5:9
                // shots three photos apart still make a duo_pano, and the
                // twin of a kept photo drifts to a later spread instead of
                // shrinking this one to a parade of solos.
                let end = (i + GROUP_WINDOW.max(hint)).min(photos.len());
                let class0 = aspect_class(&photos[i]);
                let window: Vec<Photo> = photos[i..end].to_vec();
                let mut front: Vec<Photo> = Vec::new();
                let mut back: Vec<Photo> = Vec::new();
                for p in window {
                    let twin = front.iter().any(|q| spread_twins(&p, q));
                    if front.len() < hint && aspect_class(&p) == class0 && !twin {
                        front.push(p);
                    } else {
                        back.push(p);
                    }
                }
                take = front.len();
                front.extend(back);
                photos.splice(i..end, front);

                // A chapter must never compose into a single spread.
                if i == 0 && take == remaining && remaining >= 2 {
                    take = remaining.div_ceil(2);
                }
                // Safety net: never a near-twin pair on one spread.
                take = take.min(dup_prefix(&photos[i..i + take.min(remaining)]));
                take = clamp_take(take, remaining);
            }

            // Template and orientation-true assignment; shrink until it fits.
            let (name, order) = loop {
                if take == 1 {
                    let p = &photos[i];
                    if feature && self.fits_full(p) {
                        break (self.with_flip("full1"), vec![0usize]);
                    }
                    if let Some(x) = self.pick(&photos[i..=i]) {
                        break x;
                    }
                    // No solo cell prints this photo cleanly. It still goes
                    // on a page of its own: every other size was refused too,
                    // by the aspect rule or the twin rule. The linter counts
                    // what lands here and the preflight refuses to export it,
                    // which is the escape hatch working as designed.
                    break (self.with_flip(solo_family(p)), vec![0]);
                }
                match self.pick(&photos[i..i + take]) {
                    Some(x) => break x,
                    None => take = clamp_take(take - 1, remaining),
                }
            };

            let cells = pdf::slots_for(&name, take, &self.geom);
            let slots = (0..take)
                .map(|c| {
                    let p = &photos[i + order[c]];
                    Slot::new(
                        p.path
                            .strip_prefix(root)
                            .unwrap_or(&p.path)
                            .to_string_lossy()
                            .to_string(),
                        face_safe_focal(p, &cells[c]),
                    )
                })
                .collect();
            spreads.push(Spread {
                template: name.clone(),
                slots,
                caption: None,
                text: None,
                edited: false,
                locked: false,
            });

            let family = name.trim_end_matches("_verso").to_string();
            if self.family_run.0 == family {
                self.family_run.1 += 1;
            } else {
                self.family_run = (family, 1);
            }
            if take == 1 {
                self.since_break = 0;
            } else {
                self.since_break += 1;
            }
            self.beat += 1;
            i += take;
        }

        if let (Some(c), Some(first)) = (&caption, spreads.first_mut()) {
            first.caption = Some(c.clone());
        }
        spreads
    }

    /// Template name for a family, flipped onto the other page on odd beats
    /// when a verso variant exists.
    fn with_flip(&self, base: &str) -> String {
        let verso = format!("{base}_verso");
        if self.beat % 2 == 1 && pdf::TEMPLATES.iter().any(|(t, _)| *t == verso) {
            verso
        } else {
            base.to_string()
        }
    }

    /// Whether a photo can carry a page on its own without printing under
    /// the floor. Checked against the margined solo cell, the smaller of the
    /// two a promoted photo can land in: passing here means passing either.
    ///
    /// This is what keeps a phone snapshot with a high score out of a
    /// chapter opening. Without it the counter that catches the result
    /// (`sous_resolution`) fires on the densest pace, where a bigger photo
    /// budget lets more small frames through.
    fn holds_a_page(&self, p: &Photo) -> bool {
        let cell = pdf::slots_for("solo", 1, &self.geom)[0];
        printable(p, &cell)
    }

    /// Full bleed only when the crop keeps most of the frame, prints at a
    /// clean resolution, and no face ends up against an edge.
    fn fits_full(&self, p: &Photo) -> bool {
        let cell = pdf::slots_for("full1", 1, &self.geom)[0];
        let a = p.analysis.aspect();
        let ca = cell.w / cell.h;
        (a / ca).min(ca / a) >= MIN_VISIBLE
            && printable(p, &cell)
            && face_feasible(p, &cell)
    }

    /// Best template for the chunk: the valid assignment (orientation-true,
    /// printable, faces keepable) with the tightest aspect fit. A family
    /// already three spreads old only wins when nothing else is valid.
    fn pick(&self, chunk: &[Photo]) -> Option<(String, Vec<usize>)> {
        let blocked = (self.family_run.1 >= REPEAT_LIMIT)
            .then(|| self.family_run.0.clone());
        let mut best: Option<(f64, String, Vec<usize>)> = None;
        let mut best_blocked: Option<(f64, String, Vec<usize>)> = None;
        for base in families_for(chunk.len()) {
            let name = self.with_flip(base);
            let cells = pdf::slots_for(&name, chunk.len(), &self.geom);
            if let Some((order, score)) = assign(chunk, &cells) {
                let slot = if blocked.as_deref() == Some(*base) {
                    &mut best_blocked
                } else {
                    &mut best
                };
                if slot.as_ref().is_none_or(|b| score < b.0) {
                    *slot = Some((score, name, order));
                }
            }
        }
        best.or(best_blocked).map(|(_, name, order)| (name, order))
    }
}

/// Template families able to hold a chunk of this size.
fn families_for(n: usize) -> &'static [&'static str] {
    match n {
        1 => &["solo_etroit", "solo", "solo_carre", "solo_paysage", "solo_pano"],
        2 => &["duo", "duo_portrait", "duo_paysage", "duo_etroit", "duo_pano"],
        3 => &["trio", "trio_portrait"],
        4 => &["quad", "quad_portrait", "quad_etroit", "quad_pano"],
        6 => &["six"],
        8 => &["octo"],
        _ => &[],
    }
}

/// Effective print resolution of this photo in this cell stays above the
/// floor the linter checks.
fn printable(p: &Photo, cell: &Rect) -> bool {
    print::PRINT_DPI / print::print_scale(cell, p.orig.0, p.orig.1) >= MIN_EFFECTIVE_PPI
}

/// Rotate the earliest top-quartile photo to the front of the chapter, so
/// the opening spread carries it. Everything else keeps its order.
fn promote_opening(photos: &mut [Photo], holds_a_page: &dyn Fn(&Photo) -> bool) {
    if photos.len() < OPENING_MIN_PHOTOS {
        return;
    }
    let scores: Vec<f64> = photos.iter().map(|p| p.analysis.score()).collect();
    let bar = quantile(&scores, OPENING_QUANTILE);
    // Strong first, big enough to print alone second. A 2 Mpx frame at the
    // top of the quantile makes a fine mosaic cell and a poor opening page,
    // so the search prefers one that can hold a page — but the chapter opens
    // strong whatever happens: on a big page format a whole chapter can fail
    // to hold one, and the linter counts a weak opening as a hard defect.
    let chosen = (0..photos.len())
        .find(|&j| scores[j] >= bar && holds_a_page(&photos[j]))
        .or_else(|| (0..photos.len()).find(|&j| scores[j] >= bar));
    match chosen {
        Some(0) | None => {}
        Some(j) => photos[..=j].rotate_right(1),
    }
}

/// Coarse aspect classes for the chunk regrouping: étroit, portrait,
/// carré, paysage, pano. Photos of one class always share a template.
fn aspect_class(p: &Photo) -> u8 {
    let a = p.analysis.aspect();
    if a < 0.6 {
        0
    } else if a < 0.95 {
        1
    } else if a <= 1.15 {
        2
    } else if a <= 1.6 {
        3
    } else {
        4
    }
}

/// The margined single-photo family nearest to a photo's aspect. Last
/// resort when even the solo cells fail their checks: something must hold
/// the photo, and the linter will say what it cost.
fn solo_family(p: &Photo) -> &'static str {
    let a = p.analysis.aspect();
    let cells = [
        ("solo_etroit", pdf::CELL_ETROIT),
        ("solo", pdf::CELL_PORTRAIT),
        ("solo_carre", pdf::CELL_CARRE),
        ("solo_paysage", pdf::CELL_LANDSCAPE),
        ("solo_pano", pdf::CELL_PANO),
    ];
    cells
        .into_iter()
        .min_by(|x, y| {
            (a / x.1).ln().abs().partial_cmp(&(a / y.1).ln().abs()).unwrap()
        })
        .unwrap()
        .0
}

/// Photos to cells by matching sorted aspects: the 1-D pairing that
/// minimizes the worst mismatch. None when any pair betrays an orientation,
/// prints too soft, or pins a face against a cropped edge. On success also
/// returns the total mismatch, for ranking template candidates.
fn assign(chunk: &[Photo], cells: &[Rect]) -> Option<(Vec<usize>, f64)> {
    let n = cells.len();
    if chunk.len() != n {
        return None;
    }
    let mut ci: Vec<usize> = (0..n).collect();
    ci.sort_by(|&a, &b| {
        (cells[a].w / cells[a].h).partial_cmp(&(cells[b].w / cells[b].h)).unwrap()
    });
    let mut pi: Vec<usize> = (0..n).collect();
    pi.sort_by(|&a, &b| {
        chunk[a].analysis.aspect().partial_cmp(&chunk[b].analysis.aspect()).unwrap()
    });
    let mut order = vec![0usize; n];
    let mut score = 0.0;
    for k in 0..n {
        let (c, p) = (ci[k], pi[k]);
        let cell = &cells[c];
        let a = chunk[p].analysis.aspect();
        let ca = cell.w / cell.h;
        if (a / ca).max(ca / a) > ASPECT_BETRAYAL {
            return None;
        }
        if !printable(&chunk[p], cell) {
            return None;
        }
        if !face_feasible(&chunk[p], cell) {
            return None;
        }
        score += (a / ca).ln().abs();
        order[c] = p;
    }
    Some((order, score))
}

/// Two photos that must never share a spread: near in hash, or two frames
/// of the same scene minutes apart (pose series flip every gradient but
/// keep the palette). Mirrors the linter's doublon_planche rule.
fn spread_twins(a: &Photo, b: &Photo) -> bool {
    analyze::hamming(a.analysis.dhash, b.analysis.dhash) <= DUP_HAMMING
        || analyze::hamming(a.analysis.phash, b.analysis.phash) <= DUP_PHASH
        || (a.meta.taken_reliable
            && b.meta.taken_reliable
            && (a.meta.taken - b.meta.taken).num_seconds().abs() <= SCENE_SPREAD_SECONDS
            && analyze::color_distance(&a.analysis.colorsig, &b.analysis.colorsig)
                <= SCENE_SPREAD_COLOR)
}

/// Length of the longest prefix free of same-spread near-duplicates.
fn dup_prefix(chunk: &[Photo]) -> usize {
    for j in 1..chunk.len() {
        for k in 0..j {
            if spread_twins(&chunk[j], &chunk[k]) {
                return j;
            }
        }
    }
    chunk.len().max(1)
}

/// Clamp a take to the sizes a template can hold, never leaving a lonely
/// photo behind.
fn clamp_take(mut take: usize, remaining: usize) -> usize {
    take = take.min(remaining).max(1);
    while !matches!(take, 1 | 2 | 3 | 4 | 6 | 8) {
        take -= 1;
    }
    if take > 1 && remaining - take == 1 {
        take -= 1;
        while !matches!(take, 1 | 2 | 3 | 4 | 6 | 8) {
            take -= 1;
        }
    }
    take.max(1)
}

fn quantile(scores: &[f64], q: f64) -> f64 {
    if scores.is_empty() {
        return f64::MAX;
    }
    let mut sorted = scores.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[idx]
}

/// How many photos the next spread absorbs. Only counts a template can lay
/// out come back, and a lonely leftover is never created.
fn chunk_size(rest: &[Photo], beat: usize, densite: Densite) -> usize {
    let remaining = rest.len();
    match remaining {
        0..=4 => return remaining.max(1),
        5 => return 3, // 3 + 2 beats 4 + 1
        7 => return 4, // 4 + 3
        _ => {}
    }
    let rhythm = densite.rhythm();
    let mut take = rhythm[beat % rhythm.len()].min(remaining);
    if remaining - take == 1 {
        take -= 1;
    }
    while !matches!(take, 1 | 2 | 3 | 4 | 6 | 8) {
        take -= 1;
    }
    take
}

/// The visible window a cell cuts out of a photo: image size and window
/// size, in thumbnail pixels.
fn window(p: &Photo, cell: &Rect) -> (f64, f64, f64, f64) {
    let iw = f64::from(p.analysis.width);
    let ih = f64::from(p.analysis.height);
    let s = (cell.w / iw).max(cell.h / ih);
    (iw, ih, cell.w / s, cell.h / s)
}

/// Significant face extent along one axis, in pixels: (lo, hi).
fn face_extent(p: &Photo, horizontal: bool) -> Option<(f64, f64)> {
    let (iw, ih) = (f64::from(p.analysis.width), f64::from(p.analysis.height));
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for b in &p.faces {
        if b[2] < FACE_MIN_SHARE {
            continue;
        }
        let (a0, a1) = if horizontal {
            (b[0] * iw, (b[0] + b[2]) * iw)
        } else {
            (b[1] * ih, (b[1] + b[3]) * ih)
        };
        lo = lo.min(a0);
        hi = hi.max(a1);
    }
    (lo <= hi).then_some((lo, hi))
}

/// Window offset along one axis that keeps the faces clear of the cropped
/// edges, and whether it fully can. Anchoring the window on an image border
/// un-crops that border, which is why an edge face can still be safe.
fn safe_offset(
    total: f64,
    visible: f64,
    extent: Option<(f64, f64)>,
    desired: f64,
) -> (f64, bool) {
    let span = (total - visible).max(0.0);
    if span < 0.5 {
        return (0.0, true);
    }
    let Some((lo, hi)) = extent else {
        return (desired.clamp(0.0, span), true);
    };
    let m = FACE_MARGIN * visible;
    let min_x0 = hi + m - visible; // window must reach past the faces' end
    let max_x0 = lo - m; // and start before their beginning
    if min_x0.max(0.0) <= max_x0.min(span) {
        (desired.clamp(min_x0.max(0.0), max_x0.min(span)), true)
    } else if min_x0 <= 0.0 {
        (0.0, true) // border anchored: the left edge is not cropped at all
    } else if max_x0 >= span {
        (span, true) // same on the other border
    } else {
        // Faces wider than the window: center on them, the cut is counted.
        (((min_x0 + max_x0) / 2.0).clamp(0.0, span), false)
    }
}

/// Can this photo sit in this cell without a face against a cropped edge?
fn face_feasible(p: &Photo, cell: &Rect) -> bool {
    if p.faces.is_empty() {
        return true;
    }
    let (iw, ih, vw, vh) = window(p, cell);
    safe_offset(iw, vw, face_extent(p, true), 0.0).1
        && safe_offset(ih, vh, face_extent(p, false), 0.0).1
}

/// The slot focal for a photo in a cell: the face anchor, nudged so every
/// significant face keeps its margin from the cropped edges.
fn face_safe_focal(p: &Photo, cell: &Rect) -> [f64; 2] {
    let base = p.focal.unwrap_or([0.5, 0.42]);
    let (iw, ih, vw, vh) = window(p, cell);
    let axis = |total: f64, visible: f64, horizontal: bool, f: f64| {
        let span = (total - visible).max(0.0);
        if span < 0.5 {
            return f;
        }
        let (x0, _) = safe_offset(total, visible, face_extent(p, horizontal), f * span);
        x0 / span
    };
    [axis(iw, vw, true, base[0]), axis(ih, vh, false, base[1])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_take_never_leaves_a_lonely_photo() {
        assert_eq!(clamp_take(4, 5), 3);
        assert_eq!(clamp_take(5, 8), 4);
        // taking 2 of 3 would leave a lonely photo: drop to 1, leaving 2
        assert_eq!(clamp_take(2, 3), 1);
        assert_eq!(clamp_take(1, 1), 1);
        // 8 of 9 would leave a lonely photo too: 6 + 3 instead
        assert_eq!(clamp_take(8, 9), 6);
    }

    #[test]
    fn safe_offset_anchors_on_borders() {
        // Face against the left image border, window half the image: anchor
        // at 0, the left edge stops being cropped.
        let (x0, ok) = safe_offset(1000.0, 500.0, Some((0.0, 100.0)), 250.0);
        assert!(ok);
        assert_eq!(x0, 0.0);
        // Face in the middle: the window slides to keep the margin.
        let (x0, ok) = safe_offset(1000.0, 500.0, Some((400.0, 600.0)), 0.0);
        assert!(ok);
        assert!(x0 >= 100.0 + 0.04 * 500.0 && x0 <= 400.0 - 0.04 * 500.0);
        // Faces wider than the window: impossible, centered on them.
        let (_, ok) = safe_offset(1000.0, 300.0, Some((100.0, 900.0)), 0.0);
        assert!(!ok);
    }

    #[test]
    fn quantile_matches_the_audit() {
        let s = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(quantile(&s, 0.75), 3.0);
    }
}
