//! PDF renderer built directly on lopdf. One spread per PDF page.
//! Images are embedded as JPEG (DCTDecode passthrough), cover-cropped
//! into their slot via a clip rectangle, anchored on the focal point.

use crate::model::{Album, Spread};
use anyhow::{Context, Result};
use lopdf::{dictionary, Document, Object, Stream};
use std::path::Path;

const MM_TO_PT: f64 = 72.0 / 25.4;

/// Geometry of one slot on the spread canvas, in millimetres,
/// origin bottom-left, bleed included.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct SpreadGeometry {
    pub media_w: f64,
    pub media_h: f64,
    /// White margin of margined templates, scaled to the page.
    pub margin: f64,
    /// Gap between two images, including across the fold.
    pub gutter: f64,
}

/// A resolved image ready to embed: raw JPEG bytes plus pixel size.
pub struct JpegAsset {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub focal: [f64; 2],
}

pub fn geometry(album: &Album) -> SpreadGeometry {
    // 14 mm on a 210 mm page, kept proportional so a 30 × 30 album does not
    // get a hairline margin and an A4 a fat one.
    let margin = album.trim_mm.w.min(album.trim_mm.h) * (14.0 / 210.0);
    SpreadGeometry {
        media_w: album.trim_mm.w * 2.0 + album.bleed_mm * 2.0,
        media_h: album.trim_mm.h + album.bleed_mm * 2.0,
        margin,
        gutter: margin / 2.0,
    }
}

/// The whole of one page, bleed included. Nothing ever spans both: an image
/// across the fold is swallowed by the binding.
fn full_page(right: bool, g: &SpreadGeometry) -> Rect {
    let half = g.media_w / 2.0;
    Rect { x: if right { half } else { 0.0 }, y: 0.0, w: half, h: g.media_h }
}

/// The margined content box of one page. Half a gutter is kept on the fold
/// side so two facing images do not kiss across the binding.
fn page_box(right: bool, g: &SpreadGeometry) -> Rect {
    let half = g.media_w / 2.0;
    let w = half - g.margin - g.gutter / 2.0;
    Rect {
        x: if right { half + g.gutter / 2.0 } else { g.margin },
        y: g.margin,
        w,
        h: g.media_h - 2.0 * g.margin,
    }
}

/// Cells of a grid inside a box, in reading order: top row first, left to right.
fn grid(b: Rect, cols: usize, rows: usize, gap: f64) -> Vec<Rect> {
    let cw = (b.w - (cols - 1) as f64 * gap) / cols as f64;
    let ch = (b.h - (rows - 1) as f64 * gap) / rows as f64;
    let mut out = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        // y grows upward, so the first row sits at the top
        let y = b.y + (rows - 1 - r) as f64 * (ch + gap);
        for c in 0..cols {
            out.push(Rect { x: b.x + c as f64 * (cw + gap), y, w: cw, h: ch });
        }
    }
    out
}

/// A cell of the given aspect ratio, centered in a box.
fn fitted(b: Rect, aspect: f64) -> Rect {
    let w = b.w.min(b.h * aspect);
    let h = w / aspect;
    Rect { x: b.x + (b.w - w) / 2.0, y: b.y + (b.h - h) / 2.0, w, h }
}

/// Every template the composer can emit, with the number of photos it holds.
/// The front end ports these geometries; `--dump-geometry` compares the two.
pub const TEMPLATES: &[(&str, usize)] = &[
    ("full1", 1),
    ("full1_verso", 1),
    ("solo", 1),
    ("solo_verso", 1),
    ("solo_paysage", 1),
    ("solo_paysage_verso", 1),
    ("duo", 2),
    ("trio", 3),
    ("trio_verso", 3),
    ("quad", 4),
    ("six", 6),
    ("six_verso", 6),
    ("octo", 8),
];

/// Every template's geometry for one page format, as JSON. Feeds the parity
/// check against the TypeScript port: two hand-written copies of the same
/// arithmetic drift silently otherwise.
pub fn dump_geometry(album: &Album) -> serde_json::Value {
    let g = geometry(album);
    let templates: serde_json::Map<String, serde_json::Value> = TEMPLATES
        .iter()
        .map(|(name, n)| {
            let rects = slots_for(name, *n, &g);
            let at = caption_anchor(&rects, &g);
            let slots: Vec<[f64; 4]> = rects.iter().map(|r| [r.x, r.y, r.w, r.h]).collect();
            (
                name.to_string(),
                serde_json::json!({ "slots": slots, "caption": [at.x, at.y] }),
            )
        })
        .collect();

    serde_json::json!({
        "trim_mm": { "w": album.trim_mm.w, "h": album.trim_mm.h },
        "bleed_mm": album.bleed_mm,
        "canvas": { "w": g.media_w, "h": g.media_h, "margin": g.margin, "gutter": g.gutter },
        "templates": templates,
    })
}

/// A point on the spread canvas, in millimetres, origin bottom-left.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Nominal size of the caption box, used to test whether a candidate spot is
/// clear. Generous on purpose: a caption half over a photo is still unreadable.
fn caption_box(at: Point, g: &SpreadGeometry) -> Rect {
    Rect { x: at.x, y: at.y - g.margin * 0.15, w: g.margin * 3.5, h: g.margin * 0.6 }
}

/// Where the chapter caption goes: the first spot no image covers, tried in
/// reading order. A caption printed over a full-bleed photo is unreadable,
/// and moving it costs nothing next to adding a plaque behind it.
pub fn caption_anchor(rects: &[Rect], g: &SpreadGeometry) -> Point {
    let half = g.media_w / 2.0;
    let low = g.margin * 0.36; // 5 mm on a 210 mm page
    let high = g.media_h - g.margin * 0.75;
    let left = g.margin * 0.57; // 8 mm
    let right = half + g.gutter / 2.0;

    let candidates = [
        Point { x: left, y: low },
        Point { x: right, y: low },
        Point { x: left, y: high },
        Point { x: right, y: high },
    ];
    for at in candidates {
        let b = caption_box(at, g);
        if rects.iter().all(|r| !overlaps(r, &b)) {
            return at;
        }
    }
    candidates[0]
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// Slot rectangles for a template, on the given spread canvas.
/// The `_verso` variants mirror the layout onto the other page; alternating
/// them is what keeps a long album from reading like a spreadsheet.
pub fn slots_for(template: &str, n: usize, g: &SpreadGeometry) -> Vec<Rect> {
    let verso = template.ends_with("_verso");
    // A verso template puts its lead image on the left page.
    let lead_right = !verso;
    let lead = page_box(lead_right, g);
    let facing = page_box(!lead_right, g);

    let mut v = match template.trim_end_matches("_verso") {
        // one photo, full bleed, on a single page
        "full1" => vec![full_page(lead_right, g)],
        // one photo, margined: portrait cell, then landscape cell
        "solo" => vec![fitted(lead, 0.75)],
        "solo_paysage" => vec![fitted(lead, 4.0 / 3.0)],
        // one per page, facing each other
        "duo" => {
            let (l, r) = (page_box(false, g), page_box(true, g));
            vec![l, r]
        }
        // a full page facing two stacked
        "trio" => {
            let stack = grid(facing, 1, 2, g.gutter);
            let mut v = Vec::with_capacity(3);
            if lead_right {
                v.extend(stack);
                v.push(full_page(true, g));
            } else {
                v.push(full_page(false, g));
                v.extend(stack);
            }
            v
        }
        // 2 x 2 across the spread, one column per page
        "quad" => {
            let (l, r) = (page_box(false, g), page_box(true, g));
            let (lc, rc) = (grid(l, 1, 2, g.gutter), grid(r, 1, 2, g.gutter));
            vec![lc[0], rc[0], lc[1], rc[1]]
        }
        // two stacked facing a four-up mosaic
        "six" => {
            let stack = grid(lead, 1, 2, g.gutter);
            let mosaic = grid(facing, 2, 2, g.gutter);
            let mut v = Vec::with_capacity(6);
            if lead_right {
                v.extend(mosaic);
                v.extend(stack);
            } else {
                v.extend(stack);
                v.extend(mosaic);
            }
            v
        }
        // eight-up: a four-up mosaic on each page
        "octo" => {
            let mut v = grid(page_box(false, g), 2, 2, g.gutter);
            v.extend(grid(page_box(true, g), 2, 2, g.gutter));
            v
        }
        _ => grid(page_box(false, g), 1, 1, g.gutter),
    };
    v.truncate(n.max(1));
    v
}

pub struct PdfWriter {
    doc: Document,
    page_ids: Vec<Object>,
    pages_id: lopdf::ObjectId,
    font_id: lopdf::ObjectId,
    geom: SpreadGeometry,
}

impl PdfWriter {
    pub fn new(album: &Album) -> Self {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
            "Encoding" => "WinAnsiEncoding",
        });
        Self { doc, page_ids: Vec::new(), pages_id, font_id, geom: geometry(album) }
    }

    pub fn add_spread(&mut self, spread: &Spread, assets: &[JpegAsset]) -> Result<()> {
        let rects = slots_for(&spread.template, assets.len(), &self.geom);
        let mut content = String::new();
        let mut xobjects = dictionary! {};

        for (i, (asset, rect)) in assets.iter().zip(rects.iter()).enumerate() {
            let img_id = self.doc.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => asset.width as i64,
                    "Height" => asset.height as i64,
                    "ColorSpace" => "DeviceRGB",
                    "BitsPerComponent" => 8,
                    "Filter" => "DCTDecode",
                },
                asset.data.clone(),
            ));
            let name = format!("Im{i}");
            xobjects.set(name.as_bytes(), Object::Reference(img_id));

            let (x, y, w, h) = (
                rect.x * MM_TO_PT,
                rect.y * MM_TO_PT,
                rect.w * MM_TO_PT,
                rect.h * MM_TO_PT,
            );
            // cover-crop: scale to fill, anchor on focal point, clip to slot
            let iw = asset.width as f64;
            let ih = asset.height as f64;
            let s = (w / iw).max(h / ih);
            let dw = iw * s;
            let dh = ih * s;
            let fx = asset.focal[0].clamp(0.0, 1.0);
            let fy = asset.focal[1].clamp(0.0, 1.0);
            let dx = x - (dw - w) * fx;
            // focal y is from top of the image; PDF y grows upward
            let dy = y - (dh - h) * (1.0 - fy);
            content.push_str(&format!(
                "q {x:.2} {y:.2} {w:.2} {h:.2} re W n {dw:.2} 0 0 {dh:.2} {dx:.2} {dy:.2} cm /{name} Do Q\n"
            ));
        }

        if let Some(caption) = &spread.caption {
            let at = caption_anchor(&rects, &self.geom);
            let cx = at.x * MM_TO_PT;
            let cy = at.y * MM_TO_PT;
            let text = winansi_escape(caption);
            content.push_str(&format!(
                "BT /F1 9 Tf 0.25 0.25 0.25 rg {cx:.2} {cy:.2} Td ({text}) Tj ET\n"
            ));
        }

        let content_id = self
            .doc
            .add_object(Stream::new(dictionary! {}, content.into_bytes()));

        let resources = dictionary! {
            "XObject" => xobjects,
            "Font" => dictionary! { "F1" => Object::Reference(self.font_id) },
        };
        let page_id = self.doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(self.pages_id),
            "MediaBox" => vec![
                0.into(),
                0.into(),
                (self.geom.media_w * MM_TO_PT).into(),
                (self.geom.media_h * MM_TO_PT).into(),
            ],
            "Resources" => resources,
            "Contents" => Object::Reference(content_id),
        });
        self.page_ids.push(Object::Reference(page_id));
        Ok(())
    }

    pub fn save(mut self, out: &Path) -> Result<()> {
        let count = self.page_ids.len() as i64;
        self.doc.objects.insert(
            self.pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => self.page_ids,
                "Count" => count,
            }),
        );
        let catalog_id = self.doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(self.pages_id),
        });
        self.doc.trailer.set("Root", catalog_id);
        self.doc.compress();
        self.doc.save(out).context("write pdf")?;
        Ok(())
    }
}

/// Encode a string for a PDF literal string in WinAnsi: ASCII stays as-is,
/// everything else becomes an octal escape so the content stream remains
/// pure bytes (never UTF-8 re-encoded).
fn winansi_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let b: u8 = match c {
            '(' => {
                out.push_str("\\(");
                continue;
            }
            ')' => {
                out.push_str("\\)");
                continue;
            }
            '\\' => {
                out.push_str("\\\\");
                continue;
            }
            c if c.is_ascii() => {
                out.push(c);
                continue;
            }
            'œ' => 0x9C,
            'Œ' => 0x8C,
            '€' => 0x80,
            '\u{2013}' => 0x96, // en dash
            '\u{2014}' => 0x97,
            '\u{2018}' => 0x91,
            '\u{2019}' => 0x92,
            '\u{201C}' => 0x93,
            '\u{201D}' => 0x94,
            '\u{2026}' => 0x85,
            c if (c as u32) < 256 => c as u8, // Latin-1 subset of WinAnsi
            _ => b'?',
        };
        out.push_str(&format!("\\{:03o}", b));
    }
    out
}
