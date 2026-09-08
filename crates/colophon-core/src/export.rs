//! Le manifeste d'un export : de quel album il sort, et pour quel imprimeur.
//!
//! Le prévol sait ouvrir les PDF posés à côté de l'album et juger leur
//! géométrie. Il lui restait un angle mort : un fichier **juste de géométrie
//! et vieux de contenu** passait. Même format, mêmes pages, les photographies
//! d'avant la dernière retouche — le rapport disait vert et la presse
//! imprimait le livre d'hier.
//!
//! Fermer ça par une date de fichier a été refusé, et c'était juste : un
//! `mtime` ne se défend pas, un fichier plus vieux qu'`album.json` n'étant pas
//! forcément faux. Ce qui se défend, c'est que le fichier **dise de quel album
//! il sort**. `--print` et `--cover` posent donc `export.json` à côté
//! d'`album.json`, et le prévol le confronte à l'album du jour.
//!
//! **Un manifeste à côté de l'album, jamais une empreinte dans le PDF.** La
//! mettre dans `/ID` ou dans le XMP la ferait voyager avec le fichier, ce qui
//! est séduisant — et changerait les octets de **tous** les PDF exportés,
//! cassant le banc d'octets, forçant une remesure PDF/A-2b et touchant la
//! garantie de reproductibilité. Le prévol ne regarde jamais que des fichiers
//! du dossier de l'album : un manifeste posé là couvre toute sa portée pour
//! une fraction du risque.
//!
//! **Un fichier sans entrée au manifeste ne dit rien.** Silence complet, pas
//! même un avertissement : autrement tout export antérieur à ce module
//! deviendrait un bloquant, à commencer par les fichiers déjà vérifiés qu'on
//! s'apprête à commander. Le manifeste n'ajoute que de la précision ; il ne
//! rend jamais un fichier suspect par son absence. C'est aussi pourquoi un
//! `export.json` illisible se lit comme un manifeste vide plutôt que comme une
//! erreur : notre propre comptabilité ne doit refuser aucun album.
//!
//! **On ne hache pas les photographies.** 95 Mo à chaque prévol serait payer
//! très cher un cas rare. Une photo réécrite **hors** de Colophon n'est donc
//! pas vue, et un `root` repointé sur un autre dossier aux mêmes noms non
//! plus : c'est une limite assumée, à côté de celle que le contrôle de
//! géométrie a déjà laissée.

use crate::model::Album;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Le nom du manifeste, à côté d'`album.json`, de `curation.json` et de
/// `thumbs.json`.
pub const NOM: &str = "export.json";

/// Les deux noms de la **livraison** : ce que `--print` et `--cover` écrivent,
/// ce que le manifeste note, ce que le prévol juge, ce que le mail à
/// l'imprimeur nomme. Écrits une fois, parce qu'une règle qui lirait un
/// fichier sous un nom et le chercherait au manifeste sous un autre se tairait
/// pour toujours sans que rien ne rougisse. Les aperçus, eux, ont leurs
/// propres noms — `album.pdf` et `album-cover.apercu.pdf` — et n'entrent
/// jamais ici.
pub const LIVRAISON_INTERIEUR: &str = "album-print.pdf";
pub const LIVRAISON_COUVERTURE: &str = "album-cover.pdf";

/// Ce qu'un rendu a écrit : le fichier, pour qui, depuis quel album.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artefact {
    /// Nom du fichier posé à côté d'`album.json`, jamais un chemin.
    pub fichier: String,
    /// L'identifiant du profil imprimeur qui a décidé de sa géométrie.
    pub profil: String,
    /// L'empreinte de l'album tel qu'il était au rendu.
    pub album: String,
    /// La taille du fichier écrit. Gratuite, et elle attrape un fichier
    /// remplacé depuis ; hacher 95 Mo à chaque prévol pour la même
    /// information ne se justifierait pas.
    pub octets: u64,
    /// L'instant du rendu, sous la même horloge que le PDF lui-même, donc
    /// figée par `SOURCE_DATE_EPOCH` comme lui. Elle se lit, elle ne se juge
    /// pas : une date de fichier ne prouve rien, c'est tout le point de ce
    /// module.
    pub date: String,
}

/// Tout ce que les exports ont posé dans ce dossier.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifeste {
    #[serde(default)]
    pub artefacts: Vec<Artefact>,
}

impl Manifeste {
    /// Le manifeste du dossier, ou un manifeste vide s'il n'y en a pas — ou
    /// s'il ne se lit pas. Voir l'en-tête : absent veut dire « rien à dire ».
    pub fn lire(dir: &Path) -> Manifeste {
        fs::read_to_string(dir.join(NOM))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Ce que le manifeste dit d'un fichier, s'il en dit quelque chose.
    pub fn artefact(&self, fichier: &str) -> Option<&Artefact> {
        self.artefacts.iter().find(|a| a.fichier == fichier)
    }
}

/// Note dans le manifeste le fichier que le rendu vient d'écrire dans `dir`.
///
/// Une entrée par fichier : un second `--cover` sous un autre profil remplace
/// la première, il ne s'empile pas derrière elle. Les entrées sont triées par
/// nom, pour qu'un manifeste ne dépende pas de l'ordre des deux commandes.
pub fn noter(dir: &Path, album: &Album, fichier: &str, profil: &str) -> Result<()> {
    let chemin = dir.join(fichier);
    let octets = fs::metadata(&chemin)
        .with_context(|| format!("{} : le rendu n'a rien laissé à noter", chemin.display()))?
        .len();
    let mut m = Manifeste::lire(dir);
    m.artefacts.retain(|a| a.fichier != fichier);
    m.artefacts.push(Artefact {
        fichier: fichier.to_string(),
        profil: profil.to_string(),
        album: empreinte_album(album),
        octets,
        date: crate::pdfx::stamp().to_rfc3339(),
    });
    m.artefacts.sort_by(|a, b| a.fichier.cmp(&b.fichier));
    let json = serde_json::to_string_pretty(&m)?;
    let out = dir.join(NOM);
    fs::write(&out, json + "\n").with_context(|| format!("écriture de {}", out.display()))?;
    Ok(())
}

/// L'empreinte de ce que l'album **montre**, en 32 chiffres hexadécimaux.
///
/// `version` et `root` sont neutralisés, et rien d'autre. `version` est
/// l'estampille de schéma : une migration la change sans que le livre change.
/// `root` est l'endroit où vivent les photographies, pas ce que montre le
/// livre — et `scripts/identite-fiches.py` le neutralise déjà pour exactement
/// cette raison. Tout le reste entre, parce que tout le reste se voit à
/// l'impression : le titre, le format, le fond perdu, les planches, la
/// couverture, le rythme, le colophon, les réglages, la police.
pub fn empreinte_album(album: &Album) -> String {
    let mut a = album.clone();
    a.version = 0;
    a.root = String::new();
    let octets = serde_json::to_vec(&a).expect("un album se sérialise toujours");
    hex(&empreinte(&octets))
}

/// Seize octets qui diffèrent d'un contenu à l'autre.
///
/// FNV-1a, deux fois, avec un décalage du biais de départ. C'est l'algorithme
/// que `pdfx` écrivait déjà pour le `/ID` d'un PDF, avec sa raison : « a
/// document identifier needs to differ between documents, not to resist an
/// adversary ». Le besoin est le même ici, donc la fonction est ici et les
/// deux appelants la partagent — le workspace n'a aucun crate de hachage, et
/// il n'en gagne pas un.
pub fn empreinte(octets: &[u8]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for round in 0..2usize {
        let mut h: u64 =
            0xcbf2_9ce4_8422_2325 ^ (round as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for b in octets {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        out[round * 8..round * 8 + 8].copy_from_slice(&h.to_be_bytes());
    }
    out
}

fn hex(octets: &[u8]) -> String {
    let mut s = String::with_capacity(octets.len() * 2);
    for b in octets {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Police, Reglage, Size, Slot, Spread, SCHEMA};
    use std::path::PathBuf;

    fn album() -> Album {
        let mut a = Album::new("Corse", Path::new("/photos/corse"), Size { w: 210.0, h: 210.0 });
        for i in 0..4 {
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

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir()
            .join(format!("colophon-export-{nom}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// L'empreinte est celle de ce que l'album **montre**. Une migration de
    /// schéma change l'estampille sans changer une ligne du livre, et un
    /// dossier de photographies déplacé ne change rien de ce qui s'imprime :
    /// les deux sortent de l'empreinte, et rien d'autre n'en sort.
    #[test]
    fn ni_le_schema_ni_le_dossier_des_photos_ne_changent_l_empreinte() {
        let a = album();
        let reference = empreinte_album(&a);

        let mut ailleurs = a.clone();
        ailleurs.root = "/Volumes/disque-externe/corse".into();
        assert_eq!(empreinte_album(&ailleurs), reference, "le dossier des photos");

        let mut migre = a.clone();
        migre.version = SCHEMA + 7;
        assert_eq!(empreinte_album(&migre), reference, "l'estampille de schéma");

        // Et relire puis réécrire `album.json` sans rien changer ne la bouge
        // pas non plus : c'est ce qui rend un manifeste utilisable après une
        // simple ouverture dans l'éditeur.
        let json = serde_json::to_string_pretty(&a).unwrap();
        let relu: Album = serde_json::from_str(&json).unwrap();
        assert_eq!(empreinte_album(&relu), reference, "l'aller-retour par le disque");
    }

    /// Une famille de changement par test, parce qu'une seule assertion
    /// groupée passerait encore le jour où l'empreinte cesserait de voir l'une
    /// d'entre elles.
    #[test]
    fn une_legende_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.spreads[2].slots[0].caption = Some("Bonifacio, le matin".into());
        assert_ne!(empreinte_album(&b), empreinte_album(&a));
    }

    #[test]
    fn un_recadrage_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.spreads[1].slots[0].focal = [0.42, 0.5];
        assert_ne!(empreinte_album(&b), empreinte_album(&a));

        let mut c = a.clone();
        c.spreads[1].slots[0].zoom = 1.2;
        assert_ne!(empreinte_album(&c), empreinte_album(&a));
    }

    #[test]
    fn un_reglage_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.reglages.insert(
            "1.jpg".into(),
            Reglage { expo: 0.3, contraste: 0.0, nb: false },
        );
        assert_ne!(empreinte_album(&b), empreinte_album(&a));
    }

    #[test]
    fn la_police_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.police = Some(Police {
            fichier: "police.ttf".into(),
            postscript: "HelveticaNeue".into(),
            nom: "Helvetica Neue Regular".into(),
        });
        assert_ne!(empreinte_album(&b), empreinte_album(&a));
    }

    #[test]
    fn le_titre_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.title = "Corse, 2013".into();
        assert_ne!(empreinte_album(&b), empreinte_album(&a));
    }

    /// Une entrée par fichier, et un second rendu remplace la première au lieu
    /// de s'empiler derrière elle : le prévol lirait sinon le profil d'un
    /// fichier qui n'existe plus.
    #[test]
    fn le_manifeste_garde_une_entree_par_fichier() {
        let dir = dossier("noter");
        let a = album();
        fs::write(dir.join("album-print.pdf"), b"x".repeat(11)).unwrap();
        fs::write(dir.join("album-cover.pdf"), b"y".repeat(7)).unwrap();

        noter(&dir, &a, "album-print.pdf", "generique").unwrap();
        noter(&dir, &a, "album-cover.pdf", "cloudprinter").unwrap();
        noter(&dir, &a, "album-print.pdf", "cloudprinter").unwrap();

        let m = Manifeste::lire(&dir);
        assert_eq!(m.artefacts.len(), 2, "{m:?}");
        // Triées par nom : deux commandes dans l'autre ordre rendent le même
        // fichier.
        assert_eq!(m.artefacts[0].fichier, "album-cover.pdf");
        let print = m.artefact("album-print.pdf").unwrap();
        assert_eq!(print.profil, "cloudprinter");
        assert_eq!(print.octets, 11);
        assert_eq!(print.album, empreinte_album(&a));
        assert_eq!(m.artefact("album-cover.pdf").unwrap().octets, 7);
        assert!(m.artefact("album.pdf").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    /// Notre propre comptabilité ne refuse aucun album : un `export.json`
    /// qu'on ne comprend pas se lit comme un manifeste vide, donc le prévol se
    /// tait, au lieu de bloquer une commande sur un fichier annexe.
    #[test]
    fn un_manifeste_illisible_se_lit_comme_vide() {
        let dir = dossier("illisible");
        assert!(Manifeste::lire(&dir).artefacts.is_empty());
        fs::write(dir.join(NOM), "{ ceci n'est pas du JSON").unwrap();
        assert!(Manifeste::lire(&dir).artefacts.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// L'empreinte est la fonction que `pdfx` écrivait pour le `/ID` d'un PDF,
    /// sortie de son module. Ces octets-là sont ceux d'avant le déménagement :
    /// s'ils bougeaient, chaque PDF exporté changerait, et le banc d'octets le
    /// dirait une heure plus tard au lieu d'ici.
    #[test]
    fn l_empreinte_est_celle_que_le_pdf_ecrivait_deja() {
        assert_eq!(
            hex(&empreinte(b"Corse|2023-11-14T23:13:20+01:00")),
            "932a4a473e384f8e388482902a03a995"
        );
        assert_eq!(hex(&empreinte(b"")), "cbf29ce48422232555c5e55dfb685f30");
    }
}
