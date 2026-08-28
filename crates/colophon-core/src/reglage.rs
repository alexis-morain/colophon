//! The one definition of what an adjustment does to a pixel.
//!
//! The transform is the CSS filter formula, deliberately: Filter Effects
//! Level 1 writes exact arithmetic for `grayscale()`, `brightness()` and
//! `contrast()`, applied in sRGB, with a clamp between primitives. A core
//! that implements those formulas makes the editor's DOM preview exact for
//! one line of style, and `reglage.ts` is the byte-for-byte port of this
//! file — the parity test holds the two to the same committed LUT dump.
//!
//! The chain is **grey → exposure → contrast**, because exposure and
//! contrast are judged on the black and white once it was asked for. Grey is
//! luma 709 (the coefficients of `grayscale(1)`); exposure is
//! `brightness(2^expo)`; contrast is `contrast(2^contraste)` around the 0,5
//! pivot. Everything runs in f64 and rounds to u8 exactly once at the end:
//! composing two u8 tables (exposure then contrast) would quantise twice and
//! drift from the float-working CSS filter by an octet here and there.

use crate::model::Reglage;

/// Hard bounds of both sliders: enough to rescue a shot, not a darkroom.
/// The editor clamps too; this one holds for hand-repaired `album.json`.
pub const BORNE: f64 = 1.0;

/// Luma 709: the exact coefficients of CSS `grayscale(1)`, applied to sRGB
/// values as the filter does.
const LUMA_R: f64 = 0.2126;
const LUMA_G: f64 = 0.7152;
const LUMA_B: f64 = 0.0722;

/// The mono-channel transfer, on [0,1], clamped after each step like CSS
/// clamps between primitives. This is the definition; `lut` below is its
/// table over the 256 u8 inputs.
fn transfert(v: f64, expo: f64, contraste: f64) -> f64 {
    let b = 2f64.powf(expo.clamp(-BORNE, BORNE));
    let v = (v * b).clamp(0.0, 1.0);
    let c = 2f64.powf(contraste.clamp(-BORNE, BORNE));
    ((v - 0.5) * c + 0.5).clamp(0.0, 1.0)
}

/// The 256-entry table of one adjustment's exposure and contrast, computed
/// in float as one block. `nb` is not in it: grey is a per-pixel mix of the
/// three channels, so it happens before this table (or before the float
/// transfer, for grey values that never sat on the u8 grid).
pub fn lut(r: &Reglage) -> [u8; 256] {
    let mut out = [0u8; 256];
    for (k, o) in out.iter_mut().enumerate() {
        *o = (transfert(k as f64 / 255.0, r.expo, r.contraste) * 255.0).round() as u8;
    }
    out
}

/// Apply one adjustment to decoded pixels, in place.
///
/// Colour stays colour: a channel is already on the u8 grid, so the LUT is
/// exact — one rounding, at the table's own edge. Black and white mixes the
/// channels first, in float, and runs the float transfer directly: the grey
/// never sits on the u8 grid, and quantising it before the transfer would be
/// the intermediate rounding this module exists to refuse. The result is a
/// three-channel image either way — an adjusted JPEG stays RGB with equal
/// channels, the PDF stays DeviceRGB.
pub fn appliquer(img: &mut image::RgbImage, r: &Reglage) {
    if r.nb {
        for p in img.pixels_mut() {
            let gris = (LUMA_R * f64::from(p.0[0])
                + LUMA_G * f64::from(p.0[1])
                + LUMA_B * f64::from(p.0[2]))
                / 255.0;
            let v = (transfert(gris.clamp(0.0, 1.0), r.expo, r.contraste) * 255.0).round() as u8;
            p.0 = [v, v, v];
        }
    } else {
        let table = lut(r);
        for p in img.pixels_mut() {
            p.0 = [
                table[usize::from(p.0[0])],
                table[usize::from(p.0[1])],
                table[usize::from(p.0[2])],
            ];
        }
    }
}

/// Decode a JPEG, apply one adjustment, re-encode. The preview renders
/// (`album.pdf`, the editor's re-render) resolve their pixels from cached
/// thumbnails; an adjusted thumbnail goes through here on its way into the
/// PDF. The cache itself stays untouched: it feeds the analysis, and an
/// adjusted cache would move fiches and curation.
pub fn regler_jpeg(data: &[u8], r: &Reglage, quality: u8) -> anyhow::Result<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg)?;
    let mut rgb = img.to_rgb8();
    appliquer(&mut rgb, r);
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&rgb)?;
    Ok((out, rgb.width(), rgb.height()))
}

/// The LUT over a fixed grid of adjustments, bounds included, as JSON. The
/// committed fixture of the TS parity test is this output, like the geometry
/// and the scene dumps: stale fixture, stale tests.
pub fn dump_luts() -> serde_json::Value {
    let pas = [-1.0, -0.5, -0.25, 0.0, 0.25, 0.5, 1.0];
    let mut grille = Vec::new();
    for expo in pas {
        for contraste in pas {
            let r = Reglage { expo, contraste, nb: false };
            grille.push(serde_json::json!({
                "expo": expo,
                "contraste": contraste,
                "lut": lut(&r).to_vec(),
            }));
        }
    }
    serde_json::json!({ "borne": BORNE, "grille": grille })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default adjustment is the identity to the byte: LUT entry k = k.
    /// This is what lets an absent entry and a present-but-default entry
    /// mean the same picture.
    #[test]
    fn la_lut_par_defaut_est_lidentite() {
        let table = lut(&Reglage { expo: 0.0, contraste: 0.0, nb: false });
        for (k, v) in table.iter().enumerate() {
            assert_eq!(usize::from(*v), k, "entrée {k}");
        }
    }

    /// Spot values recomputed by hand from the CSS formulas, so the code and
    /// the test cannot share a mistake.
    #[test]
    fn la_formule_est_celle_du_filtre_css() {
        // brightness(2): 128/255 × 2 clamps to 1.
        assert_eq!(lut(&Reglage { expo: 1.0, contraste: 0.0, nb: false })[128], 255);
        // brightness(√2): 128/255 × 1,41421… × 255 = 181,02 → 181.
        assert_eq!(lut(&Reglage { expo: 0.5, contraste: 0.0, nb: false })[128], 181);
        // contrast(2) around the pivot: (64/255 − 0,5) × 2 + 0,5 = 0,00196…
        // × 255 = 0,4998 → 0.
        let c = lut(&Reglage { expo: 0.0, contraste: 1.0, nb: false });
        assert_eq!(c[64], 0);
        // (200/255 − 0,5) × 2 + 0,5 = 1,068… clamps to 1.
        assert_eq!(c[200], 255);
        // …and the clamp between primitives bites: exposure first saturates,
        // then contrast works on the clamped value, not the raw product.
        let deux = lut(&Reglage { expo: 1.0, contraste: -1.0, nb: false });
        // 200/255 × 2 clamps to 1 ; (1 − 0,5) × 0,5 + 0,5 = 0,75 → 191.
        assert_eq!(deux[200], 191);
        // Out-of-bounds values from a hand-repaired file clamp to ±1.
        assert_eq!(
            lut(&Reglage { expo: 4.0, contraste: 0.0, nb: false }),
            lut(&Reglage { expo: 1.0, contraste: 0.0, nb: false }),
        );
    }

    /// Black and white is luma 709 in float: the grey of a saturated pixel
    /// is the CSS mix, not an average, and the channels come out equal.
    #[test]
    fn le_noir_et_blanc_est_le_luma_709() {
        let mut img = image::RgbImage::from_pixel(2, 1, image::Rgb([255, 0, 0]));
        appliquer(&mut img, &Reglage { expo: 0.0, contraste: 0.0, nb: true });
        // 0,2126 × 255 = 54,2 → 54, on all three channels.
        assert_eq!(img.get_pixel(0, 0).0, [54, 54, 54]);

        // The grey runs through the same transfer as everything else, with
        // no intermediate u8 between the mix and the exposure. Green 100:
        // luma 0,28047, ×√2 = 0,39665 → 101,1 → 101. A grey quantised
        // before the transfer (71,52 → 72) would land on 102: this value is
        // the one that tells the two pipelines apart.
        let mut img = image::RgbImage::from_pixel(1, 1, image::Rgb([0, 100, 0]));
        appliquer(&mut img, &Reglage { expo: 0.5, contraste: 0.0, nb: true });
        assert_eq!(img.get_pixel(0, 0).0, [101, 101, 101]);
    }

    /// An adjusted JPEG stays a 3-component RGB file: the passthrough test
    /// of `print.rs` reads `(w, h, 3)`, and the PDF declares DeviceRGB. No
    /// colour-model change to save octets.
    #[test]
    fn un_reglage_rend_du_rgb_a_trois_composantes() {
        let img = image::RgbImage::from_pixel(20, 10, image::Rgb([90, 120, 150]));
        let mut data = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, 92)
            .encode_image(&img)
            .unwrap();
        let (regle, w, h) =
            regler_jpeg(&data, &Reglage { expo: 0.0, contraste: 0.0, nb: true }, 92).unwrap();
        assert_eq!((w, h), (20, 10));
        let back = image::load_from_memory(&regle).unwrap().to_rgb8();
        let p = back.get_pixel(10, 5).0;
        assert_eq!(p[0], p[1]);
        assert_eq!(p[1], p[2]);
    }
}
