//! The destination colour profile, embedded in every exported PDF.
//!
//! A PDF/X file says which colour it was made for, and says it by carrying
//! the profile rather than by naming it: `sRGB IEC61966-2.1` as a string is
//! an assertion, an OutputIntent with a `DestOutputProfile` is a measurement
//! anyone downstream can redo. Same reasoning as the face in [`crate::font`]
//! — what the file needs travels inside the file.
//!
//! sRGB2014.icc, published by the International Color Consortium and
//! redistributable without restriction (see `assets/sRGB2014-LICENSE.md`).
//! It is the ICC's own v2 encoding of sRGB IEC 61966-2.1, which is exactly
//! what [`crate::printer::Espace::Rgb`] names.
//!
//! The header is read by hand, the way [`crate::font`] reads the TrueType
//! tables: an ICC profile is a 128-byte header of big-endian fields at known
//! offsets, and the point here is not to interpret colour but to refuse an
//! asset that is not the profile we think it is.

use anyhow::{ensure, Result};

/// The profile itself, ~3 kB, embedded whole and unmodified. The licence only
/// constrains altered copies, so this file is never rewritten: it goes into
/// the PDF byte for byte.
pub const ICC_DATA: &[u8] = include_bytes!("../assets/sRGB2014.icc");

/// The name the OutputIntent carries, and the one printers recognise. Kept
/// next to the bytes so the string and the profile cannot drift apart.
pub const CONDITION: &str = "sRGB IEC61966-2.1";

/// The ICC registry, for `/RegistryName`.
pub const REGISTRY: &str = "http://www.color.org";

/// What the PDF needs to know about the profile.
#[derive(Debug, Clone, Copy)]
pub struct Header {
    /// Size the profile declares for itself, which must match the asset.
    pub size: u32,
    /// Major version. PDF/X-4 and PDF/A-2 both refuse a v5 profile.
    pub version_major: u8,
    /// `mntr`, `prtr`, `scnr`, `spac`…
    pub class: [u8; 4],
    /// Data colour space: `RGB `, `CMYK`, `GRAY`.
    pub space: [u8; 4],
    /// Profile connection space.
    pub pcs: [u8; 4],
}

impl Header {
    /// Number of colorants, for the stream's `/N`. A reader needs it to size
    /// the alternate space without parsing the profile.
    pub fn components(&self) -> Result<i64> {
        Ok(match &self.space {
            b"GRAY" => 1,
            b"RGB " => 3,
            b"CMYK" => 4,
            other => anyhow::bail!(
                "espace couleur {} non géré par l'export",
                String::from_utf8_lossy(other)
            ),
        })
    }
}

/// Read the bundled profile.
pub fn header() -> Result<Header> {
    parse(ICC_DATA)
}

fn parse(d: &[u8]) -> Result<Header> {
    ensure!(d.len() >= 132, "profil ICC tronqué : {} octets", d.len());
    // Bytes 36..40 are the mandatory `acsp` signature. Its absence means the
    // asset is not an ICC profile at all — an HTML error page saved under the
    // right name, which is exactly how this one nearly shipped.
    ensure!(&d[36..40] == b"acsp", "signature acsp absente : ce n'est pas un profil ICC");

    let size = u32::from_be_bytes([d[0], d[1], d[2], d[3]]);
    ensure!(
        size as usize == d.len(),
        "le profil annonce {size} octets et en fait {}",
        d.len()
    );

    Ok(Header {
        size,
        version_major: d[8],
        class: [d[12], d[13], d[14], d[15]],
        space: [d[16], d[17], d[18], d[19]],
        pcs: [d[20], d[21], d[22], d[23]],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled asset really is the sRGB profile the OutputIntent claims.
    /// A swap that put a v5 profile, a CMYK one, or a downloaded HTML page in
    /// its place breaks here rather than at a printer.
    #[test]
    fn the_bundled_profile_is_srgb_v2() {
        let h = header().expect("profil lisible");
        assert_eq!(&h.class, b"mntr", "classe {:?}", h.class);
        assert_eq!(&h.space, b"RGB ", "espace {:?}", h.space);
        assert_eq!(&h.pcs, b"XYZ ");
        assert!(h.version_major <= 4, "version ICC {}", h.version_major);
        assert_eq!(h.components().unwrap(), 3);
        assert_eq!(h.size as usize, ICC_DATA.len());
    }

    /// The description tag names the profile in clear text, and the copyright
    /// tag is the one the licence file quotes. Both travel in the PDF, so
    /// both are worth checking before they do.
    #[test]
    fn the_profile_carries_its_identification() {
        let s = String::from_utf8_lossy(ICC_DATA);
        assert!(s.contains("sRGB2014"), "description absente");
        assert!(s.contains("International Color Consortium"), "copyright absent");
    }

    /// A truncated or mislabelled profile is refused, not embedded.
    #[test]
    fn a_broken_profile_is_refused() {
        assert!(parse(b"<!DOCTYPE html>").is_err());
        let mut bad = ICC_DATA.to_vec();
        bad[36] = b'x';
        assert!(parse(&bad).is_err(), "signature acsp non vérifiée");
        let short = &ICC_DATA[..ICC_DATA.len() - 1];
        assert!(parse(short).is_err(), "taille non vérifiée");
    }
}
