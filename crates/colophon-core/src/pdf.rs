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
    ("solo_pano", 1),
    ("solo_pano_verso", 1),
    ("solo_etroit", 1),
    ("solo_etroit_verso", 1),
    ("solo_carre", 1),
    ("solo_carre_verso", 1),
    ("duo", 2),
    ("duo_portrait", 2),
    ("duo_paysage", 2),
    ("duo_etroit", 2),
    ("duo_pano", 2),
    ("trio", 3),
    ("trio_verso", 3),
    ("trio_portrait", 3),
    ("trio_portrait_verso", 3),
    ("quad", 4),
    ("quad_portrait", 4),
    ("quad_etroit", 4),
    ("quad_pano", 4),
    ("six", 6),
    ("six_verso", 6),
    ("octo", 8),
];

/// Cell aspect of the margined landscape cells (stacks, quads).
pub const CELL_LANDSCAPE: f64 = 4.0 / 3.0;
/// Cell aspect of the portrait cells (solo, duo_portrait, pairs).
pub const CELL_PORTRAIT: f64 = 0.75;
/// Cell aspect of the panorama cells.
pub const CELL_PANO: f64 = 2.0;
/// Cell aspect of the narrow cells, for 18,5:9-style phone portraits.
pub const CELL_ETROIT: f64 = 0.5;
/// Cell aspect of the square cell, for square-ish photos on non-square
/// pages where no other cell comes close.
pub const CELL_CARRE: f64 = 1.0;

/// How many photos a template holds.
pub fn template_capacity(name: &str) -> usize {
    TEMPLATES
        .iter()
        .find(|(t, _)| *t == name)
        .map(|(_, n)| *n)
        .unwrap_or(1)
}

/// The template family for a photo count, with its capacity. Counts without
/// an exact template (5, 7) drop to the largest one below: a grid with a hole
/// in it is worse than one photo fewer.
pub fn template_for_count(n: usize) -> Option<(&'static str, usize)> {
    Some(match n {
        0 => return None,
        1 => ("solo", 1),
        2 => ("duo", 2),
        3 => ("trio", 3),
        4 | 5 => ("quad", 4),
        6 | 7 => ("six", 6),
        _ => ("octo", 8),
    })
}

/// Where a spread lands after losing photos: the fallback template for what
/// remains, keeping the `_verso` side when the family has one. This is the
/// single copy of the rule; the front end ports it and `dump_geometry`
/// exposes the table so the parity check catches any drift.
pub fn fallback_template(current: &str, remaining: usize) -> Option<(String, usize)> {
    let (family, capacity) = template_for_count(remaining)?;
    let verso = format!("{family}_verso");
    let keep_verso = current.ends_with("_verso") && TEMPLATES.iter().any(|(t, _)| *t == verso);
    Some((if keep_verso { verso } else { family.to_string() }, capacity))
}

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

    // Count -> [template, capacity], for every count a spread can reach.
    let fallbacks: serde_json::Map<String, serde_json::Value> = (1..=9usize)
        .filter_map(|n| {
            template_for_count(n)
                .map(|(t, cap)| (n.to_string(), serde_json::json!([t, cap])))
        })
        .collect();

    serde_json::json!({
        "trim_mm": { "w": album.trim_mm.w, "h": album.trim_mm.h },
        "bleed_mm": album.bleed_mm,
        "canvas": { "w": g.media_w, "h": g.media_h, "margin": g.margin, "gutter": g.gutter },
        "templates": templates,
        "fallbacks": fallbacks,
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

/// The first caption spot no image covers, tried in reading order, or None
/// when every candidate is covered. The linter counts the None case: it means
/// the caption will print over a photo.
pub fn caption_anchor_free(rects: &[Rect], g: &SpreadGeometry) -> Option<Point> {
    caption_candidates(g)
        .into_iter()
        .find(|at| {
            let b = caption_box(*at, g);
            rects.iter().all(|r| !overlaps(r, &b))
        })
}

fn caption_candidates(g: &SpreadGeometry) -> [Point; 4] {
    let half = g.media_w / 2.0;
    let low = g.margin * 0.36; // 5 mm on a 210 mm page
    let high = g.media_h - g.margin * 0.75;
    let left = g.margin * 0.57; // 8 mm
    let right = half + g.gutter / 2.0;
    [
        Point { x: left, y: low },
        Point { x: right, y: low },
        Point { x: left, y: high },
        Point { x: right, y: high },
    ]
}

/// Where the chapter caption goes: the first spot no image covers, tried in
/// reading order. A caption printed over a full-bleed photo is unreadable,
/// and moving it costs nothing next to adding a plaque behind it.
pub fn caption_anchor(rects: &[Rect], g: &SpreadGeometry) -> Point {
    caption_anchor_free(rects, g).unwrap_or_else(|| caption_candidates(g)[0])
}

/// The part of an image a cover-crop into `rect` shows, in image pixels:
/// `(x0, y0, vw, vh)`, top-left origin. Same arithmetic as the renderer;
/// the composer and the linter reason about face cuts with it.
pub fn crop_window(rect: &Rect, iw: f64, ih: f64, focal: [f64; 2]) -> (f64, f64, f64, f64) {
    let s = (rect.w / iw).max(rect.h / ih);
    let vw = rect.w / s;
    let vh = rect.h / s;
    let x0 = ((iw - vw) * focal[0].clamp(0.0, 1.0)).clamp(0.0, (iw - vw).max(0.0));
    let y0 = ((ih - vh) * focal[1].clamp(0.0, 1.0)).clamp(0.0, (ih - vh).max(0.0));
    (x0, y0, vw, vh)
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
        // one photo, margined: portrait, landscape or panorama cell
        "solo" => vec![fitted(lead, CELL_PORTRAIT)],
        "solo_paysage" => vec![fitted(lead, CELL_LANDSCAPE)],
        "solo_pano" => vec![fitted(lead, CELL_PANO)],
        "solo_etroit" => vec![fitted(lead, CELL_ETROIT)],
        "solo_carre" => vec![fitted(lead, CELL_CARRE)],
        // one per page, facing each other
        "duo" => {
            let (l, r) = (page_box(false, g), page_box(true, g));
            vec![l, r]
        }
        // two portraits or two landscapes facing each other
        "duo_portrait" => vec![
            fitted(page_box(false, g), CELL_PORTRAIT),
            fitted(page_box(true, g), CELL_PORTRAIT),
        ],
        "duo_paysage" => vec![
            fitted(page_box(false, g), CELL_LANDSCAPE),
            fitted(page_box(true, g), CELL_LANDSCAPE),
        ],
        "duo_etroit" => vec![
            fitted(page_box(false, g), CELL_ETROIT),
            fitted(page_box(true, g), CELL_ETROIT),
        ],
        "duo_pano" => vec![
            fitted(page_box(false, g), CELL_PANO),
            fitted(page_box(true, g), CELL_PANO),
        ],
        // a full page facing two stacked landscape cells
        "trio" => {
            let stack: Vec<Rect> = grid(facing, 1, 2, g.gutter)
                .into_iter()
                .map(|c| fitted(c, CELL_LANDSCAPE))
                .collect();
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
        // a full page facing two portraits side by side
        "trio_portrait" => {
            let pair: Vec<Rect> = grid(facing, 2, 1, g.gutter)
                .into_iter()
                .map(|c| fitted(c, CELL_PORTRAIT))
                .collect();
            let mut v = Vec::with_capacity(3);
            if lead_right {
                v.extend(pair);
                v.push(full_page(true, g));
            } else {
                v.push(full_page(false, g));
                v.extend(pair);
            }
            v
        }
        // 2 x 2 landscape cells across the spread, one column per page
        "quad" => {
            let (l, r) = (page_box(false, g), page_box(true, g));
            let (lc, rc) = (grid(l, 1, 2, g.gutter), grid(r, 1, 2, g.gutter));
            vec![
                fitted(lc[0], CELL_LANDSCAPE),
                fitted(rc[0], CELL_LANDSCAPE),
                fitted(lc[1], CELL_LANDSCAPE),
                fitted(rc[1], CELL_LANDSCAPE),
            ]
        }
        // four portraits, two per page side by side
        "quad_portrait" => {
            let mut v: Vec<Rect> = grid(page_box(false, g), 2, 1, g.gutter)
                .into_iter()
                .map(|c| fitted(c, CELL_PORTRAIT))
                .collect();
            v.extend(
                grid(page_box(true, g), 2, 1, g.gutter)
                    .into_iter()
                    .map(|c| fitted(c, CELL_PORTRAIT)),
            );
            v
        }
        // four narrow portraits, two per page side by side
        "quad_etroit" => {
            let mut v: Vec<Rect> = grid(page_box(false, g), 2, 1, g.gutter)
                .into_iter()
                .map(|c| fitted(c, CELL_ETROIT))
                .collect();
            v.extend(
                grid(page_box(true, g), 2, 1, g.gutter)
                    .into_iter()
                    .map(|c| fitted(c, CELL_ETROIT)),
            );
            v
        }
        // four panoramas, two stacked per page
        "quad_pano" => {
            let (l, r) = (page_box(false, g), page_box(true, g));
            let (lc, rc) = (grid(l, 1, 2, g.gutter), grid(r, 1, 2, g.gutter));
            vec![
                fitted(lc[0], CELL_PANO),
                fitted(rc[0], CELL_PANO),
                fitted(lc[1], CELL_PANO),
                fitted(rc[1], CELL_PANO),
            ]
        }
        // two stacked landscapes facing a four-up mosaic
        "six" => {
            let stack: Vec<Rect> = grid(lead, 1, 2, g.gutter)
                .into_iter()
                .map(|c| fitted(c, CELL_LANDSCAPE))
                .collect();
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

/// Slot colors of the template sheets, shared with scripts/pdf-png.py:
/// the raster check knows which color belongs in which cell.
pub const SHEET_PALETTE: [[u8; 3]; 8] = [
    [200, 30, 40],
    [30, 120, 200],
    [30, 160, 60],
    [230, 160, 30],
    [130, 60, 180],
    [20, 170, 170],
    [230, 90, 140],
    [90, 90, 30],
];

/// One PDF per template, every slot filled with its palette color. The
/// PDF → PNG non-regression rasterizes these and checks each cell shows
/// its color where the geometry says: it bites on placement and clipping
/// in the real renderer, where the geometry parity only checks arithmetic.
pub fn render_template_sheets(album: &Album, dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    use crate::model::{Slot, Spread};
    std::fs::create_dir_all(dir)?;
    let mut out = Vec::new();
    for (name, n) in TEMPLATES {
        let spread = Spread {
            template: (*name).to_string(),
            slots: (0..*n)
                .map(|i| Slot { src: format!("{i}"), focal: [0.5, 0.5] })
                .collect(),
            caption: None,
        };
        let assets: Vec<JpegAsset> = (0..*n)
            .map(|i| solid_jpeg(SHEET_PALETTE[i], 160, 120))
            .collect::<Result<_>>()?;
        let mut writer = PdfWriter::new(album);
        writer.add_spread(&spread, &assets)?;
        let path = dir.join(format!("{name}.pdf"));
        writer.save(&path)?;
        out.push(path);
    }
    Ok(out)
}

fn solid_jpeg(rgb: [u8; 3], w: u32, h: u32) -> Result<JpegAsset> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb(rgb));
    let mut data = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, 95)
        .encode_image(&img)
        .context("encodage de l'aplat")?;
    Ok(JpegAsset { data, width: w, height: h, focal: [0.5, 0.5] })
}

pub struct PdfWriter {
    doc: Document,
    page_ids: Vec<Object>,
    pages_id: lopdf::ObjectId,
    font_id: lopdf::ObjectId,
    geom: SpreadGeometry,
    bleed_mm: f64,
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
        Self {
            doc,
            page_ids: Vec::new(),
            pages_id,
            font_id,
            geom: geometry(album),
            bleed_mm: album.bleed_mm,
        }
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
        // TrimBox marks the finished spread inside the bleed: prepress reads
        // it for the cut, and a preflight without it flags the file.
        let b = self.bleed_mm * MM_TO_PT;
        let (mw, mh) = (self.geom.media_w * MM_TO_PT, self.geom.media_h * MM_TO_PT);
        let page_id = self.doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(self.pages_id),
            "MediaBox" => vec![0.into(), 0.into(), mw.into(), mh.into()],
            "BleedBox" => vec![0.into(), 0.into(), mw.into(), mh.into()],
            "TrimBox" => vec![b.into(), b.into(), (mw - b).into(), (mh - b).into()],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_walks_down_the_families() {
        assert_eq!(fallback_template("quad", 3), Some(("trio".into(), 3)));
        assert_eq!(fallback_template("trio", 2), Some(("duo".into(), 2)));
        assert_eq!(fallback_template("duo", 1), Some(("solo".into(), 1)));
        assert_eq!(fallback_template("solo", 0), None);
        // no 7- or 5-photo template: the spread drops one more
        assert_eq!(fallback_template("octo", 7), Some(("six".into(), 6)));
        assert_eq!(fallback_template("six", 5), Some(("quad".into(), 4)));
    }

    #[test]
    fn fallback_keeps_the_verso_side_when_it_exists() {
        assert_eq!(
            fallback_template("six_verso", 3),
            Some(("trio_verso".into(), 3))
        );
        // quad has no verso variant: fall back to the plain family
        assert_eq!(fallback_template("six_verso", 4), Some(("quad".into(), 4)));
    }

    #[test]
    fn every_fallback_target_is_a_known_template() {
        for n in 1..=9 {
            let (t, cap) = template_for_count(n).unwrap();
            assert!(cap <= n, "capacity {cap} exceeds the {n} photos left");
            assert_eq!(template_capacity(t), cap);
        }
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
