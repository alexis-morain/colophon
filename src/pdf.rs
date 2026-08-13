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
}

/// A resolved image ready to embed: raw JPEG bytes plus pixel size.
pub struct JpegAsset {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub focal: [f64; 2],
}

pub fn geometry(album: &Album) -> SpreadGeometry {
    SpreadGeometry {
        media_w: album.trim_mm.w * 2.0 + album.bleed_mm * 2.0,
        media_h: album.trim_mm.h + album.bleed_mm * 2.0,
    }
}

/// Slot rectangles for a template, on the given spread canvas.
pub fn slots_for(template: &str, n: usize, g: &SpreadGeometry) -> Vec<Rect> {
    let (bw, bh) = (g.media_w, g.media_h);
    let margin = 14.0; // inner white margin for margined templates
    let gutter = 7.0;
    let half = bw / 2.0;

    match template {
        "hero" => vec![Rect { x: 0.0, y: 0.0, w: bw, h: bh }],
        "full2_single" => {
            // single landscape on the right page, left page stays white
            vec![Rect { x: half, y: 0.0, w: half, h: bh }]
        }
        "solo" => {
            // centered portrait with generous margins, on the right page
            let w = half - 2.0 * margin;
            let h = bh - 2.0 * margin;
            let w = w.min(h * 0.75);
            vec![Rect { x: half + (half - w) / 2.0, y: margin, w, h: bh - 2.0 * margin }]
        }
        "duo" => {
            let w = (bw - 2.0 * margin - gutter) / 2.0;
            let h = bh - 2.0 * margin;
            vec![
                Rect { x: margin, y: margin, w, h },
                Rect { x: margin + w + gutter, y: margin, w, h },
            ]
        }
        "trio" => {
            // left page full bleed, right page two stacked with margins
            let rh = (bh - 2.0 * margin - gutter) / 2.0;
            let rw = half - margin - margin;
            vec![
                Rect { x: 0.0, y: 0.0, w: half, h: bh },
                Rect { x: half + margin, y: margin + rh + gutter, w: rw, h: rh },
                Rect { x: half + margin, y: margin, w: rw, h: rh },
            ]
        }
        _ => {
            // quad: 2x2 margined grid
            let w = (bw - 2.0 * margin - gutter) / 2.0;
            let h = (bh - 2.0 * margin - gutter) / 2.0;
            let mut v = vec![
                Rect { x: margin, y: margin + h + gutter, w, h },
                Rect { x: margin + w + gutter, y: margin + h + gutter, w, h },
                Rect { x: margin, y: margin, w, h },
                Rect { x: margin + w + gutter, y: margin, w, h },
            ];
            v.truncate(n.max(1));
            v
        }
    }
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
            let cx = 8.0 * MM_TO_PT;
            let cy = 5.0 * MM_TO_PT;
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
