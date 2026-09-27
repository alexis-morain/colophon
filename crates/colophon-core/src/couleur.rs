//! La couleur d'un objet libre, telle qu'`album.json` l'écrit : `#rrggbb`.
//!
//! Une chaîne et pas trois nombres, parce que le fichier se répare à la main
//! et qu'une couleur s'y lit comme on la tape dans n'importe quel outil. La
//! lecture est ici, une fois : l'émetteur, le linter et la scène passent tous
//! par [`parse`], et rien d'autre ne découpe la chaîne.

/// Les trois composantes d'une couleur `#rrggbb`, entre 0 et 1, ou `None`.
///
/// `None` sur tout ce qui n'est pas exactement un dièse et six chiffres
/// hexadécimaux : `#abc`, `rouge`, `#12345g`, une chaîne vide. Jamais une
/// panique, parce que la chaîne vient d'un fichier qu'une main a pu toucher.
/// L'appelant retombe alors sur la couleur par défaut de l'objet, et l'album
/// s'exporte.
pub fn parse(s: &str) -> Option<[f64; 3]> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let composante = |i: usize| -> Option<f64> {
        u8::from_str_radix(hex.get(i..i + 2)?, 16).ok().map(|v| f64::from(v) / 255.0)
    };
    Some([composante(0)?, composante(2)?, composante(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_hexadecimaux_donnent_trois_composantes() {
        assert_eq!(parse("#000000"), Some([0.0, 0.0, 0.0]));
        assert_eq!(parse("#ffffff"), Some([1.0, 1.0, 1.0]));
        assert_eq!(parse("#FF0000"), Some([1.0, 0.0, 0.0]));
        let [r, g, b] = parse("#c0604a").unwrap();
        assert_eq!((r * 255.0).round(), 192.0);
        assert_eq!((g * 255.0).round(), 96.0);
        assert_eq!((b * 255.0).round(), 74.0);
    }

    /// Tout le reste rend `None`, sans paniquer : la chaîne vient d'un
    /// fichier réparable à la main. Le multi-octet est là exprès, un découpage
    /// par indices d'octets y casserait une frontière de caractère.
    #[test]
    fn tout_le_reste_rend_none() {
        for s in ["", "#", "000000", "#abc", "#1234567", "#12345g", "rouge", "#ééé", "# 12345", "#-12345"] {
            assert_eq!(parse(s), None, "{s:?}");
        }
    }
}
