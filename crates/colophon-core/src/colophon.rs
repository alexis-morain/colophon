//! The colophon page. The software is called Colophon and did not print one.
//!
//! A colophon says what a book is made of, and this one can say things only
//! the machine knows: how many photographs were looked at before these were
//! kept, what stretch of time they cover, which towns they were taken in,
//! which cameras took them. Nobody would type that page, and every book
//! deserves it.
//!
//! Two rules the page shares with the report channel, and for the same
//! reason: **never a full path, never a coordinate, never a caption**. The
//! town names come from the gazetteer resolving the GPS the cameras wrote
//! ([`crate::places`]), not from what the user typed in a chapter line, and
//! no file name ever reaches the page either.
//!
//! The facts are computed once, at composition, and stored in `album.json`:
//! recomputing them would mean reopening every original, and none of them
//! changes when the album is edited.
//!
//! The paper is the one figure that does not belong to the album. Today every
//! profile reads [`crate::printer::GRAMMAGE_DEFAUT`], the same figure the
//! preflight sheet declares, because no supplier has confirmed a grammage
//! yet. [`texte`] takes it as an argument rather than reading it, so the day
//! a profile carries its own the page follows the file being written.

use crate::model::Size;
use serde::{Deserialize, Serialize};

/// The template name of the colophon spread. It carries no photo, like
/// `vide` and `texte`, so every geometry already returns nothing for it.
pub const TEMPLATE: &str = "colophon";

/// Quieter than a text page (11 pt): this is the page nobody has to read.
pub const SIZE_PT: f64 = 8.5;
pub const LEADING_MM: f64 = 4.6;

/// At most this many towns and cameras. A list of fifteen villages is a
/// database dump, not a colophon; the first ones are the ones that matter.
const MAX_LIEUX: usize = 6;
const MAX_APPAREILS: usize = 3;

/// What the composition knew about the album, kept for the page. Everything
/// here is a count, a date, a town or a camera model: nothing that could
/// identify a file on a disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Faits {
    /// Photographs the album shows, over the photographs the folder held.
    pub photos_retenues: usize,
    pub photos_scannees: usize,
    /// Span of the trusted capture dates. Absent when no photo carried one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debut: Option<chrono::NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fin: Option<chrono::NaiveDate>,
    /// Towns the gazetteer agreed on, in reading order, deduplicated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lieux: Vec<String>,
    /// Camera models from EXIF, most used first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub appareils: Vec<String>,
    pub compose_le: chrono::NaiveDate,
}

/// Gather the facts from a finished composition. Called once, at build time,
/// where the photos are already open and the chapters already resolved.
pub fn faits(
    chapitres: &[crate::pipeline::Chapter],
    photos_retenues: usize,
    photos_scannees: usize,
    aujourdhui: chrono::NaiveDate,
) -> Faits {
    let mut debut: Option<chrono::NaiveDate> = None;
    let mut fin: Option<chrono::NaiveDate> = None;
    let mut lieux: Vec<String> = Vec::new();
    // Counted, not just collected: a phone that took four hundred photos and
    // a borrowed camera that took two are not equally worth naming.
    let mut appareils: Vec<(String, usize)> = Vec::new();

    for c in chapitres {
        if let Some(ville) = crate::build::chapter_place(c) {
            if !lieux.iter().any(|l| l == ville) {
                lieux.push(ville.to_string());
            }
        }
        for p in &c.photos {
            if p.meta.taken_reliable {
                let d = p.meta.taken.date();
                debut = Some(debut.map_or(d, |x: chrono::NaiveDate| x.min(d)));
                fin = Some(fin.map_or(d, |x: chrono::NaiveDate| x.max(d)));
            }
            let Some(modele) = p.meta.model.as_ref().map(|m| m.trim()) else { continue };
            if modele.is_empty() {
                continue;
            }
            match appareils.iter_mut().find(|(m, _)| m == modele) {
                Some((_, n)) => *n += 1,
                None => appareils.push((modele.to_string(), 1)),
            }
        }
    }
    appareils.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    lieux.truncate(MAX_LIEUX);

    Faits {
        photos_retenues,
        photos_scannees,
        debut,
        fin,
        lieux,
        appareils: appareils
            .into_iter()
            .take(MAX_APPAREILS)
            .map(|(m, _)| m)
            .collect(),
        compose_le: aujourdhui,
    }
}

/// The span the album covers, as a sentence fragment: « le 21 octobre 2013 »
/// for a single day, « du 21 au 29 octobre 2013 » when one month holds both
/// ends, « du 29 décembre 2013 au 2 janvier 2014 » when it does not.
///
/// `None` when no photo carried a trusted capture date, and every page that
/// asks then says nothing about time rather than printing a copy date as a
/// shooting date. Shared with [`crate::garde`] so the two pages of a book
/// cannot come to two different readings of the same trip.
pub fn periode(
    debut: Option<chrono::NaiveDate>,
    fin: Option<chrono::NaiveDate>,
) -> Option<String> {
    use chrono::Datelike;
    let (d, e) = (debut?, fin?);
    if d == e {
        return Some(format!("le {}", crate::build::date_fr(d, true)));
    }
    let depart = if d.year() != e.year() {
        crate::build::date_fr(d, true)
    } else if d.month() != e.month() {
        crate::build::date_fr(d, false)
    } else {
        d.day().to_string()
    };
    Some(format!("du {depart} au {}", crate::build::date_fr(e, true)))
}

/// The page as it prints, line by line. Blank lines separate the three
/// blocks: what was kept, where and with what, and what the object is.
///
/// `grammage` comes from the supplier chosen at export time, which is why
/// this takes it rather than reading it from the album: the paper named on
/// the page is the paper of the file being written.
pub fn texte(f: &Faits, trim: Size, grammage: f64, version: &str) -> String {
    let mut l: Vec<String> = Vec::new();
    l.push("Colophon".into());
    l.push(String::new());

    let retenues = if f.photos_retenues == 1 {
        "1 photographie retenue".to_string()
    } else {
        format!("{} photographies retenues", f.photos_retenues)
    };
    l.push(match periode(f.debut, f.fin) {
        Some(p) => format!("{retenues} sur {}, prises {p}.", f.photos_scannees),
        // No photo carried a trusted capture date: the page says the counts
        // and stops, rather than printing a copy date as a shooting date.
        None => format!("{retenues} sur {}.", f.photos_scannees),
    });

    if !f.lieux.is_empty() {
        l.push(liste(&f.lieux) + ".");
    }
    if !f.appareils.is_empty() {
        l.push(liste(&f.appareils) + ".");
    }

    l.push(String::new());
    l.push(format!(
        "Composé le {} avec Colophon {}.",
        crate::build::date_fr(f.compose_le, true),
        version
    ));
    l.push(format!(
        "{} × {} mm, papier {} g/m\u{b2}.",
        arrondi(trim.w),
        arrondi(trim.h),
        arrondi(grammage)
    ));
    l.join("\n")
}

/// « a, b et c ». The last separator is a word, the way a sentence joins
/// things, not a comma the way a database does.
fn liste(v: &[String]) -> String {
    match v.len() {
        0 => String::new(),
        1 => v[0].clone(),
        _ => format!("{} et {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
    }
}

/// Millimetres print whole when they are whole: « 210 × 210 mm », not
/// « 210.0 × 210.0 mm ».
fn arrondi(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

/// The spread the album carries, text already rendered. A spread rather than
/// a late-bound flag on purpose: the page then counts in the pagination the
/// suppliers sanction, shows up in the editor, and travels through the
/// preflight, all without a single special case.
pub fn spread(f: &Faits, trim: Size, grammage: f64, version: &str) -> crate::model::Spread {
    crate::model::Spread {
        template: TEMPLATE.into(),
        slots: Vec::new(),
        caption: None,
        text: Some(texte(f, trim, grammage, version)),
        edited: false,
        locked: false,
        objets: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn faits_test() -> Faits {
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

    fn trim() -> Size {
        Size { w: 210.0, h: 210.0 }
    }

    #[test]
    fn the_page_says_what_only_the_machine_knows() {
        let t = texte(&faits_test(), trim(), 150.0, "0.9.0");
        assert!(t.contains("152 photographies retenues sur 575"));
        assert!(t.contains("prises du 21 au 29 octobre 2013."), "{t}");
        assert!(t.contains("Porto-Vecchio et Bonifacio."));
        assert!(t.contains("Canon EOS 550D."));
        assert!(t.contains("Composé le 17 août 2026 avec Colophon 0.9.0."));
        assert!(t.contains("210 × 210 mm, papier 150 g/m²"));
    }

    /// Same forbidden list as the report channel. A page that leaked a path
    /// would leak it to a printer, on paper, forever.
    #[test]
    fn the_page_never_carries_a_path_a_coordinate_or_a_caption() {
        let mut f = faits_test();
        f.lieux = vec!["Bonifacio".into()];
        let t = texte(&f, trim(), 150.0, "0.9.0");
        assert!(!t.contains('\\'), "{t}");
        for ext in [".jpg", ".JPG", ".png", ".heic", ".HEIC"] {
            assert!(!t.contains(ext), "{ext} dans la page : {t}");
        }
        // A coordinate would print as a decimal degree pair; only the town
        // travels, and only from the gazetteer.
        assert!(!t.contains('\u{b0}'), "{t}");
        // The one slash the page may hold is the one in « g/m² ». Any other
        // is a path that escaped.
        assert_eq!(t.matches('/').count(), 1, "{t}");
        assert!(t.contains("g/m\u{b2}"), "{t}");
    }

    /// No trusted capture date anywhere: the page counts and stops rather
    /// than printing a file copy date as a shooting date.
    #[test]
    fn without_a_trusted_date_the_page_says_nothing_about_time() {
        let mut f = faits_test();
        f.debut = None;
        f.fin = None;
        let t = texte(&f, trim(), 150.0, "0.9.0");
        assert!(t.contains("152 photographies retenues sur 575."));
        assert!(!t.contains("prises"));
    }

    /// A single day, a single photo, a single camera: no plural, no range.
    #[test]
    fn one_of_everything_reads_as_one() {
        let f = Faits {
            photos_retenues: 1,
            photos_scannees: 1,
            debut: chrono::NaiveDate::from_ymd_opt(2013, 10, 21),
            fin: chrono::NaiveDate::from_ymd_opt(2013, 10, 21),
            lieux: vec!["Bonifacio".into()],
            appareils: vec!["iPhone 5".into()],
            compose_le: chrono::NaiveDate::from_ymd_opt(2026, 8, 17).unwrap(),
        };
        let t = texte(&f, trim(), 150.0, "0.9.0");
        assert!(t.contains("1 photographie retenue sur 1, prises le 21 octobre 2013."));
        assert!(t.contains("Bonifacio."));
    }

    /// A trip across a new year keeps the first year: « du 29 décembre 2013
    /// au 2 janvier 2014 », never « du 29 décembre au 2 janvier 2014 ».
    #[test]
    fn a_span_across_two_years_names_both() {
        let mut f = faits_test();
        f.debut = chrono::NaiveDate::from_ymd_opt(2013, 12, 29);
        f.fin = chrono::NaiveDate::from_ymd_opt(2014, 1, 2);
        let t = texte(&f, trim(), 150.0, "0.9.0");
        assert!(t.contains("du 29 décembre 2013 au 2 janvier 2014"), "{t}");
    }

    /// The paper is the supplier's, so it follows the profile and not the
    /// album: two exports of the same book can name two papers.
    #[test]
    fn the_paper_follows_the_supplier() {
        let f = faits_test();
        assert!(texte(&f, trim(), 170.0, "0.9.0").contains("papier 170 g/m²"));
        assert!(texte(&f, trim(), 115.0, "0.9.0").contains("papier 115 g/m²"));
    }

    /// The spread carries no photo and no caption: every geometry, every
    /// counter and every preflight already knows what to do with that.
    #[test]
    fn the_spread_holds_text_and_nothing_else() {
        let s = spread(&faits_test(), trim(), 150.0, "0.9.0");
        assert_eq!(s.template, TEMPLATE);
        assert!(s.slots.is_empty());
        assert!(s.caption.is_none());
        assert!(!s.edited && !s.locked);
        assert!(s.text.as_deref().unwrap().starts_with("Colophon"));
    }

    /// The page prints as typed, like every text page: nothing is wrapped
    /// and nothing is cut. So it has to fit on the narrowest format, with
    /// the fullest content the generator can produce, or it would run off
    /// the paper on somebody's book. Measured with the face the PDF embeds.
    #[test]
    fn the_page_fits_the_recto_on_every_format() {
        let plein = Faits {
            photos_retenues: 1875,
            photos_scannees: 12480,
            debut: chrono::NaiveDate::from_ymd_opt(2013, 12, 29),
            fin: chrono::NaiveDate::from_ymd_opt(2014, 1, 2),
            lieux: vec![
                "Villefranche-sur-Saône".into(),
                "Saint-Rémy-de-Provence".into(),
                "Bagnères-de-Bigorre".into(),
                "Châteauneuf-du-Pape".into(),
                "Fontenay-sous-Bois".into(),
                "Boulogne-Billancourt".into(),
            ],
            appareils: vec![
                "Panasonic DMC-TZ10 / V-LUX 20".into(),
                "Canon EOS 5D Mark III".into(),
                "iPhone 15 Pro Max".into(),
            ],
            compose_le: chrono::NaiveDate::from_ymd_opt(2026, 12, 28).unwrap(),
        };
        for (nom, w, h, _) in crate::format::FORMATS {
            let mut a = crate::model::Album::new("t", std::path::Path::new("/p"), Size { w: *w, h: *h });
            a.spreads.push(spread(&plein, a.trim_mm, 150.0, "0.9.0"));
            let g = crate::pdf::geometry(&a);
            let at = crate::pdf::colophon_anchor(&g);
            // From the first baseline to the trimmed edge of the recto page.
            let place = g.media_w - g.bleed - at.x;
            for ligne in texte(&plein, a.trim_mm, 150.0, "0.9.0").lines() {
                let large = crate::font::text_width_mm(ligne, SIZE_PT);
                assert!(
                    large <= place,
                    "{nom} : « {ligne} » fait {large:.1} mm pour {place:.1} mm de place"
                );
            }
            // And the block has to stop above the bottom of the page.
            let lignes = texte(&plein, a.trim_mm, 150.0, "0.9.0").lines().count();
            let bas = at.y - (lignes - 1) as f64 * LEADING_MM;
            assert!(bas > g.bleed, "{nom} : le bloc descend sous la coupe");
        }
    }

    #[test]
    fn a_list_joins_with_a_word() {
        assert_eq!(liste(&["a".into()]), "a");
        assert_eq!(liste(&["a".into(), "b".into()]), "a et b");
        assert_eq!(liste(&["a".into(), "b".into(), "c".into()]), "a, b et c");
    }
}
