//! The sheet of one photograph, as the window shows it: what the file says
//! of itself (name, pixels, weight, format), what the camera wrote (date,
//! body, lens, exposure, place), what the user said (rating), and the
//! sharpness and exposure the quality alert reads (`qualite::mesure_photo`:
//! the relevé when the album carries one, the thumbnail otherwise). Read on demand for one photo, never during composition.
//!
//! Two doors, and the front end never holds a path: a `src` is the bare
//! file name `album.json` names a photo by, validated the way `thumb::chemin`
//! validates a cache name, and the path is formed here from `album.root`.
//! Revealing the original in the file manager goes one step further —
//! canonical path, then the album root as prefix — because a symbolic link
//! inside the folder can point anywhere on the disk.

use crate::heic;
use crate::meta;
use crate::places;
use anyhow::{bail, Context, Result};
use chrono::NaiveDateTime;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct FichePhoto {
    /// The file name, never its path.
    pub nom: String,
    /// Pixel size, EXIF orientation applied: what the photograph shows.
    pub largeur: u32,
    pub hauteur: u32,
    pub octets: u64,
    /// `JPEG`, `PNG`, `HEIC`, or `RAW (CR3)`: a label, not a MIME type.
    pub format: String,
    pub taken: NaiveDateTime,
    pub taken_reliable: bool,
    /// Maker and model on one line, the maker not repeated when the model
    /// already starts with it (« Canon Canon EOS 5D » is what the tags say).
    pub appareil: Option<String>,
    pub objectif: Option<String>,
    pub ouverture: Option<f64>,
    pub temps_de_pose: Option<String>,
    pub iso: Option<u32>,
    pub focale_mm: Option<f64>,
    /// Decimal degrees, signed.
    pub gps: Option<(f64, f64)>,
    /// The nearest town of the gazetteer within its reach, offline.
    pub lieu: Option<Lieu>,
    /// 1 to 5 stars, -1 rejected, `None` never rated.
    pub note: Option<i8>,
    /// The quality alert's measure: the relevé when the album carries one,
    /// the thumbnail otherwise; absent when neither reads.
    pub nettete: Option<f64>,
    pub exposition: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Lieu {
    pub nom: String,
    /// ISO country code.
    pub pays: String,
}

/// The original a `src` names, under `root`. Refused unless `src` is a bare
/// file name: `album.json` is hand-editable, so a value that joins as a
/// path would be a path traversal.
pub fn original(root: &Path, src: &str) -> Result<PathBuf> {
    if !crate::thumb::nom_propre(src) {
        bail!("nom de photo refusé : {src:?}");
    }
    Ok(root.join(src))
}

/// The canonical path to reveal for a `src`, refused when it does not sit
/// under the canonical album root: a link inside the folder is still a
/// file inside the folder to `original`, and not to the file manager.
pub fn a_reveler(root: &Path, src: &str) -> Result<PathBuf> {
    let fichier = original(root, src)?;
    let racine = root
        .canonicalize()
        .with_context(|| format!("dossier des photos introuvable : {}", root.display()))?;
    let cible = fichier
        .canonicalize()
        .with_context(|| format!("photo introuvable : {src}"))?;
    if !cible.starts_with(&racine) {
        bail!("{src} pointe hors du dossier des photos");
    }
    Ok(cible)
}

/// The sheet of one photo of the album in `dir`, whose photographs live in
/// `root`. Reads the file; the relevé only adds what nothing else measures.
pub fn fiche(dir: &Path, root: &Path, src: &str) -> Result<FichePhoto> {
    let fichier = original(root, src)?;
    let octets = std::fs::metadata(&fichier)
        .with_context(|| format!("photo introuvable : {src}"))?
        .len();
    let meta = meta::read(&fichier);
    // The one size rule of the project: the system for HEIC and RAW, the
    // header otherwise, then the EXIF swap. A RAW measures its sensor.
    let (largeur, hauteur) = heic::oriented_dimensions(&fichier, meta.orientation)
        .with_context(|| format!("dimensions illisibles : {src}"))?;
    // The quality alert's own measure: the relevé when the album carries
    // one, the photo's thumbnail otherwise. One source for both screens.
    let mesure = crate::qualite::mesure_photo(dir, root, src)?;
    let (nettete, exposition) = (
        mesure.as_ref().map(|m| m.nettete),
        mesure.as_ref().map(|m| m.exposition),
    );
    Ok(FichePhoto {
        nom: src.to_string(),
        largeur,
        hauteur,
        octets,
        format: format_de(&fichier),
        taken: meta.taken,
        taken_reliable: meta.taken_reliable,
        appareil: appareil(meta.make.as_deref(), meta.model.as_deref()),
        objectif: meta.lens,
        ouverture: meta.f_number,
        temps_de_pose: meta.exposure_time,
        iso: meta.iso,
        focale_mm: meta.focal_mm,
        gps: meta.gps,
        lieu: lieu_de(meta.gps),
        note: meta.rating,
        nettete,
        exposition,
    })
}

/// A label from the extension: the container the file declares, which is
/// also the only thing a reader can say of it without decoding it.
fn format_de(fichier: &Path) -> String {
    let ext = fichier
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if heic::is_raw(fichier) {
        return format!("RAW ({})", ext.to_uppercase());
    }
    match ext.as_str() {
        "jpg" | "jpeg" => "JPEG".to_string(),
        "png" => "PNG".to_string(),
        "heic" | "heif" => "HEIC".to_string(),
        "" => "?".to_string(),
        autre => autre.to_uppercase(),
    }
}

/// Maker and model on one line, each once.
pub fn appareil(make: Option<&str>, model: Option<&str>) -> Option<String> {
    match (make, model) {
        (Some(m), Some(mo)) => {
            let marque = m.split_whitespace().next().unwrap_or(m);
            if mo.to_lowercase().starts_with(&marque.to_lowercase()) {
                Some(mo.to_string())
            } else {
                Some(format!("{m} {mo}"))
            }
        }
        (Some(m), None) => Some(m.to_string()),
        (None, Some(mo)) => Some(mo.to_string()),
        (None, None) => None,
    }
}

/// The town a coordinate falls in, by the gazetteer's own rule.
pub fn lieu_de(gps: Option<(f64, f64)>) -> Option<Lieu> {
    let (lat, lon) = gps?;
    places::nearest(lat, lon).map(|(ville, _)| Lieu {
        nom: ville.name.to_string(),
        pays: ville.country.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::fixture::{jpeg_decodable_avec_exif, Ifd, Val};
    use crate::releve::Releve;
    use std::fs;

    fn dossier(nom: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("colophon-fiche-{nom}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The same rule as `thumb::chemin`: a name is a name, or it is nothing.
    #[test]
    fn un_src_qui_n_est_pas_un_nom_est_refuse() {
        let root = Path::new("/tmp/photos");
        for mauvais in ["", ".", "..", "a/b.jpg", "a\\b.jpg", "../x.jpg", "/etc/passwd", "a\0b"] {
            assert!(original(root, mauvais).is_err(), "{mauvais:?} a passé");
        }
        assert_eq!(original(root, "IMG_2193.jpg").unwrap(), root.join("IMG_2193.jpg"));
    }

    /// A symbolic link inside the folder that points outside it passes
    /// `original` (it is a name) and is refused at the reveal: the file
    /// manager would open a folder the album never named.
    #[cfg(unix)]
    #[test]
    fn un_lien_qui_sort_du_dossier_est_refuse_au_reveal() {
        let base = dossier("lien");
        let root = base.join("photos");
        let dehors = base.join("dehors");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&dehors).unwrap();
        fs::write(dehors.join("secret.jpg"), b"x").unwrap();
        fs::write(root.join("vraie.jpg"), b"x").unwrap();
        std::os::unix::fs::symlink(dehors.join("secret.jpg"), root.join("lien.jpg")).unwrap();

        assert!(original(&root, "lien.jpg").is_ok(), "un lien est un nom");
        assert!(a_reveler(&root, "lien.jpg").is_err(), "le lien sort du dossier");
        let vraie = a_reveler(&root, "vraie.jpg").unwrap();
        assert!(vraie.starts_with(root.canonicalize().unwrap()));
        assert!(a_reveler(&root, "absente.jpg").is_err(), "un fichier absent ne se révèle pas");
        let _ = fs::remove_dir_all(&base);
    }

    /// The sheet of a fixture: pixels from the header, oriented; weight
    /// from the file; the shot from the EXIF; no relevé, so no measure.
    #[test]
    fn la_fiche_lit_le_fichier_et_son_exif() {
        let base = dossier("fiche");
        let root = base.join("photos");
        let dir = base.join("album");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&dir).unwrap();
        let octets = jpeg_decodable_avec_exif(
            8,
            6,
            &[
                (Ifd::Ifd0, 0x010F, Val::Ascii("Canon".into())),
                (Ifd::Ifd0, 0x0110, Val::Ascii("Canon EOS 5D Mark IV".into())),
                (Ifd::Ifd0, 0x0112, Val::Short(6)),
                (Ifd::Exif, 0x829A, Val::Rational(1, 250)),
                (Ifd::Exif, 0x829D, Val::Rational(28, 10)),
                (Ifd::Exif, 0x8827, Val::Short(200)),
                (Ifd::Exif, 0x9003, Val::Ascii("2019:07:14 18:30:00".into())),
                (Ifd::Exif, 0x920A, Val::Rational(35, 1)),
                (Ifd::Exif, 0xA434, Val::Ascii("EF35mm f/1.4L USM".into())),
            ],
        );
        fs::write(root.join("IMG_0001.jpg"), &octets).unwrap();

        let f = fiche(&dir, &root, "IMG_0001.jpg").unwrap();
        assert_eq!(f.nom, "IMG_0001.jpg");
        assert_eq!((f.largeur, f.hauteur), (6, 8), "couchée par le tag 6");
        assert_eq!(f.octets, octets.len() as u64);
        assert_eq!(f.format, "JPEG");
        assert!(f.taken_reliable);
        assert_eq!(f.taken.to_string(), "2019-07-14 18:30:00");
        assert_eq!(f.appareil.as_deref(), Some("Canon EOS 5D Mark IV"));
        assert_eq!(f.objectif.as_deref(), Some("EF35mm f/1.4L USM"));
        assert_eq!(f.ouverture, Some(2.8));
        assert_eq!(f.temps_de_pose.as_deref(), Some("1/250"));
        assert_eq!(f.iso, Some(200));
        assert_eq!(f.focale_mm, Some(35.0));
        assert_eq!(f.gps, None);
        assert_eq!(f.lieu, None);
        assert_eq!(f.note, None);
        assert_eq!(f.nettete, None, "pas de relevé, pas de mesure");
        assert_eq!(f.exposition, None);

        assert!(fiche(&dir, &root, "absente.jpg").is_err());
        assert!(fiche(&dir, &root, "../IMG_0001.jpg").is_err());
        let _ = fs::remove_dir_all(&base);
    }

    /// Without a relevé (an album composed on the Mac from its photos), the
    /// sheet still carries the two measures: the quality alert's own,
    /// taken on the photo's thumbnail (`qualite`), never a second reading.
    #[test]
    fn la_fiche_mesure_la_vignette_quand_l_album_n_a_pas_de_releve() {
        let base = dossier("sans-releve");
        let root = base.join("photos");
        let dir = base.join("album");
        let cache = dir.join(".cache").join("thumbs");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&cache).unwrap();
        image::ImageBuffer::from_fn(640, 480, |_, _| image::Rgb([1u8, 2, 3]))
            .save(root.join("nette.png"))
            .unwrap();
        let rayures = image::ImageBuffer::from_fn(64, 48, |x, _| {
            image::Rgb(if x % 2 == 0 { [0u8, 0, 0] } else { [255, 255, 255] })
        });
        rayures.save(cache.join("v-nette.png")).unwrap();
        fs::write(dir.join("thumbs.json"), r#"{"nette.png":"v-nette.png"}"#).unwrap();

        let f = fiche(&dir, &root, "nette.png").unwrap();
        let a = crate::analyze::analyze(&image::open(cache.join("v-nette.png")).unwrap());
        assert_eq!(f.nettete.map(f64::to_bits), Some(a.sharpness.to_bits()));
        assert_eq!(f.exposition.map(f64::to_bits), Some(a.exposure.to_bits()));
        // Et la même mesure que l'alerte de qualité, au bit près.
        let r = crate::qualite::releve_album(&dir, &root).unwrap();
        assert_eq!(f.nettete, Some(r.photos["nette.png"].nettete));
        let _ = fs::remove_dir_all(&base);
    }

    /// With a relevé beside the album, the sheet carries the two measures
    /// the composition took on this photo, found by its `src`.
    #[test]
    fn la_fiche_lit_le_releve_quand_l_album_en_porte_un() {
        use crate::analyze::Analysis;
        use crate::pipeline::Photo;
        use crate::releve::{FICHIER, VERSION};
        let base = dossier("releve");
        let root = base.join("photos");
        let dir = base.join("album");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(root.join("a.jpg"), jpeg_decodable_avec_exif(8, 6, &[])).unwrap();
        let releve = Releve {
            version: VERSION,
            racine: root.clone(),
            skipped_heic: 0,
            skipped_raw: 0,
            skipped_other: 0,
            illisibles: vec![],
            editees: vec![],
            photos: vec![Photo {
                path: root.join("a.jpg"),
                meta: meta::read(&root.join("a.jpg")),
                analysis: Analysis {
                    dhash: 0,
                    phash: 0,
                    colorsig: [0; 12],
                    sharpness: 12.5,
                    exposure: 0.8,
                    width: 8,
                    height: 6,
                },
                orig: (8, 6),
                faces: vec![],
                focal: None,
            }],
            vignettes: false,
        };
        releve.ecrire(&dir.join(FICHIER)).unwrap();

        let f = fiche(&dir, &root, "a.jpg").unwrap();
        assert_eq!(f.nettete, Some(12.5));
        assert_eq!(f.exposition, Some(0.8));
        assert!(!f.taken_reliable, "sans EXIF, la date vient du fichier");
        assert_eq!(f.appareil, None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn l_appareil_ne_repete_pas_sa_marque() {
        assert_eq!(appareil(Some("Canon"), Some("Canon EOS 5D")).as_deref(), Some("Canon EOS 5D"));
        assert_eq!(appareil(Some("NIKON CORPORATION"), Some("NIKON D750")).as_deref(), Some("NIKON D750"));
        assert_eq!(appareil(Some("Apple"), Some("iPhone 13")).as_deref(), Some("Apple iPhone 13"));
        assert_eq!(appareil(Some("Apple"), None).as_deref(), Some("Apple"));
        assert_eq!(appareil(None, Some("iPhone 13")).as_deref(), Some("iPhone 13"));
        assert_eq!(appareil(None, None), None);
    }

    /// The gazetteer's own rule, offline: a town within reach, or nothing.
    #[test]
    fn le_lieu_vient_de_l_atlas() {
        let paris = lieu_de(Some((48.8566, 2.3522))).expect("Paris est dans l'atlas");
        assert_eq!(paris.nom, "Paris");
        assert_eq!(paris.pays, "FR");
        assert_eq!(lieu_de(Some((0.0, -30.0))), None, "l'Atlantique n'a pas de ville");
        assert_eq!(lieu_de(None), None);
    }
}
