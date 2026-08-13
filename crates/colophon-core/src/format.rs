//! Page formats. The trim size is chosen once, before the album is composed:
//! it drives the margins, the templates and the printer's price list.

use crate::model::Size;
use anyhow::{bail, Result};

/// Named formats, in millimetres, trim size of a single page.
pub const FORMATS: &[(&str, f64, f64, &str)] = &[
    ("carre-21", 210.0, 210.0, "carré 21 × 21, le format d'album courant"),
    ("carre-30", 300.0, 300.0, "carré 30 × 30, grand format de table"),
    ("portrait-a4", 210.0, 297.0, "A4 portrait"),
    ("paysage-a4", 297.0, 210.0, "A4 paysage"),
    ("paysage-28x21", 280.0, 210.0, "paysage 28 × 21"),
    ("portrait-20x25", 203.0, 254.0, "portrait 20 × 25, le 8 × 10 pouces"),
];

/// Accepts a preset name or a raw `LARGEURxHAUTEUR` in millimetres.
pub fn parse(spec: &str) -> Result<Size> {
    let key = spec.trim().to_lowercase();
    if let Some((_, w, h, _)) = FORMATS.iter().find(|(name, ..)| *name == key) {
        return Ok(Size { w: *w, h: *h });
    }
    if let Some((w, h)) = key.split_once(['x', '*']) {
        if let (Ok(w), Ok(h)) = (w.trim().parse::<f64>(), h.trim().parse::<f64>()) {
            if w >= 80.0 && h >= 80.0 && w <= 500.0 && h <= 500.0 {
                return Ok(Size { w, h });
            }
            bail!("format hors bornes : chaque côté doit tenir entre 80 et 500 mm");
        }
    }
    bail!("format inconnu « {spec} ». Disponibles : {}", names())
}

pub fn names() -> String {
    FORMATS
        .iter()
        .map(|(n, ..)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Help text listing every preset with its dimensions.
pub fn help() -> String {
    FORMATS
        .iter()
        .map(|(n, w, h, about)| format!("  {n:<15} {w:.0} × {h:.0} mm, {about}"))
        .collect::<Vec<_>>()
        .join("\n")
}
