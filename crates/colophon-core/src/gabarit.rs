//! The template catalogue as data. A template used to be an arm in a match,
//! written twice (`pdf.rs::slots_for` and the editor's TypeScript port); it
//! is now a `Spec`, a set of parameters the generator emits once and one
//! interpreter turns into rectangles. The editor consumes the engine's dump
//! and redeclares nothing: this file is the only place a layout dimension
//! exists.
//!
//! The generator's vocabulary is small on purpose: a page is empty, full
//! bleed, or a grid of cells fitted to a ratio; slots run page by page or
//! row by row; a verso variant is derived, never written, and only exists
//! where the layout is asymmetric, because flipping a symmetric spread
//! changes nothing. New templates are new parameter values, and the linter
//! decides which generated combinations are worth offering
//! (`Densite::offertes` is the model).

use crate::pdf::{fitted, full_page, grid, page_box, Rect, SpreadGeometry};

/// Cell aspect of the margined landscape cells (stacks, quads).
pub const CELL_LANDSCAPE: f64 = 4.0 / 3.0;
/// Cell aspect of the portrait cells (solo, duo_portrait, pairs).
pub const CELL_PORTRAIT: f64 = 0.75;
/// Cell aspect of the panorama cells.
pub const CELL_PANO: f64 = 2.0;
/// Cell aspect of the narrow cells, for 18,5:9-style phone portraits.
pub const CELL_ETROIT: f64 = 0.5;
/// Cell aspect of the square cell, for square-ish photos on non-square
/// pages where no other cell comes close.
pub const CELL_CARRE: f64 = 1.0;

/// What covers one page of a spread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Page {
    /// Nothing.
    Vide,
    /// One photo, full bleed, margins ignored.
    Pleine,
    /// `cols` × `rangs` cells in the margined box, each centred and fitted
    /// to `ratio`; `None` keeps the raw cell shape (free aspect).
    Grille { cols: usize, rangs: usize, ratio: Option<f64> },
}

/// How slot indices run across the two pages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ordre {
    /// Left page's cells, then the right page's.
    ParPage,
    /// Row by row across the whole spread (a quad reads l0 r0 l1 r1).
    ParRangee,
}

/// One template: a name, and enough parameters to draw it anywhere.
#[derive(Debug, Clone)]
pub struct Spec {
    pub nom: &'static str,
    pub capacite: usize,
    pub gauche: Page,
    pub droite: Page,
    pub ordre: Ordre,
}

impl Spec {
    /// A layout where both pages read the same is its own mirror: flipping
    /// it changes nothing, so no verso variant exists for it.
    pub fn symetrique(&self) -> bool {
        self.gauche == self.droite
    }
}

/// The families the generator emits, recto side (lead on the right page).
/// Order matters: the picker and the dump keep it.
fn familles() -> Vec<Spec> {
    let une = |ratio: Option<f64>| Page::Grille { cols: 1, rangs: 1, ratio };
    let v = |nom, capacite, gauche, droite, ordre| Spec { nom, capacite, gauche, droite, ordre };
    use Ordre::*;
    use Page::*;
    vec![
        v("full1", 1, Vide, Pleine, ParPage),
        v("solo", 1, Vide, une(Some(CELL_PORTRAIT)), ParPage),
        v("solo_paysage", 1, Vide, une(Some(CELL_LANDSCAPE)), ParPage),
        v("solo_pano", 1, Vide, une(Some(CELL_PANO)), ParPage),
        v("solo_etroit", 1, Vide, une(Some(CELL_ETROIT)), ParPage),
        v("solo_carre", 1, Vide, une(Some(CELL_CARRE)), ParPage),
        v("duo", 2, une(None), une(None), ParPage),
        v("duo_portrait", 2, une(Some(CELL_PORTRAIT)), une(Some(CELL_PORTRAIT)), ParPage),
        v("duo_paysage", 2, une(Some(CELL_LANDSCAPE)), une(Some(CELL_LANDSCAPE)), ParPage),
        v("duo_etroit", 2, une(Some(CELL_ETROIT)), une(Some(CELL_ETROIT)), ParPage),
        v("duo_pano", 2, une(Some(CELL_PANO)), une(Some(CELL_PANO)), ParPage),
        v(
            "trio",
            3,
            Grille { cols: 1, rangs: 2, ratio: Some(CELL_LANDSCAPE) },
            Pleine,
            ParPage,
        ),
        v(
            "trio_portrait",
            3,
            Grille { cols: 2, rangs: 1, ratio: Some(CELL_PORTRAIT) },
            Pleine,
            ParPage,
        ),
        v(
            "quad",
            4,
            Grille { cols: 1, rangs: 2, ratio: Some(CELL_LANDSCAPE) },
            Grille { cols: 1, rangs: 2, ratio: Some(CELL_LANDSCAPE) },
            ParRangee,
        ),
        v(
            "quad_portrait",
            4,
            Grille { cols: 2, rangs: 1, ratio: Some(CELL_PORTRAIT) },
            Grille { cols: 2, rangs: 1, ratio: Some(CELL_PORTRAIT) },
            ParPage,
        ),
        v(
            "quad_etroit",
            4,
            Grille { cols: 2, rangs: 1, ratio: Some(CELL_ETROIT) },
            Grille { cols: 2, rangs: 1, ratio: Some(CELL_ETROIT) },
            ParPage,
        ),
        v(
            "quad_pano",
            4,
            Grille { cols: 1, rangs: 2, ratio: Some(CELL_PANO) },
            Grille { cols: 1, rangs: 2, ratio: Some(CELL_PANO) },
            ParRangee,
        ),
        v(
            "six",
            6,
            Grille { cols: 2, rangs: 2, ratio: None },
            Grille { cols: 1, rangs: 2, ratio: Some(CELL_LANDSCAPE) },
            ParPage,
        ),
        v(
            "octo",
            8,
            Grille { cols: 2, rangs: 2, ratio: None },
            Grille { cols: 2, rangs: 2, ratio: None },
            ParPage,
        ),
        // Photo-less spreads the editor inserts, and the two book ends. Zero
        // capacity keeps them out of the picker and of count-driven rules.
        v("vide", 0, Vide, Vide, ParPage),
        v("texte", 0, Vide, Vide, ParPage),
        v("colophon", 0, Vide, Vide, ParPage),
        v("garde", 0, Vide, Vide, ParPage),
    ]
}

/// The whole catalogue: every family, its derived verso right behind it
/// when the layout is asymmetric. Built once; the names are leaked so the
/// catalogue keeps the `&'static str` the rest of the engine speaks.
pub fn catalogue() -> &'static [Spec] {
    static CATALOGUE: std::sync::LazyLock<Vec<Spec>> = std::sync::LazyLock::new(|| {
        let mut out = Vec::new();
        for f in familles() {
            let miroir = (!f.symetrique() && f.capacite > 0).then(|| Spec {
                nom: &*Box::leak(format!("{}_verso", f.nom).into_boxed_str()),
                capacite: f.capacite,
                gauche: f.droite,
                droite: f.gauche,
                ordre: f.ordre,
            });
            out.push(f);
            out.extend(miroir);
        }
        out
    });
    &CATALOGUE
}

pub fn spec(nom: &str) -> Option<&'static Spec> {
    catalogue().iter().find(|s| s.nom == nom)
}

/// The one interpreter: parameters in, rectangles out, origin bottom-left
/// like the PDF. `n` truncates a partially filled spread.
pub fn slots(spec: &Spec, n: usize, g: &SpreadGeometry) -> Vec<Rect> {
    if spec.capacite == 0 {
        return Vec::new();
    }
    let page = |droite: bool, p: &Page| -> Vec<Rect> {
        match p {
            Page::Vide => Vec::new(),
            Page::Pleine => vec![full_page(droite, g)],
            Page::Grille { cols, rangs, ratio } => {
                let cells = grid(page_box(droite, g), *cols, *rangs, g.gutter);
                match ratio {
                    Some(r) => cells.into_iter().map(|c| fitted(c, *r)).collect(),
                    None => cells,
                }
            }
        }
    };
    let gauche = page(false, &spec.gauche);
    let droite = page(true, &spec.droite);
    let mut v = match spec.ordre {
        Ordre::ParPage => {
            let mut v = gauche;
            v.extend(droite);
            v
        }
        // Row by row: the pages hold the same number of rows by
        // construction, and a missing side simply contributes nothing.
        Ordre::ParRangee => {
            let mut v = Vec::with_capacity(gauche.len() + droite.len());
            let rangs = gauche.len().max(droite.len());
            for i in 0..rangs {
                v.extend(gauche.get(i).copied());
                v.extend(droite.get(i).copied());
            }
            v
        }
    };
    v.truncate(n.max(1));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mirror is derived, never written: every asymmetric family has
    /// its `_verso` right behind it, symmetric ones none.
    #[test]
    fn les_versos_sont_derives_de_l_asymetrie() {
        let noms: Vec<&str> = catalogue().iter().map(|s| s.nom).collect();
        assert!(noms.contains(&"full1_verso"));
        assert!(noms.contains(&"trio_verso"));
        assert!(noms.contains(&"six_verso"));
        assert!(!noms.contains(&"duo_verso"));
        assert!(!noms.contains(&"quad_verso"));
        assert!(!noms.contains(&"octo_verso"));
        assert!(!noms.contains(&"vide_verso"));
    }

    /// The exact list the engine always had, in the same order: the front
    /// consumed it, the composer walks it, nothing may shift.
    #[test]
    fn le_catalogue_est_celui_du_match_d_avant() {
        let noms: Vec<&str> = catalogue().iter().map(|s| s.nom).collect();
        assert_eq!(
            noms,
            vec![
                "full1", "full1_verso", "solo", "solo_verso", "solo_paysage",
                "solo_paysage_verso", "solo_pano", "solo_pano_verso", "solo_etroit",
                "solo_etroit_verso", "solo_carre", "solo_carre_verso", "duo",
                "duo_portrait", "duo_paysage", "duo_etroit", "duo_pano", "trio",
                "trio_verso", "trio_portrait", "trio_portrait_verso", "quad",
                "quad_portrait", "quad_etroit", "quad_pano", "six", "six_verso",
                "octo", "vide", "texte", "colophon", "garde",
            ],
        );
    }
}
