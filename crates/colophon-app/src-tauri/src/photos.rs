//! La bibliothèque du Mac, et ce qu'on en fait.
//!
//! **Le principe de tout le module** : la photothèque n'entre jamais dans le
//! moteur. Elle produit un dossier de photographies, et `colophon-core` scanne
//! ce dossier comme il scanne n'importe quel autre. Ni `scan.rs`, ni `meta.rs`,
//! ni la curation, ni le linter, ni le prévol n'apprennent qu'Apple Photos
//! existe — et le gate reste vert sur Ubuntu et Windows sans un `#[cfg]` de
//! plus dans le moteur.
//!
//! Ce choix se payait d'une copie de la bibliothèque, et c'est la mesure du
//! 02/09 qui l'a rendu gratuit : `writeDataForAssetResource:` rend le fichier
//! d'origine **à l'octet**, et sur le même volume APFS il clone les blocs.
//! 219 Mo de photographies ont coûté 2 Mo de disque réel. Voir
//! `docs/mesures/2026-09-02-la-bibliotheque-du-mac.json`.
//!
//! La moitié Objective-C est dans `photos.m` : elle ne décide rien. Ici vivent
//! l'ordre, les noms, le refus du réseau, l'annulation et le rapport.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Ce que l'écran a le droit d'afficher. Trois états, pas deux : c'est le
/// cœur de ce module.
///
/// PhotoKit ne lève jamais pour un défaut d'accès, il rend une liste vide, et
/// **trois causes distinctes rendent cette même liste vide** : pas encore
/// autorisé, autorisé mais bibliothèque système injoignable, et réellement
/// vide. Les confondre fait dire « aucun album » à quelqu'un qui n'a rien à
/// corriger dans Colophon.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "etat", rename_all = "kebab-case")]
pub enum Etat {
    /// Le système n'a jamais posé la question. Un bouton la pose.
    ADemander,
    /// L'utilisateur a dit non, ou une règle d'entreprise l'interdit. Rien à
    /// faire depuis l'application : ça se règle dans Réglages Système.
    Refuse,
    /// Autorisé, et la bibliothèque système est introuvable. `chemin` porte ce
    /// que photolibraryd déclarait, pour que la phrase soit utile.
    Injoignable { chemin: String },
    /// Autorisé et lisible.
    Lisible { albums: Vec<AlbumPhotos> },
    /// Pas de photothèque Apple sur cette plate-forme. Construit par le module
    /// `autre`, donc jamais sur macOS — d'où le `allow`, qui dit que ce n'est
    /// pas du code mort mais du code d'un autre système.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Indisponible,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AlbumPhotos {
    pub id: String,
    pub nom: String,
    pub photos: usize,
    /// Un album intelligent est une heuristique d'Apple, pas un choix de
    /// l'utilisateur : trois photos de Corse ont été classées « Selfies » sur
    /// la machine de mesure. L'écran le dit plutôt que de faire croire à un
    /// rangement voulu.
    pub intelligent: bool,
}

/// Une photographie de l'album, avant tout octet lu.
#[derive(Deserialize, Clone, Debug)]
pub struct FichePhoto {
    pub id: String,
    pub nom: String,
    /// Vraie quand Photos porte une version rendue, donc quand l'utilisateur a
    /// retouché. C'est cette version-là qu'on écrit.
    #[allow(dead_code)]
    pub modifiee: bool,
}

/// Ce qu'un import a fait, écrit à côté des photographies.
///
/// Le rapport existe parce qu'un écran ne survit pas à la fermeture de la
/// fenêtre, et qu'un album qui perd une photo en silence est un album qui
/// ment. `scan.rs` ignore déjà l'extension `json`, donc ce fichier ne gonfle
/// aucun compteur du moteur.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RapportImport {
    pub album: String,
    pub dossier: String,
    pub demandees: usize,
    pub importees: usize,
    pub octets: u64,
    /// Nommées, jamais seulement comptées : ce sont des photographies que
    /// l'utilisateur croit avoir importées.
    pub absentes_du_mac: Vec<String>,
    /// Les autres échecs, avec leur motif tel que le système l'a écrit.
    pub echecs: Vec<EchecPhoto>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EchecPhoto {
    pub nom: String,
    pub motif: String,
}

/// Le nom d'un fichier importé : le rang, puis le nom d'origine.
///
/// `originalFilename` collisionne massivement dans une bibliothèque — dix ans
/// d'iPhone rendent plusieurs `IMG_0001.HEIC` —, et un suffixe « (1) »
/// rejouerait exactement le dégât que la session 5.1 passe son temps à
/// réparer sur les exports Google. Le rang donne un ordre de chemin stable,
/// aucune collision possible, et deux imports du même album rendent les mêmes
/// noms.
pub fn nom_de_fichier(rang: usize, nom_origine: &str) -> String {
    // Le nom vient de la bibliothèque, donc d'ailleurs : un séparateur de
    // chemin dedans écrirait hors du dossier d'import.
    let propre: String = nom_origine
        .chars()
        .map(|c| if c == '/' || c == '\\' || c == '\0' { '_' } else { c })
        .collect();
    let propre = propre.trim().trim_start_matches('.');
    let propre = if propre.is_empty() { "photo.jpg" } else { propre };
    format!("{:04}-{propre}", rang + 1)
}

/// Le dossier d'import proposé pour un album.
///
/// Visible, dans les Images de l'utilisateur, jamais un cache ni un
/// Application Support : `album.root` pointera là pour toujours, et
/// « `album.json` réparable à la main » n'a de sens que si les photographies
/// se retrouvent avec le Finder.
pub fn dossier_propose(maison: &Path, nom_album: &str) -> PathBuf {
    let propre: String = nom_album
        .chars()
        .map(|c| if c == '/' || c == '\\' || c == ':' { '-' } else { c })
        .collect();
    let propre = propre.trim();
    let propre = if propre.is_empty() { "Album" } else { propre };
    maison.join("Pictures").join("Colophon").join(propre)
}

#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::ffi::{c_char, c_int, CStr, CString};
    use std::sync::mpsc;
    use std::time::Duration;

    extern "C" {
        fn colophon_photos_statut() -> c_int;
        fn colophon_photos_demander() -> c_int;
        fn colophon_photos_albums() -> *mut c_char;
        fn colophon_photos_lister(album: *const c_char) -> *mut c_char;
        fn colophon_photos_exporter(
            asset: *const c_char,
            dest: *const c_char,
            reseau: c_int,
        ) -> *mut c_char;
        fn colophon_photos_liberer(p: *mut c_char);
    }

    /// Reprend une chaîne du pont et la libère. Toute valeur qui traverse
    /// passe par ici : il n'y a qu'un seul endroit où `free` est appelé.
    unsafe fn reprendre(p: *mut c_char) -> String {
        if p.is_null() {
            return String::new();
        }
        let s = CStr::from_ptr(p).to_string_lossy().into_owned();
        colophon_photos_liberer(p);
        s
    }

    /// La garde de délai.
    ///
    /// Mesuré le 02/09 : sous une signature ad-hoc — celle de Colophon
    /// aujourd'hui, `TeamIdentifier` absent —, une requête PhotoKit peut ne
    /// **jamais** rendre la main ; elle boucle sur `NSXPCConnection`. Une
    /// fenêtre figée serait le pire des trois états.
    ///
    /// Le fil n'est pas tué, parce qu'on ne tue pas un fil bloqué dans du code
    /// système sans laisser la mémoire dans un état qu'on ne contrôle plus. Il
    /// est abandonné : il finira, ou il attendra la fin du processus, et dans
    /// les deux cas la fenêtre a déjà repris la main.
    fn avec_delai<T, F>(secondes: u64, travail: F) -> Option<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(travail());
        });
        rx.recv_timeout(Duration::from_secs(secondes)).ok()
    }

    #[derive(Deserialize)]
    struct ReponseAlbums {
        albums: Vec<AlbumPhotos>,
        bibliotheque_systeme: String,
        bibliotheque_presente: bool,
    }

    #[derive(Deserialize)]
    struct ReponseListe {
        #[serde(default)]
        nom: String,
        #[serde(default)]
        photos: Vec<FichePhoto>,
        #[serde(default)]
        erreur: Option<String>,
    }

    #[derive(Deserialize)]
    struct ReponseExport {
        ok: bool,
        #[serde(default)]
        octets: u64,
        #[serde(default)]
        absente_du_mac: bool,
        #[serde(default)]
        motif: String,
    }

    /// Le statut brut de l'autorisation. 3 et 4 valent « on peut lire ».
    pub fn statut() -> i32 {
        unsafe { colophon_photos_statut() }
    }

    /// Demande l'accès. Bloque tant que l'utilisateur n'a pas répondu à la
    /// fenêtre du système, ce qui est voulu : il n'y a rien à afficher entre
    /// la question et la réponse.
    pub fn demander() -> i32 {
        unsafe { colophon_photos_demander() }
    }

    /// L'état de la bibliothèque, prêt pour l'écran.
    ///
    /// L'ordre des tests est la règle du module : **le statut d'abord**, la
    /// requête ensuite. L'inverse rendrait une liste vide pour trois raisons
    /// différentes sans jamais lever, et c'est le piège que ce module existe
    /// pour fermer.
    pub fn etat() -> Etat {
        match statut() {
            0 => return Etat::ADemander,
            1 | 2 => return Etat::Refuse,
            _ => {}
        }
        let brut = match avec_delai(5, || unsafe { reprendre(colophon_photos_albums()) }) {
            Some(s) => s,
            // Le délai a mordu : la bibliothèque ne répond pas. On ne connaît
            // pas son chemin, la phrase générique fera le travail.
            None => return Etat::Injoignable { chemin: String::new() },
        };
        let r: ReponseAlbums = match serde_json::from_str(&brut) {
            Ok(r) => r,
            Err(_) => return Etat::Injoignable { chemin: String::new() },
        };
        if !r.bibliotheque_presente {
            return Etat::Injoignable { chemin: r.bibliotheque_systeme };
        }
        Etat::Lisible { albums: r.albums }
    }

    /// Les photographies d'un album, sans lire un octet de pixel.
    pub fn lister(album: &str) -> Result<(String, Vec<FichePhoto>), String> {
        let id = CString::new(album).map_err(|_| "identifiant d'album invalide".to_string())?;
        let brut = avec_delai(10, move || unsafe {
            reprendre(colophon_photos_lister(id.as_ptr()))
        })
        .ok_or_else(|| "la photothèque ne répond pas".to_string())?;
        let r: ReponseListe =
            serde_json::from_str(&brut).map_err(|e| format!("réponse illisible : {e}"))?;
        if let Some(e) = r.erreur {
            return Err(e);
        }
        Ok((r.nom, r.photos))
    }

    /// Écrit une photographie dans le dossier d'import.
    ///
    /// `reseau` reste faux au premier passage, toujours : ce qui n'est pas sur
    /// ce Mac se compte et se nomme, et rien ne part chercher des octets chez
    /// Apple sans que l'utilisateur ait vu combien de photographies et combien
    /// d'octets. La promesse hors ligne est un verrou du projet.
    fn exporter(asset: &str, dest: &Path, reseau: bool) -> ReponseExport {
        let refus = |motif: &str| ReponseExport {
            ok: false,
            octets: 0,
            absente_du_mac: false,
            motif: motif.to_string(),
        };
        let (Ok(id), Ok(chemin)) = (
            CString::new(asset),
            CString::new(dest.to_string_lossy().as_bytes()),
        ) else {
            return refus("chemin ou identifiant invalide");
        };
        let brut = match avec_delai(120, move || unsafe {
            reprendre(colophon_photos_exporter(
                id.as_ptr(),
                chemin.as_ptr(),
                if reseau { 1 } else { 0 },
            ))
        }) {
            Some(s) => s,
            None => return refus("la photothèque ne répond pas"),
        };
        serde_json::from_str(&brut).unwrap_or_else(|_| refus("réponse illisible"))
    }

    /// Importe un album dans un dossier, et rend ce qui s'est passé.
    ///
    /// `progres` reçoit (fait, total) après chaque photographie ; `annule` est
    /// consulté entre deux. Un import annulé laisse le dossier tel quel, avec
    /// son rapport : c'est un dossier de photographies valide, et le moteur
    /// sait le composer.
    pub fn importer(
        album: &str,
        dossier: &Path,
        reseau: bool,
        progres: &dyn Fn(usize, usize),
        annule: &dyn Fn() -> bool,
    ) -> Result<RapportImport, String> {
        let (nom, fiches) = lister(album)?;
        std::fs::create_dir_all(dossier)
            .map_err(|e| format!("création de {} : {e}", dossier.display()))?;

        let mut rapport = RapportImport {
            album: nom,
            dossier: dossier.to_string_lossy().to_string(),
            demandees: fiches.len(),
            ..Default::default()
        };

        for (rang, fiche) in fiches.iter().enumerate() {
            if annule() {
                break;
            }
            let cible = dossier.join(nom_de_fichier(rang, &fiche.nom));
            let r = exporter(&fiche.id, &cible, reseau);
            if r.ok {
                rapport.importees += 1;
                rapport.octets += r.octets;
            } else if r.absente_du_mac {
                rapport.absentes_du_mac.push(fiche.nom.clone());
            } else {
                rapport.echecs.push(EchecPhoto {
                    nom: fiche.nom.clone(),
                    motif: r.motif,
                });
            }
            progres(rang + 1, fiches.len());
        }

        // Le rapport s'écrit même sur un import annulé ou raté : c'est
        // justement là qu'il sert.
        let _ = std::fs::write(
            dossier.join("import.json"),
            serde_json::to_string_pretty(&rapport).unwrap_or_default(),
        );
        Ok(rapport)
    }
}

/// Hors macOS il n'y a pas de photothèque Apple, et le pont n'est pas compilé.
/// Les commandes existent quand même, pour que le front n'ait pas à savoir sur
/// quelle plate-forme il tourne : elles rendent `Indisponible`.
#[cfg(not(target_os = "macos"))]
pub use autre::*;

#[cfg(not(target_os = "macos"))]
mod autre {
    use super::*;

    pub fn statut() -> i32 {
        1
    }

    pub fn demander() -> i32 {
        1
    }

    pub fn etat() -> Etat {
        Etat::Indisponible
    }

    pub fn lister(_album: &str) -> Result<(String, Vec<FichePhoto>), String> {
        Err("la photothèque Apple n'existe que sur macOS".into())
    }

    pub fn importer(
        _album: &str,
        _dossier: &Path,
        _reseau: bool,
        _progres: &dyn Fn(usize, usize),
        _annule: &dyn Fn() -> bool,
    ) -> Result<RapportImport, String> {
        Err("la photothèque Apple n'existe que sur macOS".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le rang passe devant, et deux photographies du même nom ne se marchent
    /// jamais dessus. C'est toute la raison du préfixe.
    #[test]
    fn le_rang_ecarte_les_homonymes() {
        assert_eq!(nom_de_fichier(0, "IMG_0001.HEIC"), "0001-IMG_0001.HEIC");
        assert_eq!(nom_de_fichier(41, "IMG_0001.HEIC"), "0042-IMG_0001.HEIC");
        assert_ne!(
            nom_de_fichier(0, "IMG_0001.HEIC"),
            nom_de_fichier(1, "IMG_0001.HEIC")
        );
    }

    /// Le nom vient de la bibliothèque, donc d'ailleurs. Un séparateur de
    /// chemin dedans écrirait hors du dossier d'import.
    #[test]
    fn un_nom_ne_sort_jamais_du_dossier() {
        let n = nom_de_fichier(0, "../../.ssh/authorized_keys");
        assert!(!n.contains('/'), "{n}");
        assert!(!n.contains('\\'), "{n}");
        assert!(n.starts_with("0001-"), "{n}");
    }

    #[test]
    fn un_nom_vide_reste_un_fichier() {
        assert_eq!(nom_de_fichier(0, ""), "0001-photo.jpg");
        assert_eq!(nom_de_fichier(0, "   "), "0001-photo.jpg");
    }

    /// Le dossier est dans les Images, visible, nommé par l'album.
    #[test]
    fn le_dossier_propose_est_trouvable_a_la_main() {
        let d = dossier_propose(Path::new("/Users/x"), "Corse 2013");
        assert_eq!(d, PathBuf::from("/Users/x/Pictures/Colophon/Corse 2013"));
    }

    #[test]
    fn un_nom_dalbum_ne_creuse_pas_darborescence() {
        let d = dossier_propose(Path::new("/Users/x"), "Été/2019");
        assert_eq!(d, PathBuf::from("/Users/x/Pictures/Colophon/Été-2019"));
    }

    /// Les trois états ne se sérialisent pas pareil : le front en dépend pour
    /// choisir sa phrase, et confondre « injoignable » et « vide » est le
    /// défaut que ce module existe pour éviter.
    #[test]
    fn les_trois_etats_se_distinguent_a_lecran() {
        let vide = serde_json::to_string(&Etat::Lisible { albums: vec![] }).unwrap();
        let injoignable = serde_json::to_string(&Etat::Injoignable {
            chemin: "/Users/x/Pictures/Photos Library.photoslibrary".into(),
        })
        .unwrap();
        let a_demander = serde_json::to_string(&Etat::ADemander).unwrap();

        assert!(vide.contains("\"etat\":\"lisible\""), "{vide}");
        assert!(injoignable.contains("\"etat\":\"injoignable\""), "{injoignable}");
        assert!(a_demander.contains("\"etat\":\"a-demander\""), "{a_demander}");
        // Et la phrase de l'état injoignable porte le chemin, sans quoi elle
        // n'apprend rien à personne.
        assert!(injoignable.contains("Photos Library.photoslibrary"), "{injoignable}");
    }
}
