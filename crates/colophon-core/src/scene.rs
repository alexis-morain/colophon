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

/// One visible element. It carries where it is, when it is read, and what it
/// is — and nothing else. No rotation, no matrix: an oriented box would cost
/// the linter and the preflight a different geometry for zero feature before
/// wave 6, and the day a clipart asks for one it arrives with its own linter
/// counter.
#[derive(Debug, Clone, Serialize)]
pub struct Object {
    /// Millimetres, origin bottom-left of the media box, like every rectangle
    /// the engine computes. For a text object this is the measured ink, not
    /// the placement proxy — see [`Scene::of`].
    pub rect: Rect,
    /// Rank in the reading order, which is not the depth. Derived from the
    /// template's own slot order.
    pub reading: usize,
    pub role: Role,
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
}

/// One line of a text block, already placed: `dy_mm` grows downward from the
/// block's first baseline, the way [`crate::garde::Ligne`] already does.
#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub text: String,
    pub size_pt: f64,
    pub dy_mm: f64,
}

/// Ink of one set line: the measured width, and the vertical box the engine
/// has always used around a baseline.
fn ink_box(x: f64, baseline: f64, text: &str, size_pt: f64) -> Rect {
    let size_mm = size_pt / (72.0 / 25.4);
    Rect {
        x,
        y: baseline - size_mm * 0.3,
        w: crate::font::text_width_mm(text, size_pt),
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
        let rects = pdf::slots_for(&spread.template, spread.slots.len(), g);
        let mut objects = Vec::with_capacity(spread.slots.len() + 2);

        for (cell, (slot, rect)) in spread.slots.iter().zip(rects.iter()).enumerate() {
            objects.push(Object {
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
                rect: ink_box(at.x, at.y, text, pdf::PHOTO_CAPTION_SIZE_PT),
                reading,
                role: Role::PhotoCaption { cell, text: text.clone(), at },
            });
            reading += 1;
        }

        if let Some(text) = &spread.text {
            if let Some(block) = text_block(spread, text, g) {
                objects.push(Object { reading, ..block });
                reading += 1;
            }
        }

        if let Some(text) = &spread.caption {
            let at = pdf::caption_anchor(&spread.template, &rects, g);
            objects.push(Object {
                rect: ink_box(at.x, at.y, text, pdf::SPREAD_CAPTION_SIZE_PT),
                reading,
                role: Role::ChapterCaption { text: text.clone(), at },
            });
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
            .map(|(i, o)| (i, distance_to_trim(&o.rect, g)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }
}

/// Distance from a rectangle to the trimmed edge, in millimetres. What must
/// survive the cut is measured from the cut, never from the media edge.
pub fn distance_to_trim(r: &Rect, g: &SpreadGeometry) -> f64 {
    let left = r.x - g.bleed;
    let bottom = r.y - g.bleed;
    let right = (g.media_w - g.bleed) - (r.x + r.w);
    let top = (g.media_h - g.bleed) - (r.y + r.h);
    left.min(bottom).min(right).min(top)
}

/// The one text block a spread may carry, whichever of the three pages it is.
fn text_block(spread: &Spread, text: &str, g: &SpreadGeometry) -> Option<Object> {
    let (at, lines) = if spread.template == crate::garde::TEMPLATE {
        let at = crate::garde::anchor(g);
        let place = crate::garde::place(g);
        let lines = crate::garde::mise_en_page(text, place)
            .into_iter()
            .map(|l| Line { text: l.texte, size_pt: l.taille_pt, dy_mm: l.dy_mm })
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
            })
            .collect::<Vec<_>>();
        (at, lines)
    };

    let rect = lines
        .iter()
        .map(|l| ink_box(at.x, at.y - l.dy_mm, &l.text, l.size_pt))
        .reduce(union)?;
    Some(Object { rect, reading: 0, role: Role::Text { at, lines } })
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
    use crate::model::{Album, Size, Slot};

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
}
