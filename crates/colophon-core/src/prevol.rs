//! Preflight. The gate between a composed album and a print order.
//!
//! Every check is read against a [`PrinterProfile`], never against a constant:
//! the same album passes at one supplier and fails at the next, and that is
//! the point. A blocking defect stops the export: a print run costs real
//! money and a reprint costs it twice.
//!
//! Every message names **the spread and the cause**, in words, and says what
//! to do about it. A preflight that answers `ERR_RES_LOW` sends the user to a
//! forum instead of to the crop editor.

use crate::model::Album;
use crate::printer::{Certitude, Dos, Espace, Fichiers, PdfX, PrinterProfile, GRAMMAGE_DEFAUT};
use crate::{cover, heic, imposition, meta, pdf, print, scene};
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
    /// Pages in the delivered PDF: the interior, plus the two cover leaves
    /// when the supplier binds a single file. This is the number that gets
    /// declared at the order, and Prodigi holds an order whose declared count
    /// disagrees with the file it received.
    pub pages_fichier: usize,
    pub fond_perdu_mm: crate::printer::Bleed,
    pub zone_sure_mm: f64,
    /// The flat cover sheet, in millimetres, when the supplier expects one
    /// from us. Absent when they bind a single file and build their own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feuille_couverture_mm: Option<[f64; 2]>,
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

/// The page the machine writes that the album could do without, named as the
/// Envoi screen names it. Both are ordinary spreads costing two pages like
/// any other, which is exactly why the pagination rule has to know about
/// them: they are the two spreads a user can drop in a single click.
fn page_decochable(album: &Album) -> Option<&'static str> {
    let a = |t: &str| album.spreads.iter().any(|s| s.template == t);
    if a(crate::colophon::TEMPLATE) {
        Some("de colophon")
    } else if a(crate::garde::TEMPLATE) {
        Some("de garde")
    } else {
        None
    }
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
    // La photo de couverture est un slot de plus, et pas un slot de planche :
    // elle n'apparaît nulle part dans `spreads`, donc sans cette ligne le
    // prévol de la couverture n'aurait jamais de dimensions à mesurer et se
    // tairait exactement là où il vient d'apprendre à parler.
    if let Some(slot) = album.cover.as_ref().and_then(|c| c.photo.as_ref()) {
        srcs.push(slot.src.as_str());
    }
    srcs.sort_unstable();
    srcs.dedup();
    srcs.par_iter()
        .filter_map(|src| {
            let p = root.join(src);
            let m = meta::read(&p);
            let taille = heic::oriente(heic::dimensions(&p).ok()?, m.orientation);
            Some(((*src).to_string(), taille))
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
    // Zero unless the supplier cuts the block page by page, and then every
    // rectangle below is the one the composition produced.
    let pli = imposition::pli_mm(profil);
    let pages = album.spreads.len() * 2;
    // What the supplier counts is the file, not the book block. A supplier
    // who binds one file finds the cover in it, two pages the interior does
    // not have, and their bounds are written against that total. Two is also
    // the number that has to be declared at the order, and Prodigi puts an
    // order on hold when it disagrees with the file.
    let pages_fichier = pages + if profil.fichiers == Fichiers::Un { 2 } else { 0 };

    // 1. Pagination. A binding folds sheets: an odd count does not exist, and
    // every press has a range it will not leave.
    if !profil.pagination_ok(pages_fichier) {
        let cause = if pages_fichier % profil.pas_pagination != 0 {
            format!(
                "le fichier fait {pages_fichier} pages, or {} ne relie que des multiples de {}",
                profil.nom, profil.pas_pagination
            )
        } else {
            format!(
                "le fichier fait {pages_fichier} pages, hors des bornes de {} ({} à {})",
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
            // The colophon page is two of those pages, and an album sitting
            // exactly on the upper bound is pushed over by it. Say so where
            // it is read, rather than letting somebody hunt for two pages
            // through a book of a hundred spreads.
            remede: if let Some(page) = page_decochable(album)
                .filter(|_| profil.pagination_ok(pages_fichier - 2))
            {
                format!("décochez la page {page} dans l'écran Envoi : elle vaut deux pages, et sans elle le compte tombe juste")
            } else {
                format!(
                    "ajoutez ou retirez des planches : une planche vaut deux pages, il en faut entre {} et {}",
                    profil.pages_min / 2,
                    profil.pages_max / 2
                )
            },
        });
    }

    // 1 bis. The imposition. A supplier who reads one PDF page as one page of
    // the book gets the interior cut in two, and the cut has a price: the left
    // half of the first spread is the page that faces the inside of the cover,
    // and it is not printed. That is what puts the half-title on page one,
    // where a book puts it. It only holds if that half is empty — which it is
    // of a half-title, and is not of a spread of photographs — so a spread
    // carrying something there is refused rather than quietly truncated.
    if profil.pages_simples {
        if let Some(premiere) = album.spreads.first() {
            let objets = imposition::objets_a_gauche(&scene::Scene::of(premiere, &g), &g);
            if objets > 0 {
                defauts.push(Defaut {
                    regle: "imposition",
                    bloquant: true,
                    planche: Some(1),
                    case_idx: None,
                    src: None,
                    cause: format!(
                        "{} relie une page de PDF par page de livre, et la page de gauche de la \
                         première planche ne s'imprime pas : elle porte {objets} objet{} qui \
                         seraient perdus",
                        profil.nom,
                        if objets > 1 { "s" } else { "" }
                    ),
                    remede: "cochez la page de garde dans l'écran Envoi : elle se pose en tête et \
                             sa page de gauche reste blanche"
                        .into(),
                });
            }
        }
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

    // 4. PDF/X conformance. Read from the renderer rather than restated here,
    // so nobody has to remember this line the day the declaration changes.
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
    } else if profil.pdf_x == PdfX::X4 {
        // The declaration is there and measured, but no free validator
        // certifies PDF/X-4: veraPDF ships PDF/A profiles only. Saying which
        // is which here costs one line and stops the spec sheet from
        // promising a verdict nobody delivered.
        notes.push(
            "conformité PDF/X-4 déclarée : polices incorporées, OutputIntent sRGB avec profil ICC, \
             XMP et TrimBox vérifiés à chaque export. Le contrôle indépendant disponible est \
             PDF/A-2b (veraPDF), qui couvre le même socle ; le verdict PDF/X-4 lui-même revient au \
             prévol de l'imprimeur."
                .into(),
        );
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
        let scene = scene::Scene::of(spread, &g);
        for (ci, object) in scene.objects.iter().enumerate() {
            let scene::Role::Photo { src, zoom, .. } = &object.role else { continue };
            let Some(&(ow, oh)) = dims.get(src) else { continue };
            // The rectangle **this supplier** receives. A photograph that runs
            // to the fold gains the fold bleed on the way out, so it is
            // cover-cropped a little wider for the same pixels: measuring the
            // composed rectangle would promise 250 ppi where the press gets
            // less. Identical to `object.rect` for everyone who binds spreads.
            let rect = &imposition::rect_exporte(object, &g, pli);
            let slot = &spread.slots[ci];
            let scale = print::print_scale(rect, ow, oh) * zoom.max(1.0);
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

    // 6. Safe zone. Photos bleed on purpose; text must not — and this warns
    // rather than refuses, which is the doctrine already written for the free
    // objects in 6.4: the fold stops, the margin warns. What the guillotine
    // goes through comes back mutilated and is refused; a caption a supplier
    // would rather see further in is a preference, and preferences differ by
    // a factor of two between the four profiles here.
    //
    // It had to change, and the measurement is what changed it. The caption's
    // baseline is `bleed + margin × CAPTION_SAFE`, half the margin of the
    // format: 7,00 mm on five formats and 6,75 on `portrait-20x25`. Blocking,
    // it made every captioned album fail at Prodigi and at Lulu — 8 spreads of
    // `corse-2013`, on `main`, for a rule nobody could satisfy — and the only
    // thing keeping Cloudprinter quiet was a `safe_mm` of 5 that nobody had
    // read off their specification. A threshold set to silence a rule measures
    // nothing. Soft, the three real numbers can finally be carried: 7, 10 and
    // 12,7, each the supplier's own.
    //
    // The real fix is upstream and is not this session's: a caption at
    // `max(0,5 × marge, plancher)` would move every caption of every album,
    // which is its own wave.
    for (si, spread) in album.spreads.iter().enumerate() {
        let porte_legende = spread.caption.is_some()
            || spread.slots.iter().any(|s| s.caption.is_some());
        if !porte_legende {
            continue;
        }
        // The anchor comes from the scene rather than from a second
        // derivation of the same rectangles. What is measured stays the
        // baseline, not the ink around it: measuring the box instead would
        // move the verdict, and moving a verdict belongs in a session that
        // can weigh it, not in one that promises invisibility.
        let rects = pdf::slots_for(&spread.template, spread.slots.len(), &g);
        let p = pdf::caption_anchor(&spread.template, &rects, &g);
        let marge = distance_au_rognage(p.x, p.y, &g, album.bleed_mm);
        if marge + 1e-9 < profil.safe_mm {
            defauts.push(Defaut {
                regle: "zone_sure",
                bloquant: false,
                planche: Some(si + 1),
                case_idx: None,
                src: None,
                cause: format!(
                    "la légende passe à {marge:.1} mm du rognage, {} garde {:.1} mm libres",
                    profil.nom, profil.safe_mm
                ),
                // L'ancien remède — « raccourcissez la légende ou changez le
                // gabarit » — était faux : ni l'un ni l'autre ne déplace une
                // ligne de base calculée depuis la marge du format.
                remede: "retirez la légende de cette planche si le risque ne vous \
                         convient pas : sa ligne de base vient de la marge du format, ni la \
                         raccourcir ni changer de gabarit ne la déplace"
                    .into(),
            });
        }
    }

    // 6 bis. Free objects. Two refusals and two only: the cut, and the fold.
    // The margin stays soft — a block laid to the bleed on purpose is a
    // choice, the editor said so at the gesture and the linter counts it —
    // but a block the guillotine goes through comes back mutilated, and one
    // across the fold comes back split by the binding. Neither is measured
    // here: `distance_to_trim` and `traverse_le_pli` are the doctrine, and
    // this reads them with the object's own angle.
    //
    // **Whatever fills the object.** A fleuron the cut goes through is a
    // mutilated fleuron; the box is what is measured, and an ornament's box
    // is its ink.
    for (si, spread) in album.spreads.iter().enumerate() {
        for objet in scene::Scene::of(spread, &g).objects.iter() {
            let Some(index) = objet.role.index_libre() else { continue };
            let marge = scene::distance_to_trim(&objet.rect, objet.angle, &g);
            if marge < 0.0 {
                defauts.push(Defaut {
                    regle: "objet_coupe",
                    bloquant: true,
                    planche: Some(si + 1),
                    case_idx: None,
                    src: None,
                    cause: format!(
                        "l'objet libre n° {} dépasse de {:.1} mm au-delà du rognage : \
                         la coupe passe dedans",
                        index + 1,
                        -marge
                    ),
                    remede: "déplacez l'objet vers l'intérieur de la page, ou réduisez-le".into(),
                });
            }
            if scene::traverse_le_pli(&objet.rect, objet.angle, &g) {
                defauts.push(Defaut {
                    regle: "objet_pli",
                    bloquant: true,
                    planche: Some(si + 1),
                    case_idx: None,
                    src: None,
                    cause: format!(
                        "l'objet libre n° {} est à cheval sur le pli : la reliure le \
                         coupera en deux",
                        index + 1
                    ),
                    remede: "glissez l'objet d'un seul côté du pli : l'éditeur y bute tout seul".into(),
                });
            }
        }
    }

    // 7. La couverture, mesurée sur la feuille que ce fournisseur reçoit.
    //
    // Les trois cotes du cartonné — rempli, débord, mors — n'agrandissent pas
    // le livre, elles agrandissent le carton autour de lui : chez Cloudprinter
    // la première passe de 213 × 216 à 239 × 258 mm. La photo qu'on y pose
    // remplit donc une surface un huitième plus grande avec les mêmes pixels,
    // et son ppi effectif tombe d'autant. Mesuré en session A sur les 98
    // photographies de corse-2013 : 2 étaient sous le plancher en couverture
    // souple, 36 le sont en cartonné. C'est la page que tout le monde regarde,
    // et le prévol n'en disait rien du tout.
    //
    // Le rectangle vient de `cover`, jamais reconstruit ici : une feuille à
    // plat et un feuillet volant ne sont pas la même surface, et le prévol qui
    // choisirait à leur place mesurerait une couverture que personne n'imprime.
    let cg = cover::geometry(album, profil);
    if let Some(slot) = album.cover.as_ref().and_then(|c| c.photo.as_ref()) {
        if let Some(&(ow, oh)) = dims.get(slot.src.as_str()) {
            let rect = cover::photo_rect_du_profil(album, profil);
            let scale = print::print_scale(&rect, ow, oh) * slot.zoom.max(1.0);
            let ppi = print::PRINT_DPI / scale;
            if ppi < profil.min_ppi {
                defauts.push(Defaut {
                    regle: "couverture_resolution",
                    bloquant: true,
                    planche: None,
                    case_idx: None,
                    src: Some(slot.src.clone()),
                    cause: format!(
                        "{} imprimerait à {ppi:.0} ppi sur la couverture de {:.0} × {:.0} mm, \
                         {} exige {:.0}",
                        slot.src, rect.w, rect.h, profil.nom, profil.min_ppi
                    ),
                    remede: "choisissez une photo plus définie pour la couverture : la \
                             feuille est plus grande que la page, elle en demande davantage"
                        .into(),
                });
            }
        }
    }

    // Et le dos, qui est une surface ou n'en est pas une. Sous le plancher de
    // `cover`, le titre n'est pas dessiné du tout — un titre sur un dos de
    // 6 mm rate le pli de plus que sa propre hauteur. Un dos nu est un livre
    // légitime, donc c'est un avertissement ; le taire serait laisser partir
    // une tranche vide sans que personne l'ait choisi.
    if profil.fichiers == Fichiers::Deux {
        if let Some(spine) = &cg.spine {
            if spine.w < cover::SPINE_TEXT_MIN_MM {
                defauts.push(Defaut {
                    regle: "dos_nu",
                    bloquant: false,
                    planche: None,
                    case_idx: None,
                    src: None,
                    cause: format!(
                        "le dos fait {:.1} mm : sous {:.0} mm il ne porte pas de titre, la tranche \
                         sortira nue",
                        spine.w,
                        cover::SPINE_TEXT_MIN_MM
                    ),
                    remede: "ajoutez des planches : le dos s'épaissit avec le livre, et le \
                             titre s'y pose tout seul"
                        .into(),
                });
            }
        }
    }

    // 8. The spine. Not a defect: a number that travels, and must travel with
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
            pages_fichier,
            fond_perdu_mm: profil.bleed_mm,
            zone_sure_mm: profil.safe_mm,
            // La feuille à plat, quand c'est nous qui la livrons. C'est la
            // première question d'un imprimeur qui relie du cartonné, et
            // jusqu'ici la fiche ne savait pas y répondre : les trois cotes
            // vivaient dans le profil et n'atteignaient aucun humain. Absente
            // chez qui relie un seul fichier, où la couverture est deux
            // feuillets de l'intérieur et n'a pas de feuille à elle.
            feuille_couverture_mm: (profil.fichiers == Fichiers::Deux)
                .then(|| [cg.media_w, cg.media_h]),
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
/// Distance from a point to the guillotine. One implementation of the
/// doctrine, in [`crate::scene::distance_to_trim`], reached here with a
/// rectangle of no extent: what must survive the cut is measured from the
/// cut, and it is measured the same way for everyone.
fn distance_au_rognage(x: f64, y: f64, g: &pdf::SpreadGeometry, bleed: f64) -> f64 {
    debug_assert!((g.bleed - bleed).abs() < 1e-9, "deux fonds perdus pour une planche");
    scene::distance_to_trim(&pdf::Rect { x, y, w: 0.0, h: 0.0 }, 0.0, g)
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
                objets: Vec::new(),
            });
        }
        a
    }

    /// La page que l'imposition n'imprime pas doit être vide, et c'est ici que
    /// ça se refuse. Une première planche de photos en porte une à gauche : la
    /// découpe la perdrait sans un mot, donc elle est bloquante chez qui relie
    /// page par page, et invisible chez qui impose nos planches lui-même.
    #[test]
    fn une_premiere_planche_qui_porte_a_gauche_est_refusee() {
        let mut a = album_de(12, 3.0);
        let dims: HashMap<String, (u32, u32)> = (0..24)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();

        // `solo` pose sa photo au recto : la page de gauche est blanche et
        // rien ne se perd.
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        assert!(r.ok, "défauts : {:?}", r.defauts);

        // `duo` en pose une de chaque côté.
        a.spreads[0].template = "duo".into();
        a.spreads[0].slots.push(Slot::new("12.jpg".into(), [0.5, 0.5]));
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        let refus: Vec<&Defaut> =
            r.defauts.iter().filter(|d| d.regle == "imposition").collect();
        assert_eq!(refus.len(), 1, "défauts : {:?}", r.defauts);
        assert!(refus[0].bloquant);
        assert_eq!(refus[0].planche, Some(1));
        assert!(refus[0].remede.contains("page de garde"), "{}", refus[0].remede);
        assert!(!r.ok);

        // La même planche ailleurs dans le livre ne gêne personne : c'est la
        // première, et seulement elle, dont la gauche ne s'imprime pas.
        let mut b = album_de(12, 3.0);
        b.spreads[5].template = "duo".into();
        b.spreads[5].slots.push(Slot::new("12.jpg".into(), [0.5, 0.5]));
        assert!(check(&b, PrinterProfile::par_id("cloudprinter").unwrap(), &dims).ok);

        // Et chez un imprimeur qui relie des planches, la question ne se pose
        // pas : il n'y a pas de découpe.
        let r = check(&a, PrinterProfile::par_id("generique").unwrap(), &dims);
        assert!(!r.defauts.iter().any(|d| d.regle == "imposition"), "{:?}", r.defauts);
        assert!(r.ok, "{:?}", r.defauts);
    }

    /// Le piège du fond perdu intérieur, mesuré : une pleine page recadrée par
    /// sa largeur tient les 250 ppi sur la planche composée et ne les tient
    /// plus une fois qu'elle saigne trois millimètres de plus vers le pli. Le
    /// prévol lit le rectangle que la presse reçoit, pas celui de la
    /// composition, sinon il promet une résolution que personne n'imprime.
    #[test]
    fn le_fond_perdu_du_pli_fait_tomber_une_pleine_page_sous_le_plancher() {
        let mut a = album_de(12, 3.0);
        for s in &mut a.spreads {
            s.template = "full1".into(); // page de gauche vide, pleine page à droite
        }
        // 2110 px de large pour 213 mm : 251 ppi. Pour 216 : 248. La hauteur
        // ne gouverne rien ici, la photo étant nettement plus haute que large.
        let dims: HashMap<String, (u32, u32)> = (0..12)
            .map(|i| (format!("{i}.jpg"), (2110u32, 3000u32)))
            .collect();

        let r = check(&a, PrinterProfile::par_id("generique").unwrap(), &dims);
        assert!(r.ok, "sur la planche composée elle passe : {:?}", r.defauts);

        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        let sous: Vec<&Defaut> = r.defauts.iter().filter(|d| d.regle == "resolution").collect();
        assert_eq!(sous.len(), 12, "une par planche : {:?}", r.defauts);
        assert!(sous[0].cause.contains("248 ppi"), "{}", sous[0].cause);

        // Une case en marge n'atteint pas le pli, donc rien ne bouge pour
        // elle : le rectangle d'export est le rectangle composé.
        let mut b = album_de(12, 3.0);
        for s in &mut b.spreads {
            s.template = "solo".into();
        }
        let dims: HashMap<String, (u32, u32)> = (0..12)
            .map(|i| (format!("{i}.jpg"), (2000u32, 3000u32)))
            .collect();
        let cp = check(&b, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        let gen = check(&b, PrinterProfile::par_id("generique").unwrap(), &dims);
        assert_eq!(cp.ok, gen.ok, "cp {:?} / gen {:?}", cp.defauts, gen.defauts);
    }

    /// Un objet libre que la coupe traverse, un autre que le pli traverse :
    /// deux bloquants, chacun nommé, et le même album sans eux passe. Ce sont
    /// les deux seules choses que le prévol refuse à un objet libre — la
    /// marge, elle, est molle, et c'est le linter qui la compte.
    #[test]
    fn un_objet_libre_coupe_ou_a_cheval_sur_le_pli_est_bloquant() {
        use crate::model::{Alignement, Contenu, Objet};

        let bloc = |x: f64, y: f64| Objet {
            x,
            y,
            w: 40.0,
            h: 20.0,
            angle: 0.0,
            contenu: Contenu::Texte {
                texte: "Calvi".into(),
                taille_pt: 10.0,
                interligne_mm: Some(5.0),
                alignement: Alignement::Gauche,
            },
        };

        let mut a = album_de(24, 3.0);
        let g = pdf::geometry(&a);
        let dims: HashMap<String, (u32, u32)> = (0..24)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();
        let profil = PrinterProfile::par_id("generique").unwrap();

        // Bien à l'intérieur d'une page : rien à dire, l'album passe.
        a.spreads[0].objets = vec![bloc(40.0, 60.0)];
        let r = check(&a, profil, &dims);
        assert!(r.ok, "défauts : {:?}", r.defauts);

        // Le coin gauche passe sous la coupe.
        a.spreads[0].objets = vec![bloc(g.bleed - 10.0, 60.0)];
        let r = check(&a, profil, &dims);
        let coupe: Vec<&Defaut> = r.defauts.iter().filter(|d| d.regle == "objet_coupe").collect();
        assert_eq!(coupe.len(), 1, "défauts : {:?}", r.defauts);
        assert!(coupe[0].bloquant);
        assert_eq!(coupe[0].planche, Some(1));
        assert!(!r.ok);

        // À cheval sur le pli.
        a.spreads[0].objets = vec![bloc(g.media_w / 2.0 - 20.0, 60.0)];
        let r = check(&a, profil, &dims);
        let pli: Vec<&Defaut> = r.defauts.iter().filter(|d| d.regle == "objet_pli").collect();
        assert_eq!(pli.len(), 1, "défauts : {:?}", r.defauts);
        assert!(pli[0].bloquant);
        assert!(!r.ok);

        // Et l'angle compte : droit il s'arrête 5 mm avant le pli, tourné son
        // coin le passe de 10. C'est la scène qui le dit, pas la boîte.
        let frole = Objet { w: 40.0, h: 60.0, ..bloc(g.media_w / 2.0 - 45.0, 60.0) };
        a.spreads[0].objets = vec![frole.clone()];
        let droit = check(&a, profil, &dims);
        assert!(droit.ok, "droit il ne touche rien : {:?}", droit.defauts);
        a.spreads[0].objets = vec![Objet { angle: 45.0, ..frole }];
        let r = check(&a, profil, &dims);
        assert!(
            r.defauts.iter().any(|d| d.regle == "objet_pli"),
            "le coin tourné passe le pli : {:?}",
            r.defauts
        );
    }

    /// Big originals, matching bleed, sane pagination: nothing stops the file,
    /// at the loosest supplier and at the strictest. This is the state the
    /// export has to reach, and the one the whole session was about.
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
        assert!(r.ok, "défauts : {:?}", r.defauts);
        assert_eq!(r.bloquants, 0);
        // The spine is no longer announced: Cloudprinter wrote its bulk down,
        // so the coefficient is theirs and the warning has nothing to warn
        // about. What is still unknown about that spine — it varies between
        // their production sites, by their own answer — is not a property of
        // our arithmetic and travels in the profile's reserves, which the
        // sheet carries and which the test below reads.
        assert_eq!(r.avertissements, 0, "{:?}", r.defauts);
        assert!(r.fiche.dos_mm.unwrap() > 0.0);
        assert!(
            r.reserves.iter().any(|s| s.contains("varient d'un imprimeur")),
            "la variance entre sites a quitté le rapport : {:?}",
            r.reserves
        );
        // And what the declaration rests on is said, not implied.
        assert!(
            r.notes.iter().any(|n| n.contains("PDF/A-2b")),
            "la note sur la mesure de conformité a disparu : {:?}",
            r.notes
        );
    }

    /// The same album fails or passes depending on the supplier. That is the
    /// whole reason the profile is data.
    #[test]
    fn the_same_album_answers_differently_per_supplier() {
        let a = album_de(12, 0.0); // 24 pages, no bleed
        let dims: HashMap<String, (u32, u32)> = (0..12)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();

        // Prodigi generates the bleed itself and takes 24 pages: nothing in
        // the album's own numbers bothers it. The shape of the interior used
        // to, and no longer does — the export cuts the block page by page for
        // whoever binds it that way — so it now passes clean. That is the
        // profile going from unusable to deliverable without a number of its
        // own moving.
        let r = check(&a, PrinterProfile::par_id("prodigi").unwrap(), &dims);
        assert!(r.ok, "{:?}", r.defauts);
        assert_eq!(r.bloquants, 0, "{:?}", r.defauts);

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

    /// A supplier who binds one file counts the cover in it. Eleven spreads
    /// make a 22-page book block and a 24-page file, which is exactly their
    /// minimum: counting the block instead would refuse an album they accept,
    /// and the same two pages the other way round would put a real order on
    /// hold for a count that disagrees with the file.
    #[test]
    fn a_single_file_supplier_counts_its_cover() {
        let dims: HashMap<String, (u32, u32)> = (0..11)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();
        let a = album_de(11, 0.0); // 22 pages inside, 24 in the file
        let pr = PrinterProfile::par_id("prodigi").unwrap();
        let r = check(&a, pr, &dims);
        assert!(
            !r.defauts.iter().any(|d| d.regle == "pagination"),
            "24 pages de fichier tiennent dans les bornes : {:?}",
            r.defauts
        );

        // Cloudprinter binds the cover separately, so 22 stays 22, under its
        // own minimum of 24.
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        let d = r.defauts.iter().find(|d| d.regle == "pagination").unwrap();
        assert!(d.cause.contains("22 pages"), "{}", d.cause);
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

    /// The pages the machine writes are two pages like any spread. An album
    /// sitting on the supplier's upper bound is pushed over it by one of
    /// them, and the remedy names the one click that fixes it rather than
    /// sending somebody hunting through a hundred spreads.
    #[test]
    fn the_machine_pages_are_named_when_they_are_the_two_pages_too_many() {
        let dims: HashMap<String, (u32, u32)> = (0..101)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();
        let pr = PrinterProfile::par_id("cloudprinter").unwrap(); // 24 à 200 pages

        // 100 spreads = 200 pages, exactly the bound: nothing to report.
        let a = album_de(100, 3.0);
        let r = check(&a, pr, &dims);
        assert!(!r.defauts.iter().any(|d| d.regle == "pagination"), "{:?}", r.defauts);

        // The colophon page makes 202, and the remedy says which page to drop.
        let mut avec = a.clone();
        avec.spreads.push(crate::colophon::spread(
            &crate::colophon::Faits {
                photos_retenues: 100,
                photos_scannees: 400,
                debut: None,
                fin: None,
                lieux: Vec::new(),
                appareils: Vec::new(),
                compose_le: chrono::NaiveDate::from_ymd_opt(2026, 8, 17).unwrap(),
            },
            avec.trim_mm,
            150.0,
            "0.9.0",
        ));
        let r = check(&avec, pr, &dims);
        let d = r.defauts.iter().find(|d| d.regle == "pagination").unwrap();
        assert!(d.cause.contains("202 pages"), "{}", d.cause);
        assert!(d.remede.contains("colophon"), "{}", d.remede);

        // A book without a colophon but with a half-title: the remedy names
        // the page it can actually drop, not the one it does not have.
        let mut garde = a.clone();
        let f = crate::colophon::Faits {
            photos_retenues: 100,
            photos_scannees: 400,
            debut: None,
            fin: None,
            lieux: Vec::new(),
            appareils: Vec::new(),
            compose_le: chrono::NaiveDate::from_ymd_opt(2026, 8, 17).unwrap(),
        };
        garde.spreads.insert(0, crate::garde::spread("Corse", &f, 190.0));
        let d = check(&garde, pr, &dims)
            .defauts
            .into_iter()
            .find(|d| d.regle == "pagination")
            .unwrap();
        assert!(d.remede.contains("de garde"), "{}", d.remede);

        // A count that is wrong for another reason keeps the general remedy.
        let court = album_de(4, 3.0);
        let d = check(&court, pr, &dims)
            .defauts
            .into_iter()
            .find(|d| d.regle == "pagination")
            .unwrap();
        assert!(!d.remede.contains("colophon"), "{}", d.remede);
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

    /// A bascule must not degrade the preflight, and « green » is not the
    /// bar — corse-2013 is red before any bascule, because two photographs
    /// print at 130 ppi and no format change will ever fix a photograph.
    /// The bar is: same defects, same blockers, and the only field of the
    /// whole fiche that moves is `format_page_mm`. Written as a comparison
    /// of reports, not as a list of expectations copied by hand; only the
    /// ppi inside the wording may follow the cells.
    #[test]
    fn une_bascule_ne_degrade_pas_le_prevol() {
        let a = album_de(24, 3.0);
        let mut dims: HashMap<String, (u32, u32)> = (0..24)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();
        // A photograph already under the floor before any bascule.
        dims.insert("3.jpg".into(), (600, 600));
        let profil = PrinterProfile::par_id("cloudprinter").unwrap();

        let avant = check(&a, profil, &dims);
        assert!(!avant.ok, "le cas doit avoir quelque chose à dire");

        let (basculee, _) =
            crate::bascule::bascule(&a, Size { w: 280.0, h: 210.0 }, &dims, profil);
        let apres = check(&basculee, profil, &dims);

        // Same defects, matched by what they are and where they are.
        let identite = |r: &PrevolReport| {
            r.defauts
                .iter()
                .map(|d| (d.regle, d.bloquant, d.planche, d.case_idx, d.src.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(identite(&avant), identite(&apres));
        assert_eq!(avant.bloquants, apres.bloquants);
        assert_eq!(avant.avertissements, apres.avertissements);
        assert_eq!(avant.ok, apres.ok);
        assert_eq!(avant.notes, apres.notes);

        // The whole fiche, compared as data, with the legitimate moves
        // asserted then masked. There are two, and the second is the first:
        // a bascule changes the page, and the cover sheet is built around the
        // page, so a sheet that did **not** move would be the bug.
        let mut fa = serde_json::to_value(&avant.fiche).unwrap();
        let mut fp = serde_json::to_value(&apres.fiche).unwrap();
        for champ in ["format_page_mm", "feuille_couverture_mm"] {
            assert_ne!(fa[champ], fp[champ], "{champ}");
            fa[champ] = serde_json::Value::Null;
            fp[champ] = serde_json::Value::Null;
        }
        assert_eq!(fa, fp);
    }

    /// La zone sûre avertit et ne bloque plus, et c'est la mesure qui l'a
    /// décidé : la ligne de base d'une légende vient de la marge du format,
    /// aucune main ne la déplace, et la règle refusait donc tout album légendé
    /// chez deux fournisseurs sur quatre. Ce qui bloque est ce que la coupe
    /// traverse ; ce qu'un imprimeur préfère voir plus loin du bord s'écrit et
    /// se lit — les trois chiffres vont du simple au double.
    #[test]
    fn la_zone_sure_avertit_au_lieu_de_bloquer() {
        let mut a = album_de(24, 3.0);
        // Toutes sauf la première, dont la page de gauche porte le faux-titre
        // dans un vrai livre : une légende y tomberait à gauche, et c'est
        // l'imposition qui le refuse, pas la zone sûre.
        for s in &mut a.spreads[1..] {
            s.caption = Some("une légende comme il y en a dans tout album".into());
        }
        let dims: HashMap<String, (u32, u32)> = (0..24)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();

        // Prodigi garde 10 mm, la légende passe à 7 : il le dit, et il livre.
        let r = check(&a, PrinterProfile::par_id("prodigi").unwrap(), &dims);
        let zs: Vec<&Defaut> = r.defauts.iter().filter(|d| d.regle == "zone_sure").collect();
        assert_eq!(zs.len(), 23, "une par planche légendée : {:?}", r.defauts);
        assert!(zs.iter().all(|d| !d.bloquant));
        assert!(r.ok, "un album légendé reste livrable : {:?}", r.defauts);
        assert_eq!(r.bloquants, 0);

        // Et le remède ne promet plus ce qu'il ne peut pas tenir : ni
        // raccourcir la légende ni changer de gabarit ne déplace sa ligne de
        // base. C'est cette phrase-là qui était fausse quoi qu'on décide du
        // reste, donc c'est elle que le test tient.
        assert!(!zs[0].remede.contains("raccourcissez"), "{}", zs[0].remede);
        assert!(zs[0].remede.contains("marge du format"), "{}", zs[0].remede);

        // Cloudprinter porte enfin son vrai chiffre, et la légende tombe
        // exactement dessus : 7,00 mm contre 7,0 demandés, sur un format dont
        // la marge fait 14. Rien à dire, et rien qui ait été tu.
        let r = check(&a, PrinterProfile::par_id("cloudprinter").unwrap(), &dims);
        assert!(!r.defauts.iter().any(|d| d.regle == "zone_sure"), "{:?}", r.defauts);
    }

    /// La couverture se mesure sur la feuille que ce fournisseur reçoit, et
    /// les deux feuilles n'ont pas la même taille : le carton habillé court
    /// bien au-delà du livre, le feuillet volant s'arrête au fond perdu. Une
    /// photographie peut donc être une couverture chez l'un et pas chez
    /// l'autre, avec les mêmes pixels.
    #[test]
    fn la_couverture_se_mesure_sur_la_feuille_du_fournisseur() {
        let mut a = album_de(24, 3.0);
        // 2400 px : 282 ppi sur le feuillet de 216 mm de Prodigi, 255 sur les
        // 239 mm de la feuille cartonnée. Les deux passent.
        a.cover = Some(crate::model::Cover {
            title: "t".into(),
            subtitle: String::new(),
            photo: Some(Slot::new("couv.jpg".into(), [0.5, 0.5])),
            back_text: String::new(),
        });
        let mut dims: HashMap<String, (u32, u32)> = (0..24)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();
        dims.insert("couv.jpg".into(), (2400, 2600));
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let pr = PrinterProfile::par_id("prodigi").unwrap();
        let couv = |r: &PrevolReport| -> Option<String> {
            r.defauts
                .iter()
                .find(|d| d.regle == "couverture_resolution")
                .map(|d| d.cause.clone())
        };
        assert!(couv(&check(&a, cp, &dims)).is_none());
        assert!(couv(&check(&a, pr, &dims)).is_none());

        // 2100 px : 246 ppi sur le cartonné, 247 sur le feuillet. Le premier
        // refuse, le second aussi — mais pas pour la même surface.
        dims.insert("couv.jpg".into(), (2100, 2300));
        let d = couv(&check(&a, cp, &dims)).expect("le cartonné refuse");
        assert!(d.contains("239 × 258"), "{d}");
        assert!(check(&a, cp, &dims).defauts.iter().any(|d| d.regle
            == "couverture_resolution"
            && d.bloquant));

        // Et la surface exacte est celle du fournisseur, pas une moyenne :
        // 2200 px passent chez Prodigi et pas chez Cloudprinter, avec la même
        // photographie et le même album.
        dims.insert("couv.jpg".into(), (2200, 2400));
        assert!(couv(&check(&a, cp, &dims)).is_some(), "260 ppi sur 239 mm : non");
        assert!(couv(&check(&a, pr, &dims)).is_none(), "258 ppi sur 216 mm : oui");

        // Une couverture sans photo ne se mesure pas, et un album sans
        // couverture non plus.
        a.cover.as_mut().unwrap().photo = None;
        assert!(couv(&check(&a, cp, &dims)).is_none());
        a.cover = None;
        assert!(couv(&check(&a, cp, &dims)).is_none());
    }

    /// Un dos trop mince ne porte pas de titre, et le prévol le dit au lieu de
    /// laisser partir une tranche nue que personne n'a choisie. Ce n'est pas
    /// un défaut du fichier : c'est un livre trop fin, donc un avertissement.
    #[test]
    fn un_dos_trop_mince_sort_nu_et_le_dit() {
        let dims: HashMap<String, (u32, u32)> = (0..24)
            .map(|i| (format!("{i}.jpg"), (5000u32, 5000u32)))
            .collect();
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let nu = |n: usize| -> Option<Defaut> {
            let mut r = check(&album_de(n, 3.0), cp, &dims);
            let i = r.defauts.iter().position(|d| d.regle == "dos_nu")?;
            Some(r.defauts.remove(i))
        };

        // 12 planches : 24 pages, leur minimum, et un dos de 7,62 mm.
        let d = nu(12).expect("un dos de 7,6 mm ne porte rien");
        assert!(!d.bloquant, "un dos nu est un livre légitime");
        assert!(d.cause.contains("7.6"), "{}", d.cause);

        // 24 planches : 9,24 mm, le titre tient.
        assert!(nu(24).is_none());

        // Et la question ne se pose pas chez qui fabrique son dos lui-même :
        // il n'y a pas de feuille à plat à décrire.
        let r = check(&album_de(12, 0.0), PrinterProfile::par_id("prodigi").unwrap(), &dims);
        assert!(!r.defauts.iter().any(|d| d.regle == "dos_nu"), "{:?}", r.defauts);
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
        // La première question d'un imprimeur qui relie du cartonné : de
        // quelle taille est la feuille ? Les trois cotes vivaient dans le
        // profil et n'atteignaient personne ; ici elles sortent en un nombre.
        let dos = f.dos_mm.unwrap();
        assert_eq!(
            f.feuille_couverture_mm,
            Some([210.0 * 2.0 + dos + 5.0 * 2.0 + 24.0 * 2.0, 210.0 + 24.0 * 2.0])
        );
        // Et personne ne la donne à qui fabrique la sienne.
        let r = check(&a, PrinterProfile::par_id("prodigi").unwrap(), &dims);
        assert_eq!(r.fiche.feuille_couverture_mm, None);
        // A provisional profile never travels without its reservations.
        assert!(!r.reserves.is_empty());
    }
}
