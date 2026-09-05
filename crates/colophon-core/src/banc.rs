//! The entry bench of a generated template (1.5). The generator enumerates
//! thousands of combinations (`gabarit::combinaisons`); this bench decides
//! which ones exist, the way `Densite::offertes` was decided: a candidate is
//! only green measured against the composed reference sets, through the
//! linter's own counters.
//!
//! The mechanism is substitution, one spread at a time: the composer's
//! albums are taken as they are, and everywhere a candidate is *assignable*
//! (the composer's own rules: capacity exact, no orientation betrayed past
//! `ASPECT_BETRAYAL`, every cell printable above the resolution floor, every
//! face keepable clear of the cropped edges), it is swapped in and the
//! counters recount. Substituting everywhere at once would manufacture
//! `repetition_gabarit` defects no real use would show.
//!
//! Two deliberate carve-outs, both documented at the grill:
//! - A band candidate (`legende > 0`) gets a test caption posed on the
//!   substituted spread, because `legende_manquante` is a hard counter and
//!   the bench has no user to write the line. In real use the linter keeps
//!   flagging an empty declared band, which is the counter doing its job.
//! - The chapter-structure counters (`chapitre_orphelin`,
//!   `ouverture_faible`) are judged on the base album: captions delimit
//!   chapters, so the test caption would split one, and the split is the
//!   bench's artefact, not the template's defect.
//!
//! Verdict: a candidate is green when it was assignable on at least one
//! spread of *each* reference set (a set where it never fits leaves its
//! verdict hollow) and no substitution anywhere pushed a counter past its
//! threshold.

use crate::audit::{self, PhotoInfo};
use crate::gabarit::{self, Spec};
use crate::model::{Album, Slot};
use crate::pdf::{self, SpreadGeometry};
use crate::print;
use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// The caption the bench poses on a band candidate's spread.
const LEGENDE_DU_BANC: &str = "Le banc";

/// The album files a bench directory contributes: the composed album and
/// the two alternative proposals, when their files are still beside it.
const VARIANTES: [&str; 3] = ["album.json", "album.autre-rythme.json", "album.resserree.json"];

#[derive(Debug, Serialize)]
pub struct RapportBanc {
    /// Enumerated candidates, before any verdict.
    pub candidats: usize,
    /// The reference sets seen, by their source folder.
    pub jeux: Vec<String>,
    /// Albums measured (directories × variants).
    pub albums: usize,
    /// The names that earned their place: assignable on every set, every
    /// substitution green. Paste into `gabarit::RETENUS`.
    pub verts: Vec<String>,
    /// Candidates never assignable anywhere.
    pub sans_essai: usize,
    /// Candidates assignable somewhere, but not on every set.
    pub jeu_manquant: usize,
    /// Candidates a counter refused, with the first defect of each.
    pub recales: usize,
    pub defauts: BTreeMap<String, String>,
}

/// One candidate's running state across the bench.
struct Etat {
    jeux: BTreeSet<String>,
    defaut: Option<String>,
}

/// Run the bench over composed album directories. Directories group into
/// reference sets by the album's source folder (`album.root`); the caller
/// composes them fresh (`scripts/banc-gabarits.sh`), this side only reads.
pub fn banc(dirs: &[PathBuf], progress: &dyn Fn(String)) -> Result<RapportBanc> {
    let candidats = gabarit::combinaisons();
    let mut etats: Vec<Etat> = candidats
        .iter()
        .map(|_| Etat { jeux: BTreeSet::new(), defaut: None })
        .collect();
    let mut jeux: BTreeSet<String> = BTreeSet::new();
    let mut albums = 0usize;

    for dir in dirs {
        let charges = charge_albums(dir)?;
        let (jeu, srcs) = {
            let premier = &charges[0].1;
            let root = PathBuf::from(&premier.root);
            if !root.is_dir() {
                bail!(
                    "{} : dossier de photos introuvable ({}), le banc ne mesure pas \
                     une résolution absente",
                    dir.display(),
                    root.display()
                );
            }
            let mut srcs: Vec<String> = charges
                .iter()
                .flat_map(|(_, a)| a.spreads.iter().flat_map(|s| s.slots.iter().map(|sl| sl.src.clone())))
                .collect();
            srcs.sort();
            srcs.dedup();
            (premier.root.clone(), srcs)
        };
        jeux.insert(jeu.clone());
        let root = PathBuf::from(&jeu);
        let (infos, _) = audit::mesure_photos(dir, &root, true, &srcs)
            .with_context(|| format!("mesure des photos de {}", dir.display()))?;

        for (variante, album) in &charges {
            albums += 1;
            let g = pdf::geometry(album);
            let base = audit::compteurs(album, &infos, &g);
            if !base.all().iter().all(|c| c.passes()) {
                bail!(
                    "{}/{variante} n'est pas vert avant toute substitution : \
                     le banc exige une référence propre (check.sh)",
                    dir.display()
                );
            }
            progress(format!(
                "banc : {}/{variante} ({} planches)",
                dir.display(),
                album.spreads.len()
            ));

            let resultats: Vec<(usize, bool, Option<String>)> = candidats
                .par_iter()
                .enumerate()
                .filter(|(i, _)| etats[*i].defaut.is_none())
                .map(|(i, cand)| {
                    let mut essaye = false;
                    for si in 0..album.spreads.len() {
                        match essaie_sur_planche(album, &infos, &g, cand, si) {
                            None => continue,
                            Some(Ok(())) => essaye = true,
                            Some(Err(compteur)) => {
                                return (
                                    i,
                                    true,
                                    Some(format!(
                                        "{}/{variante}, planche {} : {compteur}",
                                        dir.display(),
                                        si + 1
                                    )),
                                );
                            }
                        }
                    }
                    (i, essaye, None)
                })
                .collect();
            for (i, essaye, defaut) in resultats {
                if essaye {
                    etats[i].jeux.insert(jeu.clone());
                }
                if let Some(d) = defaut {
                    etats[i].defaut = Some(d);
                }
            }
        }
    }

    let mut verts = Vec::new();
    let mut defauts = BTreeMap::new();
    let (mut sans_essai, mut jeu_manquant, mut recales) = (0usize, 0usize, 0usize);
    for (cand, etat) in candidats.iter().zip(&etats) {
        match (&etat.defaut, etat.jeux.len()) {
            (Some(d), _) => {
                recales += 1;
                defauts.insert(cand.nom.to_string(), d.clone());
            }
            (None, 0) => sans_essai += 1,
            (None, n) if n < jeux.len() => jeu_manquant += 1,
            (None, _) => verts.push(cand.nom.to_string()),
        }
    }
    Ok(RapportBanc {
        candidats: candidats.len(),
        jeux: jeux.into_iter().collect(),
        albums,
        verts,
        sans_essai,
        jeu_manquant,
        recales,
        defauts,
    })
}

/// The albums a bench directory holds: `album.json` plus the proposals still
/// beside it. `album.origin.json` stays out: it duplicates the composed
/// album and is the reprise's reference, never an album of its own.
fn charge_albums(dir: &Path) -> Result<Vec<(&'static str, Album)>> {
    let mut out = Vec::new();
    for nom in VARIANTES {
        let path = dir.join(nom);
        if !path.is_file() {
            continue;
        }
        let album: Album = serde_json::from_str(
            &std::fs::read_to_string(&path)
                .with_context(|| format!("lecture de {}", path.display()))?,
        )
        .with_context(|| format!("{} illisible", path.display()))?;
        out.push((nom, album));
    }
    if out.is_empty() {
        bail!("{} : aucun album.json à mesurer", dir.display());
    }
    Ok(out)
}

/// Try one candidate on one spread. `None`: not assignable there (wrong
/// count, an orientation betrayed, a cell under the floor, a face pinned).
/// `Some(Ok)`: substituted and every counter held. `Some(Err(nom))`: the
/// counter that refused.
pub(crate) fn essaie_sur_planche(
    album: &Album,
    infos: &HashMap<String, PhotoInfo>,
    g: &SpreadGeometry,
    cand: &Spec,
    si: usize,
) -> Option<Result<(), String>> {
    let spread = &album.spreads[si];
    if spread.slots.is_empty() || spread.slots.len() != cand.capacite {
        return None;
    }
    let slots = assignation(cand, &spread.slots, infos, g)?;

    let mut essai = album.clone();
    essai.spreads[si].template = cand.nom.to_string();
    essai.spreads[si].slots = slots;
    if cand.legende > 0.0 && essai.spreads[si].caption.is_none() {
        essai.spreads[si].caption = Some(LEGENDE_DU_BANC.into());
    }

    let c = audit::compteurs(&essai, infos, g);
    // The chapter-structure counters are judged on the base album (the
    // bench's test caption would split a chapter; see the module doc).
    let juges: [(&str, &audit::Counter); 8] = [
        ("visage_coupe", &c.visage_coupe),
        ("orientation_trahie", &c.orientation_trahie),
        ("doublon_planche", &c.doublon_planche),
        ("sous_resolution", &c.sous_resolution),
        ("rythme_plat", &c.rythme_plat),
        ("legende_manquante", &c.legende_manquante),
        ("legende_sur_photo", &c.legende_sur_photo),
        ("repetition_gabarit", &c.repetition_gabarit),
    ];
    match juges.iter().find(|(_, c)| !c.passes()) {
        Some((nom, _)) => Some(Err((*nom).to_string())),
        None => Some(Ok(())),
    }
}

/// The composer's assignment rules, read off the audit's measurements:
/// cells and photos paired by sorted aspect, every pair orientation-true,
/// printable above the floor, faces clear. On success, the substituted
/// slots with a face-safe focal, in cell order.
fn assignation(
    cand: &Spec,
    slots: &[Slot],
    infos: &HashMap<String, PhotoInfo>,
    g: &SpreadGeometry,
) -> Option<Vec<Slot>> {
    let cells = gabarit::slots(cand, slots.len(), g);
    let n = slots.len();
    if cells.len() != n {
        return None;
    }
    let mut ci: Vec<usize> = (0..n).collect();
    ci.sort_by(|&a, &b| {
        (cells[a].w / cells[a].h).partial_cmp(&(cells[b].w / cells[b].h)).unwrap()
    });
    let mut pi: Vec<usize> = (0..n).collect();
    pi.sort_by(|&a, &b| {
        let ra = infos[&slots[a].src].w / infos[&slots[a].src].h;
        let rb = infos[&slots[b].src].w / infos[&slots[b].src].h;
        ra.partial_cmp(&rb).unwrap()
    });

    let mut out: Vec<Option<Slot>> = vec![None; n];
    for k in 0..n {
        let cell = &cells[ci[k]];
        let slot = &slots[pi[k]];
        let info = &infos[&slot.src];
        let a = info.w / info.h;
        let ca = cell.w / cell.h;
        if (a / ca).max(ca / a) > audit::ASPECT_BETRAYAL {
            return None;
        }
        let (ow, oh) = info.orig;
        if print::PRINT_DPI / print::print_scale(cell, ow, oh) < audit::MIN_EFFECTIVE_PPI {
            return None;
        }
        // The visible window of a cover crop, and the face-safe offset the
        // composer would pick: same arithmetic as `layout::face_safe_focal`.
        let s = (cell.w / info.w).max(cell.h / info.h);
        let (vw, vh) = (cell.w / s, cell.h / s);
        let mut focal = slot.focal;
        for (axe, total, visible) in [(0usize, info.w, vw), (1usize, info.h, vh)] {
            let extent =
                crate::layout::face_extent_dims(&info.faces, info.w, info.h, axe == 0);
            let (point, ok) =
                crate::layout::safe_focal_axis(total, visible, extent, focal[axe]);
            if !ok {
                return None;
            }
            focal[axe] = point;
        }
        out[ci[k]] = Some(Slot::new(slot.src.clone(), focal));
    }
    Some(out.into_iter().map(|s| s.expect("chaque case est servie")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Size, Spread};

    fn geom_album() -> (Album, SpreadGeometry) {
        let album = Album::new("banc", Path::new("/nulle-part"), Size { w: 210.0, h: 210.0 });
        let g = pdf::geometry(&album);
        (album, g)
    }

    /// A distinct, well-behaved photo: unique hashes, big original, no face.
    fn info(seed: u64, w: f64, h: f64) -> PhotoInfo {
        PhotoInfo {
            w,
            h,
            dhash: seed.wrapping_mul(0x9e37_79b9_7f4a_7c15),
            phash: seed.wrapping_mul(0xc2b2_ae3d_27d4_eb4f) | 0xf0f0,
            colorsig: {
                let mut c = [0u8; 12];
                c.iter_mut().enumerate().for_each(|(i, v)| *v = ((seed * 37 + i as u64 * 61) % 251) as u8);
                c
            },
            score: 1.0 + seed as f64 * 0.01,
            faces: Vec::new(),
            orig: (6000, (6000.0 * h / w) as u32),
            taken: None,
        }
    }

    fn planche(srcs: &[&str], template: &str, caption: Option<&str>) -> Spread {
        Spread {
            template: template.into(),
            slots: srcs.iter().map(|s| Slot::new((*s).into(), [0.5, 0.42])).collect(),
            caption: caption.map(str::to_string),
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        }
    }

    /// An album of portrait duos, every counter green: the bench's terrain.
    fn terrain() -> (Album, HashMap<String, PhotoInfo>, SpreadGeometry) {
        let (mut album, g) = geom_album();
        let mut infos = HashMap::new();
        let mut noms = Vec::new();
        for i in 0..8u64 {
            let nom = format!("p{i}.jpg");
            // Scores descending: the chapter opens on its strongest photo,
            // the way the composer would have left it.
            let mut inf = info(i + 1, 900.0, 1200.0);
            inf.score = 2.0 - i as f64 * 0.01;
            infos.insert(nom.clone(), inf);
            noms.push(nom);
        }
        album.spreads = vec![
            planche(&[&noms[0]], "solo", Some("Ouverture")),
            planche(&[&noms[1], &noms[2]], "duo_portrait", None),
            planche(&[&noms[3], &noms[4]], "duo_portrait", None),
            planche(&[&noms[5]], "solo", None),
            planche(&[&noms[6], &noms[7]], "duo_portrait", None),
        ];
        let base = audit::compteurs(&album, &infos, &g);
        assert!(base.all().iter().all(|c| c.passes()), "le terrain doit être vert");
        (album, infos, g)
    }

    /// Orientation rules refuse the assignment: two portraits never enter a
    /// panorama pair, and a spread of the wrong count is no trial at all.
    #[test]
    fn l_assignation_suit_les_regles_du_composer() {
        let (album, infos, g) = terrain();
        let pano = gabarit::parse_genere("g_1x1n_1x1n").unwrap();
        assert!(essaie_sur_planche(&album, &infos, &g, &pano, 1).is_none());
        let duo = gabarit::parse_genere("g_1x1q_1x1q").unwrap();
        assert!(matches!(essaie_sur_planche(&album, &infos, &g, &duo, 1), Some(Ok(()))));
        assert!(essaie_sur_planche(&album, &infos, &g, &duo, 0).is_none(), "capacité 2 sur une planche de 1");
    }

    /// A cell under the resolution floor is not assignable: the bench never
    /// fabricates a defect the composer's own rules would have refused.
    #[test]
    fn le_plancher_de_resolution_refuse_l_assignation() {
        let (album, mut infos, g) = terrain();
        for i in [1u64, 2] {
            infos.get_mut(&format!("p{}.jpg", i)).unwrap().orig = (900, 1200);
        }
        let duo = gabarit::parse_genere("g_1x1q_1x1q").unwrap();
        assert!(essaie_sur_planche(&album, &infos, &g, &duo, 1).is_none());
    }

    /// A band candidate poses the bench's caption (the hard counter would
    /// otherwise refuse every band forever), and the chapter counters stay
    /// the base album's: the split a test caption causes is the bench's
    /// artefact, not the template's.
    #[test]
    fn la_bande_pose_sa_legende_et_les_chapitres_sont_figes() {
        let (album, infos, g) = terrain();
        let bande = gabarit::parse_genere("g_1x1q_1x1q_b8").unwrap();
        assert!(
            matches!(essaie_sur_planche(&album, &infos, &g, &bande, 2), Some(Ok(()))),
            "la bande sur une planche muette doit passer par la légende du banc"
        );
    }

    /// A zero-band candidate whose cells smother every caption spot fails
    /// `legende_sur_photo` on a captioned spread: the genuinely reachable
    /// defect, and the counter deciding is the whole point of the bench.
    #[test]
    fn une_legende_etouffee_recale_le_candidat() {
        let (mut album, mut infos, g) = terrain();
        let mut carre = info(20, 1200.0, 1150.0);
        carre.score = 3.0; // l'ouverture reste la plus forte du chapitre
        infos.insert("q1.jpg".into(), carre);
        album.spreads[0] = planche(&["q1.jpg"], "solo_carre", Some("Ouverture"));
        let base = audit::compteurs(&album, &infos, &g);
        assert!(base.all().iter().all(|c| c.passes()));
        // Full bleed on the recto covers the low caption spots and the high
        // ones on that page; free ratio on the verso covers the left ones.
        let etouffe = gabarit::parse_genere("g_p_p").unwrap();
        album.spreads.insert(1, planche(&["p1.jpg", "p2.jpg"], "duo", Some("Chapitre")));
        let base = audit::compteurs(&album, &infos, &g);
        assert!(
            base.all().iter().all(|c| c.passes()),
            "la planche insérée doit laisser la référence verte : sans ça, le \
             legende_sur_photo mesuré plus bas serait le sien, pas celui du candidat"
        );
        match essaie_sur_planche(&album, &infos, &g, &etouffe, 1) {
            Some(Err(nom)) => assert_eq!(nom, "legende_sur_photo"),
            autre => panic!("attendu legende_sur_photo, obtenu {autre:?}"),
        }
    }
}
