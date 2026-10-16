//! Le dossier de photos d'un album, retrouvé d'un lancement à l'autre sous le
//! bac à sable du Mac App Store.
//!
//! `album.root` est un chemin absolu. Hors bac à sable il suffit ; dedans, le
//! dossier que la personne a choisi n'est lisible que le temps du processus
//! qui l'a choisi. Au lancement suivant, il faut un **marque-page de
//! sécurité** : écrit à côté d'`album.json`, sous le nom `root.bookmark`,
//! jamais dedans — `album.json` reste lisible et réparable à la main, et son
//! schéma ne bouge pas. À l'ouverture d'un album, on le résout et on ouvre
//! l'accès **avant** toute lecture de photo, et l'accès reste ouvert tant que
//! l'album l'est ([`Acces`], refermé quand il tombe).
//!
//! Tout ce qui parle au système n'existe qu'en saveur `mas` sur macOS ;
//! ailleurs les mêmes fonctions ne font rien, et le DMG ne change pas.
//! Ce qui se décide sans le système — le dossier rechoisi est-il le bon ? —
//! est pur, et se teste dans les deux saveurs.

use std::path::{Path, PathBuf};

/// Le fichier du marque-page, à côté d'`album.json`.
#[cfg_attr(not(all(target_os = "macos", feature = "mas")), allow(dead_code))]
pub const FICHIER: &str = "root.bookmark";

/// L'accès ouvert au dossier de photos d'un album. Le refermer est le
/// `Drop` : remplacer l'accès de l'album d'avant par celui du suivant le
/// referme sans y penser.
pub struct Acces {
    #[cfg(all(target_os = "macos", feature = "mas"))]
    poignee: *mut std::ffi::c_void,
    /// Le chemin que le marque-page désigne aujourd'hui.
    #[allow(dead_code)]
    pub chemin: PathBuf,
}

// La poignée est un NSURL retenu ; rien ne le partage, on ne fait que le
// rendre au système depuis le fil qui laisse tomber l'accès.
#[cfg(all(target_os = "macos", feature = "mas"))]
unsafe impl Send for Acces {}

#[cfg(all(target_os = "macos", feature = "mas"))]
impl Drop for Acces {
    fn drop(&mut self) {
        unsafe { ffi::colophon_signet_finir(self.poignee) }
    }
}

/// Écrit le marque-page du dossier de photos `root` à côté de l'album. Le
/// processus doit y avoir accès à cet instant : juste après le choix du
/// dossier, ou pendant qu'un marque-page précédent est ouvert. Ne fait rien
/// hors saveur `mas`.
pub fn poser(album: &Path, root: &Path) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "mas"))]
    {
        let octets = creer(root).ok_or_else(|| {
            format!("marque-page refusé par le système pour {}", root.display())
        })?;
        std::fs::write(album.join(FICHIER), octets)
            .map_err(|e| format!("écriture de {FICHIER} : {e}"))?;
    }
    #[cfg(not(all(target_os = "macos", feature = "mas")))]
    let _ = (album, root);
    Ok(())
}

/// Ouvre l'accès au dossier de photos de l'album par son marque-page, et le
/// réécrit s'il est périmé. `None` sans marque-page, ou quand le système ne
/// le résout plus : c'est alors à la personne de revoir le dossier.
pub fn ouvrir(album: &Path) -> Option<Acces> {
    #[cfg(all(target_os = "macos", feature = "mas"))]
    {
        let octets = std::fs::read(album.join(FICHIER)).ok()?;
        let (acces, perime) = commencer(&octets)?;
        if perime {
            // L'accès est ouvert : c'est le moment de réécrire. Un échec ne
            // coûte que la même réécriture au prochain lancement.
            let _ = poser(album, &acces.chemin);
        }
        Some(acces)
    }
    #[cfg(not(all(target_os = "macos", feature = "mas")))]
    {
        let _ = album;
        None
    }
}

/// Si le dossier que la personne vient de rechoisir est bien celui de
/// l'album : même nom que `root`, et toutes les photos posées s'y trouvent.
/// Un dossier déplacé passe ; un autre dossier, même homonyme, non.
///
/// Le refus est un code, `refus-revoir:autre-nom:<nom>` ou
/// `refus-revoir:manquantes:<n>:<total>`, que l'interface traduit en phrase
/// (`racine.ts`) : la raison est ce que la personne doit lire, dans sa langue.
pub fn dossier_revu(root: &Path, choisi: &Path, srcs: &[String]) -> Result<(), String> {
    if root.file_name() != choisi.file_name() {
        return Err(format!(
            "refus-revoir:autre-nom:{}",
            root.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    let manquantes = srcs.iter().filter(|s| !choisi.join(s).is_file()).count();
    if manquantes == 0 {
        Ok(())
    } else {
        Err(format!("refus-revoir:manquantes:{manquantes}:{}", srcs.len()))
    }
}

/// Le nouveau chemin du dossier de photos quand le marque-page l'a suivi
/// ailleurs : un dossier déplacé sur le même volume, le système le retrouve
/// tout seul. `None` s'il n'a pas bougé, ou si ce qu'il désigne n'est plus
/// sûrement le dossier de l'album ([`dossier_revu`]) : la personne le revoit.
pub fn suivre(root: &Path, resolu: &Path, srcs: &[String]) -> Option<PathBuf> {
    (resolu != root && dossier_revu(root, resolu, srcs).is_ok()).then(|| resolu.to_path_buf())
}

/// Si l'app peut lire le dossier : sous le bac à sable, un dossier sans
/// accès peut encore exister au sens de `stat`, c'est sa lecture qui échoue.
pub fn lisible(root: &Path) -> bool {
    std::fs::read_dir(root).is_ok()
}

#[cfg(all(target_os = "macos", feature = "mas"))]
fn creer(root: &Path) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(root.as_os_str().as_bytes()).ok()?;
    let mut longueur = 0usize;
    unsafe {
        let p = ffi::colophon_signet_creer(c.as_ptr(), &mut longueur);
        if p.is_null() {
            return None;
        }
        let octets = std::slice::from_raw_parts(p, longueur).to_vec();
        ffi::colophon_signet_liberer(p.cast());
        Some(octets)
    }
}

#[cfg(all(target_os = "macos", feature = "mas"))]
fn commencer(octets: &[u8]) -> Option<(Acces, bool)> {
    use std::os::unix::ffi::OsStrExt;
    let mut chemin: *mut std::ffi::c_char = std::ptr::null_mut();
    let mut perime: std::ffi::c_int = 0;
    unsafe {
        let poignee =
            ffi::colophon_signet_commencer(octets.as_ptr(), octets.len(), &mut chemin, &mut perime);
        if poignee.is_null() {
            return None;
        }
        let lu = std::ffi::CStr::from_ptr(chemin).to_bytes().to_vec();
        ffi::colophon_signet_liberer(chemin.cast());
        let chemin = PathBuf::from(std::ffi::OsStr::from_bytes(&lu));
        Some((Acces { poignee, chemin }, perime != 0))
    }
}

/// Montrer un fichier dans le Finder, ouvrir un dossier, ouvrir une page :
/// par NSWorkspace, l'API que le bac à sable prévoit, au lieu d'un `open`
/// lancé en processus.
#[cfg(all(target_os = "macos", feature = "mas"))]
pub mod bureau {
    use super::ffi;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    fn c(octets: &[u8]) -> std::io::Result<CString> {
        CString::new(octets).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
    }

    fn accepte(ok: std::ffi::c_int) -> std::io::Result<()> {
        if ok == 1 {
            Ok(())
        } else {
            Err(std::io::Error::other("refusé par NSWorkspace"))
        }
    }

    pub fn reveler(cible: &Path) -> std::io::Result<()> {
        let c = c(cible.as_os_str().as_bytes())?;
        accepte(unsafe { ffi::colophon_bureau_reveler(c.as_ptr()) })
    }

    pub fn ouvrir_dossier(dir: &Path) -> std::io::Result<()> {
        let c = c(dir.as_os_str().as_bytes())?;
        accepte(unsafe { ffi::colophon_bureau_ouvrir(c.as_ptr(), 1) })
    }

    pub fn ouvrir_url(url: &str) -> std::io::Result<()> {
        let c = c(url.as_bytes())?;
        accepte(unsafe { ffi::colophon_bureau_ouvrir(c.as_ptr(), 0) })
    }
}

#[cfg(all(target_os = "macos", feature = "mas"))]
mod ffi {
    use std::ffi::{c_char, c_int, c_void};
    extern "C" {
        pub fn colophon_signet_creer(chemin: *const c_char, longueur: *mut usize) -> *mut u8;
        pub fn colophon_signet_liberer(p: *mut c_void);
        pub fn colophon_signet_commencer(
            octets: *const u8,
            longueur: usize,
            chemin: *mut *mut c_char,
            perime: *mut c_int,
        ) -> *mut c_void;
        pub fn colophon_signet_finir(poignee: *mut c_void);
        pub fn colophon_bureau_reveler(chemin: *const c_char) -> c_int;
        pub fn colophon_bureau_ouvrir(cible: *const c_char, dossier: c_int) -> c_int;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `<temp>/colophon-signet-<tag>-<pid>/<nom>`, avec ses photos, neuf.
    fn dossier(tag: &str, nom: &str, photos: &[&str]) -> PathBuf {
        let base = std::env::temp_dir().join(format!("colophon-signet-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let d = base.join(nom);
        std::fs::create_dir_all(&d).unwrap();
        for p in photos {
            std::fs::write(d.join(p), b"x").unwrap();
        }
        d
    }

    #[test]
    fn le_dossier_deplace_avec_ses_photos_est_le_bon() {
        let choisi = dossier("deplace", "corse-2013", &["a.jpg", "b.jpg"]);
        let root = Path::new("/Volumes/ancien/corse-2013");
        let srcs = vec!["a.jpg".to_string(), "b.jpg".to_string()];
        assert_eq!(dossier_revu(root, &choisi, &srcs), Ok(()));
    }

    /// Le refus arrive à l'écran par un code que l'interface traduit : la
    /// raison est la phrase, pas un détail replié en français seul.
    #[test]
    fn un_autre_nom_n_est_pas_le_dossier_de_l_album() {
        let choisi = dossier("autre", "islande", &["a.jpg"]);
        let e = dossier_revu(Path::new("/x/corse-2013"), &choisi, &["a.jpg".into()]).unwrap_err();
        assert_eq!(e, "refus-revoir:autre-nom:corse-2013");
    }

    #[test]
    fn un_homonyme_sans_les_photos_est_refuse_et_le_dit() {
        let choisi = dossier("homonyme", "corse-2013", &["a.jpg"]);
        let srcs = vec!["a.jpg".to_string(), "b.jpg".to_string(), "c.jpg".to_string()];
        let e = dossier_revu(Path::new("/x/corse-2013"), &choisi, &srcs).unwrap_err();
        assert_eq!(e, "refus-revoir:manquantes:2:3");
    }

    /// Un marque-page suit le dossier qu'on déplace : à l'ouverture, l'album
    /// le suit aussi, au lieu de demander un dossier que l'app lit déjà.
    #[test]
    fn l_album_suit_le_marque_page_d_un_dossier_deplace() {
        let resolu = dossier("suivre", "corse-2013", &["a.jpg", "b.jpg"]);
        let srcs = vec!["a.jpg".to_string(), "b.jpg".to_string()];
        let root = Path::new("/Users/x/Pictures/corse-2013");
        assert_eq!(suivre(root, &resolu, &srcs), Some(resolu.clone()));
    }

    #[test]
    fn un_marque_page_au_meme_endroit_ne_change_rien() {
        let resolu = dossier("meme", "corse-2013", &["a.jpg"]);
        assert_eq!(suivre(&resolu, &resolu, &["a.jpg".into()]), None);
    }

    /// Renommé, ou vidé de ses photos : ce n'est plus sûrement le dossier de
    /// l'album, la personne le revoit.
    #[test]
    fn un_marque_page_vers_un_dossier_renomme_ne_suit_pas() {
        let resolu = dossier("renomme", "corse", &["a.jpg"]);
        let root = Path::new("/x/corse-2013");
        assert_eq!(suivre(root, &resolu, &["a.jpg".into()]), None);
        let vide = dossier("vide", "corse-2013", &[]);
        assert_eq!(suivre(root, &vide, &["a.jpg".into()]), None);
    }

    /// Hors saveur `mas`, rien ne s'écrit ni ne s'ouvre : le DMG garde ses
    /// albums tels qu'ils sont.
    #[cfg(not(feature = "mas"))]
    #[test]
    fn hors_saveur_mas_aucun_marque_page() {
        let d = dossier("dmg", "album", &[]);
        poser(&d, &d).unwrap();
        assert!(!d.join(FICHIER).exists());
        assert!(ouvrir(&d).is_none());
    }
}
