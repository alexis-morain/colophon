//! La bascule : le même album dans un autre format.
//!
//! Changing the trim size rebuilds half of what an album is — the margins,
//! the cell shapes, the cover sheet, the printed resolution. What it must
//! **not** rebuild is the album: the same spreads, in the same order, holding
//! the same photographs. A bascule that recomposes is a bascule that destroys
//! the work by hand, which is the reproach the project makes of CEWE and of
//! Saal, and the reason wave 3 exists.
//!
//! So this is not a recomposition. [`crate::build::build_album`] with
//! `pinned` rebuilds everything not pinned; the bascule rebuilds nothing. It
//! carries every spread across and re-fits only what stopped fitting.
//!
//! **It decodes no photograph.** The only measurement it needs is each
//! photo's pixel size. An album carrying a relevé ([`crate::releve`]) gives
//! it in one file, and that album may hold no photograph at all — which is
//! what makes the bascule verifiable in full in the portable gate, on the
//! three OS. An album composed *from* photographs carries no relevé, and the
//! sizes then come from the originals' own headers: a few bytes per file,
//! never a decode. Either way a bascule answers in a second rather than in
//! the minutes a recomposition costs.
//!
//! Two things can change, and nothing else: `trim_mm`, and the `template` of
//! a spread whose photos would betray their new cells. Everything the bilan
//! names is something a human would otherwise have discovered too late.

use crate::audit::MIN_EFFECTIVE_PPI;
use crate::cover;
use crate::gabarit;
use crate::model::{Album, Size};
use crate::pdf::{self, Rect, SpreadGeometry};
use crate::print;
use crate::printer::PrinterProfile;
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

/// Pixel sizes of the album's photographs, EXIF orientation applied, keyed by
/// the path a `Slot` carries (relative to the album root). Built from the
/// relevé: see [`tailles_du_releve`].
pub type Tailles = HashMap<String, (u32, u32)>;

/// One photograph the new format pushes under the printable floor. The first
/// section of the bilan, because it is the only damage a hand cannot undo:
/// a template that no longer suits is visible and switchable, a photo short
/// of pixels needs another photograph.
#[derive(Debug, Clone, Serialize)]
pub struct SousResolution {
    /// 1-based, like every index the interface shows.
    pub planche: usize,
    pub src: String,
    pub ppi_avant: f64,
    pub ppi_apres: f64,
}

/// A spread whose template stopped fitting and was given another one.
#[derive(Debug, Clone, Serialize)]
pub struct Repli {
    pub planche: usize,
    pub avant: String,
    pub apres: String,
}

/// A spread whose template stopped fitting and was kept anyway, no template
/// of the same capacity being fit for its photos. Kept rather than shrunk:
/// a smaller template drops photographs, and a photograph lost to a bascule
/// is exactly what this module refuses. A betrayed cell is visible, the
/// linter counts it, and one click changes it.
#[derive(Debug, Clone, Serialize)]
pub struct Inapte {
    pub planche: usize,
    pub gabarit: String,
    /// How far the worst photo is from its cell, ×1 being a perfect match.
    pub trahison: f64,
}

/// What a bascule did, and what it cost. Data, not a sentence: the CLI
/// prints it, the interface renders it, and a test reads it.
#[derive(Debug, Clone, Serialize)]
pub struct Bilan {
    pub trim_avant: Size,
    pub trim_apres: Size,
    pub planches: usize,
    /// Spreads carried across with their template untouched. On a healthy
    /// bascule this is very nearly `planches`.
    pub planches_inchangees: usize,
    /// Photographs the new format pushes under [`MIN_EFFECTIVE_PPI`], having
    /// been above it. The bascule's own damage.
    pub sous_resolution: Vec<SousResolution>,
    /// Photographs already under the floor before the bascule. Not this
    /// operation's doing, and not hidden either.
    pub deja_sous_resolution: usize,
    pub replis: Vec<Repli>,
    pub inaptes: Vec<Inapte>,
    /// Pinned spreads (edited or locked) whose template had to move. A
    /// geometry change happens under everyone; the flag shields from a
    /// recomposition, not from the page changing shape. Named here because
    /// « aucune planche éditée perdue sans être nommée ».
    pub epinglees_touchees: Vec<usize>,
    /// Photographs absent from the relevé: their spreads were carried across
    /// untouched rather than judged on a measurement nobody has.
    pub tailles_manquantes: Vec<String>,
    /// True when the front-cover photograph falls under the floor. The cover
    /// is the largest printed image of the book, so an enlargement hits it
    /// first.
    pub couverture_sous_resolution: Option<SousResolution>,
}

impl Bilan {
    /// Nothing was rebuilt and nothing was lost.
    pub fn sans_degat(&self) -> bool {
        self.sous_resolution.is_empty()
            && self.inaptes.is_empty()
            && self.couverture_sous_resolution.is_none()
    }
}

/// The photo sizes of a relevé, keyed the way a `Slot` names its photograph.
///
/// Through `Releve::src`, and never through `path` directly: `Releve::lire`
/// recomposes every path from the root the file names, so a fiche read back
/// carries `corse-2013/photo.jpg` where a slot carries `photo.jpg`. Keyed on
/// the raw path the map matches nothing at all, and the bascule then reports
/// every spread as untouched — a no-op that looks exactly like a success, on
/// precisely the albums this path exists to serve.
pub fn tailles_du_releve(releve: &crate::releve::Releve) -> Tailles {
    releve.photos.iter().map(|p| (releve.src(&p.path), p.orig)).collect()
}

/// Every photograph's **original** pixel size, for one album folder.
///
/// Original, and never a thumbnail's: the aspect ratio would survive the
/// reduction, but the resolution would not, and a bascule judging 1600 px
/// thumbnails would declare the whole album under the printable floor. The
/// two things this module measures need the same number, so `Tailles` means
/// one thing only.
///
/// Two sources, in order. A relevé answers everything and costs one file —
/// that is the portable path, and an album composed without photographs has
/// nothing else. Otherwise the originals' own headers answer, which is a
/// read of a few bytes per file and never a decode. A photograph neither can
/// account for is simply absent from the map: its spread is then carried
/// across untouched and named in the bilan, rather than judged on a guess.
pub fn tailles_du_dossier(dir: &std::path::Path, album: &Album) -> Result<Tailles> {
    if let Some(releve) = crate::releve::Releve::dans_album(dir)? {
        return Ok(tailles_du_releve(&releve));
    }
    let root = std::path::Path::new(&album.root);
    let mut out = Tailles::new();
    let mut srcs: BTreeSet<&str> = album
        .spreads
        .iter()
        .flat_map(|s| s.slots.iter())
        .map(|s| s.src.as_str())
        .collect();
    if let Some(slot) = album.cover.as_ref().and_then(|c| c.photo.as_ref()) {
        srcs.insert(slot.src.as_str());
    }
    for src in srcs {
        // Two traps, both measured rather than guessed. One:
        // `heic::dimensions` and never `image::image_dimensions`, the former
        // being the project's one dispatch — the direct call silently lost
        // every iPhone photograph, 31 of mauritanie-2019's. Two: the EXIF
        // orientation must be applied, because `Photo::orig` is oriented and
        // a raw header is not — without it every rotated photograph reads
        // landscape when it is portrait, and the two paths disagreed on
        // three sets out of three.
        let p = root.join(src);
        if let Ok(taille) = crate::heic::oriented_dimensions(&p, crate::meta::read(&p).orientation)
        {
            out.insert(src.to_string(), taille);
        }
    }
    Ok(out)
}

/// The same album in another format, read from and written back to a folder.
///
/// `album.origin.json` is never touched: it is the reprise's reference, and
/// 3.3 decides the rest. The previous `album.json` survives as `.bak`, like
/// every other save.
pub fn bascule_dossier(
    dir: &std::path::Path,
    trim: Size,
    profil: &PrinterProfile,
    ecrire: bool,
) -> Result<(Album, Bilan)> {
    let path = dir.join("album.json");
    let album: Album = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .with_context(|| format!("lecture de {}", path.display()))?,
    )
    .context("album.json illisible")?;
    let tailles = tailles_du_dossier(dir, &album)?;
    let (apres, bilan) = bascule(&album, trim, &tailles, profil);
    if ecrire {
        crate::build::write_album_json(dir, &apres)?;
    }
    Ok((apres, bilan))
}

/// The same album, in another format.
///
/// `tailles` comes from the relevé; a photograph missing from it leaves its
/// spread untouched and its name in the bilan, never a silent judgement on a
/// measurement nobody has.
pub fn bascule(
    album: &Album,
    trim: Size,
    tailles: &Tailles,
    profil: &PrinterProfile,
) -> (Album, Bilan) {
    let mut apres = album.clone();
    apres.trim_mm = trim;

    let g_avant = pdf::geometry(album);
    let g_apres = pdf::geometry(&apres);

    let mut bilan = Bilan {
        trim_avant: album.trim_mm,
        trim_apres: trim,
        planches: album.spreads.len(),
        planches_inchangees: 0,
        sous_resolution: Vec::new(),
        deja_sous_resolution: 0,
        replis: Vec::new(),
        inaptes: Vec::new(),
        epinglees_touchees: Vec::new(),
        tailles_manquantes: Vec::new(),
        couverture_sous_resolution: None,
    };

    let mut manquantes = BTreeSet::new();

    // 1. The templates. Every spread keeps its photographs; only the name of
    //    the layout may move, and only when the new cells would betray them.
    for (i, spread) in apres.spreads.iter_mut().enumerate() {
        let planche = i + 1;
        // A text page, the colophon, a breathing page: no photograph, no
        // cell, nothing a ratio can betray.
        if spread.slots.is_empty() {
            bilan.planches_inchangees += 1;
            continue;
        }

        // In slot order, and only in slot order: `gabarit::trahison` zips
        // these against the spread's rectangles. A spread carrying the same
        // photograph twice is ordinary, so a map lookup is fine — the order
        // it is read back in is not.
        let mut aspects = Vec::with_capacity(spread.slots.len());
        for slot in &spread.slots {
            match tailles.get(&slot.src) {
                Some(&(w, h)) if w > 0 && h > 0 => aspects.push(f64::from(w) / f64::from(h)),
                _ => {
                    manquantes.insert(slot.src.clone());
                }
            }
        }
        if aspects.len() != spread.slots.len() {
            // Judged on nothing, so judged not at all.
            bilan.planches_inchangees += 1;
            continue;
        }

        let courant = gabarit::spec(&spread.template);
        let tient = courant
            .map(|s| gabarit::trahison(s, &aspects, &g_apres) <= crate::audit::ASPECT_BETRAYAL)
            .unwrap_or(false);
        if tient {
            bilan.planches_inchangees += 1;
            continue;
        }

        match replacant(&spread.template, &aspects, &g_apres) {
            Some(choisi) => {
                bilan.replis.push(Repli {
                    planche,
                    avant: spread.template.clone(),
                    apres: choisi.to_string(),
                });
                if spread.pinned() {
                    bilan.epinglees_touchees.push(planche);
                }
                spread.template = choisi.to_string();
            }
            None => {
                bilan.inaptes.push(Inapte {
                    planche,
                    gabarit: spread.template.clone(),
                    trahison: courant
                        .map(|s| gabarit::trahison(s, &aspects, &g_apres))
                        .unwrap_or(f64::INFINITY),
                });
            }
        }
    }

    // 2. The resolution, once the templates have settled: the rectangle a
    //    photograph prints into is the one it ends up in, not the one it
    //    started from.
    for (i, (av, ap)) in album.spreads.iter().zip(&apres.spreads).enumerate() {
        let rects_avant = pdf::slots_for(&av.template, av.slots.len(), &g_avant);
        let rects_apres = pdf::slots_for(&ap.template, ap.slots.len(), &g_apres);
        for (ci, slot) in ap.slots.iter().enumerate() {
            let (Some(&(w, h)), Some(ra), Some(rb)) =
                (tailles.get(&slot.src), rects_avant.get(ci), rects_apres.get(ci))
            else {
                continue;
            };
            let avant = print::effective_ppi(ra, w, h, slot.zoom);
            let apres_ppi = print::effective_ppi(rb, w, h, slot.zoom);
            if apres_ppi < MIN_EFFECTIVE_PPI {
                if avant < MIN_EFFECTIVE_PPI {
                    bilan.deja_sous_resolution += 1;
                } else {
                    bilan.sous_resolution.push(SousResolution {
                        planche: i + 1,
                        src: slot.src.clone(),
                        ppi_avant: avant,
                        ppi_apres: apres_ppi,
                    });
                }
            }
        }
    }

    // 3. The cover. Free to regenerate — its photograph is a `Slot` like any
    //    other and its focal has been ratio-invariant since 3.1 — but it is
    //    the biggest image in the book, so it is the first to run out of
    //    pixels when the format grows.
    if let (Some(c_av), Some(c_ap)) = (album.cover.as_ref(), apres.cover.as_ref()) {
        if let (Some(slot), Some(&(w, h))) = (
            c_ap.photo.as_ref(),
            c_ap.photo.as_ref().and_then(|s| tailles.get(&s.src)),
        ) {
            let _ = c_av;
            let ra = cover::photo_rect(&cover::geometry(album, profil));
            let rb = cover::photo_rect(&cover::geometry(&apres, profil));
            let avant = print::effective_ppi(&ra, w, h, slot.zoom);
            let apres_ppi = print::effective_ppi(&rb, w, h, slot.zoom);
            if apres_ppi < MIN_EFFECTIVE_PPI {
                bilan.couverture_sous_resolution = Some(SousResolution {
                    planche: 0,
                    src: slot.src.clone(),
                    ppi_avant: avant,
                    ppi_apres: apres_ppi,
                });
            }
        }
    }

    bilan.tailles_manquantes = manquantes.into_iter().collect();
    (apres, bilan)
}

/// The template a spread falls back to when its own stops fitting: fit for
/// the photographs, and **of the same capacity**, so not one photograph is
/// dropped. Among those, the closest to what the spread had — same family
/// first (a `duo` prefers `duo_paysage` to a `trio`), then the offered
/// order, which the picker and the dump already agree on.
fn replacant(
    courant: &str,
    aspects: &[f64],
    g: &SpreadGeometry,
) -> Option<&'static str> {
    let n = aspects.len();
    let famille = courant.trim_end_matches("_verso");
    let candidats: Vec<&'static str> = gabarit::compatibles(aspects, g)
        .into_iter()
        .filter(|nom| gabarit::spec(nom).is_some_and(|s| s.capacite == n))
        .collect();
    candidats
        .iter()
        .find(|nom| nom.trim_end_matches("_verso").starts_with(famille))
        .or_else(|| candidats.first())
        .copied()
}

/// The rectangle a slot prints into, for callers that need it without
/// rebuilding a geometry. Kept next to the bascule because it is the one
/// question a format change makes everybody ask.
pub fn rect_du_slot(album: &Album, planche: usize, case: usize) -> Option<Rect> {
    let spread = album.spreads.get(planche)?;
    pdf::slots_for(&spread.template, spread.slots.len(), &pdf::geometry(album))
        .get(case)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Slot, Spread};

    fn profil() -> &'static PrinterProfile {
        PrinterProfile::par_id("generique").unwrap()
    }

    fn carre() -> Size {
        Size { w: 210.0, h: 210.0 }
    }

    fn paysage() -> Size {
        Size { w: 280.0, h: 210.0 }
    }

    fn planche(template: &str, srcs: &[&str]) -> Spread {
        Spread {
            template: template.into(),
            slots: srcs.iter().map(|s| Slot::new((*s).into(), [0.5, 0.5])).collect(),
            caption: None,
            text: None,
            edited: false,
            locked: false,
        }
    }

    fn album_de(spreads: Vec<Spread>, trim: Size) -> Album {
        let mut a = Album::new("t", std::path::Path::new("/p"), trim);
        a.spreads = spreads;
        a
    }

    /// Photos big enough that resolution never enters the picture, so a test
    /// about templates is only about templates.
    fn tailles(entries: &[(&str, u32, u32)]) -> Tailles {
        entries.iter().map(|(s, w, h)| ((*s).to_string(), (*w, *h))).collect()
    }

    /// The promise, and the whole reason the module is not a recomposition:
    /// same spreads, same order, same photographs, same captions, same flags.
    #[test]
    fn la_structure_traverse_intacte() {
        let mut spreads = vec![
            planche("duo", &["a.jpg", "b.jpg"]),
            planche("full1", &["c.jpg"]),
            planche("trio", &["d.jpg", "e.jpg", "f.jpg"]),
        ];
        spreads[1].caption = Some("12 mars".into());
        spreads[2].locked = true;
        let album = album_de(spreads, carre());
        let t = tailles(&[
            ("a.jpg", 4000, 3000),
            ("b.jpg", 4000, 3000),
            ("c.jpg", 4000, 3000),
            ("d.jpg", 3000, 4000),
            ("e.jpg", 4000, 3000),
            ("f.jpg", 4000, 3000),
        ]);

        let (apres, bilan) = bascule(&album, paysage(), &t, profil());

        assert_eq!(apres.trim_mm.w, 280.0);
        assert_eq!(apres.trim_mm.h, 210.0);
        assert_eq!(apres.spreads.len(), album.spreads.len());
        for (av, ap) in album.spreads.iter().zip(&apres.spreads) {
            let srcs_av: Vec<&str> = av.slots.iter().map(|s| s.src.as_str()).collect();
            let srcs_ap: Vec<&str> = ap.slots.iter().map(|s| s.src.as_str()).collect();
            assert_eq!(srcs_av, srcs_ap, "le jeu de photos d'une planche ne bouge pas");
            assert_eq!(av.caption, ap.caption);
            assert_eq!(av.text, ap.text);
            assert_eq!(av.edited, ap.edited);
            assert_eq!(av.locked, ap.locked);
        }
        assert_eq!(bilan.planches, 3);
        assert!(bilan.tailles_manquantes.is_empty());
    }

    /// The adjustments table crosses the bascule whole: the module clones
    /// the album, so there is nothing to code and this test holds the
    /// nothing. A réglage is a property of the photograph, and the
    /// photograph does not change page shape.
    #[test]
    fn les_reglages_traversent_la_bascule() {
        let mut album = album_de(vec![planche("duo", &["a.jpg", "b.jpg"])], carre());
        album.reglages.insert(
            "a.jpg".into(),
            crate::model::Reglage { expo: 0.5, contraste: -1.0, nb: true },
        );
        let t = tailles(&[("a.jpg", 4000, 3000), ("b.jpg", 4000, 3000)]);
        let (apres, _) = bascule(&album, paysage(), &t, profil());
        assert_eq!(apres.reglages, album.reglages);
    }

    /// Every focal is untouched. This is 3.1's promise, and the bascule is
    /// its first real user: the crop means the same thing on any page shape,
    /// so there is nothing to convert and nothing to lose.
    #[test]
    fn aucun_recadrage_ne_bouge() {
        let mut p = planche("duo", &["a.jpg", "b.jpg"]);
        p.slots[0].focal = [0.31, 0.72];
        p.slots[0].zoom = 1.9;
        p.slots[1].focal = [0.8, 0.1];
        let album = album_de(vec![p], carre());
        let t = tailles(&[("a.jpg", 4000, 3000), ("b.jpg", 3000, 4000)]);

        let (apres, _) = bascule(&album, paysage(), &t, profil());

        assert_eq!(apres.spreads[0].slots[0].focal, [0.31, 0.72]);
        assert_eq!(apres.spreads[0].slots[0].zoom, 1.9);
        assert_eq!(apres.spreads[0].slots[1].focal, [0.8, 0.1]);
    }

    /// Basculer vers le format courant n'est pas un événement.
    #[test]
    fn basculer_sur_place_ne_change_rien() {
        let album = album_de(vec![planche("duo", &["a.jpg", "b.jpg"])], carre());
        let t = tailles(&[("a.jpg", 4000, 3000), ("b.jpg", 4000, 3000)]);

        let (apres, bilan) = bascule(&album, carre(), &t, profil());

        assert_eq!(apres.spreads[0].template, album.spreads[0].template);
        assert_eq!(bilan.planches_inchangees, bilan.planches);
        assert!(bilan.replis.is_empty());
        assert!(bilan.sous_resolution.is_empty());
        assert!(bilan.sans_degat());
    }

    /// A photograph the relevé does not carry leaves its spread alone and
    /// says so. The alternative — judging it on a guessed ratio — is how a
    /// template gets swapped for no reason at all.
    #[test]
    fn une_photo_sans_fiche_laisse_sa_planche_tranquille() {
        let album = album_de(vec![planche("duo", &["a.jpg", "inconnue.jpg"])], carre());
        let t = tailles(&[("a.jpg", 4000, 3000)]);

        let (apres, bilan) = bascule(&album, paysage(), &t, profil());

        assert_eq!(apres.spreads[0].template, "duo");
        assert_eq!(bilan.tailles_manquantes, vec!["inconnue.jpg".to_string()]);
        assert!(bilan.replis.is_empty());
    }

    /// Never a photograph fewer. Four panoramas in a quad are betrayed ×6 by
    /// their cells on any page shape, and no four-cell template suits them:
    /// the spread keeps the one it has and the bilan names it. Shrinking to a
    /// trio would have dropped a photograph, which is the one thing a bascule
    /// must never do.
    #[test]
    fn jamais_une_photo_de_moins() {
        let album = album_de(
            vec![planche("quad", &["a.jpg", "b.jpg", "c.jpg", "d.jpg"])],
            carre(),
        );
        let t = tailles(&[
            ("a.jpg", 8000, 1000),
            ("b.jpg", 8000, 1000),
            ("c.jpg", 8000, 1000),
            ("d.jpg", 8000, 1000),
        ]);

        let (apres, bilan) = bascule(&album, paysage(), &t, profil());

        assert_eq!(apres.spreads[0].slots.len(), 4, "aucune photo ne disparaît");
        assert_eq!(apres.spreads[0].template, "quad", "le gabarit se garde");
        assert!(bilan.replis.is_empty(), "{:?}", bilan.replis);
        assert_eq!(bilan.inaptes.len(), 1, "{:?}", bilan.inaptes);
        assert_eq!(bilan.inaptes[0].planche, 1);
        assert_eq!(bilan.inaptes[0].gabarit, "quad");
        assert!(
            bilan.inaptes[0].trahison > crate::audit::ASPECT_BETRAYAL,
            "trahison mesurée {} sous le seuil {}",
            bilan.inaptes[0].trahison,
            crate::audit::ASPECT_BETRAYAL
        );
        assert_eq!(bilan.planches_inchangees, 0, "elle a bien été jugée");
        assert!(!bilan.sans_degat());
    }

    /// A pinned spread is re-fitted like any other — the page changed shape
    /// under it — but never in silence. Two portraits in a free-cell `duo`
    /// stop fitting when the page turns landscape, and the fallback is real.
    #[test]
    fn une_planche_epinglee_repliee_est_nommee() {
        let mut p = planche("duo", &["p1.jpg", "p2.jpg"]);
        p.edited = true;
        let album = album_de(vec![p], carre());
        let t = tailles(&[("p1.jpg", 3000, 4000), ("p2.jpg", 3000, 4000)]);

        let (apres, bilan) = bascule(&album, paysage(), &t, profil());

        assert_eq!(apres.spreads[0].template, "duo_portrait");
        assert_eq!(apres.spreads[0].slots.len(), 2);
        assert!(apres.spreads[0].edited, "le drapeau ne se perd pas");
        assert_eq!(bilan.replis.len(), 1);
        assert_eq!(bilan.replis[0].avant, "duo");
        assert_eq!(bilan.replis[0].apres, "duo_portrait");
        assert_eq!(
            bilan.epinglees_touchees,
            vec![1],
            "une planche épinglée touchée se nomme"
        );
    }

    /// Une planche épinglée qu'on n'a pas touchée n'entre pas dans la liste
    /// des touchées — elle serait fausse — mais son gabarit inapte est nommé.
    /// Les deux listes disent deux choses différentes.
    #[test]
    fn une_epinglee_gardee_inapte_est_nommee_ailleurs() {
        let mut p = planche("solo_pano", &["pano.jpg"]);
        p.edited = true;
        let album = album_de(vec![p], carre());
        let t = tailles(&[("pano.jpg", 2000, 6000)]);

        let (apres, bilan) = bascule(&album, paysage(), &t, profil());

        assert_eq!(apres.spreads[0].template, "solo_pano", "rien ne lui a été fait");
        assert!(
            bilan.epinglees_touchees.is_empty(),
            "on ne déclare pas touché ce qu'on n'a pas touché"
        );
        assert_eq!(bilan.inaptes.len(), 1);
        assert_eq!(bilan.inaptes[0].planche, 1);
    }

    /// The damage a hand cannot undo. Enlarging the page divides the printed
    /// resolution by the same factor, and 250 ppi is a floor the project does
    /// not reopen.
    #[test]
    fn lagrandissement_nomme_ce_quil_fait_tomber_sous_le_plancher() {
        let album = album_de(vec![planche("full1", &["petite.jpg"])], carre());
        // Sized to sit just above the floor on a 210 page: full-bleed on a
        // 210 mm page is 216 mm tall with bleed, so ~2150 px is ~253 ppi.
        let t = tailles(&[("petite.jpg", 2150, 2150)]);

        let (_, sur_place) = bascule(&album, carre(), &t, profil());
        assert!(
            sur_place.sous_resolution.is_empty(),
            "avant la bascule elle tient : {:?}",
            sur_place.sous_resolution
        );

        let (_, grand) = bascule(&album, Size { w: 300.0, h: 300.0 }, &t, profil());
        assert_eq!(grand.sous_resolution.len(), 1, "{:?}", grand.sous_resolution);
        let s = &grand.sous_resolution[0];
        assert_eq!(s.src, "petite.jpg");
        assert!(s.ppi_avant >= MIN_EFFECTIVE_PPI, "{}", s.ppi_avant);
        assert!(s.ppi_apres < MIN_EFFECTIVE_PPI, "{}", s.ppi_apres);
        assert!(!grand.sans_degat());
    }

    /// A photograph already short of pixels is not this bascule's doing, and
    /// is not hidden under it either.
    #[test]
    fn ce_qui_etait_deja_sous_le_plancher_se_compte_a_part() {
        let album = album_de(vec![planche("full1", &["minuscule.jpg"])], carre());
        let t = tailles(&[("minuscule.jpg", 600, 600)]);

        let (_, bilan) = bascule(&album, paysage(), &t, profil());

        assert!(bilan.sous_resolution.is_empty(), "elle ne traverse rien, elle y était");
        assert_eq!(bilan.deja_sous_resolution, 1);
    }

    /// Round trip: what a bascule did not have to touch, a bascule back
    /// gives back identical.
    #[test]
    fn aller_retour_rend_les_gabarits_intacts() {
        let album = album_de(
            vec![
                planche("duo", &["a.jpg", "b.jpg"]),
                planche("full1", &["c.jpg"]),
            ],
            carre(),
        );
        let t = tailles(&[
            ("a.jpg", 4000, 3000),
            ("b.jpg", 4000, 3000),
            ("c.jpg", 3000, 4000),
        ]);

        let (aller, bilan_aller) = bascule(&album, paysage(), &t, profil());
        let (retour, _) = bascule(&aller, carre(), &t, profil());

        assert_eq!(retour.trim_mm.w, album.trim_mm.w);
        assert_eq!(retour.trim_mm.h, album.trim_mm.h);
        for (i, (av, ap)) in album.spreads.iter().zip(&retour.spreads).enumerate() {
            let replie = bilan_aller.replis.iter().any(|r| r.planche == i + 1);
            if !replie {
                assert_eq!(
                    av.template,
                    ap.template,
                    "planche {} n'a pas été repliée à l'aller",
                    i + 1
                );
            }
            assert_eq!(
                av.slots.iter().map(|s| s.focal).collect::<Vec<_>>(),
                ap.slots.iter().map(|s| s.focal).collect::<Vec<_>>(),
                "les recadrages font l'aller-retour sans bouger"
            );
        }
    }

    /// A text page has no cell to betray and no pixel to run out of.
    #[test]
    fn une_page_de_texte_traverse_sans_bruit() {
        let mut p = planche("texte", &[]);
        p.text = Some("deux lignes\net une autre".into());
        let album = album_de(vec![p], carre());

        let (apres, bilan) = bascule(&album, paysage(), &Tailles::new(), profil());

        assert_eq!(apres.spreads[0].text.as_deref(), Some("deux lignes\net une autre"));
        assert_eq!(apres.spreads[0].template, "texte");
        assert_eq!(bilan.planches_inchangees, 1);
        assert!(bilan.tailles_manquantes.is_empty());
    }

    /// Le relevé écrit puis relu doit rendre des clés que les slots
    /// reconnaissent. `Releve::lire` recompose chaque chemin depuis la racine
    /// que le fichier nomme, donc une fiche relue porte
    /// `corse-2013/photo.jpg` là où un slot porte `photo.jpg` : indexé sur le
    /// chemin brut, le tableau ne correspond à rien et la bascule déclare
    /// toutes les planches inchangées. Un faux vert parfait, sur exactement
    /// les albums que ce chemin existe pour servir — mesuré sur les trois
    /// jeux avant d'être corrigé.
    #[test]
    fn un_releve_relu_donne_des_cles_que_les_slots_reconnaissent() {
        use crate::analyze::Analysis;
        use crate::meta::PhotoMeta;
        use crate::pipeline::Photo;
        use crate::releve::Releve;

        let dir = std::env::temp_dir()
            .join(format!("colophon-bascule-releve-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let racine = std::path::PathBuf::from("/ailleurs/corse-2013");

        let fiche = |nom: &str, orig: (u32, u32)| Photo {
            path: racine.join(nom),
            meta: PhotoMeta {
                taken: chrono::NaiveDateTime::parse_from_str(
                    "2013-10-27 15:34:11",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
                taken_reliable: true,
                orientation: 1,
                gps: None,
                model: None,
                rating: None,
            },
            analysis: Analysis {
                dhash: 1,
                phash: 2,
                colorsig: [0; 12],
                sharpness: 1.0,
                exposure: 0.5,
                width: 500,
                height: 500,
            },
            orig,
            faces: Vec::new(),
            focal: None,
        };

        let releve = Releve {
            version: crate::releve::VERSION,
            racine: racine.clone(),
            skipped_heic: 0,
            skipped_other: 0,
            illisibles: Vec::new(),
            editees: Vec::new(),
            photos: vec![fiche("p1.jpg", (3000, 4000)), fiche("p2.jpg", (3000, 4000))],
            vignettes: false,
        };
        let chemin = dir.join(crate::releve::FICHIER);
        releve.ecrire(&chemin).unwrap();
        let relu = Releve::lire(&chemin).unwrap();

        let tailles = tailles_du_releve(&relu);
        assert_eq!(
            tailles.get("p1.jpg"),
            Some(&(3000, 4000)),
            "clés relues : {:?}",
            tailles.keys().collect::<Vec<_>>()
        );

        // Et le tableau doit vraiment servir : une planche jugée, pas une
        // planche « inchangée » faute de mesure.
        let album = album_de(vec![planche("duo", &["p1.jpg", "p2.jpg"])], carre());
        let (apres, bilan) = bascule(&album, paysage(), &tailles, profil());
        assert!(
            bilan.tailles_manquantes.is_empty(),
            "manquantes : {:?}",
            bilan.tailles_manquantes
        );
        assert_eq!(apres.spreads[0].template, "duo_portrait");
        assert_eq!(bilan.replis.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `album.origin.json` is the reprise's reference and never moves: a
    /// bascule writes `album.json`, and the proposal beside it stays
    /// byte-identical while the album's trim changes. Measured through the
    /// folder path, the one that owns every write. And the reprise, read on
    /// that same folder, withdraws its verdict rather than counting the
    /// machine's folds as hands.
    #[test]
    fn une_bascule_ne_reecrit_jamais_l_origine() {
        let dir = std::env::temp_dir()
            .join(format!("colophon-bascule-origine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let album = album_de(vec![planche("duo", &["p1.jpg", "p2.jpg"])], carre());
        let json = serde_json::to_string_pretty(&album).unwrap();
        std::fs::write(dir.join("album.json"), &json).unwrap();
        std::fs::write(dir.join("album.origin.json"), &json).unwrap();
        let origine_avant = std::fs::read(dir.join("album.origin.json")).unwrap();

        bascule_dossier(&dir, paysage(), profil(), true).unwrap();

        let relu: Album =
            serde_json::from_str(&std::fs::read_to_string(dir.join("album.json")).unwrap())
                .unwrap();
        assert_eq!((relu.trim_mm.w, relu.trim_mm.h), (paysage().w, paysage().h));
        assert_eq!(
            std::fs::read(dir.join("album.origin.json")).unwrap(),
            origine_avant,
            "le trim de l'origine n'a pas bougé pendant que celui de l'album bouge"
        );

        let r = crate::reprise::reprise(&dir).unwrap();
        assert_eq!(r.verdict, "non mesurable");
        assert!(r.ok);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
