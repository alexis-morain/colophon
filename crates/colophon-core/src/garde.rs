//! The half-title page: the first leaf of the book, before the photographs.
//!
//! Real books open on a half-title, the title alone on a recto under a lot of
//! white. This one adds the two things the machine measured and nobody would
//! type twice: when the album was shot, and where. Three lines, never more.
//!
//! It says nothing else, and the doctrine is the engine's own. **Never
//! invent**: a line here is a date the cameras wrote or a town the gazetteer
//! agreed on, never a turn of phrase. **Keep quiet in doubt**: no trusted
//! date, no date line; no town, no town line. **Never a path, a coordinate or
//! a file name**, the forbidden list of [`crate::colophon`], for the same
//! reason: what reaches this page reaches a printer, on paper, forever.
//!
//! The facts come from [`crate::colophon::Faits`], measured once at the
//! composition and carried by `album.json`, so the page costs one string to
//! build. The title comes from the album: renaming the book rewrites the
//! first line and moves nothing else.
//!
//! The page is an ordinary spread, like the colophon at the other end of the
//! book. Pagination, preflight, audit and the editor already know what to do
//! with a spread that holds no photograph.

use crate::colophon::Faits;
use crate::pdf::{Point, SpreadGeometry};

/// The template name of the half-title spread. No photo, like `vide`,
/// `texte` and `colophon`, so every geometry already returns nothing for it.
pub const TEMPLATE: &str = "garde";

/// The title as it prints when it fits: a title page, not a caption.
pub const TITRE_PT: f64 = 18.0;
/// And the floor it shrinks to when it does not: the colophon's own size,
/// the quietest type the book ever prints. Only a title made of the widest
/// glyph in the face reaches it; an ordinary title of [`TITRE_MAX`]
/// characters never shrinks at all, on any of the six formats.
pub const TITRE_PT_MIN: f64 = 8.5;

/// The two quiet lines under the title: the dates, then the towns.
pub const LIGNE_PT: f64 = 9.5;
pub const LIGNE_LEADING_MM: f64 = 5.0;
/// From the title's baseline to the first quiet line's. A half-title is
/// mostly air; this is the air that says the title is the title.
pub const APRES_TITRE_MM: f64 = 14.0;

/// The longest title the editor accepts, in characters. It is a page
/// constant rather than an interface one: the guarantee that the title fits
/// the narrowest format is measured here, against the face the PDF embeds,
/// and the field that enforces it (`album.ts::TITRE_MAX`) is the mirror.
pub const TITRE_MAX: usize = 64;

/// At most this many towns on the line. The colophon page lists what the
/// album crossed; the half-title names where it is set, and three names is
/// where that stops being a title and starts being an itinerary.
const MAX_LIEUX: usize = 3;

/// Where the title's baseline sits: left margin of the recto page, in the
/// upper third. High on the page and ranged left, the way a half-title is
/// set, rather than centred in the middle of nothing.
pub fn anchor(g: &SpreadGeometry) -> Point {
    Point { x: g.media_w / 2.0 + g.gutter / 2.0, y: g.media_h * 0.68 }
}

/// The room a line has on that page: the recto's margined box, edge to edge.
/// Nothing here runs into the margin, which is what the colophon page allows
/// itself at the foot of the book and a title page must not.
pub fn place(g: &SpreadGeometry) -> f64 {
    g.media_w / 2.0 - g.margin - g.gutter / 2.0
}

/// The page as it prints: the title, a blank, then what is known of the when
/// and the where. Either quiet line is dropped when the facts do not carry
/// it, and a page with neither is a half-title with a title on it, which is
/// exactly what a book does.
///
/// `place_mm` is the room on the page, from [`place`]: the town line is
/// built to fit it rather than run off the paper, and the format is fixed
/// before the composition, so measuring it once here is measuring it once.
pub fn texte(titre: &str, f: &Faits, place_mm: f64) -> String {
    let mut quiet: Vec<String> = Vec::new();
    if let Some(p) = crate::colophon::periode(f.debut, f.fin) {
        quiet.push(majuscule(&p));
    }
    if let Some(v) = villes(&f.lieux, place_mm) {
        quiet.push(v);
    }
    let titre = titre.trim();
    if quiet.is_empty() {
        return titre.to_string();
    }
    format!("{titre}\n\n{}", quiet.join("\n"))
}

/// The spread the album carries at its head, text already rendered.
pub fn spread(titre: &str, f: &Faits, place_mm: f64) -> crate::model::Spread {
    crate::model::Spread {
        template: TEMPLATE.into(),
        slots: Vec::new(),
        caption: None,
        text: Some(texte(titre, f, place_mm)),
        edited: false,
        locked: false,
    }
}

/// One line of the page, ready to draw: what it says, at what size, how far
/// under the anchor. The two sizes are the whole point of the page, so the
/// layout is computed once here and drawn by the renderer and mirrored by
/// the editor, rather than reasoned about twice.
#[derive(Debug, Clone, PartialEq)]
pub struct Ligne {
    pub texte: String,
    pub taille_pt: f64,
    pub dy_mm: f64,
}

/// The stored text laid out: first line is the title, every other non-empty
/// line is quiet. Reading the structure back from the text rather than from a
/// field is what keeps `album.json` repairable with an editor.
pub fn mise_en_page(text: &str, place_mm: f64) -> Vec<Ligne> {
    mise_en_page_avec(text, place_mm, |s, pt| crate::font::text_width_mm(s, pt))
}

/// The same layout under a caller-supplied measure. The renderer measures in
/// the embedded face; the geometry dump measures synthetically so the
/// editor's port can be compared against the exact same arithmetic, shrink
/// formula included, without sharing a font.
pub fn mise_en_page_avec(
    text: &str,
    place_mm: f64,
    mesure: impl Fn(&str, f64) -> f64,
) -> Vec<Ligne> {
    let mut lignes = text.lines();
    let Some(titre) = lignes.next() else { return Vec::new() };
    let mut out = vec![Ligne {
        texte: titre.to_string(),
        taille_pt: taille_titre_avec(titre, place_mm, &mesure),
        dy_mm: 0.0,
    }];
    for (i, l) in lignes.filter(|l| !l.trim().is_empty()).enumerate() {
        out.push(Ligne {
            texte: l.to_string(),
            taille_pt: LIGNE_PT,
            dy_mm: APRES_TITRE_MM + i as f64 * LIGNE_LEADING_MM,
        });
    }
    out
}

/// The size the title prints at: [`TITRE_PT`] when it fits the page, scaled
/// down to fit when it does not. Widths are linear in the size, so the fit is
/// one division rather than a search, and the editor mirrors it exactly.
///
/// A title is never cut and never wrapped: the whole of what somebody typed
/// prints, or the page is not a title page.
pub fn taille_titre(titre: &str, place_mm: f64) -> f64 {
    taille_titre_avec(titre, place_mm, |s, pt| crate::font::text_width_mm(s, pt))
}

fn taille_titre_avec(
    titre: &str,
    place_mm: f64,
    mesure: impl Fn(&str, f64) -> f64,
) -> f64 {
    let large = mesure(titre, TITRE_PT);
    if large <= place_mm || large <= 0.0 {
        return TITRE_PT;
    }
    (TITRE_PT * place_mm / large).max(TITRE_PT_MIN)
}

/// « Porto-Vecchio, Bonifacio ». Commas, not the colophon's « et »: this is
/// the place line of a title page, not a sentence about the book. Towns are
/// dropped from the end until the line fits the page, because a name the
/// gazetteer knows can be « Saint-Remy-en-Bouzemont-Saint-Genest-et-Isson ».
fn villes(lieux: &[String], place_mm: f64) -> Option<String> {
    let mut pris: Vec<&str> = lieux.iter().take(MAX_LIEUX).map(|s| s.as_str()).collect();
    while !pris.is_empty() {
        let ligne = pris.join(", ");
        if crate::font::text_width_mm(&ligne, LIGNE_PT) <= place_mm {
            return Some(ligne);
        }
        pris.pop();
    }
    None
}

/// « du 21 au 29 octobre 2013 » opens a sentence on the colophon page and a
/// line of its own here.
fn majuscule(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Size;

    fn faits() -> Faits {
        Faits {
            photos_retenues: 152,
            photos_scannees: 575,
            debut: chrono::NaiveDate::from_ymd_opt(2013, 10, 21),
            fin: chrono::NaiveDate::from_ymd_opt(2013, 10, 29),
            lieux: vec!["Porto-Vecchio".into(), "Bonifacio".into()],
            appareils: vec!["Canon EOS 550D".into()],
            compose_le: chrono::NaiveDate::from_ymd_opt(2026, 8, 17).unwrap(),
        }
    }

    /// The geometry of one format, for the tests that need room on a page.
    fn geom(w: f64, h: f64) -> SpreadGeometry {
        let a = crate::model::Album::new("t", std::path::Path::new("/p"), Size { w, h });
        crate::pdf::geometry(&a)
    }

    #[test]
    fn the_page_says_the_title_then_when_then_where() {
        let t = texte("Corse 2013", &faits(), place(&geom(210.0, 210.0)));
        assert_eq!(
            t,
            "Corse 2013\n\nDu 21 au 29 octobre 2013\nPorto-Vecchio, Bonifacio"
        );
    }

    /// Three lines, never more: the page has no room for the cameras, the
    /// counts or anything else the colophon page carries.
    #[test]
    fn the_page_is_three_lines_at_most() {
        let mut f = faits();
        f.lieux = vec!["A".into(), "B".into(), "C".into(), "D".into(), "E".into()];
        let t = texte("Corse 2013", &f, place(&geom(210.0, 210.0)));
        assert_eq!(t.lines().filter(|l| !l.trim().is_empty()).count(), 3);
        assert!(t.ends_with("A, B, C"), "{t}");
        assert!(!t.contains(", D"), "{t}");
    }

    /// No trusted date: the page says nothing about time, exactly like the
    /// colophon. No town either: the title is the whole page, which is what
    /// a half-title is.
    #[test]
    fn without_facts_the_page_falls_back_to_the_title_alone() {
        let mut f = faits();
        f.debut = None;
        f.fin = None;
        let t = texte("Corse 2013", &f, place(&geom(210.0, 210.0)));
        assert_eq!(t, "Corse 2013\n\nPorto-Vecchio, Bonifacio");
        f.lieux = Vec::new();
        assert_eq!(texte("Corse 2013", &f, place(&geom(210.0, 210.0))), "Corse 2013");
    }

    /// Same forbidden list as the colophon page and the report channel.
    #[test]
    fn the_page_never_carries_a_path_a_coordinate_or_a_file_name() {
        let t = texte("Corse 2013", &faits(), place(&geom(210.0, 210.0)));
        assert!(!t.contains('/') && !t.contains('\\'), "{t}");
        assert!(!t.contains('\u{b0}'), "{t}");
        for ext in [".jpg", ".JPG", ".png", ".heic", ".HEIC"] {
            assert!(!t.contains(ext), "{ext} dans la page : {t}");
        }
    }

    /// The spread holds text and nothing else: no slot, no caption, and no
    /// edit flag, so a recomposition rebuilds it like any machine page.
    #[test]
    fn the_spread_holds_text_and_nothing_else() {
        let s = spread("Corse 2013", &faits(), place(&geom(210.0, 210.0)));
        assert_eq!(s.template, TEMPLATE);
        assert!(s.slots.is_empty());
        assert!(s.caption.is_none());
        assert!(!s.edited && !s.locked);
        assert!(s.text.as_deref().unwrap().starts_with("Corse 2013"));
        assert!(crate::pdf::slots_for(TEMPLATE, 0, &geom(210.0, 210.0)).is_empty());
    }

    /// The title is the first line, the quiet ones follow at their own size
    /// under the air that separates them. The blank line of the stored text
    /// is spacing, not a line to draw.
    #[test]
    fn the_layout_gives_the_title_its_own_size() {
        let p = place(&geom(210.0, 210.0));
        let l = mise_en_page(&texte("Corse 2013", &faits(), p), p);
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].texte, "Corse 2013");
        assert_eq!(l[0].taille_pt, TITRE_PT);
        assert_eq!(l[0].dy_mm, 0.0);
        assert_eq!(l[1].taille_pt, LIGNE_PT);
        assert_eq!(l[1].dy_mm, APRES_TITRE_MM);
        assert_eq!(l[2].dy_mm, APRES_TITRE_MM + LIGNE_LEADING_MM);
    }

    /// A title that fits keeps the full size, and a title anybody would
    /// actually type fits: sixty-four ordinary characters still print at
    /// eighteen points. One that does not shrinks to the page rather than
    /// running off it, and nothing is ever cut.
    #[test]
    fn a_long_title_shrinks_instead_of_overflowing() {
        let p = place(&geom(210.0, 210.0));
        assert_eq!(taille_titre("Corse 2013", p), TITRE_PT);
        let long = "Vacances en Corse avec les cousins et les cousines, octobre 2013";
        assert_eq!(long.chars().count(), TITRE_MAX);
        assert_eq!(taille_titre(long, p), TITRE_PT);
        let large: String = std::iter::repeat_n('W', TITRE_MAX).collect();
        let t = taille_titre(&large, p);
        assert!(t < TITRE_PT, "{t}");
        assert!(crate::font::text_width_mm(&large, t) <= p + 1e-9);
    }

    /// The guarantee the interface leans on: the longest title the editor
    /// accepts, set in the widest glyph the renderer can print, fits the
    /// narrowest format, and the two quiet lines fit under it with the
    /// longest town names the gazetteer holds. Measured with the face the
    /// PDF embeds, on the six formats, at the margin, not at the trim.
    #[test]
    fn the_longest_title_the_editor_accepts_fits_every_format() {
        // The widest printable character, taken from the face itself rather
        // than guessed: a nastier glyph added tomorrow fails this test.
        let pire = (0x20u8..=0x7Eu8)
            .map(char::from)
            .chain(['É', 'Œ', 'Æ', '—', '…', '«', '»', '%', '@'])
            .max_by(|a, b| {
                crate::font::text_width_mm(&a.to_string(), 100.0)
                    .partial_cmp(&crate::font::text_width_mm(&b.to_string(), 100.0))
                    .unwrap()
            })
            .unwrap();
        let titre: String = std::iter::repeat_n(pire, TITRE_MAX).collect();
        let mut f = faits();
        f.debut = chrono::NaiveDate::from_ymd_opt(2013, 12, 29);
        f.fin = chrono::NaiveDate::from_ymd_opt(2014, 1, 2);
        f.lieux = vec![
            "Saint-Remy-en-Bouzemont-Saint-Genest-et-Isson".into(),
            "Beaujeu-Saint-Vallier-Pierrejux-et-Quitteur".into(),
            "Châteauneuf-de-Galaure".into(),
        ];
        for (nom, w, h, _) in crate::format::FORMATS {
            let g = geom(*w, *h);
            let p = place(&g);
            let at = anchor(&g);
            for l in mise_en_page(&texte(&titre, &f, p), p) {
                let large = crate::font::text_width_mm(&l.texte, l.taille_pt);
                assert!(
                    large <= p + 1e-9,
                    "{nom} : « {} » fait {large:.1} mm pour {p:.1} mm de place",
                    l.texte
                );
                assert!(l.taille_pt >= TITRE_PT_MIN, "{nom} : titre sous le plancher");
                // And the block stays on the page, above the trimmed foot.
                assert!(at.y - l.dy_mm > g.bleed, "{nom} : le bloc descend sous la coupe");
            }
        }
    }
}
