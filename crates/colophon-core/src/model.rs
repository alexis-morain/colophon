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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slot {
    /// Path relative to the album root (the scanned folder).
    pub src: String,
    /// Focal point in [0,1]x[0,1], (0,0) = top-left. Cover-crop anchors here.
    pub focal: [f64; 2],
}

/// One photo the curation set aside, and why. `curation.json` holds the
/// full list next to album.json: it is the sorting view's input, everything
/// the album does not show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discard {
    /// Path relative to the album root, like `Slot::src`.
    pub src: String,
    /// Machine-readable reason: `parasite`, `doublon`, `jumeau`,
    /// `meme_moment`, `hors_budget`.
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
        }
    }

    /// Portrait, landscape or square page. Drives the template choice.
    pub fn page_aspect(&self) -> f64 {
        self.trim_mm.w / self.trim_mm.h
    }
}
