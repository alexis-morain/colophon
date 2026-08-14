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
    /// Machine-readable reason: `parasite`, `panorama`, `definition`,
    /// `doublon`, `jumeau`, `meme_moment`, `hors_budget`.
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
            version: 1,
            title: title.to_string(),
            root: root.to_string_lossy().to_string(),
            trim_mm,
            bleed_mm: 3.0,
            spreads: Vec::new(),
            cover: None,
            densite: crate::layout::Densite::default(),
        }
    }

    /// Portrait, landscape or square page. Drives the template choice.
    pub fn page_aspect(&self) -> f64 {
        self.trim_mm.w / self.trim_mm.h
    }
}
