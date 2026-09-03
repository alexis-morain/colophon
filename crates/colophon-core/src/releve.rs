//! The relevé: what one reading of a photo folder measured, without the
//! photographs.
//!
//! Scanning, thumbnailing, analysis and face detection are the only half of a
//! composition that touches a pixel, and the only half that needs the 5.7 GB
//! of reference sets. What they measure fits in a few hundred kilobytes.
//! Serialized as fiches, the relevé replays the composing half on a machine
//! that holds no photograph at all: that is what makes the linter portable,
//! and with it the gate.
//!
//! A fiche is [`crate::pipeline::Photo`] itself, not a parallel model. A
//! second model would be a second source of truth for what a photo is, and it
//! would diverge the first time the analysis grows a field.

use crate::pipeline::Photo;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Bumped when a field the composition reads changes shape. A relevé from
/// another version is refused rather than read half-way: the whole point is
/// that composing from fiches gives the same album as composing from photos,
/// and a fiche missing a field would give a different one, quietly.
pub const VERSION: u32 = 1;

/// The name a relevé takes inside an album folder. An album composed without
/// the photographs carries the relevé it was composed from, because that is
/// the only thing left for the linter to measure.
pub const FICHIER: &str = "releve.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Releve {
    pub version: u32,
    /// The scanned folder: absolute in memory, reduced to its name in the
    /// file. An absolute path in a versioned file is one machine's path,
    /// false everywhere else and talkative the day the repository goes
    /// public. Every path below is relative to it, and recomposed from it.
    pub racine: PathBuf,
    /// What the scan walked past. Neither reaches `album.json`; both reach
    /// the lines the composition prints, which the path without photographs
    /// has no other way to say.
    #[serde(default)]
    pub skipped_heic: usize,
    /// Absent from every fiche that predates RAW support, and zero on the
    /// three reference sets: the default keeps them byte-identical.
    #[serde(default, skip_serializing_if = "est_zero")]
    pub skipped_raw: usize,
    #[serde(default)]
    pub skipped_other: usize,
    /// Files the decoder refused. Nothing was measured on them — there was
    /// nothing to measure — but they enter `curation.json` with the reason
    /// `illisible`, and an album that quietly loses them is an album that
    /// lies. The decoder's message is not kept: nothing reads it past the
    /// progress line, and it would churn the file at every image bump.
    #[serde(default)]
    pub illisibles: Vec<PathBuf>,
    /// Originals a Google Takeout keeps next to their `-edited` version:
    /// `(originale écartée, éditée gardée)`. Keeping both would print the
    /// same photograph twice, so the original is set aside before analysis
    /// — nothing was measured on it — and enters `curation.json` as
    /// `originale_editee`, its winner named. Absent from every fiche that
    /// predates the field and from every non-Takeout folder, which is what
    /// keeps the reference fiches byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub editees: Vec<(PathBuf, PathBuf)>,
    pub photos: Vec<Photo>,
    /// True while the thumbnails these fiches were measured from are still
    /// in the album folder's cache. False for a relevé read back from a
    /// file: the composition then stops before the first pixel. A fact about
    /// the relevé rather than a mode the caller picks, which is why it never
    /// travels in the file.
    #[serde(skip)]
    pub vignettes: bool,
}

impl Releve {
    /// Photographs the folder held, readable or not. The colophon page
    /// prints this figure, so the two paths have to agree on it: every
    /// scanned image is a fiche, an unreadable file, or an original set
    /// aside for its edited version — never none of the three.
    pub fn photos_scannees(&self) -> usize {
        self.photos.len() + self.illisibles.len() + self.editees.len()
    }

    /// The source an album file names a photo by: its path relative to the
    /// root. `album.json`'s slots and `curation.json`'s discards carry this
    /// string, and so does the linter's map — the same rule `build` applies
    /// on its way out.
    pub fn src(&self, p: &Path) -> String {
        relatif(p, &self.racine).to_string_lossy().to_string()
    }

    /// Write the fiches, every path relative to the folder's name.
    pub fn ecrire(&self, path: &Path) -> Result<()> {
        let racine = PathBuf::from(self.racine.file_name().unwrap_or(self.racine.as_os_str()));
        let portable = Releve {
            version: VERSION,
            racine,
            skipped_heic: self.skipped_heic,
            skipped_raw: self.skipped_raw,
            skipped_other: self.skipped_other,
            illisibles: self.illisibles.iter().map(|p| relatif(p, &self.racine)).collect(),
            editees: self
                .editees
                .iter()
                .map(|(o, e)| (relatif(o, &self.racine), relatif(e, &self.racine)))
                .collect(),
            photos: self
                .photos
                .iter()
                .map(|p| {
                    let mut p = p.clone();
                    p.path = relatif(&p.path, &self.racine);
                    p
                })
                .collect(),
            vignettes: false,
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(&portable)?)
            .with_context(|| format!("écriture du relevé {}", path.display()))
    }

    /// Read fiches back, recomposing every path from the root the file names.
    pub fn lire(path: &Path) -> Result<Self> {
        let texte = fs::read_to_string(path)
            .with_context(|| format!("lecture du relevé {}", path.display()))?;
        let mut releve: Releve = serde_json::from_str(&texte)
            .with_context(|| format!("relevé illisible : {}", path.display()))?;
        anyhow::ensure!(
            releve.version == VERSION,
            "relevé en version {}, cette version de Colophon lit la {VERSION} : \
             régénérez les fiches (scripts/fiches.sh)",
            releve.version
        );
        let racine = releve.racine.clone();
        for p in &mut releve.photos {
            p.path = racine.join(&p.path);
        }
        for p in &mut releve.illisibles {
            *p = racine.join(&*p);
        }
        for (o, e) in &mut releve.editees {
            *o = racine.join(&*o);
            *e = racine.join(&*e);
        }
        Ok(releve)
    }

    /// The relevé an album folder carries, when it has one. An album composed
    /// from photographs has none: its thumbnails are still there to measure.
    pub fn dans_album(dir: &Path) -> Result<Option<Self>> {
        let path = dir.join(FICHIER);
        if !path.exists() {
            return Ok(None);
        }
        Self::lire(&path).map(Some)
    }
}

fn est_zero(n: &usize) -> bool {
    *n == 0
}

fn relatif(p: &Path, racine: &Path) -> PathBuf {
    p.strip_prefix(racine).unwrap_or(p).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::Analysis;
    use crate::meta::PhotoMeta;

    fn photo(racine: &Path, nom: &str) -> Photo {
        Photo {
            path: racine.join(nom),
            meta: PhotoMeta {
                taken: chrono::NaiveDateTime::parse_from_str(
                    "2013-10-27 15:34:11",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
                taken_reliable: true,
                orientation: 6,
                gps: Some((42.5623, 8.741866999924259)),
                model: Some("Canon".into()),
                rating: Some(3),
            },
            analysis: Analysis {
                dhash: 15506529376606216,
                phash: 667202521446746851,
                colorsig: [112, 83, 69, 94, 70, 63, 119, 111, 104, 125, 113, 110],
                sharpness: 365.84453050297424,
                exposure: 0.465576171875,
                width: 500,
                height: 500,
            },
            orig: (4000, 3000),
            faces: vec![[0.1, 0.2, 0.3, 0.4]],
            focal: Some([0.39499999999999996, 0.42]),
        }
    }

    /// The whole point of the file: what went in comes back, field for field
    /// and to the last bit of every float, with the machine's path gone. A
    /// relevé that rounds a focal or keeps an absolute path is a relevé that
    /// composes a different album on another machine.
    #[test]
    fn un_releve_se_relit_a_l_identique_et_sans_chemin_de_machine() {
        let tmp = std::env::temp_dir().join("releve-test");
        let racine = tmp.join("vacances");
        let releve = Releve {
            version: VERSION,
            racine: racine.clone(),
            skipped_heic: 3,
            skipped_raw: 2,
            skipped_other: 1,
            illisibles: vec![racine.join("casse.jpg")],
            editees: vec![(racine.join("IMG_2.jpg"), racine.join("IMG_2-edited.jpg"))],
            photos: vec![photo(&racine, "plage/IMG_0001.jpg")],
            vignettes: true,
        };
        let fichier = tmp.join("fiches.json");
        releve.ecrire(&fichier).unwrap();

        // The absolute root stops at the door: the file only knows the
        // folder's name, which is what a versioned file may say.
        let texte = fs::read_to_string(&fichier).unwrap();
        assert!(!texte.contains(&*tmp.to_string_lossy()));
        assert!(texte.contains("\"racine\": \"vacances\""));

        let relu = Releve::lire(&fichier).unwrap();
        assert_eq!(relu.photos.len(), 1);
        assert_eq!(relu.racine, PathBuf::from("vacances"));
        assert_eq!(relu.photos[0].path, PathBuf::from("vacances/plage/IMG_0001.jpg"));
        assert_eq!(relu.illisibles, vec![PathBuf::from("vacances/casse.jpg")]);
        assert_eq!(
            relu.editees,
            vec![(
                PathBuf::from("vacances/IMG_2.jpg"),
                PathBuf::from("vacances/IMG_2-edited.jpg")
            )]
        );
        assert_eq!(relu.photos_scannees(), 3);
        assert!(!relu.vignettes, "un relevé relu n'a pas de vignettes");

        // Bit-for-bit: floats survive the round trip whole, or the identity
        // between the two composition paths dies on eighteen quiet lines.
        let (a, b) = (&releve.photos[0], &relu.photos[0]);
        assert_eq!(a.analysis.sharpness.to_bits(), b.analysis.sharpness.to_bits());
        assert_eq!(a.focal.unwrap()[0].to_bits(), b.focal.unwrap()[0].to_bits());
        assert_eq!(a.meta.gps.unwrap().1.to_bits(), b.meta.gps.unwrap().1.to_bits());
        assert_eq!(a.meta.taken, b.meta.taken);
        assert_eq!(a.orig, b.orig);

        // And writing what was read gives the same bytes: regenerating
        // unchanged fiches must leave the repository untouched.
        let refichier = tmp.join("fiches-bis.json");
        relu.ecrire(&refichier).unwrap();
        assert_eq!(fs::read(&fichier).unwrap(), fs::read(&refichier).unwrap());

        fs::remove_dir_all(&tmp).ok();
    }

    /// A relevé from another era is refused whole: half-read fiches would
    /// compose a quietly different album, which is the one forbidden outcome.
    #[test]
    fn un_releve_d_une_autre_version_est_refuse() {
        let tmp = std::env::temp_dir().join("releve-test-version");
        fs::create_dir_all(&tmp).unwrap();
        let fichier = tmp.join("fiches.json");
        fs::write(
            &fichier,
            format!(
                "{{\"version\": {}, \"racine\": \"x\", \"photos\": []}}",
                VERSION + 1
            ),
        )
        .unwrap();
        let err = Releve::lire(&fichier).unwrap_err().to_string();
        assert!(err.contains("fiches.sh"), "{err}");
        fs::remove_dir_all(&tmp).ok();
    }
}
