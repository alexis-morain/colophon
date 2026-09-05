//! What one spread holds, as objects.
//!
//! A spread used to be walked five separate times: the emitter drew it, the
//! print pass sized its JPEGs from it, the linter counted defects on it, and
//! the preflight measured its caption against the trim — each rebuilding the
//! same rectangles and each knowing, separately, that `garde`, `texte` and
//! `colophon` are special. A scene is that walk done once.
//!
//! **A scene is derived, never stored.** `album.json` carries a template and
//! its slots; everything a template generates is reconstructible from them, so
//! writing it down would create a second source of truth for a rectangle the
//! template already owns. Nothing here reaches the file, and nothing here
//! needs a migration. The first thing ever stored will be the free objects of
//! wave 6, precisely because no template can produce them.
//!
//! **The order is the depth.** Objects come out back to front, in the exact
//! order [`crate::pdf::PdfWriter::add_spread`] emits them: that is what lets a
//! port of the emitter prove it displaced nothing.
//!
//! **The reading order is not the depth**, and it is not a heuristic either.
//! [`crate::gabarit::Ordre`] already declares it — `ParPage` reads one page
//! then the other, `ParRangee` reads row by row across the spread — so the
//! slot order *is* the reading order, verified across all 387 templates the
//! dump offers. The accessibility layer of 2.4 gets its tab order from here
//! rather than inventing one over a canvas that has none.
//!
//! **A role is a code, never a sentence.** Strings born in the engine stay in
//! the language they were born in; an interface that has to say "photo 3 of 4"
//! in two languages needs the 3, the 4 and the role, not the sentence.

use crate::gabarit;
use crate::model::Spread;
use crate::pdf::{self, Point, Rect, SpreadGeometry};
use serde::Serialize;

/// Everything visible on one spread, back to front. The index is the depth:
/// object `n` prints over object `n - 1`, exactly as the content stream lays
/// them down.
#[derive(Debug, Clone, Serialize)]
pub struct Scene {
    pub objects: Vec<Object>,
}

/// One visible element. It carries where it is, how it is turned, when it is
/// read, and what it is — and nothing else.
///
/// **An angle and an origin, never a matrix.** The day of the free objects
/// arrived (wave 6, decision of 04/09) and with it the oriented box the
/// earlier version of this comment refused. What it does not bring is a
/// general transform: everything that measures an object has to recover its
/// four corners, and a matrix would let a shear in through the same door.
/// Everything a template produces is upright, and [`corners`] returns its
/// rectangle untouched — bit for bit, which is the whole reason the reduction
/// is provable rather than merely plausible.
#[derive(Debug, Clone, Serialize)]
pub struct Object {
    /// Millimetres, origin bottom-left of the media box, like every rectangle
    /// the engine computes. For one of the three text *pages* this is the
    /// measured ink, not the placement proxy — see [`Scene::of`]. For a free
    /// object it is the box the reader drew, because that is the thing they
    /// placed and the thing every guard has to measure.
    pub rect: Rect,
    /// Degrees, counter-clockwise, around the centre of `rect`. Zero for
    /// everything a template produces.
    #[serde(default, skip_serializing_if = "est_nul")]
    pub angle: f64,
    /// Rank in the reading order, which is not the depth. Derived from the
    /// template's own slot order.
    pub reading: usize,
    pub role: Role,
}

fn est_nul(v: &f64) -> bool {
    *v == 0.0
}

impl Object {
    /// The four corners of the object, rotation included: bottom-left,
    /// bottom-right, top-right, top-left before the angle is applied.
    pub fn corners(&self) -> [Point; 4] {
        corners(&self.rect, self.angle)
    }
}

/// The four corners of an oriented rectangle, counter-clockwise from the
/// bottom-left, in the engine's frame.
///
/// **The upright case returns the rectangle's own numbers**, and that is not
/// an optimisation: `(x + w/2) - w/2` is not `x` in binary floating point, so
/// routing an upright object through the rotation would move it by an ulp
/// and quietly change what every album has always measured against the cut.
pub fn corners(r: &Rect, angle_deg: f64) -> [Point; 4] {
    let coins = [
        (r.x, r.y),
        (r.x + r.w, r.y),
        (r.x + r.w, r.y + r.h),
        (r.x, r.y + r.h),
    ];
    if angle_deg == 0.0 {
        return coins.map(|(x, y)| Point { x, y });
    }
    let centre = Point { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 };
    coins.map(|(x, y)| tourner(Point { x, y }, centre, angle_deg))
}

/// One point turned around another, counter-clockwise, in the engine's frame.
///
/// The one place the rotation is written down. The emitter turns a baseline
/// with it, [`corners`] turns the four corners with it, and `scene.ts` mirrors
/// it once with the sign the screen's downward y demands — so a preview that
/// turned the other way would be a bug in one line, not in four files.
pub fn tourner(p: Point, centre: Point, angle_deg: f64) -> Point {
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    let (dx, dy) = (p.x - centre.x, p.y - centre.y);
    Point { x: centre.x + dx * cos - dy * sin, y: centre.y + dx * sin + dy * cos }
}

/// The centre a free object turns around: the middle of its box.
pub fn centre(r: &Rect) -> Point {
    Point { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 }
}

/// Whether an oriented rectangle runs across the fold.
///
/// The fold is the middle of the media box. No image has ever crossed it —
/// the doctrine is older than this module — and a free object does not
/// either. The editor stops a gesture with this, and the preflight refuses
/// with it (`objet_pli`, wave 6.4): the editor butts, so a block astride the
/// fold can only come from an `album.json` repaired by hand, which is exactly
/// what a preflight is for.
pub fn traverse_le_pli(r: &Rect, angle_deg: f64, g: &SpreadGeometry) -> bool {
    let pli = g.media_w / 2.0;
    let xs = corners(r, angle_deg).map(|p| p.x);
    let min = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    min < pli && max > pli
}

/// Whether an oriented rectangle leaves the safe zone.
///
/// **The margin is soft**, where the fold is hard: a block deliberately set
/// to bleed is a choice, so this warns and never refuses. The editor shows it
/// under the hand (`scene.ts::horsMarge`), the linter counts what an album
/// slipped past anyway (`objet_hors_marge`), and the two have to agree — so
/// there is one function, ported once, exactly as [`traverse_le_pli`] is.
///
/// The zone is the one the engine already anchors a chapter caption in:
/// [`crate::pdf::CAPTION_SAFE`] of the margin, inside the cut. Reusing it
/// rather than declaring a second number is the point — a block and a caption
/// are both ink that must survive the guillotine, and two safe zones on one
/// spread would be two answers to one question.
pub fn hors_marge(r: &Rect, angle_deg: f64, g: &SpreadGeometry) -> bool {
    distance_to_trim(r, angle_deg, g) < marge_sure(g)
}

/// How much clearance the safe zone asks for, in millimetres. Named because
/// a counter that reports a distance has to report what it was short of.
pub fn marge_sure(g: &SpreadGeometry) -> f64 {
    pdf::CAPTION_SAFE * g.margin
}

/// What an object is, with what the interface needs to name it.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Role {
    /// A photograph in its cell, framed by its focal point and manual zoom.
    Photo { cell: usize, src: String, focal: [f64; 2], zoom: f64 },
    /// The caption printed under one photo. `at` is the baseline: a set line
    /// is placed by where it sits, not by the box its ink happens to cover,
    /// and neither derives from the other without re-encoding the convention.
    PhotoCaption { cell: usize, text: String, at: Point },
    /// The chapter caption of the spread, at its baseline.
    ChapterCaption { text: String, at: Point },
    /// A block of set text: the half-title, a text page, the colophon. The
    /// three used to be three special cases in every renderer; here they are
    /// one role whose lines are already laid out, each `dy_mm` below `at`.
    Text { at: Point, lines: Vec<Line> },
    /// A block of text the reader placed and may have turned. Not the same
    /// role as [`Role::Text`], and deliberately: those three are pages the
    /// engine composes, this one is an object the reader owns, indexed into
    /// `spread.objets` the way a photograph is indexed into `spread.slots`.
    ///
    /// `at` is the first baseline **in the object's own upright frame**. The
    /// renderer turns the whole box around its centre and then draws upright
    /// inside it: one transform per object, and no line that has to know
    /// about the angle.
    FreeText {
        index: usize,
        at: Point,
        lines: Vec<Line>,
        align: crate::model::Alignement,
        /// The set text is taller than the box it was drawn in. Signalled,
        /// never cut: what does not fit runs past the bottom and says so.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        overflow: bool,
        /// One word is wider than the box. It prints past the edge rather
        /// than being broken, because breaking a word is a decision about
        /// someone's language and this engine does not take it.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        trop_large: bool,
    },
}

/// Cut a text into the lines it sets as, inside a box `largeur` wide.
///
/// A typed newline is a hard break and an empty paragraph keeps its turn —
/// same convention as a text page. Inside a paragraph the break is greedy and
/// at word boundaries, measured in the face the album is set in, which is why
/// the measure arrives as an argument here exactly as it does everywhere else
/// in this module.
///
/// **A word wider than the box is put on its line whole.** Hyphenating is a
/// decision about a language; the caller is told instead.
pub fn replier(
    texte: &str,
    largeur: f64,
    taille_pt: f64,
    mesure: &dyn Fn(&str, f64) -> f64,
) -> (Vec<String>, bool) {
    let mut lignes: Vec<String> = Vec::new();
    let mut trop_large = false;
    for para in texte.split('\n') {
        let mots: Vec<&str> = para.split(' ').filter(|m| !m.is_empty()).collect();
        if mots.is_empty() {
            // The blank line of a stored text is spacing, and here it has to
            // become a real line to take its turn: a wrapped block counts its
            // leading off the lines it produced, not off the ones that were
            // typed, so dropping the blank would close the gap the reader
            // left. That is the one place this parts from a text page.
            lignes.push(String::new());
            continue;
        }
        let mut courante = String::new();
        for mot in mots {
            if courante.is_empty() {
                courante = mot.to_string();
                continue;
            }
            let candidat = format!("{courante} {mot}");
            if mesure(&candidat, taille_pt) <= largeur {
                courante = candidat;
            } else {
                trop_large |= mesure(&courante, taille_pt) > largeur;
                lignes.push(std::mem::take(&mut courante));
                courante = mot.to_string();
            }
        }
        trop_large |= mesure(&courante, taille_pt) > largeur;
        lignes.push(courante);
    }
    (lignes, trop_large)
}

/// One line of a text block, already placed: `dy_mm` grows downward from the
/// block's first baseline, the way [`crate::garde::Ligne`] already does, and
/// `dx_mm` runs rightward from its left edge.
///
/// `dx_mm` exists for the alignment of a free block, and it is computed here
/// rather than in each renderer for the reason everything in this module is
/// computed here: a line the canvas centred and the PDF did not would be a
/// preview that lies. It is zero for the three text pages, which have never
/// been anything but left-aligned.
#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub text: String,
    pub size_pt: f64,
    pub dy_mm: f64,
    #[serde(default, skip_serializing_if = "est_nul")]
    pub dx_mm: f64,
}

/// Ink of one set line: the measured width, and the vertical box the engine
/// has always used around a baseline.
fn ink_box(
    x: f64,
    baseline: f64,
    text: &str,
    size_pt: f64,
    mesure: &dyn Fn(&str, f64) -> f64,
) -> Rect {
    let size_mm = size_pt / (72.0 / 25.4);
    Rect {
        x,
        y: baseline - size_mm * 0.3,
        w: mesure(text, size_pt),
        h: size_mm * 1.35,
    }
}


/// The union of two boxes.
fn union(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect {
        x,
        y,
        w: (a.x + a.w).max(b.x + b.w) - x,
        h: (a.y + a.h).max(b.y + b.h) - y,
    }
}

impl Scene {
    /// The scene of one spread.
    ///
    /// The emission order is the emitter's: photographs, then their captions,
    /// then the text block, then the chapter caption. A caller that needs the
    /// rectangles of a template it has not chosen yet — the composer trying a
    /// candidate, the geometry dump describing the catalogue — wants
    /// [`crate::pdf::slots_for`] instead; that question is about a template,
    /// this one is about a spread.
    ///
    /// A template the catalogue does not know still renders: `slots_for`
    /// falls back to one margined box, and so does this.
    pub fn of(spread: &Spread, g: &SpreadGeometry) -> Self {
        Self::of_avec(spread, g, &crate::font::text_width_mm)
    }

    /// The same scene under a caller-supplied measure, which is how the
    /// album's own face reaches the one place a set line is laid out.
    ///
    /// The sibling exists for the reason [`crate::garde::mise_en_page_avec`]
    /// exists, and the TypeScript port has taken its measure as an argument
    /// since it was written: what a line is worth depends on the face, and
    /// the face is a property of the album, not of this module. Everything
    /// that reasons about a spread without rendering it — the linter, the
    /// preflight, the geometry dump — keeps [`Scene::of`] and the face this
    /// crate ships.
    pub fn of_avec(
        spread: &Spread,
        g: &SpreadGeometry,
        mesure: &dyn Fn(&str, f64) -> f64,
    ) -> Self {
        let rects = pdf::slots_for(&spread.template, spread.slots.len(), g);

        let mut objects = Vec::with_capacity(spread.slots.len() + 2);

        for (cell, (slot, rect)) in spread.slots.iter().zip(rects.iter()).enumerate() {
            objects.push(Object {
                angle: 0.0,
                rect: *rect,
                reading: cell,
                role: Role::Photo {
                    cell,
                    src: slot.src.clone(),
                    focal: slot.focal,
                    zoom: slot.zoom,
                },
            });
        }
        // The reading rank of everything below continues past the photographs:
        // a caption is read after the picture it names, and what belongs to
        // the whole spread comes last.
        let mut reading = spread.slots.len();

        for (cell, (slot, rect)) in spread.slots.iter().zip(rects.iter()).enumerate() {
            let Some(text) = &slot.caption else { continue };
            if text.is_empty() {
                continue;
            }
            let baseline = rect.y - pdf::PHOTO_CAPTION_DROP_MM;
            let at = Point { x: rect.x, y: baseline };
            objects.push(Object {
                angle: 0.0,
                rect: ink_box(at.x, at.y, text, pdf::PHOTO_CAPTION_SIZE_PT, mesure),

                reading,
                role: Role::PhotoCaption { cell, text: text.clone(), at },
            });
            reading += 1;
        }

        if let Some(text) = &spread.text {
            if let Some(block) = text_block(spread, text, g, mesure) {

                objects.push(Object { reading, ..block });
                reading += 1;
            }
        }

        if let Some(text) = &spread.caption {
            let at = pdf::caption_anchor(&spread.template, &rects, g);
            objects.push(Object {
                angle: 0.0,
                rect: ink_box(at.x, at.y, text, pdf::SPREAD_CAPTION_SIZE_PT, mesure),

                reading,
                role: Role::ChapterCaption { text: text.clone(), at },
            });
            reading += 1;
        }

        // The free objects come last, so they are on top: the order is the
        // depth here as everywhere, and what the reader placed by hand covers
        // what the template produced. Their own order among themselves is the
        // order they sit in on the spread, which is the only depth they have.
        for (index, objet) in spread.objets.iter().enumerate() {
            objects.push(objet_libre(index, objet, reading, mesure));
            reading += 1;
        }

        Scene { objects }
    }

    /// How far the closest edge of every object sits from the guillotine, in
    /// millimetres, and which object that is. Negative means the cut runs
    /// through it.
    ///
    /// One implementation of the doctrine instead of one per caller: the
    /// preflight measured this on its own for the caption alone, and the
    /// linter did not measure it at all.
    pub fn closest_to_trim(&self, g: &SpreadGeometry) -> Option<(usize, f64)> {
        self.objects
            .iter()
            .enumerate()
            .map(|(i, o)| (i, distance_to_trim(&o.rect, o.angle, g)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }
}

/// Distance from an oriented rectangle to the trimmed edge, in millimetres.
/// What must survive the cut is measured from the cut, never from the media
/// edge. Negative means the cut runs through it.
///
/// **One function, angle included**, rather than one for upright boxes and
/// one for turned ones: this module exists because the same doctrine used to
/// have four implementations, and a second distance to the guillotine would
/// be the first step back. An upright object reduces to the four numbers this
/// function has always returned — not close, equal, because [`corners`] hands
/// back the rectangle's own coordinates and `min` is exact.
pub fn distance_to_trim(r: &Rect, angle_deg: f64, g: &SpreadGeometry) -> f64 {
    corners(r, angle_deg)
        .iter()
        .map(|p| {
            (p.x - g.bleed)
                .min(p.y - g.bleed)
                .min((g.media_w - g.bleed) - p.x)
                .min((g.media_h - g.bleed) - p.y)
        })
        .fold(f64::INFINITY, f64::min)
}

/// The one text block a spread may carry, whichever of the three pages it is.
fn text_block(
    spread: &Spread,
    text: &str,
    g: &SpreadGeometry,
    mesure: &dyn Fn(&str, f64) -> f64,
) -> Option<Object> {
    let (at, lines) = if spread.template == crate::garde::TEMPLATE {
        let at = crate::garde::anchor(g);
        let place = crate::garde::place(g);
        // The half-title shrinks its title until it fits, so the size it
        // prints at is a function of the face: laid out here, in the face
        // the document will actually be set in.
        let lines = crate::garde::mise_en_page_avec(text, place, mesure)
            .into_iter()
            .map(|l| Line { text: l.texte, size_pt: l.taille_pt, dy_mm: l.dy_mm, dx_mm: 0.0 })
            .collect::<Vec<_>>();
        (at, lines)
    } else {
        let colophon = spread.template == crate::colophon::TEMPLATE;
        let at = if colophon { pdf::colophon_anchor(g) } else { pdf::text_anchor(g) };
        let (size_pt, leading) = if colophon {
            (crate::colophon::SIZE_PT, crate::colophon::LEADING_MM)
        } else {
            (pdf::TEXT_SIZE_PT, pdf::TEXT_LEADING_MM)
        };
        // An empty line prints nothing and still takes its turn: the blank
        // line of a stored text is spacing, and the index is what spaces it.
        let lines = text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.is_empty())
            .map(|(i, l)| Line {
                text: l.to_string(),
                size_pt,
                dy_mm: i as f64 * leading,
                dx_mm: 0.0,
            })
            .collect::<Vec<_>>();
        (at, lines)
    };

    let rect = lines
        .iter()
        .map(|l| ink_box(at.x, at.y - l.dy_mm, &l.text, l.size_pt, mesure))

        .reduce(union)?;
    Some(Object { rect, angle: 0.0, reading: 0, role: Role::Text { at, lines } })
}

/// Where a line sits inside the box it was wrapped to, given its alignment.
///
/// One function rather than a `match` copied into the assembler, the dump and
/// the port: a line the emitter centred and a renderer did not is exactly the
/// class of bug this module exists to make impossible.
pub fn decalage(alignement: crate::model::Alignement, boite: f64, ligne: f64) -> f64 {
    match alignement {
        crate::model::Alignement::Gauche => 0.0,
        crate::model::Alignement::Centre => (boite - ligne) / 2.0,
        crate::model::Alignement::Droite => boite - ligne,
    }
}

/// One free object, laid out inside the box the reader drew.
///
/// Everything here happens in the object's own upright frame; the angle rides
/// on the object and the renderer applies it once, around the box's centre.
/// The first baseline sits one type size below the top edge, so a line's caps
/// stand inside the box rather than on it.
fn objet_libre(
    index: usize,
    objet: &crate::model::Objet,
    reading: usize,
    mesure: &dyn Fn(&str, f64) -> f64,
) -> Object {
    let rect = Rect { x: objet.x, y: objet.y, w: objet.w, h: objet.h };
    let crate::model::Contenu::Texte { texte, taille_pt, alignement, .. } = &objet.contenu;
    let taille_pt = *taille_pt;
    let interligne = objet.interligne();
    let taille_mm = taille_pt / (72.0 / 25.4);

    let (textes, trop_large) = replier(texte, objet.w, taille_pt, mesure);
    let lines: Vec<Line> = textes
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let dx_mm = decalage(*alignement, objet.w, mesure(t, taille_pt));
            Line { text: t.clone(), size_pt: taille_pt, dy_mm: i as f64 * interligne, dx_mm }
        })
        .collect();

    // The block's set height: the drop to the last baseline, plus the line box
    // that baseline carries. Taller than the box means the text runs past the
    // bottom edge — which it is allowed to do, out loud.
    let hauteur = lines.last().map_or(0.0, |l| l.dy_mm) + taille_mm * 1.35;
    let overflow = hauteur > objet.h + 1e-9;

    let at = Point { x: objet.x, y: objet.y + objet.h - taille_mm };
    Object {
        rect,
        angle: objet.angle,
        reading,
        role: Role::FreeText {
            index,
            at,
            lines,
            align: *alignement,
            overflow,
            trop_large,
        },
    }
}

/// The scene of every spread of an album, in order.
pub fn album(album: &crate::model::Album) -> Vec<Scene> {
    let g = pdf::geometry(album);
    album.spreads.iter().map(|s| Scene::of(s, &g)).collect()
}

/// Every template the catalogue offers, as `(name, capacity)`, for the
/// exhaustive parity test and the dump.
pub fn offered() -> Vec<(&'static str, usize)> {
    gabarit::offerts().iter().map(|s| (s.nom, s.capacite)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Album, Alignement, Contenu, Objet, Size, Slot};

    /// A measure that does not need a face: one millimetre per character per
    /// point of size, scaled so the numbers are easy to reason about. The
    /// wrapping tests are about where the break falls, not about a typeface.
    fn regle(s: &str, taille_pt: f64) -> f64 {
        s.chars().count() as f64 * taille_pt * 0.2
    }

    fn bloc(texte: &str, w: f64, h: f64) -> Objet {
        Objet {
            x: 20.0,
            y: 20.0,
            w,
            h,
            angle: 0.0,
            contenu: Contenu::Texte {
                texte: texte.into(),
                taille_pt: 10.0,
                interligne_mm: Some(5.0),
                alignement: Alignement::Gauche,
            },
        }
    }

    fn geom() -> SpreadGeometry {
        SpreadGeometry { media_w: 426.0, media_h: 216.0, margin: 14.0, gutter: 7.0, bleed: 3.0 }
    }

    fn spread(template: &str, n: usize) -> Spread {
        Spread {
            template: template.into(),
            slots: (0..n).map(|i| Slot::new(format!("{i}.jpg"), [0.5, 0.42])).collect(),
            caption: None,
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        }
    }

    /// The load-bearing claim of the whole session: for every template the
    /// catalogue offers, at every photo count, on every page shape, the
    /// scene's photo rectangles are the ones `slots_for` has always produced.
    /// Not close — equal.
    #[test]
    fn les_rectangles_de_la_scene_sont_ceux_du_moteur() {
        let formats: [(f64, f64); 6] = [
            (210.0, 210.0),
            (300.0, 300.0),
            (280.0, 210.0),
            (297.0, 210.0),
            (200.0, 250.0),
            (210.0, 297.0),
        ];
        let mut compares = 0usize;
        for (w, h) in formats {
            let mut a = Album::new("t", std::path::Path::new("."), Size { w, h });
            a.bleed_mm = 3.0;
            let g = pdf::geometry(&a);
            for (nom, capacite) in offered() {
                for n in 1..=capacite.max(1) {
                    let attendu = pdf::slots_for(nom, n, &g);
                    let scene = Scene::of(&spread(nom, n), &g);
                    let obtenu: Vec<Rect> = scene
                        .objects
                        .iter()
                        .filter_map(|o| match o.role {
                            Role::Photo { .. } => Some(o.rect),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(
                        obtenu.len(),
                        attendu.len(),
                        "{nom} à {n} photos sur {w}×{h} : {} objets pour {} cases",
                        obtenu.len(),
                        attendu.len()
                    );
                    for (i, (got, want)) in obtenu.iter().zip(attendu.iter()).enumerate() {
                        assert!(
                            (got.x - want.x).abs() < 1e-9
                                && (got.y - want.y).abs() < 1e-9
                                && (got.w - want.w).abs() < 1e-9
                                && (got.h - want.h).abs() < 1e-9,
                            "{nom} case {i} à {n} photos sur {w}×{h}"
                        );
                        compares += 1;
                    }
                }
            }
        }
        // The count is asserted so a filter that silently stops matching
        // cannot turn this into a test of nothing.
        assert!(compares > 10_000, "seulement {compares} cases comparées");
    }

    /// The index is the depth, and the depth is the emitter's order:
    /// photographs, their captions, the text block, the chapter caption.
    #[test]
    fn l_ordre_des_objets_est_l_ordre_d_emission() {
        let g = geom();
        let mut s = spread("duo", 2);
        s.slots[0].caption = Some("la plage".into());
        s.caption = Some("Corse, 2013".into());
        let scene = Scene::of(&s, &g);
        let roles: Vec<&str> = scene
            .objects
            .iter()
            .map(|o| match o.role {
                Role::Photo { .. } => "photo",
                Role::PhotoCaption { .. } => "legende_photo",
                Role::ChapterCaption { .. } => "legende_chapitre",
                Role::Text { .. } => "texte",
                Role::FreeText { .. } => "libre",
            })
            .collect();
        assert_eq!(roles, ["photo", "photo", "legende_photo", "legende_chapitre"]);
    }

    /// The reading order is the template's own slot order, and everything
    /// that belongs to the spread is read after the pictures.
    #[test]
    fn le_rang_de_lecture_suit_le_gabarit() {
        let g = geom();
        let mut s = spread("quad", 4);
        s.caption = Some("un chapitre".into());
        let scene = Scene::of(&s, &g);
        let lecture: Vec<usize> = scene.objects.iter().map(|o| o.reading).collect();
        assert_eq!(lecture, [0, 1, 2, 3, 4]);
        // `quad` is a ParRangee template: it reads row by row across the
        // spread, so cell 1 sits on the other page at the same height as 0.
        let cells: Vec<Rect> = scene
            .objects
            .iter()
            .filter_map(|o| matches!(o.role, Role::Photo { .. }).then_some(o.rect))
            .collect();
        assert!((cells[0].y - cells[1].y).abs() < 1e-9, "0 et 1 pas sur la même rangée");
        assert!(cells[0].x < cells[1].x, "0 doit précéder 1 en travers");
        assert!(cells[2].y < cells[0].y, "la seconde rangée est plus bas");
    }

    /// An empty caption prints nothing, so it is nothing on the scene either:
    /// an object with no ink would give the accessibility layer a focusable
    /// stop over a blank.
    #[test]
    fn une_legende_vide_ne_fait_pas_un_objet() {
        let g = geom();
        let mut s = spread("duo", 2);
        s.slots[0].caption = Some(String::new());
        assert_eq!(Scene::of(&s, &g).objects.len(), 2);
    }

    /// The three text pages are one role. The half-title keeps its two type
    /// sizes, the text page keeps its regular leading, and the blank line of
    /// a stored text still spaces what follows it.
    #[test]
    fn les_trois_pages_de_texte_sont_un_seul_role() {
        let g = geom();
        let mut garde = spread(crate::garde::TEMPLATE, 0);
        garde.text = Some("Un été\n\n2013\nCalvi, Corse".into());
        let lines = match &Scene::of(&garde, &g).objects[0].role {
            Role::Text { lines, .. } => lines.clone(),
            other => panic!("attendu un bloc de texte, reçu {other:?}"),
        };
        assert_eq!(lines.len(), 3, "titre plus deux lignes calmes");
        assert!(lines[0].size_pt > lines[1].size_pt, "le titre est plus grand");
        assert_eq!(lines[0].dy_mm, 0.0);

        let mut texte = spread("texte", 0);
        texte.text = Some("premi\u{e8}re\n\ntroisi\u{e8}me".into());
        let lines = match &Scene::of(&texte, &g).objects[0].role {
            Role::Text { lines, .. } => lines.clone(),
            other => panic!("attendu un bloc de texte, reçu {other:?}"),
        };
        assert_eq!(lines.len(), 2, "la ligne vide ne s'imprime pas");
        // …mais elle a pris son tour : la troisième ligne est au troisième cran.
        assert_eq!(lines[1].dy_mm, 2.0 * pdf::TEXT_LEADING_MM);
    }

    /// A text object carries its measured ink, not the placement proxy. The
    /// two are different things on purpose: `caption_box` reserves room for a
    /// caption of unknown length, because the geometry dump has no album to
    /// read; a scene has the text in hand.
    #[test]
    fn un_objet_de_texte_porte_son_encre_mesuree() {
        let g = geom();
        let mut court = spread("duo", 2);
        court.caption = Some("Corse".into());
        let mut long = spread("duo", 2);
        long.caption = Some("Corse, octobre 2013, la traversée".into());
        let l = |s: &Spread| Scene::of(s, &g).objects.last().unwrap().rect.w;
        assert!(l(&long) > l(&court), "l'encre ne suit pas le texte");
        assert!(l(&long) < g.margin * 3.5, "l'encre vaut le proxy, pas le texte");
    }

    /// A template nobody knows still renders one margined box, silently, the
    /// way it always has: `album.json` is repairable by hand, and a typo in a
    /// template name must not empty the page.
    #[test]
    fn un_gabarit_inconnu_garde_son_repli() {
        let g = geom();
        let s = spread("gabarit-qui-n-existe-pas", 3);
        let scene = Scene::of(&s, &g);
        assert_eq!(scene.objects.len(), 1);
        assert_eq!(scene.objects[0].rect.x, pdf::slots_for("vide-inconnu", 1, &g)[0].x);
    }

    /// The doctrine has one implementation now. A full-bleed photograph runs
    /// past the cut by exactly the bleed; a margined cell stands clear of it.
    #[test]
    fn la_distance_au_rognage_se_mesure_depuis_la_coupe() {
        let g = geom();
        let pleine = Scene::of(&spread("full1", 1), &g);
        let (_, d) = pleine.closest_to_trim(&g).unwrap();
        assert!((d + g.bleed).abs() < 1e-9, "une pleine page déborde du fond perdu");

        let margee = Scene::of(&spread("duo_portrait", 2), &g);
        let (_, d) = margee.closest_to_trim(&g).unwrap();
        assert!(d > 0.0, "une case margée ne doit pas toucher la coupe");
    }

    /// **The reduction, proved rather than asserted.** The angle went into the
    /// one function that decides whether a book can be printed; an upright
    /// object has to come out of it with the float it came out with before,
    /// on every template, at every photo count, on every page shape. Not
    /// close — equal, bit for bit, which is why this compares with `==` and
    /// not with an epsilon.
    #[test]
    fn au_degre_zero_la_distance_ne_bouge_pas() {
        // The formula as it stood before the angle existed, written out here
        // so the comparison is against the old code and not against the new
        // code calling itself.
        fn avant(r: &Rect, g: &SpreadGeometry) -> f64 {
            let left = r.x - g.bleed;
            let bottom = r.y - g.bleed;
            let right = (g.media_w - g.bleed) - (r.x + r.w);
            let top = (g.media_h - g.bleed) - (r.y + r.h);
            left.min(bottom).min(right).min(top)
        }
        let formats: [(f64, f64); 6] = [
            (210.0, 210.0),
            (300.0, 300.0),
            (280.0, 210.0),
            (297.0, 210.0),
            (200.0, 250.0),
            (210.0, 297.0),
        ];
        let mut compares = 0usize;
        for (w, h) in formats {
            let mut a = Album::new("t", std::path::Path::new("."), Size { w, h });
            a.bleed_mm = 3.0;
            let g = pdf::geometry(&a);
            for (nom, capacite) in offered() {
                for n in 1..=capacite.max(1) {
                    for r in pdf::slots_for(nom, n, &g) {
                        assert_eq!(
                            distance_to_trim(&r, 0.0, &g).to_bits(),
                            avant(&r, &g).to_bits(),
                            "{nom} à {n} photos sur {w}×{h}"
                        );
                        compares += 1;
                    }
                }
            }
        }
        assert!(compares > 10_000, "seulement {compares} rectangles comparés");
    }

    /// The corners of an upright object are the rectangle's own numbers, to
    /// the bit. This is what the test above rests on, stated on its own so a
    /// regression names itself.
    #[test]
    fn les_coins_d_un_objet_droit_sont_ceux_du_rectangle() {
        // Chosen so that `(x + w/2) - w/2` is *not* `x` in binary floating
        // point — 11.2 comes back 11.200000000000003 — so a rotation applied
        // at zero degrees really would move this box.
        let r = Rect { x: 11.2, y: 18.2, w: 87.3, h: 140.6 };
        let c = corners(&r, 0.0);
        assert_eq!(c[0].x.to_bits(), r.x.to_bits());
        assert_eq!(c[0].y.to_bits(), r.y.to_bits());
        assert_eq!(c[2].x.to_bits(), (r.x + r.w).to_bits());
        assert_eq!(c[2].y.to_bits(), (r.y + r.h).to_bits());
        // And the detour really would have moved it, so the special case is
        // load-bearing and not superstition.
        let centre_x = r.x + r.w / 2.0;
        assert_ne!((centre_x + (r.x - centre_x)).to_bits(), r.x.to_bits());
    }

    /// The reason decision (a) of 04/09 had to enter the geometry at the
    /// first stored object: a block that clears the guillotine upright can
    /// cross it once it is turned, and the number that decides whether a book
    /// ships has to see it.
    #[test]
    fn un_bloc_tourne_peut_mordre_la_coupe_qu_il_evitait_droit() {
        let g = geom();
        // A wide, flat block lying just inside the top trim line.
        let r = Rect { x: 100.0, y: 195.0, w: 120.0, h: 8.0 };
        let droit = distance_to_trim(&r, 0.0, &g);
        assert!(droit > 0.0, "droit, il évite la coupe : {droit}");
        let tourne = distance_to_trim(&r, 30.0, &g);
        assert!(tourne < 0.0, "tourné à 30°, il doit la mordre : {tourne}");
        // A quarter turn is not a rotation the geometry may forget: the box
        // swaps its dimensions and the distance follows.
        let quart = distance_to_trim(&r, 90.0, &g);
        let echange = Rect { x: 154.0, y: 139.0, w: 8.0, h: 120.0 };
        assert!(
            (quart - distance_to_trim(&echange, 0.0, &g)).abs() < 1e-9,
            "un quart de tour vaut le rectangle échangé"
        );
    }

    /// The fold is not a matter of degree: crossing it is refused, and a
    /// turned object crosses it with its corners, not with its box.
    #[test]
    fn le_pli_se_mesure_sur_les_coins() {
        let g = geom();
        let pli = g.media_w / 2.0;
        let a_gauche = Rect { x: pli - 60.0, y: 100.0, w: 50.0, h: 10.0 };
        assert!(!traverse_le_pli(&a_gauche, 0.0, &g));
        assert!(traverse_le_pli(&Rect { x: pli - 25.0, y: 100.0, w: 50.0, h: 10.0 }, 0.0, &g));
        // Upright it stops 10 mm short of the fold; turned, its corner reaches
        // across. The guard has to see the corner.
        // Upright, its right edge stops 5 mm short of the fold; turned by 45°
        // its corner swings 6.8 mm past it.
        let frole = Rect { x: pli - 55.0, y: 100.0, w: 50.0, h: 40.0 };
        assert!(!traverse_le_pli(&frole, 0.0, &g));
        assert!(traverse_le_pli(&frole, 45.0, &g), "le coin tourné passe le pli");
    }

    /// A free object is stored, so it is the one thing on the scene that no
    /// template produced — and it prints over everything the template did.
    #[test]
    fn un_objet_libre_est_au_dessus_de_tout() {
        let g = geom();
        let mut s = spread("duo", 2);
        s.caption = Some("Corse, 2013".into());
        s.objets = vec![bloc("un mot", 40.0, 20.0), bloc("un autre", 40.0, 20.0)];
        let scene = Scene::of_avec(&s, &g, &regle);
        let roles: Vec<&str> = scene
            .objects
            .iter()
            .map(|o| match o.role {
                Role::Photo { .. } => "photo",
                Role::PhotoCaption { .. } => "legende_photo",
                Role::ChapterCaption { .. } => "legende_chapitre",
                Role::Text { .. } => "texte",
                Role::FreeText { .. } => "libre",
            })
            .collect();
        assert_eq!(roles, ["photo", "photo", "legende_chapitre", "libre", "libre"]);
        // Their order among themselves is their depth, and their index into
        // `spread.objets` is what the editor sends an edit back to.
        let index: Vec<usize> = scene
            .objects
            .iter()
            .filter_map(|o| match o.role {
                Role::FreeText { index, .. } => Some(index),
                _ => None,
            })
            .collect();
        assert_eq!(index, [0, 1]);
        // The reading order does not stop at the template either.
        let lecture: Vec<usize> = scene.objects.iter().map(|o| o.reading).collect();
        assert_eq!(lecture, [0, 1, 2, 3, 4]);
    }

    /// A box has a width, and a width is what a box means: the block wraps at
    /// word boundaries, in the face the album is set in.
    #[test]
    fn le_bloc_libre_revient_a_la_ligne_dans_sa_boite() {
        let g = geom();
        let mut s = spread("duo", 2);
        // Six characters cost 12 mm at 10 pt under `regle`; a 30 mm box holds
        // "un deux" (7 chars, 14 mm) but not "un deux trois".
        s.objets = vec![bloc("un deux trois quatre", 30.0, 40.0)];
        let scene = Scene::of_avec(&s, &g, &regle);
        let Role::FreeText { lines, overflow, trop_large, .. } = &scene.objects[2].role else {
            panic!("attendu un objet libre");
        };
        assert!(lines.len() > 1, "rien n'a été replié");
        for l in lines {
            assert!(regle(&l.text, 10.0) <= 30.0, "« {} » dépasse la boîte", l.text);
        }
        // Nothing was lost and nothing was invented: the words come back in
        // order, whole.
        let recolle = lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join(" ");
        assert_eq!(recolle, "un deux trois quatre");
        assert!(!overflow, "quatre lignes tiennent dans 40 mm");
        assert!(!trop_large);
        // Each produced line takes its own turn downward.
        assert_eq!(lines[1].dy_mm, 5.0);
    }

    /// Breaking a word is a decision about someone's language, and this engine
    /// does not take it. It says so instead.
    #[test]
    fn un_mot_plus_large_que_la_boite_est_signale_jamais_coupe() {
        let g = geom();
        let mut s = spread("duo", 2);
        s.objets = vec![bloc("court anticonstitutionnellement", 30.0, 40.0)];
        let scene = Scene::of_avec(&s, &g, &regle);
        let Role::FreeText { lines, trop_large, .. } = &scene.objects[2].role else {
            panic!("attendu un objet libre");
        };
        assert!(trop_large, "le mot trop large n'est pas signalé");
        assert!(
            lines.iter().any(|l| l.text == "anticonstitutionnellement"),
            "le mot a été coupé : {lines:?}"
        );
    }

    /// What does not fit runs past the bottom edge, out loud. The block is
    /// never shortened: the reader decides what to do about it.
    #[test]
    fn le_debordement_se_mesure_en_hauteur() {
        let g = geom();
        let mut s = spread("duo", 2);
        let tient = Scene::of_avec(
            &{ let mut s = s.clone(); s.objets = vec![bloc("un\ndeux", 40.0, 20.0)]; s },
            &g,
            &regle,
        );
        let Role::FreeText { overflow, .. } = tient.objects[2].role else { panic!() };
        assert!(!overflow, "deux lignes tiennent dans 20 mm");

        s.objets = vec![bloc("un\ndeux\ntrois\nquatre\ncinq", 40.0, 20.0)];
        let deborde = Scene::of_avec(&s, &g, &regle);
        let Role::FreeText { overflow, lines, .. } = &deborde.objects[2].role else { panic!() };
        assert!(overflow, "cinq lignes ne tiennent pas dans 20 mm");
        assert_eq!(lines.len(), 5, "et rien n'a été retiré pour autant");
    }

    /// The blank line of a stored text is spacing. A text page drops it and
    /// counts its leading off the typed index; a wrapped block cannot, so it
    /// keeps the line — and the two conventions are tested side by side so
    /// nobody unifies them by accident.
    #[test]
    fn la_ligne_vide_d_un_bloc_libre_garde_son_tour() {
        let g = geom();
        let mut s = spread("duo", 2);
        s.objets = vec![bloc("un\n\ntrois", 40.0, 40.0)];
        let scene = Scene::of_avec(&s, &g, &regle);
        let Role::FreeText { lines, .. } = &scene.objects[2].role else { panic!() };
        assert_eq!(lines.len(), 3, "la ligne vide est là, pour espacer");
        assert_eq!(lines[1].text, "");
        assert_eq!(lines[2].dy_mm, 10.0, "la troisième ligne est au troisième cran");
    }

    /// The alignment is computed once, here, so the emitter and the two
    /// renderers cannot each centre a line differently.
    #[test]
    fn l_alignement_est_un_decalage_calcule_une_fois() {
        let g = geom();
        let mut s = spread("duo", 2);
        let pose = |a: Alignement| {
            let mut o = bloc("abc", 40.0, 20.0);
            let Contenu::Texte { alignement, .. } = &mut o.contenu;
            *alignement = a;
            o
        };
        // "abc" is 6 mm wide at 10 pt: 34 mm of room in a 40 mm box.
        s.objets = vec![pose(Alignement::Gauche), pose(Alignement::Centre), pose(Alignement::Droite)];
        let scene = Scene::of_avec(&s, &g, &regle);
        let dx: Vec<f64> = scene
            .objects
            .iter()
            .filter_map(|o| match &o.role {
                Role::FreeText { lines, .. } => Some(lines[0].dx_mm),
                _ => None,
            })
            .collect();
        assert_eq!(dx, [0.0, 17.0, 34.0]);
    }

    /// The angle reaches the scene from the stored object, and the object's
    /// rectangle is the box the reader drew — not the ink, which is what the
    /// three text *pages* carry. The two conventions are different on purpose.
    #[test]
    fn l_objet_libre_porte_sa_boite_et_son_angle() {
        let g = geom();
        let mut s = spread("duo", 2);
        let mut o = bloc("un", 40.0, 20.0);
        o.angle = 30.0;
        s.objets = vec![o];
        let scene = Scene::of_avec(&s, &g, &regle);
        let libre = &scene.objects[2];
        assert_eq!(libre.angle, 30.0);
        assert_eq!(libre.rect.w, 40.0);
        assert_eq!(libre.rect.h, 20.0);
        // And the closest thing to the guillotine now answers about it.
        let (at, _) = scene.closest_to_trim(&g).unwrap();
        assert!(at < scene.objects.len());
    }

    /// An absent leading is the natural leading of the size, said in one
    /// place so the emitter and the renderers cannot each pick their own.
    #[test]
    fn l_interligne_absent_est_celui_de_la_taille() {
        let mut o = bloc("un", 40.0, 20.0);
        let Contenu::Texte { interligne_mm, .. } = &mut o.contenu;
        *interligne_mm = None;
        assert!((o.interligne() - 10.0 / (72.0 / 25.4) * 1.35).abs() < 1e-12);
    }
}
