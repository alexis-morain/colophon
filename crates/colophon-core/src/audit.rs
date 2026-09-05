//! Album linter: a read pass over a composed album folder that counts the
//! defect classes a human should never have to report. `colophon --audit`
//! prints the JSON report and exits non-zero when a counter passes son seuil;
//! check.sh runs it on the reference sets as a non-regression gate.
//!
//! The counters measure what we thought to count. Any spread that bothers
//! the eye without tripping a counter is a missing defect class: add it here.

use crate::model::Album;
use crate::{analyze, face, meta, pdf, print};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// Two photos this close in dHash on one spread read as duplicates.
pub const DUP_HAMMING: u32 = 24;
/// Same rule on the DCT pHash: either hash agreeing is enough. Kept tight
/// (8 bits): low-frequency DCT makes any two seascapes look related at 12.
pub const DUP_PHASH: u32 = 8;
/// Pose series: two frames of the same scene seconds apart read as twins
/// to the eye even when the hashes disagree (a handstand flips every
/// gradient). Same spread + same minutes + same colors = doublon.
pub const SCENE_SPREAD_SECONDS: i64 = 180;
pub const SCENE_SPREAD_COLOR: u32 = 20;
/// Faces keep at least this share of the visible window between them and
/// every cropped edge; below it a face reads as cut.
pub const FACE_MARGIN: f64 = 0.04;
/// A face narrower than this share of the image width is detector noise.
pub const FACE_MIN_SHARE: f64 = 0.04;
/// Photo aspect vs cell aspect beyond this betrays the orientation.
pub const ASPECT_BETRAYAL: f64 = 1.4;
/// Below this effective resolution a cell prints visibly soft. 300 dpi is
/// the target, 250 the floor: an 8 Mpx frame on a full page sits at 288 and
/// prints fine, nobody distinguishes it from 300.
pub const MIN_EFFECTIVE_PPI: f64 = 250.0;
/// Longest run of spreads without a full page before the rhythm reads flat.
const FLAT_RUN: usize = 6;
/// The same template family this many times in a row reads as a repetition.
const REPEAT_RUN: usize = 4;
/// A chapter needs at least this many photos for the opening-quartile rule:
/// below that a dedicated opening page would eat the whole chapter.
pub const OPENING_MIN_PHOTOS: usize = 6;
/// The chapter opening must score in this top quantile of its chapter.
const OPENING_QUANTILE: f64 = 0.75;
/// Each star multiplies the score by this. A measurement made on pixels
/// guesses at what a photo is worth; a star is the answer, so five of them
/// roughly double the score and settle any comparison a sharpness reading
/// would have won on its own. The composer and this linter both go through
/// `rating_factor`: a photo the user starred must be free to open its
/// chapter without the opening counter calling that opening weak.
const RATING_STEP: f64 = 1.18;

/// Score multiplier for the rating read off the file. Neutral for the
/// unrated, which is nearly every photo of nearly every folder, and neutral
/// for the rejected too: those never reach a score, the curation sets them
/// aside before anything is compared (see `pipeline::split_rejected`).
pub fn rating_factor(rating: Option<i8>) -> f64 {
    match rating {
        Some(stars @ 1..=5) => RATING_STEP.powi(i32::from(stars)),
        _ => 1.0,
    }
}

#[derive(Debug, Serialize)]
pub struct Finding {
    /// 1-based spread index, as the ruler shows it.
    pub planche: usize,
    #[serde(rename = "case", skip_serializing_if = "Option::is_none")]
    pub case_idx: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    pub info: String,
}

#[derive(Debug, Serialize)]
pub struct Counter {
    pub count: usize,
    pub seuil: usize,
    /// A hard counter must be exactly zero before humans see the album.
    pub dur: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<Finding>,
}

impl Counter {
    fn new(seuil: usize, dur: bool, details: Vec<Finding>) -> Self {
        Self { count: details.len(), seuil, dur, details }
    }

    pub fn passes(&self) -> bool {
        self.count <= self.seuil
    }
}

/// Field order is the report order.
#[derive(Debug, Serialize)]
pub struct Counters {
    pub visage_coupe: Counter,
    pub orientation_trahie: Counter,
    pub doublon_planche: Counter,
    pub sous_resolution: Counter,
    pub chapitre_orphelin: Counter,
    pub ouverture_faible: Counter,
    pub rythme_plat: Counter,
    pub legende_manquante: Counter,
    pub legende_sur_photo: Counter,
    pub repetition_gabarit: Counter,
    /// A free block whose box leaves the safe zone (wave 6.4). Soft, because
    /// a block set to bleed on purpose is a choice and the editor already
    /// warned under the hand; the counter is here so nobody ships one by
    /// accident.
    pub objet_hors_marge: Counter,
    /// A free block whose set text runs past the bottom of its box. Soft for
    /// the same reason, and never silent: nothing is ever wrapped away or
    /// cut, so what does not fit says so here.
    pub objet_deborde: Counter,
}

impl Counters {
    pub(crate) fn all(&self) -> [&Counter; 12] {
        [
            &self.visage_coupe,
            &self.orientation_trahie,
            &self.doublon_planche,
            &self.sous_resolution,
            &self.chapitre_orphelin,
            &self.ouverture_faible,
            &self.rythme_plat,
            &self.legende_manquante,
            &self.legende_sur_photo,
            &self.repetition_gabarit,
            &self.objet_hors_marge,
            &self.objet_deborde,
        ]
    }
}

#[derive(Debug, Serialize)]
pub struct AuditReport {
    pub album: String,
    pub planches: usize,
    pub ok: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    pub compteurs: Counters,
}

/// What the audit knows about one photo, all measured on its cached
/// thumbnail except the original pixel size. The bench (`banc`) reads the
/// same measurements to judge candidate templates, hence the crate scope.
pub(crate) struct PhotoInfo {
    pub(crate) w: f64,
    pub(crate) h: f64,
    pub(crate) dhash: u64,
    pub(crate) phash: u64,
    pub(crate) colorsig: [u8; 12],
    pub(crate) score: f64,
    pub(crate) faces: Vec<[f64; 4]>,
    /// Original size, EXIF orientation applied. Not optional: a photo whose
    /// original size cannot be established is an audit that refuses to note,
    /// never a resolution counter that quietly counts nothing.
    pub(crate) orig: (u32, u32),
    /// Capture time, when the original and its EXIF are reachable.
    pub(crate) taken: Option<chrono::NaiveDateTime>,
}

pub fn audit(dir: &Path) -> Result<AuditReport> {
    let album: Album = serde_json::from_str(
        &fs::read_to_string(dir.join("album.json"))
            .with_context(|| format!("lecture de {}", dir.join("album.json").display()))?,
    )
    .context("album.json illisible")?;

    let root = PathBuf::from(&album.root);
    let root_ok = root.is_dir();

    let mut srcs: Vec<String> = album
        .spreads
        .iter()
        .flat_map(|s| s.slots.iter().map(|sl| sl.src.clone()))
        .collect();
    srcs.sort();
    srcs.dedup();

    let (infos, notes) = mesure_photos(dir, &root, root_ok, &srcs)?;
    // Un bloc libre revient à la ligne dans la face du livre, donc le compteur
    // de débordement se mesure dans cette face-là. C'est la règle du projet —
    // mesurer dans une face et dessiner dans une autre est ce qui fait sortir
    // un titre du massicot — et c'est la seule chose de ce module qui dépende
    // de la fonte : les dix compteurs d'avant lisent des rectangles, que
    // `slots_for` produit sans jamais ouvrir un fichier de police.
    let face = crate::font::face_album(dir, album.police.as_ref().map(|p| p.fichier.as_str()));
    let mesure = |s: &str, pt: f64| face.face.largeur_mm(s, pt);
    let compteurs = compteurs_avec(&album, &infos, &pdf::geometry(&album), &mesure);
    let ok = compteurs.all().iter().all(|c| c.passes());

    Ok(AuditReport {
        album: dir.display().to_string(),
        planches: album.spreads.len(),
        ok,
        notes,
        compteurs,
    })
}

/// Measure every photo an album (or a set of albums sharing a thumbnail
/// cache) touches: the read half of the audit, reusable by the bench.
/// Returns what it measured, and where from — a linter that measures
/// something other than what it says is how a gate starts lying.
pub(crate) fn mesure_photos(
    dir: &Path,
    root: &Path,
    root_ok: bool,
    srcs: &[String],
) -> Result<(HashMap<String, PhotoInfo>, Vec<String>)> {
    // The relevé first, when the album carries one: a composition without
    // the photographs left no thumbnails to re-measure, and the fiches hold
    // the very measurements the thumbnails would give — they were taken on
    // them.
    if let Some(releve) = crate::releve::Releve::dans_album(dir)? {
        let note = format!(
            "mesuré depuis le relevé de l'album ({} fiches) : les photos n'ont pas \
             été rouvertes",
            releve.photos.len()
        );
        return Ok((depuis_releve(&releve, srcs)?, vec![note]));
    }
    // Neither the fiches nor the originals. The resolution counter would
    // then count nothing and pass in silence: a green worth less than the
    // Mac's green, without saying so. That is exactly how a portable gate
    // becomes a gate that lies, so it refuses instead.
    anyhow::ensure!(
        root_ok,
        "dossier de photos introuvable ({}) et pas de {} dans l'album : la \
         résolution ne peut pas être mesurée, et un audit qui saute ce compteur \
         rend un vert qui ne vaut rien. Remettez le dossier de photos en place, \
         ou composez l'album depuis ses fiches (--depuis-fiches).",
        root.display(),
        crate::releve::FICHIER
    );
    let thumbs: HashMap<String, String> =
        serde_json::from_str(&fs::read_to_string(dir.join("thumbs.json"))?)
            .context("thumbs.json illisible")?;
    srcs
        .par_iter()
        .map_init(face::new_detector, |det, src| -> Result<(String, PhotoInfo)> {
            let name = thumbs
                .get(src)
                .with_context(|| format!("{src} absent de thumbs.json"))?;
            let path = dir.join(".cache").join("thumbs").join(name);
            let img = image::open(&path)
                .with_context(|| format!("vignette illisible pour {src}, régénérez l'album"))?;
            let analysis = analyze::analyze(&img);
            let faces = face::face_boxes(det.as_mut(), &img);
            let p = root.join(src);
            let m = meta::read(&p);
            // A photo whose header will not give its size is named and
            // refused, never averaged over: the counter it feeds is the one
            // that keeps a book above 250 ppi.
            let orig = crate::heic::oriented_dimensions(&p, m.orientation)
                .with_context(|| {
                    format!("taille d'origine illisible pour {src}, régénérez l'album")
                })?;
            let (taken, rating) = (m.taken_reliable.then_some(m.taken), m.rating);
            Ok((
                src.clone(),
                PhotoInfo {
                    w: f64::from(analysis.width),
                    h: f64::from(analysis.height),
                    dhash: analysis.dhash,
                    phash: analysis.phash,
                    colorsig: analysis.colorsig,
                    // Same score the composer ranked the chapter with, stars
                    // included: the opening counter judges the choice that
                    // was actually available to it.
                    score: analysis.score() * rating_factor(rating),
                    faces,
                    orig,
                    taken,
                },
            ))
        })
        .collect::<Result<_>>()
        .map(|infos| (infos, Vec::new()))
}

/// The same measurements, read rather than re-measured. Nothing is
/// approximated here: a fiche was taken on the very thumbnail the other path
/// re-opens, so the two readings are the same numbers by construction.
fn depuis_releve(
    releve: &crate::releve::Releve,
    srcs: &[String],
) -> Result<HashMap<String, PhotoInfo>> {
    let par_src: HashMap<String, &crate::pipeline::Photo> =
        releve.photos.iter().map(|p| (releve.src(&p.path), p)).collect();
    srcs.iter()
        .map(|src| {
            let p = par_src.get(src).with_context(|| {
                format!(
                    "{src} absent du relevé de l'album : ces fiches ne décrivent pas \
                     cet album, régénérez-les (scripts/fiches.sh)"
                )
            })?;
            Ok((
                src.clone(),
                PhotoInfo {
                    w: f64::from(p.analysis.width),
                    h: f64::from(p.analysis.height),
                    dhash: p.analysis.dhash,
                    phash: p.analysis.phash,
                    colorsig: p.analysis.colorsig,
                    score: p.analysis.score() * rating_factor(p.meta.rating),
                    faces: p.faces.clone(),
                    orig: p.orig,
                    taken: p.meta.taken_reliable.then_some(p.meta.taken),
                },
            ))
        })
        .collect()
}

/// The counting half of the audit: pure and in-memory, given the album, the
/// measurements and the geometry. The bench re-runs it after every candidate
/// substitution; `audit` runs it once.
pub(crate) fn compteurs(
    album: &Album,
    infos: &HashMap<String, PhotoInfo>,
    g: &pdf::SpreadGeometry,
) -> Counters {
    compteurs_avec(album, infos, g, &crate::font::text_width_mm)
}

/// The same counters under a caller-supplied measure, which is how the
/// album's own face reaches the one counter that depends on it.
///
/// The sibling exists for the reason [`crate::scene::Scene::of_avec`] exists,
/// and it is the same shape on purpose: the bench tries candidate templates
/// on photographs and never on free blocks, so it keeps the plain
/// [`compteurs`] and the face this crate ships.
pub(crate) fn compteurs_avec(
    album: &Album,
    infos: &HashMap<String, PhotoInfo>,
    g: &pdf::SpreadGeometry,
    mesure: &dyn Fn(&str, f64) -> f64,
) -> Counters {
    // One walk of the spreads, one scene each: the cells the linter judges
    // are the objects the emitter draws, and so are the free blocks. A
    // counter can never grade a rectangle the PDF does not contain, and the
    // two counters of wave 6.4 read what the scene already decided rather
    // than deciding it a second time — a second implementation of the
    // doctrine is precisely what that module was written to remove.
    let scenes: Vec<crate::scene::Scene> = album
        .spreads
        .iter()
        .map(|s| crate::scene::Scene::of_avec(s, g, mesure))
        .collect();
    let rects_of: Vec<Vec<pdf::Rect>> = scenes
        .iter()
        .map(|scene| {
            scene
                .objects
                .iter()
                .filter(|o| matches!(o.role, crate::scene::Role::Photo { .. }))
                .map(|o| o.rect)
                .collect()
        })
        .collect();

    // -- visage coupé, orientation trahie, sous 300 ppi : par case
    let mut visage = Vec::new();
    let mut orientation = Vec::new();
    let mut ppi = Vec::new();
    for (si, spread) in album.spreads.iter().enumerate() {
        for (ci, (slot, rect)) in spread.slots.iter().zip(&rects_of[si]).enumerate() {
            let info = &infos[&slot.src];

            for side in face_cuts(rect, info.w, info.h, slot.focal, slot.zoom, &info.faces) {
                visage.push(Finding {
                    planche: si + 1,
                    case_idx: Some(ci),
                    src: Some(slot.src.clone()),
                    info: format!("visage à moins de {:.0} % du bord {side}", FACE_MARGIN * 100.0),
                });
            }

            let photo_aspect = info.w / info.h;
            let cell_aspect = rect.w / rect.h;
            let betrayal = (photo_aspect / cell_aspect).max(cell_aspect / photo_aspect);
            if betrayal > ASPECT_BETRAYAL {
                orientation.push(Finding {
                    planche: si + 1,
                    case_idx: Some(ci),
                    src: Some(slot.src.clone()),
                    info: format!(
                        "photo {:.2} dans une case {:.2}, écart ×{betrayal:.2}",
                        photo_aspect, cell_aspect
                    ),
                });
            }

            // Zooming shows fewer source pixels: the effective print
            // resolution drops with it.
            let (ow, oh) = info.orig;
            let effectif = print::effective_ppi(rect, ow, oh, slot.zoom);
            if effectif < MIN_EFFECTIVE_PPI {
                ppi.push(Finding {
                    planche: si + 1,
                    case_idx: Some(ci),
                    src: Some(slot.src.clone()),
                    info: format!("{effectif:.0} ppi effectifs"),
                });
            }
        }
    }

    // -- quasi-doublons sur la même planche
    let mut doublons = Vec::new();
    for (si, spread) in album.spreads.iter().enumerate() {
        for i in 0..spread.slots.len() {
            for j in i + 1..spread.slots.len() {
                let (a, b) = (&infos[&spread.slots[i].src], &infos[&spread.slots[j].src]);
                let d = analyze::hamming(a.dhash, b.dhash);
                let p = analyze::hamming(a.phash, b.phash);
                let same_scene = match (a.taken, b.taken) {
                    (Some(ta), Some(tb)) => {
                        (ta - tb).num_seconds().abs() <= SCENE_SPREAD_SECONDS
                            && analyze::color_distance(&a.colorsig, &b.colorsig)
                                <= SCENE_SPREAD_COLOR
                    }
                    _ => false,
                };
                if d <= DUP_HAMMING || p <= DUP_PHASH || same_scene {
                    doublons.push(Finding {
                        planche: si + 1,
                        case_idx: Some(i),
                        src: Some(spread.slots[j].src.clone()),
                        info: format!(
                            "cases {i} et {j} à {d} bits (pHash {p}, couleur {})",
                            analyze::color_distance(&a.colorsig, &b.colorsig)
                        ),
                    });
                }
            }
        }
    }

    // -- chapitres : délimités par les légendes posées à la composition
    let chapters = chapter_ranges(album);
    let orphelins = chapters
        .iter()
        .filter(|r| r.len() == 1)
        .map(|r| Finding {
            planche: r.start + 1,
            case_idx: None,
            src: None,
            info: "chapitre d'une seule planche".into(),
        })
        .collect();

    let mut ouverture = Vec::new();
    for r in &chapters {
        let scores: Vec<f64> = album.spreads[r.clone()]
            .iter()
            .flat_map(|s| s.slots.iter().map(|sl| infos[&sl.src].score))
            .collect();
        if scores.len() < OPENING_MIN_PHOTOS {
            continue;
        }
        let bar = quantile(&scores, OPENING_QUANTILE);
        let first = &album.spreads[r.start].slots[0];
        let got = infos[&first.src].score;
        if got < bar {
            ouverture.push(Finding {
                planche: r.start + 1,
                case_idx: Some(0),
                src: Some(first.src.clone()),
                info: format!("score {got:.2} sous le quartile haut du chapitre ({bar:.2})"),
            });
        }
    }

    // -- rythme plat et répétition de gabarit : sur la suite des planches
    let templates: Vec<&str> = album.spreads.iter().map(|s| s.template.as_str()).collect();
    let counts: Vec<usize> = album.spreads.iter().map(|s| s.slots.len()).collect();
    let rythme = flat_runs(&counts)
        .into_iter()
        .map(|(start, len)| Finding {
            planche: start + 1,
            case_idx: None,
            src: None,
            info: format!("{len} planches consécutives sans page de respiration"),
        })
        .collect();
    let repetition = repeat_runs(&templates)
        .into_iter()
        .map(|(start, len, family)| Finding {
            planche: start + 1,
            case_idx: None,
            src: None,
            info: format!("gabarit {family} répété {len} fois"),
        })
        .collect();

    // -- légendes : les deux compteurs lisent la hauteur signée. Une bande
    // déclarée (positive) qui reste vide imprimerait un blanc réservé pour
    // rien ; une hauteur négative (surimpression déclarée, ou tous les
    // emplacements libres recouverts) imprime le texte sur une photo.
    let mut legende_manquante: Vec<Finding> = album
        .spreads
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.caption.is_none()
                && crate::gabarit::spec(&s.template).is_some_and(|sp| sp.legende > 0.0)
        })
        .map(|(si, _)| Finding {
            planche: si + 1,
            case_idx: None,
            src: None,
            info: "le gabarit réserve une bande de légende, restée vide".into(),
        })
        .collect();
    if let Some(i) = premiere_photo(album) {
        if album.spreads[i].caption.is_none() {
            legende_manquante.push(Finding {
                planche: i + 1,
                case_idx: None,
                src: None,
                info: "l'album s'ouvre sans légende de chapitre".into(),
            });
        }
    }
    let legende_sur_photo = album
        .spreads
        .iter()
        .enumerate()
        .filter(|(si, s)| {
            s.caption.is_some()
                && pdf::caption_height(&s.template, &rects_of[*si], g) < 0.0
        })
        .map(|(si, _)| Finding {
            planche: si + 1,
            case_idx: None,
            src: None,
            info: "la légende imprimerait sur une photo".into(),
        })
        .collect();

    // -- les objets libres : deux compteurs, lus sur la scène.
    //
    // Ils lisent, ils ne recalculent pas. `hors_marge` est la fonction que le
    // calque de l'éditeur appelle sous la main, `overflow` est le champ que le
    // rôle `FreeText` a posé en se composant : ce que l'écran a averti et ce
    // que le linter compte sont la même mesure, ou l'un des deux ment.
    //
    // Ce qu'ils ne comptent pas, et c'est délibéré : `trop_large`, le mot plus
    // large que sa boîte. Il est signalé par la scène et montré par l'éditeur,
    // mais son encre déborde à droite sans que la boîte, elle, bouge — un
    // compteur qui le lirait mesurerait de l'encre là où les deux autres
    // mesurent une boîte. Il attend sa propre classe.
    let mut hors_marge = Vec::new();
    let mut deborde = Vec::new();
    for (si, scene) in scenes.iter().enumerate() {
        for object in &scene.objects {
            let crate::scene::Role::FreeText { index, overflow, .. } = &object.role else {
                continue;
            };
            if crate::scene::hors_marge(&object.rect, object.angle, g) {
                let d = crate::scene::distance_to_trim(&object.rect, object.angle, g);
                hors_marge.push(Finding {
                    planche: si + 1,
                    case_idx: None,
                    src: None,
                    info: format!(
                        "le bloc {} passe à {d:.1} mm du rognage, la zone sûre en garde {:.1}",
                        index + 1,
                        crate::scene::marge_sure(g)
                    ),
                });
            }
            if *overflow {
                deborde.push(Finding {
                    planche: si + 1,
                    case_idx: None,
                    src: None,
                    info: format!(
                        "le bloc {} : le texte composé dépasse le bas de sa boîte",
                        index + 1
                    ),
                });
            }
        }
    }

    Counters {
        visage_coupe: Counter::new(0, true, visage),
        orientation_trahie: Counter::new(0, true, orientation),
        doublon_planche: Counter::new(0, true, doublons),
        sous_resolution: Counter::new(3, false, ppi),
        chapitre_orphelin: Counter::new(1, false, orphelins),
        ouverture_faible: Counter::new(0, true, ouverture),
        rythme_plat: Counter::new(1, false, rythme),
        legende_manquante: Counter::new(0, true, legende_manquante),
        legende_sur_photo: Counter::new(0, true, legende_sur_photo),
        repetition_gabarit: Counter::new(1, false, repetition),
        // Mous, seuil zéro. Un bloc posé volontairement à fond perdu est un
        // choix, et l'éditeur a déjà averti : le linter est là pour qu'on n'en
        // expédie pas un par accident, pas pour l'interdire.
        objet_hors_marge: Counter::new(0, false, hors_marge),
        objet_deborde: Counter::new(0, false, deborde),
    }
}

/// Which crop edges cut a face, as edge names. Only edges the crop actually
/// created count: a face the photographer framed against the border is not
/// our defect, and no recadrage of ours can fix it.
fn face_cuts(
    rect: &pdf::Rect,
    iw: f64,
    ih: f64,
    focal: [f64; 2],
    zoom: f64,
    faces: &[[f64; 4]],
) -> Vec<&'static str> {
    let (x0, y0, vw, vh) = pdf::crop_window(rect, iw, ih, focal, zoom);
    let (mx, my) = (FACE_MARGIN * vw, FACE_MARGIN * vh);
    let cropped_left = x0 > 0.5;
    let cropped_right = x0 + vw < iw - 0.5;
    let cropped_top = y0 > 0.5;
    let cropped_bottom = y0 + vh < ih - 0.5;

    let mut cuts = Vec::new();
    for b in faces {
        if b[2] < FACE_MIN_SHARE {
            continue;
        }
        let (bx, by, bw, bh) = (b[0] * iw, b[1] * ih, b[2] * iw, b[3] * ih);
        if cropped_left && bx < x0 + mx {
            cuts.push("gauche");
        } else if cropped_right && bx + bw > x0 + vw - mx {
            cuts.push("droit");
        } else if cropped_top && by < y0 + my {
            cuts.push("haut");
        } else if cropped_bottom && by + bh > y0 + vh - my {
            cuts.push("bas");
        }
    }
    cuts
}

/// Spread ranges of the chapters, delimited by the captions the composer
/// posed, from the first spread of photographs (see [`premiere_photo`]). A
/// headless album (no caption where the photographs start) still yields one
/// range, and the missing-caption counter reports it separately.
fn chapter_ranges(album: &Album) -> Vec<Range<usize>> {
    let n = album.spreads.len();
    let Some(debut) = premiere_photo(album) else { return Vec::new() };
    let mut starts: Vec<usize> = album
        .spreads
        .iter()
        .enumerate()
        .skip(debut)
        .filter(|(_, s)| s.caption.is_some())
        .map(|(i, _)| i)
        .collect();
    if starts.first() != Some(&debut) {
        starts.insert(0, debut);
    }
    starts
        .iter()
        .zip(starts.iter().skip(1).chain(std::iter::once(&n)))
        .map(|(&a, &b)| a..b)
        .collect()
}

/// The first spread that carries photographs. The pages the machine writes
/// about the book, the half-title at the head and the colophon at the foot,
/// are not chapters and have no opening: the chapter structure starts where
/// the photographs do. Without this, an album opening on its half-title
/// reads as a one-spread chapter with no caption, and two counters go red
/// over a page nobody composed.
fn premiere_photo(album: &Album) -> Option<usize> {
    album.spreads.iter().position(|s| !s.slots.is_empty())
}

/// Runs of more than [`FLAT_RUN`] consecutive spreads without a breathing
/// page (a spread holding a single photo, full bleed or margined), as
/// (start index, length).
fn flat_runs(slot_counts: &[usize]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = 0usize;
    let mut len = 0usize;
    for (i, &c) in slot_counts.iter().enumerate() {
        if c <= 1 {
            if len > FLAT_RUN {
                runs.push((start, len));
            }
            len = 0;
        } else {
            if len == 0 {
                start = i;
            }
            len += 1;
        }
    }
    if len > FLAT_RUN {
        runs.push((start, len));
    }
    runs
}

/// Runs of [`REPEAT_RUN`] or more spreads sharing a template family
/// (verso variants included), as (start index, length, family).
fn repeat_runs<'a>(templates: &[&'a str]) -> Vec<(usize, usize, &'a str)> {
    let mut runs = Vec::new();
    let mut start = 0usize;
    for i in 1..=templates.len() {
        let same = i < templates.len()
            && templates[i].trim_end_matches("_verso")
                == templates[start].trim_end_matches("_verso");
        if !same {
            let len = i - start;
            if len >= REPEAT_RUN {
                runs.push((start, len, templates[start].trim_end_matches("_verso")));
            }
            start = i;
        }
    }
    runs
}

fn quantile(scores: &[f64], q: f64) -> f64 {
    let mut sorted = scores.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un bloc hors marge et un bloc qui déborde font monter leurs deux
    /// compteurs — **et eux seuls**. La seconde moitié est la vraie
    /// assertion : les dix compteurs d'avant lisent des photos et des
    /// gabarits, et un objet libre n'a rien à y changer. On compare les douze
    /// à ceux du même album sans les blocs, plutôt que de les citer un par un :
    /// un onzième compteur ajouté demain entre dans la comparaison tout seul.
    #[test]
    fn les_deux_compteurs_dobjet_libre_montent_et_rien_dautre() {
        use crate::model::{Album, Contenu, Objet, Size, Slot, Spread};

        let mut album = Album::new("t", std::path::Path::new("/p"), Size { w: 210.0, h: 210.0 });
        album.spreads = (0..4)
            .map(|i| Spread {
                template: "solo".into(),
                slots: vec![Slot::new(format!("{i}.jpg"), [0.5, 0.5])],
                caption: (i == 0).then(|| "Chapitre".to_string()),
                text: None,
                edited: false,
                locked: false,
                objets: Vec::new(),
            })
            .collect();
        let infos: HashMap<String, PhotoInfo> = (0..4)
            .map(|i| {
                (
                    format!("{i}.jpg"),
                    PhotoInfo {
                        w: 4000.0,
                        h: 3000.0,
                        dhash: 0x0f0f_0f0f_0f0f_0f0f ^ (i as u64) << 40,
                        phash: 0xf0f0_f0f0_f0f0_f0f0 ^ (i as u64) << 8,
                        colorsig: [i as u8 * 40; 12],
                        score: 1.0 + i as f64,
                        faces: Vec::new(),
                        orig: (6000, 4500),
                        taken: None,
                    },
                )
            })
            .collect();
        let g = pdf::geometry(&album);
        let avant = compteurs(&album, &infos, &g);

        // Un bloc collé au bord : dans le fond perdu de la coupe, donc hors de
        // la zone sûre. Son texte tient dans sa boîte — il ne doit lever que
        // le premier compteur.
        let colle = Objet {
            x: 5.0,
            y: 5.0,
            w: 40.0,
            h: 30.0,
            angle: 0.0,
            contenu: Contenu::Texte {
                texte: "court".into(),
                taille_pt: 9.0,
                interligne_mm: None,
                alignement: Default::default(),
            },
        };
        // Et un bloc au milieu de la page de gauche, bien au chaud, dont le
        // texte ne tient pas dans la hauteur : lui ne doit lever que le second.
        let plein = Objet {
            x: 60.0,
            y: 60.0,
            w: 50.0,
            h: 6.0,
            angle: 0.0,
            contenu: Contenu::Texte {
                texte: "un texte bien trop long pour six millimètres de hauteur, \
                        qui reviendra donc à la ligne plusieurs fois"
                    .into(),
                taille_pt: 11.0,
                interligne_mm: None,
                alignement: Default::default(),
            },
        };
        // Les deux hypothèses du test, vérifiées plutôt que supposées : sans
        // elles, un test vert pourrait ne prouver que « les deux blocs sont
        // hors marge », ou « les deux débordent ».
        let rect = |o: &Objet| pdf::Rect { x: o.x, y: o.y, w: o.w, h: o.h };
        assert!(crate::scene::hors_marge(&rect(&colle), 0.0, &g), "le bloc collé au bord");
        assert!(!crate::scene::hors_marge(&rect(&plein), 0.0, &g), "le bloc du milieu");

        album.spreads[1].objets = vec![colle];
        album.spreads[2].objets = vec![plein];
        let apres = compteurs(&album, &infos, &g);

        assert_eq!(apres.objet_hors_marge.count, 1, "{:?}", apres.objet_hors_marge.details);
        assert_eq!(apres.objet_deborde.count, 1, "{:?}", apres.objet_deborde.details);
        assert_eq!(apres.objet_hors_marge.details[0].planche, 2, "la planche est nommée");
        assert_eq!(apres.objet_deborde.details[0].planche, 3);
        // Mous : deux blocs posés exprès ne rougissent pas le gate à eux seuls.
        assert!(!apres.objet_hors_marge.dur && !apres.objet_deborde.dur);
        assert_eq!((apres.objet_hors_marge.seuil, apres.objet_deborde.seuil), (0, 0));

        // Et les dix autres n'ont pas bougé d'une unité.
        let (a, b) = (avant.all(), apres.all());
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate().take(10) {
            assert_eq!(x.count, y.count, "compteur {i} a bougé : {:?}", y.details);
        }
        assert_eq!((a[10].count, a[11].count), (0, 0), "aucun objet, aucun compteur");
    }

    #[test]
    fn flat_runs_finds_long_stretches_only() {
        let t = vec![2usize; 7];
        assert_eq!(flat_runs(&t), vec![(0, 7)]);
        let mut t = vec![2usize; 6];
        t.push(1);
        t.extend(vec![3usize; 7]);
        assert_eq!(flat_runs(&t), vec![(7, 7)]);
        let t = [2usize, 1, 2];
        assert!(flat_runs(&t).is_empty());
    }

    /// A book opening on its half-title: the chapters start at the first
    /// photographs, so the page nobody composed is neither a chapter of one
    /// spread nor an album opening without a caption. Both counters are
    /// strict enough that getting this wrong turns every album red.
    #[test]
    fn the_machine_pages_are_outside_the_chapter_structure() {
        let mut a = crate::model::Album::new(
            "t",
            std::path::Path::new("/p"),
            crate::model::Size { w: 210.0, h: 210.0 },
        );
        let planche = |caption: Option<&str>| crate::model::Spread {
            template: "duo".into(),
            slots: vec![
                crate::model::Slot::new("a.jpg".into(), [0.5, 0.5]),
                crate::model::Slot::new("b.jpg".into(), [0.5, 0.5]),
            ],
            caption: caption.map(str::to_string),
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        };
        let faits = crate::colophon::Faits {
            photos_retenues: 6,
            photos_scannees: 9,
            debut: None,
            fin: None,
            lieux: Vec::new(),
            appareils: Vec::new(),
            compose_le: chrono::NaiveDate::from_ymd_opt(2026, 8, 18).unwrap(),
        };
        a.spreads = vec![
            crate::garde::spread("Corse", &faits, 190.0),
            planche(Some("Porto-Vecchio")),
            planche(None),
            planche(Some("Bonifacio")),
            crate::colophon::spread(&faits, a.trim_mm, 150.0, "0.9.0"),
        ];
        assert_eq!(premiere_photo(&a), Some(1));
        assert_eq!(chapter_ranges(&a), vec![1..3, 3..5]);
        // And the caption the audit looks for is the first chapter's, on the
        // first spread of photographs.
        assert!(a.spreads[premiere_photo(&a).unwrap()].caption.is_some());
    }

    #[test]
    fn repeat_runs_ignores_verso_variants() {
        let t = ["trio", "trio_verso", "trio", "trio_verso", "duo"];
        assert_eq!(repeat_runs(&t), vec![(0, 4, "trio")]);
        let t = ["duo", "trio", "duo", "trio"];
        assert!(repeat_runs(&t).is_empty());
    }

    #[test]
    fn face_cut_only_counts_cropped_edges() {
        // Square cell over a landscape image: crop cuts left and right only.
        let rect = pdf::Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 };
        // Face against the left edge of the visible window: cut.
        let faces = [[0.05, 0.4, 0.2, 0.3]];
        let cuts = face_cuts(&rect, 2000.0, 1000.0, [0.5, 0.5], 1.0, &faces);
        assert_eq!(cuts, vec!["gauche"]);
        // Same face, but the crop anchored left keeps it fully visible.
        let cuts = face_cuts(&rect, 2000.0, 1000.0, [0.0, 0.5], 1.0, &faces);
        assert!(cuts.is_empty());
        // A face against the top border was framed that way by the
        // photographer: vertical edges are not cropped here, no cut.
        let faces = [[0.45, 0.0, 0.2, 0.3]];
        let cuts = face_cuts(&rect, 2000.0, 1000.0, [0.5, 0.5], 1.0, &faces);
        assert!(cuts.is_empty());
        // Zoomed in around the centre, the top edge becomes a crop too:
        // the same top-border face now reads as cut.
        let cuts = face_cuts(&rect, 2000.0, 1000.0, [0.5, 0.5], 1.5, &faces);
        assert_eq!(cuts, vec!["haut"]);
    }

    #[test]
    fn quantile_picks_the_upper_bar() {
        let s = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(quantile(&s, 0.75), 3.0);
    }
}
