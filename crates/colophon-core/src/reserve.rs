//! La réserve classée : les photos du dossier qu'aucune planche ne montre,
//! rangées pour une planche donnée (chantier E, décision E3).
//!
//! Le classement vit ici parce que les mesures y vivent : le dHash et le
//! score d'une photo se lisent dans le relevé quand l'album en porte un, et
//! sur ses vignettes sinon — les deux chemins de `audit::mesure_photos`, et
//! pas un troisième chiffre. L'app ne reçoit que des codes (`Raison`) : le
//! libellé est à elle.
//!
//! **C'est une aide à la main, et la main décide** (E5) : rien d'ici n'entre
//! dans le Composer, et une photo écartée par la curation reste candidate.

use crate::analyze;
use crate::audit::DUP_HAMMING;
use crate::model::{default_focal, Album, Discard};
use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Douze au plus : un popover de vignettes, pas un second tiroir.
pub const AU_PLUS: usize = 12;

/// Pourquoi une photo est là où elle est dans la liste. Un code, jamais une
/// phrase : la phrase est à l'app, dans sa langue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Raison {
    /// Prise le jour de la planche.
    MemeJour,
    /// Prise un autre jour ; les plus proches d'abord.
    Proche,
    /// Sans date à comparer : classée sur son score seul.
    Nette,
}

/// Une photo de la réserve, telle que le popover la montre.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    /// Relative à la racine, comme `Slot::src`.
    pub src: String,
    /// `Analysis::score`, la formule du relevé.
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taken: Option<NaiveDateTime>,
    pub raison: Raison,
    /// Le point focal qu'elle gardera une fois posée : celui que la curation
    /// lui a trouvé (un visage), sinon le défaut du Composer. Sans lui l'app
    /// devrait relire `curation.json` pour construire la case.
    pub focal: [f64; 2],
}

/// Ce que la commande rend : la liste, et ce qu'il faut savoir d'elle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reserve {
    pub candidats: Vec<Candidate>,
    /// Ce qui a manqué pour classer tout à fait (pas de date, vignette
    /// illisible), dit plutôt que tu. Absent quand rien n'a manqué.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Ce que le classement lit d'une photo. Une fiche réduite à trois
/// nombres, pour que le classement se teste sans vignette ni relevé.
#[derive(Debug, Clone, PartialEq)]
pub struct Fait {
    pub dhash: u64,
    pub score: f64,
    /// La prise de vue quand elle est fiable (EXIF ou sidecar), jamais un
    /// mtime : une date de fichier classerait sur l'ordre de copie.
    pub taken: Option<NaiveDateTime>,
}

/// Une photo que l'album ne montre pas, avec le focal qu'elle emportera.
#[derive(Debug, Clone, PartialEq)]
pub struct EnReserve {
    pub src: String,
    pub focal: [f64; 2],
}

/// Le jour qu'une planche raconte : la date médiane de ses photos datées.
/// Une planche sans photo datée (page de texte, planche vide) prend le jour
/// de ses voisines, les plus proches d'abord, les deux côtés confondus. Rien
/// nulle part : `None`, et la réserve se classe sur le score seul.
pub fn jour_de_reference(
    album: &Album,
    planche: usize,
    date_de: &dyn Fn(&str) -> Option<NaiveDateTime>,
) -> Option<NaiveDate> {
    let n = album.spreads.len();
    if planche >= n {
        return None;
    }
    let jours = |i: usize| -> Vec<NaiveDate> {
        album.spreads[i]
            .slots
            .iter()
            .filter_map(|s| date_de(&s.src).map(|d| d.date()))
            .collect()
    };
    for distance in 0..n {
        let mut dates = Vec::new();
        if let Some(i) = planche.checked_sub(distance) {
            dates.extend(jours(i));
        }
        if distance > 0 && planche + distance < n {
            dates.extend(jours(planche + distance));
        }
        if let Some(m) = mediane(dates) {
            return Some(m);
        }
    }
    None
}

fn mediane(mut dates: Vec<NaiveDate>) -> Option<NaiveDate> {
    if dates.is_empty() {
        return None;
    }
    dates.sort();
    Some(dates[dates.len() / 2])
}

/// Le classement lui-même, pur et en mémoire. Dans l'ordre : les photos du
/// jour de la planche, puis par écart de date croissant, à égalité de jour
/// par score décroissant ; les photos sans date ferment la marche, par score.
/// Écartées : les quasi-doublons d'une photo déjà posée sur la planche, au
/// seuil dHash de la curation, et les photos dont rien n'a été mesuré (une
/// illisible n'a pas de dHash, donc pas de place). Douze au plus.
pub fn classer(
    album: &Album,
    planche: usize,
    reserve: &[EnReserve],
    faits: &HashMap<String, Fait>,
    date_de: &dyn Fn(&str) -> Option<NaiveDateTime>,
) -> Vec<Candidate> {
    let Some(spread) = album.spreads.get(planche) else {
        return Vec::new();
    };
    let jour = jour_de_reference(album, planche, date_de);
    let posees: Vec<u64> = spread
        .slots
        .iter()
        .filter_map(|s| faits.get(&s.src).map(|f| f.dhash))
        .collect();

    let mut rangees: Vec<(Option<i64>, Candidate)> = reserve
        .iter()
        .filter_map(|r| {
            let fait = faits.get(&r.src)?;
            if posees.iter().any(|&d| analyze::hamming(d, fait.dhash) <= DUP_HAMMING) {
                return None;
            }
            let ecart = match (jour, fait.taken) {
                (Some(j), Some(t)) => Some((t.date() - j).num_days().abs()),
                _ => None,
            };
            let raison = match ecart {
                Some(0) => Raison::MemeJour,
                Some(_) => Raison::Proche,
                None => Raison::Nette,
            };
            Some((
                ecart,
                Candidate {
                    src: r.src.clone(),
                    score: fait.score,
                    taken: fait.taken,
                    raison,
                    focal: r.focal,
                },
            ))
        })
        .collect();
    // Stable : deux photos au même jour et au même score gardent l'ordre de
    // la réserve, qui est celui du dossier.
    rangees.sort_by(|(ea, a), (eb, b)| {
        let ka = (ea.is_none(), ea.unwrap_or(0));
        let kb = (eb.is_none(), eb.unwrap_or(0));
        ka.cmp(&kb).then_with(|| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    rangees.truncate(AU_PLUS);
    rangees.into_iter().map(|(_, c)| c).collect()
}

/// La réserve d'un dossier d'album pour une planche : ce que l'album ne
/// montre pas, mesuré, puis classé. Rend toujours une liste, vide au pire,
/// et dit dans `note` ce qui lui a manqué.
///
/// L'album voyage en argument et non par le fichier : l'éditeur tient une
/// version que le disque n'a pas encore (une photo retirée à l'instant est
/// déjà de la réserve), comme `gabarits_compatibles` reçoit ses `srcs`.
pub fn reserve_classee(dir: &Path, album: &Album, planche: usize) -> Result<Reserve> {
    anyhow::ensure!(
        planche < album.spreads.len(),
        "planche {planche} hors de l'album ({} planches)",
        album.spreads.len()
    );
    let montrees: HashSet<&str> = album
        .spreads
        .iter()
        .flat_map(|s| s.slots.iter().map(|sl| sl.src.as_str()))
        .collect();
    let focal_de: HashMap<String, [f64; 2]> = lire_curation(dir)?
        .into_iter()
        .map(|d| (d.src, d.focal))
        .collect();

    let releve = crate::releve::Releve::dans_album(dir)?;
    let univers: Vec<String> = match &releve {
        Some(r) => r.photos.iter().map(|p| r.src(&p.path)).collect(),
        None => match lire_thumbs(dir)? {
            Some(thumbs) => {
                let mut srcs: Vec<String> = thumbs.into_keys().collect();
                srcs.sort();
                srcs
            }
            None => {
                return Ok(Reserve {
                    candidats: Vec::new(),
                    note: Some(
                        "ni relevé ni vignettes dans le dossier de l'album : la réserve \
                         est vide, recomposez l'album"
                            .into(),
                    ),
                })
            }
        },
    };
    let reserve: Vec<EnReserve> = univers
        .into_iter()
        .filter(|src| !montrees.contains(src.as_str()))
        .map(|src| {
            let focal = focal_de.get(&src).copied().unwrap_or_else(default_focal);
            EnReserve { src, focal }
        })
        .collect();

    let mut cibles: Vec<String> = reserve.iter().map(|r| r.src.clone()).collect();
    cibles.extend(album.spreads[planche].slots.iter().map(|s| s.src.clone()));
    let Mesure { faits, notes, racine } = mesurer(dir, album, releve.as_ref(), &cibles);

    let date_de = |src: &str| -> Option<NaiveDateTime> {
        if let Some(f) = faits.get(src) {
            return f.taken;
        }
        match &racine {
            Racine::Releve(dates) => dates.get(src).copied(),
            Racine::Dossier(root) => {
                let m = crate::meta::read(&root.join(src));
                m.taken_reliable.then_some(m.taken)
            }
            Racine::Absente => None,
        }
    };
    let candidats = classer(album, planche, &reserve, &faits, &date_de);
    let note = (!notes.is_empty()).then(|| notes.join(" ; "));
    Ok(Reserve { candidats, note })
}

/// D'où les dates des photos posées se lisent quand elles n'ont pas été
/// mesurées avec la réserve : du relevé, des originaux, ou de nulle part.
enum Racine {
    Releve(HashMap<String, NaiveDateTime>),
    Dossier(std::path::PathBuf),
    Absente,
}

struct Mesure {
    faits: HashMap<String, Fait>,
    notes: Vec<String>,
    racine: Racine,
}

/// Les faits des `srcs` demandées : du relevé quand l'album en porte un
/// (ses fiches sont les mesures mêmes des vignettes), des vignettes sinon,
/// la date venant alors de l'original quand le dossier de photos est là.
/// Une vignette illisible ou absente ne fait pas échouer : elle manque, et
/// la note le dit.
fn mesurer(dir: &Path, album: &Album, releve: Option<&crate::releve::Releve>, srcs: &[String]) -> Mesure {
    if let Some(r) = releve {
        let fiches: HashMap<String, &crate::pipeline::Photo> =
            r.photos.iter().map(|p| (r.src(&p.path), p)).collect();
        let faits = srcs
            .iter()
            .filter_map(|src| {
                let p = fiches.get(src)?;
                Some((
                    src.clone(),
                    Fait {
                        dhash: p.analysis.dhash,
                        score: p.analysis.score(),
                        taken: p.meta.taken_reliable.then_some(p.meta.taken),
                    },
                ))
            })
            .collect();
        let dates = fiches
            .iter()
            .filter_map(|(src, p)| p.meta.taken_reliable.then(|| (src.clone(), p.meta.taken)))
            .collect();
        return Mesure { faits, notes: Vec::new(), racine: Racine::Releve(dates) };
    }

    let root = std::path::PathBuf::from(&album.root);
    let root_ok = root.is_dir();
    let mut notes = Vec::new();
    let thumbs = match lire_thumbs(dir) {
        Ok(Some(t)) => t,
        Ok(None) => HashMap::new(),
        Err(e) => {
            notes.push(format!("{e:#}"));
            HashMap::new()
        }
    };
    let mesures: Vec<Option<(String, Fait)>> = srcs
        .par_iter()
        .map(|src| {
            let name = thumbs.get(src)?;
            let chemin = crate::thumb::chemin(dir, name)?;
            let (dhash, score) = analyse_de_vignette(&chemin)?;
            let taken = root_ok.then(|| crate::meta::read(&root.join(src))).and_then(|m| {
                m.taken_reliable.then_some(m.taken)
            });
            Some((src.clone(), Fait { dhash, score, taken }))
        })
        .collect();
    let manquantes = mesures.iter().filter(|m| m.is_none()).count();
    let faits: HashMap<String, Fait> = mesures.into_iter().flatten().collect();
    if !root_ok {
        notes.push(format!(
            "dossier de photos introuvable ({}) : sans date, la réserve se classe \
             sur la netteté seule",
            root.display()
        ));
    }
    if manquantes > 0 {
        notes.push(format!(
            "{manquantes} photo(s) sans vignette lisible, hors réserve : recomposez \
             l'album pour les retrouver"
        ));
    }
    let racine = if root_ok { Racine::Dossier(root) } else { Racine::Absente };
    Mesure { faits, notes, racine }
}

/// Le dHash et le score d'une vignette, mesurés une fois par processus.
///
/// Mesuré le 02/10 sur corse-2013 (572 vignettes, 131 Mo) : rouvrir toute la
/// réserve coûte 3,7 s sur dix cœurs, et le popover qui la montre s'ouvre
/// au clic droit. La vignette d'une photo ne change qu'à la recomposition,
/// qui réécrit le fichier : la clé est son chemin, sa date et sa taille, et
/// une vignette réécrite se remesure d'elle-même. Rien n'est écrit sur le
/// disque : un cache de mesures à côté de l'album serait une seconde source
/// de vérité pour ce que le relevé sait déjà écrire.
fn analyse_de_vignette(chemin: &Path) -> Option<(u64, f64)> {
    static MESURES: std::sync::OnceLock<
        std::sync::RwLock<HashMap<std::path::PathBuf, (std::time::SystemTime, u64, u64, f64)>>,
    > = std::sync::OnceLock::new();
    let meta = std::fs::metadata(chemin).ok()?;
    let (modifie, taille) = (meta.modified().ok()?, meta.len());
    let cache = MESURES.get_or_init(Default::default);
    if let Some((m, t, dhash, score)) = cache.read().ok().and_then(|c| c.get(chemin).copied()) {
        if m == modifie && t == taille {
            return Some((dhash, score));
        }
    }
    let img = image::open(chemin).ok()?;
    let analysis = analyze::analyze(&img);
    let (dhash, score) = (analysis.dhash, analysis.score());
    if let Ok(mut c) = cache.write() {
        c.insert(chemin.to_path_buf(), (modifie, taille, dhash, score));
    }
    Some((dhash, score))
}

fn lire_curation(dir: &Path) -> Result<Vec<Discard>> {
    let path = dir.join("curation.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let texte = std::fs::read_to_string(&path)
        .with_context(|| format!("lecture de {}", path.display()))?;
    serde_json::from_str(&texte).context("curation.json illisible")
}

/// `thumbs.json`, ou `None` quand le dossier n'en a pas (album composé sans
/// ses photos). Un fichier présent et illisible est une erreur, pas un vide.
fn lire_thumbs(dir: &Path) -> Result<Option<HashMap<String, String>>> {
    let path = dir.join("thumbs.json");
    if !path.is_file() {
        return Ok(None);
    }
    let texte = std::fs::read_to_string(&path)
        .with_context(|| format!("lecture de {}", path.display()))?;
    serde_json::from_str(&texte).map(Some).context("thumbs.json illisible")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Size, Slot, Spread};

    fn date(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn planche(srcs: &[&str]) -> Spread {
        Spread {
            template: "duo".into(),
            slots: srcs.iter().map(|s| Slot::new(s.to_string(), [0.5, 0.42])).collect(),
            caption: None,
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        }
    }

    fn album(planches: Vec<Spread>) -> Album {
        let mut a = Album::new("test", Path::new("/nulle-part"), Size { w: 210.0, h: 210.0 });
        a.spreads = planches;
        a
    }

    /// Des dHash tous à plus de 24 bits les uns des autres, sauf ceux qu'on
    /// rapproche exprès : chaque photo prend un motif de 8 bits répété.
    fn dhash(n: u64) -> u64 {
        let motif = [0x00, 0xff, 0x0f, 0xf0, 0x33, 0xcc, 0x55, 0xaa, 0x3c, 0xc3][n as usize % 10];
        (0..8).fold(0u64, |acc, i| acc | (motif << (8 * i)))
    }

    fn fait(n: u64, score: f64, taken: Option<&str>) -> Fait {
        Fait { dhash: dhash(n), score, taken: taken.map(date) }
    }

    fn reserve(srcs: &[&str]) -> Vec<EnReserve> {
        srcs.iter().map(|s| EnReserve { src: s.to_string(), focal: [0.5, 0.42] }).collect()
    }

    fn date_de(faits: &HashMap<String, Fait>) -> impl Fn(&str) -> Option<NaiveDateTime> + '_ {
        move |src| faits.get(src).and_then(|f| f.taken)
    }

    fn srcs(c: &[Candidate]) -> Vec<&str> {
        c.iter().map(|c| c.src.as_str()).collect()
    }

    #[test]
    fn le_meme_jour_gagne_sur_un_meilleur_score_d_un_autre_jour() {
        let a = album(vec![planche(&["p.jpg"])]);
        let faits: HashMap<String, Fait> = [
            ("p.jpg".to_string(), fait(0, 5.0, Some("2013-10-27 10:00:00"))),
            ("loin.jpg".to_string(), fait(1, 9.0, Some("2013-10-29 10:00:00"))),
            ("jour.jpg".to_string(), fait(2, 2.0, Some("2013-10-27 18:00:00"))),
            ("veille.jpg".to_string(), fait(3, 8.0, Some("2013-10-26 18:00:00"))),
        ]
        .into_iter()
        .collect();
        let c = classer(&a, 0, &reserve(&["loin.jpg", "jour.jpg", "veille.jpg"]), &faits, &date_de(&faits));
        assert_eq!(srcs(&c), ["jour.jpg", "veille.jpg", "loin.jpg"]);
        assert_eq!(c[0].raison, Raison::MemeJour);
        assert_eq!(c[1].raison, Raison::Proche);
        assert_eq!(c[2].raison, Raison::Proche);
    }

    #[test]
    fn un_quasi_doublon_d_une_photo_de_la_planche_est_ecarte() {
        let a = album(vec![planche(&["p.jpg"])]);
        let mut faits: HashMap<String, Fait> = [
            ("p.jpg".to_string(), fait(0, 5.0, Some("2013-10-27 10:00:00"))),
            ("autre.jpg".to_string(), fait(1, 5.0, Some("2013-10-27 10:00:00"))),
        ]
        .into_iter()
        .collect();
        // À 23 bits de la photo posée : doublon au seuil de la curation.
        let proche = Fait { dhash: dhash(0) ^ ((1u64 << 23) - 1), ..fait(0, 9.0, Some("2013-10-27 11:00:00")) };
        assert_eq!(analyze::hamming(proche.dhash, dhash(0)), 23);
        faits.insert("doublon.jpg".into(), proche);
        let c = classer(&a, 0, &reserve(&["doublon.jpg", "autre.jpg"]), &faits, &date_de(&faits));
        assert_eq!(srcs(&c), ["autre.jpg"]);
    }

    #[test]
    fn a_egalite_de_jour_le_score_departage() {
        let a = album(vec![planche(&["p.jpg"])]);
        let faits: HashMap<String, Fait> = [
            ("p.jpg".to_string(), fait(0, 5.0, Some("2013-10-27 10:00:00"))),
            ("faible.jpg".to_string(), fait(1, 1.0, Some("2013-10-27 12:00:00"))),
            ("forte.jpg".to_string(), fait(2, 7.0, Some("2013-10-27 13:00:00"))),
            ("moyenne.jpg".to_string(), fait(3, 4.0, Some("2013-10-27 14:00:00"))),
        ]
        .into_iter()
        .collect();
        let c = classer(&a, 0, &reserve(&["faible.jpg", "forte.jpg", "moyenne.jpg"]), &faits, &date_de(&faits));
        assert_eq!(srcs(&c), ["forte.jpg", "moyenne.jpg", "faible.jpg"]);
        assert!(c.iter().all(|c| c.raison == Raison::MemeJour));
    }

    #[test]
    fn une_planche_sans_photo_prend_le_jour_de_ses_voisines() {
        // Une page de texte entre deux planches : la plus proche datée gagne,
        // et la voisine de gauche à distance 1 l'emporte sur celle à distance 2.
        let a = album(vec![planche(&["a.jpg"]), planche(&[]), planche(&["b.jpg"]), planche(&["c.jpg"])]);
        let faits: HashMap<String, Fait> = [
            ("a.jpg".to_string(), fait(0, 5.0, Some("2013-10-20 10:00:00"))),
            ("b.jpg".to_string(), fait(1, 5.0, None)),
            ("c.jpg".to_string(), fait(2, 5.0, Some("2013-10-29 10:00:00"))),
            ("r20.jpg".to_string(), fait(3, 1.0, Some("2013-10-20 15:00:00"))),
            ("r29.jpg".to_string(), fait(4, 9.0, Some("2013-10-29 15:00:00"))),
        ]
        .into_iter()
        .collect();
        assert_eq!(jour_de_reference(&a, 1, &date_de(&faits)), Some(date("2013-10-20 00:00:00").date()));
        let c = classer(&a, 1, &reserve(&["r29.jpg", "r20.jpg"]), &faits, &date_de(&faits));
        assert_eq!(srcs(&c), ["r20.jpg", "r29.jpg"]);
        assert_eq!(c[0].raison, Raison::MemeJour);
    }

    #[test]
    fn la_mediane_des_dates_d_une_planche_fait_son_jour() {
        let a = album(vec![planche(&["a.jpg", "b.jpg", "c.jpg"])]);
        let faits: HashMap<String, Fait> = [
            ("a.jpg".to_string(), fait(0, 1.0, Some("2013-10-01 10:00:00"))),
            ("b.jpg".to_string(), fait(1, 1.0, Some("2013-10-27 10:00:00"))),
            ("c.jpg".to_string(), fait(2, 1.0, Some("2013-10-28 10:00:00"))),
        ]
        .into_iter()
        .collect();
        assert_eq!(jour_de_reference(&a, 0, &date_de(&faits)), Some(date("2013-10-27 00:00:00").date()));
    }

    #[test]
    fn sans_aucune_date_tout_est_nette_par_score() {
        let a = album(vec![planche(&["p.jpg"])]);
        let faits: HashMap<String, Fait> = [
            ("p.jpg".to_string(), fait(0, 5.0, None)),
            ("x.jpg".to_string(), fait(1, 2.0, None)),
            ("y.jpg".to_string(), fait(2, 8.0, Some("2013-10-27 10:00:00"))),
        ]
        .into_iter()
        .collect();
        let c = classer(&a, 0, &reserve(&["x.jpg", "y.jpg"]), &faits, &date_de(&faits));
        assert_eq!(srcs(&c), ["y.jpg", "x.jpg"]);
        assert!(c.iter().all(|c| c.raison == Raison::Nette));
    }

    #[test]
    fn les_photos_sans_date_ferment_la_marche() {
        let a = album(vec![planche(&["p.jpg"])]);
        let faits: HashMap<String, Fait> = [
            ("p.jpg".to_string(), fait(0, 5.0, Some("2013-10-27 10:00:00"))),
            ("sans.jpg".to_string(), fait(1, 9.0, None)),
            ("loin.jpg".to_string(), fait(2, 1.0, Some("2013-12-01 10:00:00"))),
        ]
        .into_iter()
        .collect();
        let c = classer(&a, 0, &reserve(&["sans.jpg", "loin.jpg"]), &faits, &date_de(&faits));
        assert_eq!(srcs(&c), ["loin.jpg", "sans.jpg"]);
        assert_eq!(c[1].raison, Raison::Nette);
    }

    #[test]
    fn douze_au_plus_et_une_photo_sans_fait_n_entre_pas() {
        let a = album(vec![planche(&["p.jpg"])]);
        let mut faits: HashMap<String, Fait> =
            [("p.jpg".to_string(), fait(0, 5.0, Some("2013-10-27 10:00:00")))].into_iter().collect();
        let mut noms: Vec<String> = Vec::new();
        for i in 1..=20u64 {
            let nom = format!("r{i:02}.jpg");
            // Des dHash éloignés les uns des autres et de la photo posée.
            faits.insert(nom.clone(), Fait { dhash: dhash(i % 9 + 1) ^ (i << 40), score: i as f64, taken: Some(date("2013-10-27 12:00:00")) });
            noms.push(nom);
        }
        noms.push("illisible.jpg".into());
        let en_reserve: Vec<EnReserve> = noms.iter().map(|s| EnReserve { src: s.clone(), focal: [0.5, 0.42] }).collect();
        let c = classer(&a, 0, &en_reserve, &faits, &date_de(&faits));
        assert_eq!(c.len(), AU_PLUS);
        assert_eq!(c[0].src, "r20.jpg");
        assert!(c.iter().all(|c| c.src != "illisible.jpg"));
    }

    #[test]
    fn le_focal_de_la_curation_voyage_avec_la_candidate() {
        let a = album(vec![planche(&["p.jpg"])]);
        let faits: HashMap<String, Fait> = [
            ("p.jpg".to_string(), fait(0, 5.0, None)),
            ("v.jpg".to_string(), fait(1, 5.0, None)),
        ]
        .into_iter()
        .collect();
        let r = vec![EnReserve { src: "v.jpg".into(), focal: [0.3, 0.2] }];
        let c = classer(&a, 0, &r, &faits, &date_de(&faits));
        assert_eq!(c[0].focal, [0.3, 0.2]);
    }

    /// Un dossier d'album réduit à son `album.json` : ni relevé, ni vignettes,
    /// ni dossier de photos. La commande rend une liste vide et le dit, jamais
    /// une panique ni une erreur — c'est un état atteignable à la main.
    #[test]
    fn un_dossier_sans_releve_ni_vignettes_rend_une_liste_vide_et_le_dit() {
        let dir = std::env::temp_dir().join(format!("colophon-reserve-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = album(vec![planche(&["p.jpg"])]);
        let r = reserve_classee(&dir, &a, 0).unwrap();
        assert!(r.candidats.is_empty());
        assert!(r.note.as_deref().unwrap_or("").contains("ni relevé ni vignettes"));
        assert!(reserve_classee(&dir, &a, 7).is_err(), "une planche hors de l'album se refuse");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Des vignettes sans le dossier de photos : la réserve se classe sur la
    /// netteté et la note dit l'absence de date.
    #[test]
    fn des_vignettes_sans_originaux_classent_sur_la_nettete_et_le_disent() {
        let dir = std::env::temp_dir().join(format!("colophon-reserve-vignettes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = dir.join(".cache").join("thumbs");
        std::fs::create_dir_all(&cache).unwrap();
        // Trois vignettes synthétiques aux gradients opposés, donc à des
        // dHash éloignés (un dHash lit le gradient horizontal : un dégradé
        // horizontal et un vertical sont à 64 bits ; un motif « pseudo-
        // aléatoire » au pixel, lui, se moyenne en gris au rééchantillonnage
        // et tombe à zéro comme le vertical), et une entrée dont le fichier
        // manque.
        let mut thumbs = HashMap::new();
        let dessins: [(&str, Box<dyn Fn(u32, u32) -> u8>); 3] = [
            ("a.jpg", Box::new(|x, _| (x * 4) as u8)),
            ("b.jpg", Box::new(|_, y| (y * 4) as u8)),
            // Un dégradé horizontal en haut, son inverse en bas : 32 bits du
            // dHash de a (tout à un) comme de celui de b (tout à zéro).
            ("c.jpg", Box::new(|x, y| if y < 32 { (x * 4) as u8 } else { 255 - (x * 4) as u8 })),
        ];
        for (nom, f) in dessins {
            let img = image::ImageBuffer::from_fn(64, 64, |x, y| {
                let v = f(x, y);
                image::Rgb([v, 255 - v, v / 2])
            });
            let fichier = format!("v-{nom}");
            img.save(cache.join(&fichier)).unwrap();
            thumbs.insert(nom.to_string(), fichier);
        }
        thumbs.insert("perdue.jpg".to_string(), "v-perdue.jpg".to_string());
        std::fs::write(dir.join("thumbs.json"), serde_json::to_string(&thumbs).unwrap()).unwrap();
        let a = album(vec![planche(&["a.jpg"])]);
        let r = reserve_classee(&dir, &a, 0).unwrap();
        let note = r.note.clone().unwrap_or_default();
        assert!(note.contains("sans date"), "{note}");
        assert!(note.contains("1 photo(s) sans vignette lisible"), "{note}");
        assert!(r.candidats.iter().all(|c| c.raison == Raison::Nette));
        let noms = srcs(&r.candidats);
        assert_eq!(noms.len(), 2, "{noms:?}");
        assert!(noms.contains(&"b.jpg") && noms.contains(&"c.jpg"), "{noms:?}");
        assert!(r.candidats[0].score >= r.candidats[1].score);

        // La vignette de la photo posée est réécrite en copie de b : le cache
        // par processus la remesure sur sa date et sa taille, et b devient un
        // doublon de la planche. Un cache qui rendrait l'ancienne mesure
        // laisserait b dans la liste.
        std::thread::sleep(std::time::Duration::from_millis(20));
        let b = image::ImageBuffer::from_fn(64, 64, |_, y| {
            let v = (y * 4) as u8;
            image::Rgb([v, 255 - v, v / 2])
        });
        b.save(cache.join("v-a.jpg")).unwrap();
        let r = reserve_classee(&dir, &a, 0).unwrap();
        assert_eq!(srcs(&r.candidats), ["c.jpg"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Le banc de l'œil : la réserve d'un vrai album, pour chaque planche
    /// demandée, et ce que coûte le second appel une fois les vignettes
    /// mesurées. `COLOPHON_ALBUM=.albums/corse-2013 cargo test -p
    /// colophon-core --release banc_reserve_du_mac -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn banc_reserve_du_mac() {
        let dir = std::path::PathBuf::from(
            std::env::var("COLOPHON_ALBUM").expect("COLOPHON_ALBUM=<dossier d'album>"),
        );
        let album: Album =
            serde_json::from_str(&std::fs::read_to_string(dir.join("album.json")).unwrap())
                .unwrap();
        for planche in [0, album.spreads.len() / 2, album.spreads.len() - 1] {
            let t0 = std::time::Instant::now();
            let r = reserve_classee(&dir, &album, planche).unwrap();
            let premier = t0.elapsed();
            let t1 = std::time::Instant::now();
            let _ = reserve_classee(&dir, &album, planche).unwrap();
            let second = t1.elapsed();
            eprintln!(
                "planche {} : {} candidates en {:.2} s, puis {:.3} s ; note = {:?}",
                planche + 1,
                r.candidats.len(),
                premier.as_secs_f64(),
                second.as_secs_f64(),
                r.note
            );
            for c in r.candidats.iter().take(4) {
                eprintln!("    {:?} {:.2} {:?} {}", c.raison, c.score, c.taken, c.src);
            }
        }
    }
}
