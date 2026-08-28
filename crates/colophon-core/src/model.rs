//! Album data model. `album.json` is the project's interchange format:
//! human-readable, diffable, hand-repairable.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub version: u32,
    pub title: String,
    /// Absolute path of the scanned folder. Slot sources are relative to it,
    /// so an album reopened later still finds its photos.
    #[serde(default)]
    pub root: String,
    /// Trim size of a single page, in millimetres.
    pub trim_mm: Size,
    pub bleed_mm: f64,
    pub spreads: Vec<Spread>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Cover>,
    /// The pace the album was composed at, chosen at the first build. Kept
    /// in the file so a recomposition keeps it: an album that changed
    /// density halfway would rebuild itself around spreads the user pinned
    /// under the other pace. Absent on albums composed before the choice
    /// existed, and read as the default.
    #[serde(default, skip_serializing_if = "crate::layout::Densite::is_default")]
    pub densite: crate::layout::Densite,
    /// What the composition knew about the album, kept for the colophon
    /// page. Stored rather than recomputed: the counts, the span, the towns
    /// and the cameras all cost a full reopen of every original, and the
    /// facts do not change when the album is edited. Absent on albums
    /// composed before the page existed, and the page is then simply not
    /// offered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colophon: Option<crate::colophon::Faits>,
    /// Non-destructive adjustments, keyed by `Slot::src`. A property of the
    /// photograph, never of the cell: a photo on a spread and on the cover is
    /// adjusted once, a recomposition that rebuilds the spreads cannot lose
    /// it, and `reprise` — which reads `spreads` only — stays blind to it by
    /// construction. Applied where pixels are resolved (screen and export),
    /// never written to an original. An identity entry leaves the table at
    /// the edit that produced it, so absence means « no adjustment ».
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub reglages: std::collections::BTreeMap<String, Reglage>,
}

/// One photograph's adjustments: exposure, contrast, black and white.
/// The transform these numbers name is defined once, in [`crate::reglage`],
/// and it is the CSS filter formula — which is what lets the editor show it
/// with one line of style.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Reglage {
    /// Exposure, in stops of the CSS `brightness(2^expo)`, clamped to ±1:
    /// enough to rescue a shot, not a darkroom.
    #[serde(default)]
    pub expo: f64,
    /// Contrast, `contrast(2^contraste)` around the 0,5 pivot, clamped to ±1.
    #[serde(default)]
    pub contraste: f64,
    /// Black and white: luma 709, the coefficients of `grayscale(1)`.
    #[serde(default)]
    pub nb: bool,
}

impl Reglage {
    /// The adjustment that adjusts nothing. Never stored: an identity entry
    /// leaves the table at the edit that produced it.
    pub fn est_identite(&self) -> bool {
        self.expo == 0.0 && self.contraste == 0.0 && !self.nb
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spread {
    pub template: String,
    pub slots: Vec<Slot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Free text of a `texte` spread. Lines are printed as typed: the editor
    /// signals overlong lines, nothing is ever wrapped or cut silently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Touched by hand in the editor. A recomposition never rebuilds it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub edited: bool,
    /// Pinned by the user without being edited. Same recomposition shield.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

impl Spread {
    /// Survives a recomposition untouched.
    pub fn pinned(&self) -> bool {
        self.edited || self.locked
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slot {
    /// Path relative to the album root (the scanned folder).
    pub src: String,
    /// Focal point in [0,1]x[0,1], (0,0) = top-left. Cover-crop anchors here.
    pub focal: [f64; 2],
    /// Manual zoom past the cover fill, 1.0 = exact fill. Albums from before
    /// the crop editor carry no field and read as 1.0.
    #[serde(default = "default_zoom", skip_serializing_if = "is_default_zoom")]
    pub zoom: f64,
    /// Caption printed under the photo. Absent = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

impl Slot {
    pub fn new(src: String, focal: [f64; 2]) -> Self {
        Self { src, focal, zoom: 1.0, caption: None }
    }
}

/// The book's cover. Absent on albums composed before the cover editor;
/// the interface then seeds it from the album title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cover {
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subtitle: String,
    /// Front-cover photo, cropped like any slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<Slot>,
    /// Back-cover text (the quatrième de couverture), optional.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub back_text: String,
}

pub fn default_zoom() -> f64 {
    1.0
}

/// The `album.json` schema this build writes.
///
/// 1 read `focal` as a fraction of the leftover room inside the cell, which
/// is cell-dependent: the same number showed a different part of the photo
/// as soon as the format changed, and a bascule therefore destroyed manual
/// work in silence. 2 reads it as a point of the image, which is a property
/// of the photograph and survives any ratio.
pub const SCHEMA: u32 = 2;

/// One schema-1 `focal`, converted into a schema-2 `focal`.
///
/// A schema-1 focal placed the window at `x0 = (iw - vw) · focal`, so the
/// point of the image that window was centred on is `(x0 + vw/2) / iw`.
/// Written with `r = vw / iw` that is `focal · (1 - r) + r/2`, and `r` needs
/// no pixel count at all — only the two aspect ratios and the zoom:
///
/// ```text
/// rx = min(1, cellule / image) / zoom      ry = min(1, image / cellule) / zoom
/// ```
///
/// A thumbnail therefore answers as well as the original, its aspect ratio
/// being the original's to within one rounded pixel — about 0,03 % on a
/// 1600 px box, which moves a focal by 2·10⁻⁴.
///
/// There is no case to write for a photo with no room: `r` is 1 there, and
/// the formula gives 0,5 — the centre, which is exactly what "the whole axis
/// shows" means. The value stored under schema 1 was dead data; this reads
/// it as what it always meant.
pub fn point_from_room(
    cell_ratio: f64,
    image_ratio: f64,
    focal: [f64; 2],
    zoom: f64,
) -> [f64; 2] {
    if !(cell_ratio > 0.0) || !(image_ratio > 0.0) {
        return focal;
    }
    let z = zoom.max(1.0);
    let rx = ((cell_ratio / image_ratio).min(1.0) / z).clamp(0.0, 1.0);
    let ry = ((image_ratio / cell_ratio).min(1.0) / z).clamp(0.0, 1.0);
    [
        focal[0].clamp(0.0, 1.0) * (1.0 - rx) + rx / 2.0,
        focal[1].clamp(0.0, 1.0) * (1.0 - ry) + ry / 2.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An album.json written before S2 (no zoom, no captions per photo, no
    /// edited/locked flags, no cover) must open unchanged: zoom reads 1.0
    /// and every new field defaults quiet.
    #[test]
    fn pre_s2_album_json_still_opens() {
        let json = r#"{
            "version": 1,
            "title": "corse",
            "root": "/photos/corse",
            "trim_mm": { "w": 210.0, "h": 210.0 },
            "bleed_mm": 3.0,
            "spreads": [{
                "template": "duo",
                "slots": [
                    { "src": "a.jpg", "focal": [0.5, 0.42] },
                    { "src": "b.jpg", "focal": [0.2, 0.5] }
                ],
                "caption": "12 mars 2013"
            }]
        }"#;
        let album: Album = serde_json::from_str(json).expect("ancien album lisible");
        let slot = &album.spreads[0].slots[0];
        assert_eq!(slot.zoom, 1.0);
        assert!(slot.caption.is_none());
        assert!(!album.spreads[0].pinned());
        assert!(album.cover.is_none());

        // And the new fields stay off the file until they carry something:
        // album.json remains diffable across the schema change.
        let out = serde_json::to_string(&album).unwrap();
        assert!(!out.contains("zoom"));
        assert!(!out.contains("edited"));
        assert!(!out.contains("locked"));
        assert!(!out.contains("cover"));
        assert!(!out.contains("reglages"));
    }

    /// The adjustments table survives the round trip, and an album without
    /// one writes no field at all: `reglages` is additive, absence means
    /// « no adjustment », the schema stays at 2.
    #[test]
    fn reglages_round_trip_et_absence_muette() {
        let mut album = Album::new("t", std::path::Path::new("/p"), Size { w: 210.0, h: 210.0 });
        assert!(album.reglages.is_empty());
        album.reglages.insert(
            "a.jpg".into(),
            Reglage { expo: 0.5, contraste: -0.25, nb: true },
        );
        let back: Album =
            serde_json::from_str(&serde_json::to_string(&album).unwrap()).unwrap();
        let r = back.reglages.get("a.jpg").expect("l'entrée survit");
        assert_eq!(r.expo, 0.5);
        assert_eq!(r.contraste, -0.25);
        assert!(r.nb);
        // A hand-repaired entry may name one field only: the others default.
        let partiel: Reglage = serde_json::from_str(r#"{ "nb": true }"#).unwrap();
        assert_eq!(partiel, Reglage { expo: 0.0, contraste: 0.0, nb: true });
        assert!(Reglage { expo: 0.0, contraste: 0.0, nb: false }.est_identite());
        assert!(!partiel.est_identite());
    }

    /// La conversion d'un focal de schéma 1 vers le schéma 2 se lit sur la
    /// fenêtre : le point rendu doit être celui sur lequel l'ancienne fenêtre
    /// était centrée. On le vérifie en refaisant les deux arithmétiques à la
    /// main, sans passer par crop_window, pour que les deux se surveillent.
    #[test]
    fn le_focal_migre_tombe_au_centre_de_lancienne_fenetre() {
        let (iw, ih) = (4000.0_f64, 3000.0_f64);
        let (rw, rh) = (300.0_f64, 200.0_f64);
        for zoom in [1.0_f64, 1.6, 3.0] {
            for f in [[0.0, 0.0], [0.42, 0.5], [1.0, 1.0], [0.73, 0.18]] {
                let s = (rw / iw).max(rh / ih) * zoom;
                let (vw, vh) = (rw / s, rh / s);
                // Ce que le schéma 1 montrait.
                let x0 = ((iw - vw) * f[0]).clamp(0.0, (iw - vw).max(0.0));
                let y0 = ((ih - vh) * f[1]).clamp(0.0, (ih - vh).max(0.0));
                let attendu = [(x0 + vw / 2.0) / iw, (y0 + vh / 2.0) / ih];

                let got = point_from_room(rw / rh, iw / ih, f, zoom);
                assert!(
                    (got[0] - attendu[0]).abs() < 1e-9,
                    "zoom {zoom} focal {f:?} : x {} attendu {}", got[0], attendu[0]
                );
                assert!(
                    (got[1] - attendu[1]).abs() < 1e-9,
                    "zoom {zoom} focal {f:?} : y {} attendu {}", got[1], attendu[1]
                );
            }
        }
    }

    /// Sans jeu, la valeur du schéma 1 ne voulait rien dire et n'était lue par
    /// personne. La migration rend le centre, et n'a aucun cas particulier.
    #[test]
    fn sans_jeu_la_migration_rend_le_centre() {
        // Cellule au ratio exact de l'image, zoom 1 : aucun jeu sur aucun axe.
        let got = point_from_room(4.0 / 3.0, 4.0 / 3.0, [0.13, 0.87], 1.0);
        assert!((got[0] - 0.5).abs() < 1e-12, "{got:?}");
        assert!((got[1] - 0.5).abs() < 1e-12, "{got:?}");
    }

    /// Un album neuf porte le schéma courant, jamais un littéral. Sans cette
    /// assertion, `Album::new` peut retomber sur un `1` et une composition
    /// toute neuve se ferait convertir au premier rendu comme si elle portait
    /// l'ancien sens de `focal`.
    #[test]
    fn un_album_neuf_porte_le_schema_courant() {
        let a = Album::new(
            "t",
            std::path::Path::new("/p"),
            Size { w: 210.0, h: 210.0 },
        );
        assert_eq!(a.version, SCHEMA);
    }

    /// A manual crop survives the round trip.
    #[test]
    fn zoomed_slot_round_trips() {
        let mut album = Album::new("t", std::path::Path::new("/p"), Size { w: 210.0, h: 210.0 });
        let mut slot = Slot::new("a.jpg".into(), [0.3, 0.6]);
        slot.zoom = 1.8;
        slot.caption = Some("la plage".into());
        album.spreads.push(Spread {
            template: "solo".into(),
            slots: vec![slot],
            caption: None,
            text: None,
            edited: true,
            locked: false,
        });
        let back: Album =
            serde_json::from_str(&serde_json::to_string(&album).unwrap()).unwrap();
        assert_eq!(back.spreads[0].slots[0].zoom, 1.8);
        assert_eq!(back.spreads[0].slots[0].caption.as_deref(), Some("la plage"));
        assert!(back.spreads[0].edited);
    }
}

fn is_default_zoom(z: &f64) -> bool {
    (*z - 1.0).abs() < 1e-9
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One photo the curation set aside, and why. `curation.json` holds the
/// full list next to album.json: it is the sorting view's input, everything
/// the album does not show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discard {
    /// Path relative to the album root, like `Slot::src`.
    pub src: String,
    /// Machine-readable reason: `rejetee`, `parasite`, `panorama`,
    /// `definition`, `doublon`, `jumeau`, `meme_moment`, `hors_budget`.
    pub reason: String,
    /// The photo that won over this one, when the drop came from a
    /// comparison. Relative to the album root too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
    /// Focal point, same convention as `Slot::focal`, so a rescued photo is
    /// cropped like any other. Face anchor when one was found.
    #[serde(default = "default_focal")]
    pub focal: [f64; 2],
}

/// Same default as the composer: slightly above centre.
pub fn default_focal() -> [f64; 2] {
    [0.5, 0.42]
}

impl Album {
    pub fn new(title: &str, root: &std::path::Path, trim_mm: Size) -> Self {
        Self {
            // Le schéma courant, jamais un littéral : un album neuf estampillé
            // 1 serait « migré » au premier rendu, donc converti comme s'il
            // portait l'ancien sens de `focal`. Le pire des deux mondes.
            version: SCHEMA,
            title: title.to_string(),
            root: root.to_string_lossy().to_string(),
            trim_mm,
            bleed_mm: 3.0,
            spreads: Vec::new(),
            cover: None,
            densite: crate::layout::Densite::default(),
            colophon: None,
            reglages: std::collections::BTreeMap::new(),
        }
    }

    /// Portrait, landscape or square page. Drives the template choice.
    pub fn page_aspect(&self) -> f64 {
        self.trim_mm.w / self.trim_mm.h
    }
}
