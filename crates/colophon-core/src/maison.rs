//! Le dossier personnel de la personne, le vrai.
//!
//! `$HOME` ment sous le bac à sable du Mac App Store : il y vaut le conteneur
//! de l'app (`~/Library/Containers/fr.morain.colophon/Data`), et tout ce qui
//! s'en déduit — `~/Pictures`, `~/Library/Fonts` — vise un dossier que
//! personne ne voit. La base des comptes, elle, dit toujours la vérité :
//! `getpwuid(getuid())->pw_dir` rend `/Users/<nom>` dans les deux saveurs.
//! Hors bac à sable les deux coïncident, donc le DMG ne change pas.

use std::path::PathBuf;

/// Le dossier personnel tel que la base des comptes le connaît, `None` si
/// elle ne le connaît pas. Ne lit jamais `$HOME`.
#[cfg(unix)]
pub fn vrai_dossier_personnel() -> Option<PathBuf> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;
    // `getpwuid_r`, pas `getpwuid` : le second rend un tampon statique que
    // n'importe quel autre fil peut réécrire pendant qu'on le lit.
    let mut tampon = vec![0u8; 4096];
    let mut entree: libc::passwd = unsafe { std::mem::zeroed() };
    let mut resultat: *mut libc::passwd = std::ptr::null_mut();
    let code = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            &mut entree,
            tampon.as_mut_ptr().cast(),
            tampon.len(),
            &mut resultat,
        )
    };
    if code != 0 || resultat.is_null() || entree.pw_dir.is_null() {
        return None;
    }
    let dir = unsafe { CStr::from_ptr(entree.pw_dir) };
    let dir = PathBuf::from(std::ffi::OsStr::from_bytes(dir.to_bytes()));
    (!dir.as_os_str().is_empty()).then_some(dir)
}

/// Ailleurs qu'en Unix, pas de bac à sable qui déplace le dossier : on garde
/// ce que le système annonce.
#[cfg(not(unix))]
pub fn vrai_dossier_personnel() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le bac à sable, simulé : `$HOME` pointé sur un conteneur. Le dossier
    /// rendu ne doit pas le suivre. Le test remet `$HOME` avant de juger,
    /// pour ne rien laisser derrière lui aux tests voisins.
    #[cfg(unix)]
    #[test]
    fn le_dossier_personnel_ne_suit_pas_home() {
        let avant = std::env::var_os("HOME");
        let conteneur = "/tmp/colophon-conteneur-simule/Data";
        std::env::set_var("HOME", conteneur);
        let lu = vrai_dossier_personnel();
        match avant {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        let lu = lu.expect("la base des comptes connaît l'utilisateur courant");
        assert_ne!(lu, PathBuf::from(conteneur));
        assert!(lu.is_absolute(), "{lu:?}");
    }
}
