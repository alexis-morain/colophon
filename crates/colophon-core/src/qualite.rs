//! L'alerte de qualité (chantier G) : ce que l'app doit savoir d'une photo
//! pour dire qu'elle imprimera mal, lu une fois par album.
//!
//! Quatre nombres par photo, rien d'autre : la taille de l'original (le ppi
//! se calcule côté app, dans la case, par `effectivePpi`), la netteté et
//! l'exposition de l'analyse. Ils viennent du relevé quand l'album en porte
//! un, et des vignettes sinon — les deux chemins de `audit::mesure_photos`,
//! jamais un troisième chiffre : une fiche a été prise sur la vignette même
//! que l'autre chemin rouvre.
//!
//! **Le seuil de flou n'est pas une constante.** La curation n'écarte
//! aucune photo pour son flou dans l'absolu : la netteté ne sert qu'à
//! départager une rafale ou une scène (`pipeline::dedup`), et l'analyse
//! prévient qu'elle ne se compare qu'entre photos de tailles voisines. Une
//! photo est donc dite floue quand elle tombe dans le dernier décile de
//! netteté de son dossier. Sous `pipeline::PETIT_DOSSIER`, où la curation
//! coupe ses propres règles statistiques, le décile se coupe aussi : sur
//! cinq photos, il désignerait toujours la moins nette, floue ou non.

use crate::analyze;
use crate::releve::Releve;
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Ce qu'une photo dit d'elle-même à l'alerte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mesure {
    /// Taille de l'original, orientation appliquée ; le capteur pour un RAW.
    pub largeur: u32,
    pub hauteur: u32,
    /// `Analysis::sharpness`, mesurée sur la vignette.
    pub nettete: f64,
    /// `Analysis::exposure`, 0..1, celle de l'original.
    pub exposition: f64,
}

/// Le relevé tel que l'app le lit : par `src`, et le seuil qui en découle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReleveAlbum {
    pub photos: BTreeMap<String, Mesure>,
    /// Netteté à partir de laquelle, et au-dessous, une photo est floue :
    /// le haut du dernier décile du dossier. `None` sous `PETIT_DOSSIER`.
    pub seuil_flou: Option<f64>,
}

/// La part du dossier dite floue : le dernier décile.
pub const PART_FLOUE: f64 = 0.1;

/// La netteté la plus haute du dernier décile, ou `None` quand le dossier
/// est trop petit pour qu'un décile veuille dire quelque chose. Une photo
/// est floue quand sa netteté est au plus ce seuil.
pub fn seuil_de_flou(nettetes: &[f64]) -> Option<f64> {
    if nettetes.len() < crate::pipeline::PETIT_DOSSIER {
        return None;
    }
    let mut v = nettetes.to_vec();
    v.sort_by(f64::total_cmp);
    let n = (v.len() as f64 * PART_FLOUE).ceil() as usize;
    Some(v[n.max(1) - 1])
}

/// Le relevé de l'album rangé dans `dir`, dont les photos vivent sous
/// `root`. Une photo dont la vignette ou l'en-tête ne se lit pas manque,
/// et une photo qui manque ne porte aucune alerte : l'alerte ne s'invente
/// rien.
pub fn releve_album(dir: &Path, root: &Path) -> Result<ReleveAlbum> {
    let photos: BTreeMap<String, Mesure> = match Releve::dans_album(dir)? {
        Some(releve) => releve
            .photos
            .iter()
            .map(|p| {
                (
                    releve.src(&p.path),
                    Mesure {
                        largeur: p.orig.0,
                        hauteur: p.orig.1,
                        nettete: p.analysis.sharpness,
                        exposition: p.analysis.exposure,
                    },
                )
            })
            .collect(),
        None => depuis_les_vignettes(dir, root)?,
    };
    let nettetes: Vec<f64> = photos.values().map(|m| m.nettete).collect();
    Ok(ReleveAlbum { seuil_flou: seuil_de_flou(&nettetes), photos })
}

/// Chaque photo de `thumbs.json` — tout le dossier, réserve comprise :
/// sa vignette pour la netteté, l'en-tête de son original pour la taille,
/// par la seule règle de taille du projet (`heic::oriented_dimensions`).
fn depuis_les_vignettes(dir: &Path, root: &Path) -> Result<BTreeMap<String, Mesure>> {
    let index = dir.join("thumbs.json");
    if !index.is_file() {
        return Ok(BTreeMap::new());
    }
    let thumbs: HashMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(&index)?).context("thumbs.json illisible")?;
    Ok(thumbs
        .par_iter()
        .filter_map(|(src, nom)| {
            let vignette = crate::thumb::chemin(dir, nom)?;
            let img = image::open(vignette).ok()?;
            let (nettete, exposition) = analyze::nettete_et_exposition(&img);
            let original = root.join(src);
            let m = crate::meta::read(&original);
            let (largeur, hauteur) =
                crate::heic::oriented_dimensions(&original, m.orientation).ok()?;
            Some((src.clone(), Mesure { largeur, hauteur, nettete, exposition }))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn dossier(nom: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("colophon-qualite-{nom}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Une fiche réduite à ce que l'alerte lit.
    fn photo(racine: &Path, nom: &str, nettete: f64) -> crate::pipeline::Photo {
        crate::pipeline::Photo {
            path: racine.join(nom),
            meta: crate::meta::PhotoMeta {
                taken: chrono::NaiveDateTime::default(),
                taken_reliable: false,
                orientation: 1,
                gps: None,
                model: None,
                rating: None,
                make: None,
                lens: None,
                f_number: None,
                exposure_time: None,
                iso: None,
                focal_mm: None,
            },
            analysis: crate::analyze::Analysis {
                dhash: 0,
                phash: 0,
                colorsig: [0; 12],
                sharpness: nettete,
                exposure: 0.25,
                width: 400,
                height: 300,
            },
            orig: (4000, 3000),
            faces: vec![],
            focal: None,
        }
    }

    #[test]
    fn le_seuil_est_le_haut_du_dernier_decile() {
        // Trente photos, nettetés 1 à 30 dans le désordre : le dernier
        // décile en compte trois, 1, 2 et 3.
        let v: Vec<f64> = (1..=30).rev().map(f64::from).collect();
        assert_eq!(seuil_de_flou(&v), Some(3.0));
        // Trente et une : le décile en prend quatre, arrondi au-dessus,
        // jamais zéro photo dans un dossier qui en a assez.
        let v: Vec<f64> = (1..=31).map(f64::from).collect();
        assert_eq!(seuil_de_flou(&v), Some(4.0));
    }

    #[test]
    fn sous_le_petit_dossier_aucune_photo_n_est_floue() {
        let n = crate::pipeline::PETIT_DOSSIER;
        let v: Vec<f64> = (1..n).map(|i| i as f64).collect();
        assert_eq!(seuil_de_flou(&v), None);
        let v: Vec<f64> = (1..=n).map(|i| i as f64).collect();
        assert!(seuil_de_flou(&v).is_some());
    }

    /// Un album composé depuis ses fiches : le relevé fait foi, ses tailles
    /// d'origine et ses mesures passent telles quelles, sous la clé
    /// `Releve::src` (une fiche relue porte `jeu/photo.jpg`, un slot
    /// `photo.jpg`).
    #[test]
    fn le_releve_de_l_album_fait_foi() {
        let dir = dossier("releve");
        let racine = dir.join("vacances");
        let photos = (0..30)
            .map(|i| {
                photo(&racine, &format!("p{i:02}.jpg"), f64::from(i) + 0.5)
            })
            .collect();
        let releve = Releve {
            version: crate::releve::VERSION,
            racine,
            skipped_heic: 0,
            skipped_raw: 0,
            skipped_other: 0,
            illisibles: vec![],
            editees: vec![],
            photos,
            vignettes: false,
        };
        releve.ecrire(&dir.join(crate::releve::FICHIER)).unwrap();
        let r = releve_album(&dir, Path::new("/nulle/part")).unwrap();
        assert_eq!(r.photos.len(), 30);
        assert_eq!(
            r.photos["p00.jpg"],
            Mesure { largeur: 4000, hauteur: 3000, nettete: 0.5, exposition: 0.25 }
        );
        assert_eq!(r.seuil_flou, Some(2.5));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Sans relevé : les vignettes de `thumbs.json` pour la netteté, l'en-tête
    /// de l'original pour la taille. Une vignette absente ou un original
    /// introuvable font manquer la photo, sans faire échouer le reste.
    #[test]
    fn sans_releve_les_vignettes_et_les_en_tetes() {
        let dir = dossier("vignettes");
        let root = dir.join("photos");
        let cache = dir.join(".cache").join("thumbs");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(&root).unwrap();
        let rayures = image::ImageBuffer::from_fn(64, 48, |x, _| {
            image::Rgb(if x % 2 == 0 { [0u8, 0, 0] } else { [255, 255, 255] })
        });
        let uni = image::ImageBuffer::from_fn(64, 48, |_, _| image::Rgb([128u8, 128, 128]));
        rayures.save(cache.join("v-nette.png")).unwrap();
        uni.save(cache.join("v-floue.png")).unwrap();
        // Les originaux : un PNG de 640 × 480, un de 320 × 240.
        image::ImageBuffer::from_fn(640, 480, |_, _| image::Rgb([1u8, 2, 3]))
            .save(root.join("nette.png"))
            .unwrap();
        image::ImageBuffer::from_fn(320, 240, |_, _| image::Rgb([1u8, 2, 3]))
            .save(root.join("floue.png"))
            .unwrap();
        let thumbs: HashMap<&str, &str> = [
            ("nette.png", "v-nette.png"),
            ("floue.png", "v-floue.png"),
            ("sans-vignette.png", "v-perdue.png"),
            ("sans-original.png", "v-nette.png"),
        ]
        .into_iter()
        .collect();
        fs::write(dir.join("thumbs.json"), serde_json::to_string(&thumbs).unwrap()).unwrap();

        let r = releve_album(&dir, &root).unwrap();
        assert_eq!(r.photos.len(), 2, "{:?}", r.photos.keys());
        let (nette, floue) = (&r.photos["nette.png"], &r.photos["floue.png"]);
        assert_eq!((nette.largeur, nette.hauteur), (640, 480));
        assert_eq!((floue.largeur, floue.hauteur), (320, 240));
        // Les mesures sont celles de l'analyse elle-même, au bit près.
        let a = analyze::analyze(&image::open(cache.join("v-nette.png")).unwrap());
        assert_eq!(nette.nettete.to_bits(), a.sharpness.to_bits());
        assert_eq!(nette.exposition.to_bits(), a.exposure.to_bits());
        assert!(nette.nettete > floue.nettete);
        assert_eq!(r.seuil_flou, None, "deux photos, aucun décile");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_dossier_sans_releve_ni_vignettes_rend_un_releve_vide() {
        let dir = dossier("vide");
        assert_eq!(releve_album(&dir, &dir).unwrap(), ReleveAlbum::default());
        let _ = fs::remove_dir_all(&dir);
    }
}
