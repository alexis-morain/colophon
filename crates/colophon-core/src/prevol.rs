//! Preflight. The gate between a composed album and a print order.
//!
//! Every check is read against a [`PrinterProfile`], never against a constant:
//! the same album passes at one supplier and fails at the next, and that is
//! the point. A blocking defect stops the export — a print run costs real
//! money and a reprint costs it twice.
//!
//! Every message names **the spread and the cause**, in words, and says what
//! to do about it. A preflight that answers `ERR_RES_LOW` sends the user to a
//! forum instead of to the crop editor.

use crate::model::Album;
use crate::printer::{Certitude, Dos, Espace, Fichiers, PdfX, PrinterProfile, GRAMMAGE_DEFAUT};
use crate::{heic, meta, pdf, print};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// One thing wrong with the file, named the way a human would name it.
#[derive(Debug, Serialize)]
pub struct Defaut {
    /// Rule that fired, for grouping. The human reads `cause`, not this.
    pub regle: &'static str,
    /// A blocking defect stops the export. A warning is worth knowing.
    pub bloquant: bool,
    /// 1-based, as the ruler shows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planche: Option<usize>,
    #[serde(rename = "case", skip_serializing_if = "Option::is_none")]
    pub case_idx: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    pub cause: String,
    /// The gesture that fixes it, in the editor.
    pub remede: String,
}

/// The sheet handed to whoever receives the PDF. Everything a printer asks
/// on the phone, written down once.
#[derive(Debug, Serialize)]
pub struct Fiche {
    pub imprimeur: &'static str,
    pub format_page_mm: [f64; 2],
    pub planches: usize,
    pub pages_interieur: usize,
    pub fond_perdu_mm: crate::printer::Bleed,
    pub zone_sure_mm: f64,
    pub espace: Espace,
    pub output_intent: &'static str,
    pub conformite: PdfX,
    pub fichiers: Fichiers,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dos_mm: Option<f64>,
    pub grammage_g_m2: f64,
    pub resolution_cible_dpi: f64,
}

#[derive(Debug, Serialize)]
pub struct PrevolReport {
    pub album: String,
    pub profil: &'static str,
    /// False as soon as one blocking defect stands.
    pub ok: bool,
    pub bloquants: usize,
    pub avertissements: usize,
    pub fiche: Fiche,
    /// What the profile itself is still waiting on, copied through so a
    /// provisional number never travels silently.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reserves: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    pub defauts: Vec<Defaut>,
}

/// Run the preflight over a composed album folder.
pub fn prevol(dir: &Path, profil: &'static PrinterProfile) -> Result<PrevolReport> {
    let json = dir.join("album.json");
    let album: Album = serde_json::from_str(
        &fs::read_to_string(&json).with_context(|| format!("lecture de {}", json.display()))?,
    )
    .context("album.json illisible")?;
    let dims = original_dimensions(&album);
    Ok(check(&album, profil, &dims))
}

/// Original pixel sizes, EXIF orientation applied, keyed by slot source.
/// Absent entries mean the folder moved: resolution then goes unchecked and
/// the report says so rather than passing quietly.
fn original_dimensions(album: &Album) -> HashMap<String, (u32, u32)> {
    let root = PathBuf::from(&album.root);
    if !root.is_dir() {
        return HashMap::new();
    }
    let mut srcs: Vec<&str> = album
        .spreads
        .iter()
        .flat_map(|s| s.slots.iter().map(|sl| sl.src.as_str()))
        .collect();
    srcs.sort_unstable();
    srcs.dedup();
    srcs.par_iter()
        .filter_map(|src| {
            let p = root.join(src);
            let m = meta::read(&p);
            let (w, h) = heic::dimensions(&p).ok()?;
            let (w, h) = if (5..=8).contains(&m.orientation) { (h, w) } else { (w, h) };
            Some(((*src).to_string(), (w, h)))
        })
        .collect()
}

/// The whole preflight on values already in memory, so the tests need no
/// photo folder.
pub fn check(
    album: &Album,
    profil: &'static PrinterProfile,
    dims: &HashMap<String, (u32, u32)>,
) -> PrevolReport {
    let mut defauts: Vec<Defaut> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let g = pdf::geometry(album);
    let pages = album.spreads.len() * 2;

    // 1. Pagination. A binding folds sheets: an odd count does not exist, and
    // every press has a range it will not leave.
    if !profil.pagination_ok(pages) {
        let cause = if pages % profil.pas_pagination != 0 {
            format!(
                "l'intérieur fait {pages} pages, or {} ne relie que des multiples de {}",
                profil.nom, profil.pas_pagination
            )
        } else {
            format!(
                "l'intérieur fait {pages} pages, hors des bornes de {} ({} à {})",
                profil.nom, profil.pages_min, profil.pages_max
            )
        };
        defauts.push(Defaut {
            regle: "pagination",
            bloquant: true,
            planche: None,
            case_idx: None,
            src: None,
            cause,
            remede: format!(
                "ajoutez ou retirez des planches : une planche vaut deux pages, il en faut entre {} et {}",
                profil.pages_min / 2,
                profil.pages_max / 2
            ),
        });
    }

    // 2. Bleed. The album carries one value; the profile wants a value per
    // edge. Rendering less than the printer trims puts white on the cut.
    let requis = profil.bleed_mm.max();
    if album.bleed_mm + 1e-9 < requis {
        defauts.push(Defaut {
            regle: "fond_perdu",
            bloquant: true,
            planche: None,
            case_idx: None,
            src: None,
            cause: format!(
                "l'album est composé avec {:.1} mm de fond perdu, {} en demande {:.1}",
                album.bleed_mm, profil.nom, requis
            ),
            remede: "recomposez l'album au fond perdu du profil : le fond perdu est fixé à la composition".into(),
        });
    }

    // 3. Colour space. We render RGB and never convert: a CMYK conversion done
    // blind is worse than no conversion at all, and lcms2 is not in yet.
    if profil.espace == Espace::Fogra39 {
        defauts.push(Defaut {
            regle: "espace",
            bloquant: true,
            planche: None,
            case_idx: None,
            src: None,
            cause: format!(
                "{} imprime en CMJN {} et Colophon n'exporte qu'en RVB",
                profil.nom,
                profil.espace.output_intent()
            ),
            remede: "choisissez un profil qui accepte le RVB, ou demandez à l'imprimeur de convertir".into(),
        });
    }

    // 4. PDF/X conformance. The face is embedded now, so the glyphs are in the
    // file; what a PDF/X file still owes is the declaration. Read from the
    // renderer rather than restated here, so the day it emits an OutputIntent
    // nobody has to remember this line.
    if profil.pdf_x == PdfX::X4 && !pdf::EMITS_PDF_X {
        defauts.push(Defaut {
            regle: "conformite",
            bloquant: true,
            planche: None,
            case_idx: None,
            src: None,
            cause: format!(
                "{} demande du PDF/X-4 et le fichier ne se déclare pas comme tel : les polices \
                 sont incorporées, mais l'OutputIntent et les métadonnées XMP manquent encore",
                profil.nom
            ),
            remede: "choisissez un profil sans conformité PDF/X, ou demandez à l'imprimeur s'il accepte un PDF simple en RVB".into(),
        });
    }

    // 5. Resolution, cell by cell. The one defect nobody sees on screen and
    // everybody sees on paper.
    if dims.is_empty() && album.spreads.iter().any(|s| !s.slots.is_empty()) {
        notes.push(
            "dossier de photos introuvable : la résolution effective n'a pas pu être vérifiée"
                .into(),
        );
    }
    for (si, spread) in album.spreads.iter().enumerate() {
        let rects = pdf::slots_for(&spread.template, spread.slots.len(), &g);
        for (ci, (slot, rect)) in spread.slots.iter().zip(rects.iter()).enumerate() {
            let Some(&(ow, oh)) = dims.get(&slot.src) else { continue };
            let scale = print::print_scale(rect, ow, oh) * slot.zoom.max(1.0);
            let ppi = print::PRINT_DPI / scale;
            if ppi < profil.min_ppi {
                defauts.push(Defaut {
                    regle: "resolution",
                    bloquant: true,
                    planche: Some(si + 1),
                    case_idx: Some(ci),
                    src: Some(slot.src.clone()),
                    cause: format!(
                        "{} imprimerait à {ppi:.0} ppi dans cette case, {} exige {:.0}",
                        slot.src, profil.nom, profil.min_ppi
                    ),
                    remede: "réduisez le zoom, mettez la photo dans une case plus petite, ou remplacez-la".into(),
                });
            }
        }
    }

    // 6. Safe zone. Photos bleed on purpose; text must not.
    for (si, spread) in album.spreads.iter().enumerate() {
        let rects = pdf::slots_for(&spread.template, spread.slots.len(), &g);
        let porte_legende = spread.caption.is_some()
            || spread.slots.iter().any(|s| s.caption.is_some());
        if !porte_legende {
            continue;
        }
        let p = pdf::caption_anchor(&rects, &g);
        let marge = distance_au_rognage(p.x, p.y, &g, album.bleed_mm);
        if marge + 1e-9 < profil.safe_mm {
            defauts.push(Defaut {
                regle: "zone_sure",
                bloquant: true,
                planche: Some(si + 1),
                case_idx: None,
                src: None,
                cause: format!(
                    "la légende passe à {marge:.1} mm du rognage, {} garde {:.1} mm libres",
                    profil.nom, profil.safe_mm
                ),
                remede: "raccourcissez la légende ou changez le gabarit de la planche".into(),
            });
        }
    }

    // 7. The spine. Not a defect: a number that travels, and must travel with
    // its provenance attached.
    let dos_mm = profil.dos_mm(pages, GRAMMAGE_DEFAUT);
    if let Dos::Calcule { certitude: Certitude::Provisoire, .. } = profil.dos {
        if let Some(d) = dos_mm {
            defauts.push(Defaut {
                regle: "dos",
                bloquant: false,
                planche: None,
                case_idx: None,
                src: None,
                cause: format!(
                    "dos calculé à {d:.1} mm avec un coefficient provisoire, non confirmé par {}",
                    profil.nom
                ),
                remede: "confirmez la formule auprès de l'imprimeur avant un tirage, ou mesurez sur le premier album reçu".into(),
            });
        }
    }

    let bloquants = defauts.iter().filter(|d| d.bloquant).count();
    let avertissements = defauts.len() - bloquants;

    PrevolReport {
        album: album.title.clone(),
        profil: profil.id,
        ok: bloquants == 0,
        bloquants,
        avertissements,
        fiche: Fiche {
            imprimeur: profil.nom,
            format_page_mm: [album.trim_mm.w, album.trim_mm.h],
            planches: album.spreads.len(),
            pages_interieur: pages,
            fond_perdu_mm: profil.bleed_mm,
            zone_sure_mm: profil.safe_mm,
            espace: profil.espace,
            output_intent: profil.espace.output_intent(),
            conformite: profil.pdf_x,
            fichiers: profil.fichiers,
            dos_mm,
            grammage_g_m2: GRAMMAGE_DEFAUT,
            resolution_cible_dpi: print::PRINT_DPI,
        },
        reserves: profil.reserves.iter().map(|s| (*s).to_string()).collect(),
        notes,
        defauts,
    }
}

/// Shortest distance from a point on the spread to the trimmed edge, in
/// millimetres. The media is the trim plus the bleed on all four sides.
fn distance_au_rognage(x: f64, y: f64, g: &pdf::SpreadGeometry, bleed: f64) -> f64 {
    let gauche = x - bleed;
    let bas = y - bleed;
    let droite = (g.media_w - bleed) - x;
    let haut = (g.media_h - bleed) - y;
    gauche.min(bas).min(droite).min(haut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Size, Slot, Spread};

    fn album_de(n: usize, bleed: f64) -> Album {
        let mut a = Album::new("t", Path::new("/p"), Size { w: 210.0, h: 210.0 });
        a.bleed_mm = bleed;
        for i in 0..n {
            a.spreads.push(Spread {
                template: "solo".into(),
                slots: vec![Slot::new(format!("{i}.jpg"), [0.5, 0.5])],
                caption: None,
                text: None,
                edited: false,
                locked: false,
            });
        }
        a
    }

    /// Big originals, matching bleed, sane pagination: nothing about the album
    /// itself stops the file. On a profile asking for no conformance it goes
    /// straight out; on Cloudprinter the only thing left standing is the
    /// PDF/X-4 declaration the writer does not yet emit.
    #[test]
    fn a_clean_album_passes() {
        let a = album_de(24, 3.0);
        let dims: HashMap<String, (u32, u32)> = (0..24)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();

        let r = check(&a, PrinterProfile::par_id("generique").unwrap(), &dims);
        assert!(r.ok, "défauts : {:?}", r.defauts);
        assert_eq!(r.bloquants, 0);

        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        let bloquants: Vec<&str> = r.defauts.iter().filter(|d| d.bloquant).map(|d| d.regle).collect();
        assert_eq!(bloquants, vec!["conformite"], "défauts : {:?}", r.defauts);
        // The provisional spine is still announced, as a warning.
        assert_eq!(r.avertissements, 1);
        assert!(r.fiche.dos_mm.unwrap() > 0.0);
    }

    /// The same album fails or passes depending on the supplier. That is the
    /// whole reason the profile is data.
    #[test]
    fn the_same_album_answers_differently_per_supplier() {
        let a = album_de(12, 0.0); // 24 pages, no bleed
        let dims: HashMap<String, (u32, u32)> = (0..12)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();

        // Prodigi generates the bleed itself and takes 24 pages: nothing about
        // the album bothers it, only the conformance declaration.
        let r = check(&a, PrinterProfile::par_id("prodigi").unwrap(), &dims);
        let bloquants: Vec<&str> = r.defauts.iter().filter(|d| d.bloquant).map(|d| d.regle).collect();
        assert_eq!(bloquants, vec!["conformite"], "{:?}", r.defauts);

        // Cloudprinter wants 3 mm of bleed we did not render: blocked.
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        assert!(!r.ok);
        assert!(r.defauts.iter().any(|d| d.regle == "fond_perdu" && d.bloquant));

        // Lulu wants CMYK, which we do not produce, and 32 pages minimum.
        let r = check(&a, PrinterProfile::par_id("lulu").unwrap(), &dims);
        assert!(!r.ok);
        assert!(r.defauts.iter().any(|d| d.regle == "espace"));
        assert!(r.defauts.iter().any(|d| d.regle == "pagination"));
    }

    /// An odd page count cannot be bound, and the message says so in words.
    #[test]
    fn pagination_is_named_in_words() {
        let mut a = album_de(11, 3.0); // 22 pages, under the minimum of 24
        a.bleed_mm = 3.0;
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &HashMap::new());
        let d = r.defauts.iter().find(|d| d.regle == "pagination").unwrap();
        assert!(d.cause.contains("22 pages"), "{}", d.cause);
        assert!(!d.cause.contains("ERR"), "aucun code dans le message");
        assert!(!d.remede.is_empty());
    }

    /// A small original in a full-page cell is caught, and the finding names
    /// the spread, the cell and the file.
    #[test]
    fn low_resolution_names_the_spread_and_the_file() {
        let a = album_de(12, 3.0);
        let mut dims: HashMap<String, (u32, u32)> = (0..12)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();
        dims.insert("3.jpg".into(), (600, 600)); // far under print need
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        let d = r.defauts.iter().find(|d| d.regle == "resolution").unwrap();
        assert_eq!(d.planche, Some(4));
        assert_eq!(d.src.as_deref(), Some("3.jpg"));
        assert!(d.cause.contains("ppi"), "{}", d.cause);
        assert!(!r.ok);
    }

    /// Missing photos never turn into a pass: the report says the check did
    /// not run.
    #[test]
    fn unreachable_photos_are_said_out_loud() {
        let a = album_de(12, 3.0);
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &HashMap::new());
        assert!(r.notes.iter().any(|n| n.contains("résolution")));
    }

    /// A profile demanding PDF/X-4 is blocked while the writer does not
    /// declare it, and a profile that asks for no conformance goes through.
    /// The check follows the renderer, so this test flips on its own the day
    /// the OutputIntent lands.
    #[test]
    fn pdf_x_conformance_blocks_only_the_profiles_that_ask_for_it() {
        let a = album_de(12, 3.0);
        let dims: HashMap<String, (u32, u32)> = (0..12)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();

        let r = check(&a, PrinterProfile::par_id("prodigi").unwrap(), &dims);
        assert_eq!(
            r.defauts.iter().any(|d| d.regle == "conformite" && d.bloquant),
            !crate::pdf::EMITS_PDF_X
        );

        let r = check(&a, PrinterProfile::par_id("generique").unwrap(), &dims);
        assert!(!r.defauts.iter().any(|d| d.regle == "conformite"));
    }

    /// The face really is in the file, and its licence really does allow it.
    /// The structural blocker that stood before this session.
    #[test]
    fn the_text_face_is_embeddable() {
        let m = crate::font::metrics().expect("police lisible");
        assert!(m.embeddable());
    }

    /// The spec sheet carries what a printer asks on the phone.
    #[test]
    fn the_sheet_answers_the_printers_questions() {
        let a = album_de(24, 3.0);
        let dims: HashMap<String, (u32, u32)> = (0..24)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        let f = &r.fiche;
        assert_eq!(f.format_page_mm, [210.0, 210.0]);
        assert_eq!(f.pages_interieur, 48);
        assert_eq!(f.fichiers, Fichiers::Deux);
        assert_eq!(f.output_intent, "sRGB IEC61966-2.1");
        assert!(f.dos_mm.is_some());
        // A provisional profile never travels without its reservations.
        assert!(!r.reserves.is_empty());
    }
}
