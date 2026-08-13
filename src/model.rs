//! Album data model. `album.json` is the project's interchange format:
//! human-readable, diffable, hand-repairable.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub version: u32,
    pub title: String,
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

impl Album {
    pub fn new(title: &str) -> Self {
        Self {
            version: 1,
            title: title.to_string(),
            trim_mm: Size { w: 210.0, h: 210.0 },
            bleed_mm: 3.0,
            spreads: Vec::new(),
        }
    }
}
