//! Colophon engine. Everything between a folder of photos and a rendered
//! album lives here: scanning, metadata, curation, layout, PDF. The CLI and
//! the desktop app are both thin shells over this crate.

pub mod analyze;
pub mod audit;
pub mod build;
pub mod face;
pub mod format;
pub mod heic;
pub mod layout;
pub mod meta;
pub mod model;
pub mod pdf;
pub mod pipeline;
pub mod print;
pub mod scan;
pub mod thumb;

pub use build::{build_album, render_album_pdf, write_album_json, BuildOptions, BuildReport};
pub use print::render_print_pdf;
pub use model::{Album, Size, Slot, Spread};
// The shells reason about capture times (pinned-spread anchors) and decode
// thumbnails (detected-focal lookups) without carrying their own pins.
pub use chrono;
pub use image;
