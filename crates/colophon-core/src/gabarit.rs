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
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

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
    /// Signed caption height, in millimetres. Positive reserves a white band
    /// under the frame (the cells lift to clear it, the baseline hangs in
    /// it); negative declares an overlay printed over the photo; zero leaves
    /// the caption to the free-spot hunt in `pdf::caption_anchor`. One
    /// number, and the linter's two caption counters read its sign.
    pub legende: f64,
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
    // The historic catalogue declares no band and no overlay: its captions
    // keep hunting the free margin spots, exactly as before the sign existed.
    let v = |nom, capacite, gauche, droite, ordre| Spec {
        nom,
        capacite,
        gauche,
        droite,
        ordre,
        legende: 0.0,
    };
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
                legende: f.legende,
            });
            out.push(f);
            out.extend(miroir);
        }
        out
    });
    &CATALOGUE
}

pub fn spec(nom: &str) -> Option<&'static Spec> {
    if let Some(s) = offerts().iter().find(|s| s.nom == nom) {
        return Some(s);
    }
    // An album.json may carry a generated name the offered list no longer
    // holds (another machine, a later measurement): the name encodes its
    // parameters, so the geometry is still exactly reconstructible. This is
    // the hand-repairable album.json doctrine applied to generated templates.
    spec_generee(nom)
}

// ---------------------------------------------------------------------------
// The parametric generator (1.5). It enumerates the combination space inside
// the vocabulary above; the bench (`banc.rs`) substitutes each candidate into
// the composed reference sets and the linter's counters decide which ones
// exist. A candidate only reaches `RETENUS` measured green on the three sets,
// exactly like a pace reaches `Densite::offertes`.
// ---------------------------------------------------------------------------

/// Canonical caption band of the generated templates, in millimetres.
/// One height, not a continuum: the linter judges combinations, and two
/// bands 1 mm apart are the same combination to every eye.
pub const BANDE_GENEREE: f64 = 8.0;

/// Ratio vocabulary of the generated names: one letter per cell shape.
const RATIOS: [(char, Option<f64>); 6] = [
    ('e', Some(CELL_ETROIT)),
    ('q', Some(CELL_PORTRAIT)),
    ('c', Some(CELL_CARRE)),
    ('l', Some(CELL_LANDSCAPE)),
    ('n', Some(CELL_PANO)),
    ('f', None),
];

/// Capacities the composer's chunker actually produces. A generated template
/// of capacity 5 or 7 would never be emitted on a spread, so its verdict
/// would be hollow by construction: it is not enumerated.
const CAPACITES: [usize; 6] = [1, 2, 3, 4, 6, 8];

fn capacite_page(p: &Page) -> usize {
    match p {
        Page::Vide => 0,
        Page::Pleine => 1,
        Page::Grille { cols, rangs, .. } => cols * rangs,
    }
}

/// The page states the generator combines, in a fixed order (the order is
/// the canonical form: a pair is only emitted with its earlier state on the
/// left, the derived verso covers the mirror).
fn etats_page() -> Vec<Page> {
    let mut v = vec![Page::Vide, Page::Pleine];
    for cols in 1..=4 {
        for rangs in 1..=4 {
            for (_, ratio) in RATIOS {
                v.push(Page::Grille { cols, rangs, ratio });
            }
        }
    }
    v
}

fn code_ratio(ratio: Option<f64>) -> char {
    RATIOS
        .iter()
        .find(|(_, r)| match (r, ratio) {
            (None, None) => true,
            (Some(a), Some(b)) => (a - b).abs() < 1e-12,
            _ => false,
        })
        .map(|(c, _)| *c)
        .expect("un ratio généré vient de RATIOS")
}

fn code_page(p: &Page) -> String {
    match p {
        Page::Vide => "v".into(),
        Page::Pleine => "p".into(),
        Page::Grille { cols, rangs, ratio } => {
            format!("{cols}x{rangs}{}", code_ratio(*ratio))
        }
    }
}

fn parse_page(code: &str) -> Option<Page> {
    match code {
        "v" => return Some(Page::Vide),
        "p" => return Some(Page::Pleine),
        _ => {}
    }
    let (dims, ratio) = code.split_at(code.len().checked_sub(1)?);
    let ratio = RATIOS
        .iter()
        .find(|(c, _)| c.to_string() == ratio)
        .map(|(_, r)| *r)?;
    let (cols, rangs) = dims.split_once('x')?;
    let (cols, rangs) = (cols.parse().ok()?, rangs.parse().ok()?);
    if !(1..=9).contains(&cols) || !(1..=9).contains(&rangs) {
        return None;
    }
    Some(Page::Grille { cols, rangs, ratio })
}

/// The generated name encodes the parameters, whole: `g_<gauche>_<droite>`
/// plus `_b<mm>` when a band is declared. The parser below inverts it.
fn nom_genere(gauche: &Page, droite: &Page, legende: f64) -> String {
    let mut nom = format!("g_{}_{}", code_page(gauche), code_page(droite));
    if legende > 0.0 {
        nom.push_str(&format!("_b{}", legende as u32));
    }
    nom
}

/// A `Spec` reconstructed from a generated name, `_verso` included, or None
/// when the name does not speak the generated grammar.
pub fn parse_genere(nom: &str) -> Option<Spec> {
    let base = nom.strip_suffix("_verso");
    let corps = base.unwrap_or(nom);
    let mut parts = corps.split('_');
    if parts.next()? != "g" {
        return None;
    }
    let gauche = parse_page(parts.next()?)?;
    let droite = parse_page(parts.next()?)?;
    let legende = match parts.next() {
        None => 0.0,
        Some(b) => f64::from(b.strip_prefix('b')?.parse::<u32>().ok()?),
    };
    if parts.next().is_some() {
        return None;
    }
    let capacite = capacite_page(&gauche) + capacite_page(&droite);
    if capacite == 0 {
        return None;
    }
    // A verso name mirrors the pages, exactly like the derived versos of the
    // catalogue; a symmetric layout has no verso to name.
    let (gauche, droite) = if base.is_some() {
        if gauche == droite {
            return None;
        }
        (droite, gauche)
    } else {
        (gauche, droite)
    };
    Some(Spec {
        nom: &*Box::leak(nom.to_string().into_boxed_str()),
        capacite,
        gauche,
        droite,
        ordre: Ordre::ParPage,
        legende,
    })
}

/// Every candidate the bench measures, recto side, in a stable order.
///
/// The bounds are deliberate: the vocabulary of `Spec` only, cols and rangs
/// 1..=4, the five cell ratios plus the free cell, capacity in the sizes the
/// chunker produces, caption in {none, one canonical band}. `ParRangee` is
/// not enumerated (it renumbers, it does not move a rectangle) and neither
/// is the overlay (its geometry is its zero twin's, and the linter counts
/// every caption on it as a defect: nothing measurable to offer). A pair is
/// emitted once, canonically ordered; the verso is derived like everywhere
/// else. Combinations parameter-identical to a historical family (either
/// way round) are excluded: those are already offered.
pub fn combinaisons() -> &'static [Spec] {
    static COMBINAISONS: std::sync::LazyLock<Vec<Spec>> = std::sync::LazyLock::new(|| {
        let etats = etats_page();
        let historiques: Vec<(Page, Page)> = familles()
            .iter()
            .map(|f| (f.gauche, f.droite))
            .collect();
        let mut out = Vec::new();
        for i in 0..etats.len() {
            for j in i..etats.len() {
                let (gauche, droite) = (etats[i], etats[j]);
                let capacite = capacite_page(&gauche) + capacite_page(&droite);
                if !CAPACITES.contains(&capacite) {
                    continue;
                }
                for legende in [0.0, BANDE_GENEREE] {
                    let deja = legende == 0.0
                        && historiques.iter().any(|(g, d)| {
                            (*g == gauche && *d == droite) || (*g == droite && *d == gauche)
                        });
                    if deja {
                        continue;
                    }
                    out.push(Spec {
                        nom: &*Box::leak(
                            nom_genere(&gauche, &droite, legende).into_boxed_str(),
                        ),
                        capacite,
                        gauche,
                        droite,
                        ordre: Ordre::ParPage,
                        legende,
                    });
                }
            }
        }
        out
    });
    &COMBINAISONS
}

/// Generated templates measured green on the three reference sets — the
/// bench's verdict, pasted from `scripts/banc-gabarits.sh`. The name is the
/// whole declaration: it encodes the parameters, and `offerts()` interprets
/// it.
///
/// Measured 2026-08-19: 3 sets × 6 formats × 3 proposals (54 albums),
/// 1893 candidates enumerated, 186 green (99 with a caption band),
/// 1455 never assignable, 251 short of a set, 1 refused by a counter
/// (g_p_p, legende_sur_photo). Re-run the bench before touching this list.
pub const RETENUS: &[&str] = &[
    "g_v_p_b8",
    "g_v_1x1q_b8",
    "g_v_1x1c_b8",
    "g_v_1x1l_b8",
    "g_v_1x1n_b8",
    "g_v_1x1f",
    "g_v_1x1f_b8",
    "g_v_1x2q",
    "g_v_1x2q_b8",
    "g_v_1x2c",
    "g_v_1x2c_b8",
    "g_v_1x2l",
    "g_v_1x2l_b8",
    "g_v_1x2f",
    "g_v_1x2f_b8",
    "g_v_1x3q",
    "g_v_1x3q_b8",
    "g_v_1x3c",
    "g_v_1x3c_b8",
    "g_v_1x3l",
    "g_v_1x3l_b8",
    "g_v_1x4c",
    "g_v_1x4c_b8",
    "g_v_1x4l",
    "g_v_1x4l_b8",
    "g_v_2x1q",
    "g_v_2x1q_b8",
    "g_v_2x1c",
    "g_v_2x1c_b8",
    "g_v_2x1l",
    "g_v_2x1l_b8",
    "g_v_2x1f",
    "g_v_2x1f_b8",
    "g_v_2x2c",
    "g_v_2x2c_b8",
    "g_v_2x2l",
    "g_v_2x2l_b8",
    "g_v_2x2f",
    "g_v_2x2f_b8",
    "g_v_3x1q",
    "g_v_3x1q_b8",
    "g_v_3x1c",
    "g_v_3x1c_b8",
    "g_v_3x1l",
    "g_v_3x1l_b8",
    "g_v_4x1c",
    "g_v_4x1c_b8",
    "g_v_4x1l",
    "g_v_4x1l_b8",
    "g_p_p_b8",
    "g_p_1x1q",
    "g_p_1x1q_b8",
    "g_p_1x1c",
    "g_p_1x1c_b8",
    "g_p_1x1l",
    "g_p_1x1l_b8",
    "g_p_1x1f",
    "g_p_1x1f_b8",
    "g_p_1x2q",
    "g_p_1x2q_b8",
    "g_p_1x2c",
    "g_p_1x2c_b8",
    "g_p_1x2l_b8",
    "g_p_1x3c",
    "g_p_1x3c_b8",
    "g_p_1x3l",
    "g_p_1x3l_b8",
    "g_p_2x1q_b8",
    "g_p_2x1c",
    "g_p_2x1c_b8",
    "g_p_2x1l",
    "g_p_2x1l_b8",
    "g_p_3x1c",
    "g_p_3x1c_b8",
    "g_p_3x1l",
    "g_p_3x1l_b8",
    "g_1x1q_1x1q_b8",
    "g_1x1q_1x1c",
    "g_1x1q_1x1c_b8",
    "g_1x1q_1x1f",
    "g_1x1q_1x1f_b8",
    "g_1x1q_1x2q",
    "g_1x1q_1x2q_b8",
    "g_1x1q_1x2c",
    "g_1x1q_1x2c_b8",
    "g_1x1q_2x1q",
    "g_1x1q_2x1q_b8",
    "g_1x1q_2x1c",
    "g_1x1q_2x1c_b8",
    "g_1x1c_1x1c",
    "g_1x1c_1x1c_b8",
    "g_1x1c_1x1l",
    "g_1x1c_1x1l_b8",
    "g_1x1c_1x1f",
    "g_1x1c_1x1f_b8",
    "g_1x1c_1x2q",
    "g_1x1c_1x2q_b8",
    "g_1x1c_1x2c",
    "g_1x1c_1x2c_b8",
    "g_1x1c_1x2l",
    "g_1x1c_1x2l_b8",
    "g_1x1c_1x3c",
    "g_1x1c_1x3c_b8",
    "g_1x1c_1x3l",
    "g_1x1c_1x3l_b8",
    "g_1x1c_2x1q",
    "g_1x1c_2x1q_b8",
    "g_1x1c_2x1c",
    "g_1x1c_2x1c_b8",
    "g_1x1c_2x1l",
    "g_1x1c_2x1l_b8",
    "g_1x1c_3x1c",
    "g_1x1c_3x1c_b8",
    "g_1x1c_3x1l",
    "g_1x1c_3x1l_b8",
    "g_1x1l_1x1l_b8",
    "g_1x1l_1x1f",
    "g_1x1l_1x1f_b8",
    "g_1x1l_1x2c",
    "g_1x1l_1x2c_b8",
    "g_1x1l_1x2l",
    "g_1x1l_1x2l_b8",
    "g_1x1l_1x3c",
    "g_1x1l_1x3c_b8",
    "g_1x1l_1x3l",
    "g_1x1l_1x3l_b8",
    "g_1x1l_2x1c",
    "g_1x1l_2x1c_b8",
    "g_1x1l_2x1l",
    "g_1x1l_2x1l_b8",
    "g_1x1l_3x1c",
    "g_1x1l_3x1c_b8",
    "g_1x1l_3x1l",
    "g_1x1l_3x1l_b8",
    "g_1x1f_1x1f_b8",
    "g_1x1f_1x2q",
    "g_1x1f_1x2q_b8",
    "g_1x1f_1x2c",
    "g_1x1f_1x2c_b8",
    "g_1x1f_1x2l",
    "g_1x1f_1x2l_b8",
    "g_1x1f_1x3c",
    "g_1x1f_1x3c_b8",
    "g_1x1f_1x3l",
    "g_1x1f_1x3l_b8",
    "g_1x1f_2x1q",
    "g_1x1f_2x1q_b8",
    "g_1x1f_2x1c",
    "g_1x1f_2x1c_b8",
    "g_1x1f_2x1l",
    "g_1x1f_2x1l_b8",
    "g_1x1f_3x1c",
    "g_1x1f_3x1c_b8",
    "g_1x1f_3x1l",
    "g_1x1f_3x1l_b8",
    "g_1x2c_1x2c",
    "g_1x2c_1x2c_b8",
    "g_1x2c_1x2l",
    "g_1x2c_1x2l_b8",
    "g_1x2c_1x2f",
    "g_1x2c_1x2f_b8",
    "g_1x2c_2x1c",
    "g_1x2c_2x1c_b8",
    "g_1x2c_2x1l",
    "g_1x2c_2x1l_b8",
    "g_1x2l_1x2l_b8",
    "g_1x2l_1x2f",
    "g_1x2l_1x2f_b8",
    "g_1x2l_2x1c",
    "g_1x2l_2x1c_b8",
    "g_1x2l_2x1l",
    "g_1x2l_2x1l_b8",
    "g_1x2f_1x2f",
    "g_1x2f_1x2f_b8",
    "g_1x2f_2x1c",
    "g_1x2f_2x1c_b8",
    "g_1x2f_2x1l",
    "g_1x2f_2x1l_b8",
    "g_2x1c_2x1c",
    "g_2x1c_2x1c_b8",
    "g_2x1c_2x1l",
    "g_2x1c_2x1l_b8",
    "g_2x1l_2x1l",
    "g_2x1l_2x1l_b8",
    "g_2x1f_2x1f",
    "g_2x1f_2x1f_b8",
];

/// The whole offered list: the locked historical catalogue, then every
/// retained generated template with its derived verso. The picker, the G
/// cycle, `compatibles` and the dump all read this; the composer keeps
/// walking the historical catalogue only.
pub fn offerts() -> &'static [Spec] {
    static OFFERTS: std::sync::LazyLock<Vec<Spec>> = std::sync::LazyLock::new(|| {
        let mut out = catalogue().to_vec();
        for nom in RETENUS {
            let spec = parse_genere(nom)
                .unwrap_or_else(|| panic!("RETENUS porte un nom illisible : {nom}"));
            let miroir = (!spec.symetrique()).then(|| Spec {
                nom: &*Box::leak(format!("{nom}_verso").into_boxed_str()),
                capacite: spec.capacite,
                gauche: spec.droite,
                droite: spec.gauche,
                ordre: spec.ordre,
                legende: spec.legende,
            });
            out.push(spec);
            out.extend(miroir);
        }
        out
    });
    &OFFERTS
}

/// Parse-and-cache fallback of [`spec`]: the leaked `Spec` is kept so a
/// renderer calling in a loop does not leak one copy per call.
fn spec_generee(nom: &str) -> Option<&'static Spec> {
    static CACHE: std::sync::LazyLock<Mutex<HashMap<String, Option<&'static Spec>>>> =
        std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut cache = CACHE.lock().unwrap();
    *cache
        .entry(nom.to_string())
        .or_insert_with(|| parse_genere(nom).map(|s| &*Box::leak(Box::new(s))))
}

/// The one interpreter: parameters in, rectangles out, origin bottom-left
/// like the PDF. `n` truncates a partially filled spread.
pub fn slots(spec: &Spec, n: usize, g: &SpreadGeometry) -> Vec<Rect> {
    if spec.capacite == 0 {
        return Vec::new();
    }
    // A positive caption height lifts the content off a band under the
    // frame; overlay (negative) and hunt (zero) leave the geometry alone.
    let bande = spec.legende.max(0.0);
    let page = |droite: bool, p: &Page| -> Vec<Rect> {
        match p {
            Page::Vide => Vec::new(),
            Page::Pleine => {
                let mut r = full_page(droite, g);
                if bande > 0.0 {
                    // What must survive the cut is measured from the cut: a
                    // full-bleed page with a band stops bleeding at the
                    // bottom, `bande` millimetres above the trim line.
                    let bas = g.bleed + bande;
                    r.h -= bas - r.y;
                    r.y = bas;
                }
                vec![r]
            }
            Page::Grille { cols, rangs, ratio } => {
                let mut b = page_box(droite, g);
                b.y += bande;
                b.h -= bande;
                let cells = grid(b, *cols, *rangs, g.gutter);
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

/// Templates a spread can switch to right now, judged the way the linter
/// counts: the capacity fits the photo count (a smaller one keeps the head
/// and drops the tail, exactly like the switch does), and no kept photo
/// would betray its cell's orientation past `audit::ASPECT_BETRAYAL`, the
/// threshold the composer places with. One rule, engine side: the picker
/// and the keyboard cycle both read this list, neither rewrites it.
pub fn compatibles(aspects: &[f64], g: &SpreadGeometry) -> Vec<&'static str> {
    offerts().iter().filter(|s| apte(s, aspects, g)).map(|s| s.nom).collect()
}

/// The worst orientation betrayal a template inflicts on these photos, taken
/// in slot order: 1.0 when every photo matches its cell, growing as the two
/// shapes diverge. `audit::ASPECT_BETRAYAL` is where the composer stops
/// placing and the linter starts counting.
///
/// Measured on the rectangles the spread actually renders — `slots` truncates
/// to the photo count — so a template carrying fewer photos than its capacity
/// is judged on the cells that hold something, never on empty ones. For a
/// full spread this is the arithmetic `compatibles` has always run.
pub fn trahison(spec: &Spec, aspects: &[f64], g: &SpreadGeometry) -> f64 {
    slots(spec, aspects.len(), g)
        .iter()
        .zip(aspects)
        .map(|(r, a)| {
            let c = r.w / r.h;
            (a / c).max(c / a)
        })
        .fold(1.0, f64::max)
}

/// Whether a template can carry these photos on this geometry: it holds all
/// of them, and none betrays its cell. The project's single fitness rule —
/// the picker, the keyboard cycle and the bascule read it, none rewrites it.
pub fn apte(spec: &Spec, aspects: &[f64], g: &SpreadGeometry) -> bool {
    spec.capacite > 0
        && spec.capacite <= aspects.len()
        && trahison(spec, aspects, g) <= crate::audit::ASPECT_BETRAYAL
}

/// `compatibles` for photos of a saved album, named by their `src`, in
/// slot order. The aspects are read on the thumbnail headers (a thumbnail
/// keeps its photo's shape and a header costs nothing), the geometry is
/// the album's own. The live spread travels as the src list, so an
/// unsaved edit still filters right. Feeds the Tauri command and
/// `--gabarits`.
pub fn compatibles_srcs(dir: &Path, srcs: &[String]) -> Result<Vec<&'static str>> {
    let album: crate::model::Album = serde_json::from_str(
        &std::fs::read_to_string(dir.join("album.json"))
            .with_context(|| format!("lecture de {}", dir.join("album.json").display()))?,
    )
    .context("album.json illisible")?;
    let thumbs: HashMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("thumbs.json"))?)
            .context("thumbs.json illisible")?;
    let aspects = srcs
        .iter()
        .map(|src| {
            let name = thumbs
                .get(src)
                .with_context(|| format!("{src} absent de thumbs.json"))?;
            let (w, h) =
                image::image_dimensions(dir.join(".cache").join("thumbs").join(name))
                    .with_context(|| format!("vignette illisible pour {src}"))?;
            Ok(f64::from(w) / f64::from(h))
        })
        .collect::<Result<Vec<f64>>>()?;
    Ok(compatibles(&aspects, &crate::pdf::geometry(&album)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom() -> SpreadGeometry {
        SpreadGeometry { media_w: 426.0, media_h: 216.0, margin: 14.0, gutter: 7.0, bleed: 3.0 }
    }

    /// The historic catalogue declares no band and no overlay: its geometry
    /// must not move an atom. The day a generated band template earns its
    /// place (1.5), it enters beside these, not instead of them.
    #[test]
    fn le_catalogue_historique_ne_declare_aucune_legende() {
        assert!(catalogue().iter().all(|s| s.legende == 0.0));
    }

    /// A positive height lifts the cells off a band under the frame: same
    /// left edge, same width, the bottom raised by exactly the band.
    #[test]
    fn la_bande_positive_souleve_les_cases() {
        let g = geom();
        let duo = spec("duo").unwrap();
        let avant = slots(duo, 2, &g);
        let bande = Spec { legende: 8.0, ..duo.clone() };
        let apres = slots(&bande, 2, &g);
        for (a, b) in avant.iter().zip(&apres) {
            assert!((b.y - (a.y + 8.0)).abs() < 1e-9);
            assert!((b.h - (a.h - 8.0)).abs() < 1e-9);
            assert_eq!(a.x, b.x);
            assert_eq!(a.w, b.w);
        }
    }

    /// What must survive the cut is measured from the cut: a full-bleed page
    /// with a band stops exactly `legende` above the trim line, and still
    /// bleeds at the top.
    #[test]
    fn la_bande_arrete_le_plein_fond_au_dessus_de_la_coupe() {
        let g = geom();
        let bande = Spec { legende: 8.0, ..spec("full1").unwrap().clone() };
        let r = slots(&bande, 1, &g)[0];
        assert!((r.y - (g.bleed + 8.0)).abs() < 1e-9);
        assert!((r.y + r.h - g.media_h).abs() < 1e-9);
    }

    /// A declared overlay prints over the photo: the geometry is exactly the
    /// zero-height one, only the caption's place and the linter's verdict
    /// change.
    #[test]
    fn la_surimpression_ne_touche_pas_la_geometrie() {
        let g = geom();
        let duo = spec("duo").unwrap();
        let avant = slots(duo, 2, &g);
        let sur = Spec { legende: -6.0, ..duo.clone() };
        for (a, b) in avant.iter().zip(&slots(&sur, 2, &g)) {
            assert_eq!((a.x, a.y, a.w, a.h), (b.x, b.y, b.w, b.h));
        }
    }

    /// The list follows the count and the orientation: a template only
    /// enters when its capacity fits and no kept photo would betray its
    /// cell past the linter's threshold.
    #[test]
    fn compatibles_suit_le_nombre_et_l_orientation() {
        let g = geom();
        let portraits = [0.75, 0.75];
        let c = compatibles(&portraits, &g);
        assert!(c.contains(&"duo_portrait"));
        assert!(c.contains(&"solo"), "une capacité moindre juge les photos gardées");
        assert!(!c.contains(&"duo_pano"), "un portrait trahirait la case panorama");
        assert!(!c.contains(&"trio"), "trois cases pour deux photos, jamais de trou");
        let panos = [2.0, 2.0];
        let cp = compatibles(&panos, &g);
        assert!(cp.contains(&"duo_pano"));
        assert!(!cp.contains(&"duo_portrait"));
        assert!(compatibles(&[], &g).is_empty());
    }

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

    /// The enumeration stays inside its bounds: thousands of combinations,
    /// every capacity one the chunker produces, no parameter twin of a
    /// historical family, and a stable order (the bench's verdict must name
    /// the same candidate tomorrow).
    #[test]
    fn les_combinaisons_sont_bornees_et_stables() {
        let c = combinaisons();
        assert!(c.len() >= 1500, "des milliers de combinaisons, pas {}", c.len());
        assert!(c.iter().all(|s| CAPACITES.contains(&s.capacite)));
        assert!(c.iter().all(|s| s.ordre == Ordre::ParPage));
        assert!(c.iter().all(|s| s.legende == 0.0 || s.legende == BANDE_GENEREE));
        for s in c {
            for f in familles() {
                let jumeau = s.legende == f.legende
                    && ((s.gauche == f.gauche && s.droite == f.droite)
                        || (s.gauche == f.droite && s.droite == f.gauche));
                assert!(!jumeau, "{} redéclare {}", s.nom, f.nom);
            }
        }
        let noms: std::collections::HashSet<&str> = c.iter().map(|s| s.nom).collect();
        assert_eq!(noms.len(), c.len(), "deux candidats partagent un nom");
        assert_eq!(c[0].nom, "g_v_p_b8", "l'ordre d'énumération a bougé");
    }

    /// The name is the whole declaration: every enumerated candidate parses
    /// back to its own parameters, and a verso name mirrors the pages.
    #[test]
    fn le_nom_encode_les_parametres() {
        for s in combinaisons() {
            let p = parse_genere(s.nom).expect(s.nom);
            assert_eq!((p.capacite, p.gauche, p.droite, p.ordre), (s.capacite, s.gauche, s.droite, s.ordre));
            assert_eq!(p.legende, s.legende);
        }
        let v = parse_genere("g_v_2x1q_verso").unwrap();
        assert_eq!(v.gauche, Page::Grille { cols: 2, rangs: 1, ratio: Some(CELL_PORTRAIT) });
        assert_eq!(v.droite, Page::Vide);
        assert!(parse_genere("g_v_v").is_none(), "capacité zéro");
        assert!(parse_genere("g_1x1q_1x1q_verso").is_none(), "un symétrique n'a pas de verso");
        assert!(parse_genere("solo").is_none());
        assert!(parse_genere("g_v_9z9x").is_none());
    }

    /// With nothing retained, the offered list is the locked catalogue and
    /// nothing else; `spec` still resolves any generated name through the
    /// parser, so a hand-carried album.json renders exactly.
    #[test]
    fn offerts_et_le_repli_du_parseur() {
        if RETENUS.is_empty() {
            let a: Vec<&str> = offerts().iter().map(|s| s.nom).collect();
            let b: Vec<&str> = catalogue().iter().map(|s| s.nom).collect();
            assert_eq!(a, b);
        } else {
            assert!(offerts().len() > catalogue().len());
        }
        let s = spec("g_v_2x1q_b8").expect("le repli parse un nom généré");
        assert_eq!(s.capacite, 2);
        assert_eq!(s.legende, 8.0);
        let g = geom();
        let r = slots(s, 2, &g);
        assert_eq!(r.len(), 2);
        assert!(spec("g_v_2x1q_b8").unwrap().nom == s.nom, "le cache rend le même Spec");
        assert!(spec("gabarit_inconnu").is_none());
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
