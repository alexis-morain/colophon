//! PDF renderer built directly on lopdf. One spread per PDF page.
//! Images are embedded as JPEG (DCTDecode passthrough), cover-cropped
//! into their slot via a clip rectangle, anchored on the focal point.

use crate::font;
use crate::model::{Album, Alignement, Spread};
use crate::pdfx;
use anyhow::{Context, Result};
use lopdf::{dictionary, Document, Object, Stream};
use std::path::Path;

const MM_TO_PT: f64 = 72.0 / 25.4;

/// Whether the writer's output may call itself PDF/X. Defined once, in
/// [`crate::pdfx`] next to the declaration it describes, and re-exported here
/// because `pdf::EMITS_PDF_X` is what the preflight has always read.
pub use crate::pdfx::EMITS_PDF_X;

/// Geometry of one slot on the spread's media box, in millimetres,
/// origin bottom-left, bleed included.
#[derive(Debug, Clone, Copy, serde::Serialize)]
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
    /// Bleed on every side. The trimmed spread is the media inset by this
    /// much: anything that must survive the cut is measured from there,
    /// never from the media edge.
    pub bleed: f64,
}

/// The two boxes a page needs: the sheet, and the finished piece inside it.
/// `trim` is `[x0, y0, x1, y1]` in millimetres, and it is asymmetric on a
/// cover, where the bleed differs edge by edge.
pub(crate) struct Boxes {
    pub media: [f64; 2],
    pub trim: [f64; 4],
}

/// The face a document sets its type in, and the glyphs it has drawn so far.
///
/// The two travel together because they answer each other: `/W` and
/// `/ToUnicode` describe exactly the glyphs that were drawn, and only the
/// face that drew them can say how wide they are. Passing the pair down to
/// every emitter is also what lets the cover — which writes its own content
/// stream, outside the writer — be counted like everything else.
pub(crate) struct Ecrivain {
    /// Borrowed for the album's own face, which is opened once for the whole
    /// process; owned the day a face comes from somewhere else. A copy per
    /// document would be 430 kB of memcpy for every sheet rendered.
    face: std::borrow::Cow<'static, font::Embarquee>,
    utilises: font::Utilises,
}

impl Ecrivain {
    /// The face this crate ships. Test-only since the album's folder reaches
    /// every writer: in production the face is always resolved from the
    /// album, and « no face chosen » is one of that resolution's answers.
    #[cfg(test)]
    pub(crate) fn incorporee() -> Self {
        Self::depuis(font::face_projet())
    }


    /// A writer over the face an album chose. The `Cow` is the whole point:
    /// the project's face is opened once for the process and borrowed, an
    /// album's own is read once per document and owned.
    pub(crate) fn depuis(face: std::borrow::Cow<'static, font::Embarquee>) -> Self {
        Self { face, utilises: font::Utilises::default() }
    }

    /// How wide a string sets in **this document's** face.
    ///
    /// Every line break the renderers decide goes through here rather than
    /// through [`font::text_width_mm`], which knows only the face this crate
    /// ships. Measuring in one face and drawing in another is the two-
    /// geometries trap with a font in it, and it shows up as a title running
    /// past the guillotine.
    pub(crate) fn largeur_mm(&self, s: &str, size_pt: f64) -> f64 {
        self.face.largeur_mm(s, size_pt)
    }
}


/// One run of text at a baseline, in millimetres, in the album's only face.
///
/// The single place a string turns into content operators. Under Identity-H a
/// string is glyph ids, two bytes each, so it is written as a hex string:
/// there is no encoding left between the byte in the stream and the glyph the
/// reader draws, and nothing to escape.
pub(crate) fn text_op(
    content: &mut String,
    ecrivain: &mut Ecrivain,
    x_mm: f64,
    y_mm: f64,
    size_pt: f64,
    rgb: [f64; 3],
    s: &str,
) {
    if s.is_empty() {
        return;
    }
    let glyphes = ecrivain.face.glyphes(s);
    if glyphes.is_empty() {
        return;
    }
    let mut hex = String::with_capacity(glyphes.len() * 4);
    for (gid, c) in glyphes {
        // Noted as it is drawn, so the file describes what it shows and
        // nothing else.
        ecrivain.utilises.noter(gid, c);
        hex.push_str(&format!("{gid:04X}"));
    }
    let (x, y) = (x_mm * MM_TO_PT, y_mm * MM_TO_PT);
    let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
    content.push_str(&format!(
        "BT /F1 {size_pt} Tf {r} {g} {b} rg {x:.2} {y:.2} Td <{hex}> Tj ET\n"
    ));
}

/// Set one line whose baseline is turned.
///
/// **`Td` is left alone for upright text.** An album that carries no turned
/// object has to produce the byte-for-byte PDF it produced before free
/// objects existed, and the cheapest way to guarantee that is for the upright
/// path to be the same code it always was. Above zero degrees the placement
/// moves to `Tm`, which is the text matrix rather than the graphics one: no
/// `q`/`Q` to balance, and nothing outside the `BT`/`ET` pair is disturbed.
///
/// `x_mm`/`y_mm` are the baseline's origin **already turned** — the caller
/// owns the centre it turns around, because the centre is a property of the
/// object and not of a line.
pub(crate) fn text_op_tourne(
    content: &mut String,
    ecrivain: &mut Ecrivain,
    x_mm: f64,
    y_mm: f64,
    angle_deg: f64,
    size_pt: f64,
    rgb: [f64; 3],
    s: &str,
) {
    if angle_deg == 0.0 {
        text_op(content, ecrivain, x_mm, y_mm, size_pt, rgb, s);
        return;
    }
    if s.is_empty() {
        return;
    }
    let glyphes = ecrivain.face.glyphes(s);
    if glyphes.is_empty() {
        return;
    }
    let mut hex = String::with_capacity(glyphes.len() * 4);
    for (gid, c) in glyphes {
        ecrivain.utilises.noter(gid, c);
        hex.push_str(&format!("{gid:04X}"));
    }
    let (x, y) = (x_mm * MM_TO_PT, y_mm * MM_TO_PT);
    let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    content.push_str(&format!(
        "BT /F1 {size_pt} Tf {r} {g} {b} rg \
{cos:.5} {sin:.5} {msin:.5} {cos:.5} {x:.2} {y:.2} Tm <{hex}> Tj ET\n",
        msin = -sin,
    ));
}

/// A resolved image ready to embed: raw JPEG bytes plus pixel size.
pub struct JpegAsset {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub focal: [f64; 2],
    /// Manual zoom past the cover fill, 1.0 = exact fill.
    pub zoom: f64,
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
        bleed: album.bleed_mm,
    }
}

/// The whole of one page, bleed included. Nothing ever spans both: an image
/// across the fold is swallowed by the binding.
pub(crate) fn full_page(right: bool, g: &SpreadGeometry) -> Rect {
    let half = g.media_w / 2.0;
    Rect { x: if right { half } else { 0.0 }, y: 0.0, w: half, h: g.media_h }
}

/// One content box as the dump carries it: `[x, y, w, h]`, engine frame.
fn let_page(r: Rect) -> serde_json::Value {
    serde_json::json!([r.x, r.y, r.w, r.h])
}

/// The margined content box of one page. Half a gutter is kept on the fold
/// side so two facing images do not kiss across the binding.
pub(crate) fn page_box(right: bool, g: &SpreadGeometry) -> Rect {
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
pub(crate) fn grid(b: Rect, cols: usize, rows: usize, gap: f64) -> Vec<Rect> {
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
pub(crate) fn fitted(b: Rect, aspect: f64) -> Rect {
    let w = b.w.min(b.h * aspect);
    let h = w / aspect;
    Rect { x: b.x + (b.w - w) / 2.0, y: b.y + (b.h - h) / 2.0, w, h }
}

/// Every template the composer can emit, with the number of photos it
/// holds. Derived from the generated catalogue (`gabarit::catalogue`): the
/// list is data, and this is a view of it, kept for every caller that
/// walks names and capacities.
pub static TEMPLATES: std::sync::LazyLock<Vec<(&'static str, usize)>> =
    std::sync::LazyLock::new(|| {
        crate::gabarit::catalogue().iter().map(|s| (s.nom, s.capacite)).collect()
    });

pub use crate::gabarit::{CELL_CARRE, CELL_ETROIT, CELL_LANDSCAPE, CELL_PANO, CELL_PORTRAIT};

/// How many photos a template holds. Resolves through `gabarit::spec`, so
/// a generated name outside the offered list still answers exactly.
pub fn template_capacity(name: &str) -> usize {
    crate::gabarit::spec(name).map(|s| s.capacite).unwrap_or(1)
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
    // The dump speaks for the offered list, not just the historical
    // catalogue: a retained generated template must reach the editor's
    // picker and previews through the same single source.
    let offerts: Vec<(&'static str, usize)> =
        crate::gabarit::offerts().iter().map(|s| (s.nom, s.capacite)).collect();
    let templates: serde_json::Map<String, serde_json::Value> = offerts
        .iter()
        .map(|(name, n)| {
            let rects = slots_for(name, *n, &g);
            let at = caption_anchor(name, &rects, &g);
            let slots: Vec<[f64; 4]> = rects.iter().map(|r| [r.x, r.y, r.w, r.h]).collect();
            // The caption anchor moves with the rectangles, and a spread may
            // hold fewer photos than its template's capacity: one anchor per
            // count, index = photo count (0 falls back like slots_for does).
            let captions: Vec<[f64; 2]> = (0..=*n)
                .map(|k| {
                    let r = slots_for(name, k, &g);
                    let a = caption_anchor(name, &r, &g);
                    [a.x, a.y]
                })
                .collect();
            (
                name.to_string(),
                serde_json::json!({
                    "slots": slots,
                    "caption": [at.x, at.y],
                    "captions": captions,
                    "legende": crate::gabarit::spec(name).map_or(0.0, |s| s.legende),
                }),
            )
        })
        .collect();

    // The catalogue's own order: a JSON map sorts its keys, and the picker
    // shows families in catalogue order, so the order travels separately.
    let ordre: Vec<serde_json::Value> = offerts
        .iter()
        .map(|(name, n)| serde_json::json!([name, n]))
        .collect();

    // Fixed anchors and type constants the editor used to redeclare. All in
    // millimetres (or ratios), converted here once.
    const PT_MM: f64 = 0.352778;
    let texte = text_anchor(&g);
    let colophon = colophon_anchor(&g);
    let garde = crate::garde::anchor(&g);
    let anchors = serde_json::json!({
        "texte": [texte.x, texte.y],
        "colophon": [colophon.x, colophon.y],
        "garde": [garde.x, garde.y],
        "garde_place": crate::garde::place(&g),
    });
    let constantes = serde_json::json!({
        "caption_size_mm": CAPTION_SIZE_PT * PT_MM,
        "caption_safe": CAPTION_SAFE,
        "photo_caption_size_mm": PHOTO_CAPTION_SIZE_PT * PT_MM,
        "photo_caption_drop_mm": PHOTO_CAPTION_DROP_MM,
        "text_size_mm": TEXT_SIZE_PT * PT_MM,
        "text_leading_mm": TEXT_LEADING_MM,
        "colophon_size_mm": crate::colophon::SIZE_PT * PT_MM,
        "colophon_leading_mm": crate::colophon::LEADING_MM,
        "garde_titre_mm": crate::garde::TITRE_PT * PT_MM,
        "garde_titre_min_mm": crate::garde::TITRE_PT_MIN * PT_MM,
        "garde_ligne_mm": crate::garde::LIGNE_PT * PT_MM,
        "garde_ligne_leading_mm": crate::garde::LIGNE_LEADING_MM,
        "garde_apres_titre_mm": crate::garde::APRES_TITRE_MM,
        "titre_max": crate::garde::TITRE_MAX,
        "spine_text_min_mm": crate::cover::SPINE_TEXT_MIN_MM,
        "grammage_reference": crate::printer::GRAMMAGE_REFERENCE,
        "grammage_defaut": crate::printer::GRAMMAGE_DEFAUT,
        "min_effective_ppi": crate::audit::MIN_EFFECTIVE_PPI,
        "thumb_size": crate::thumb::THUMB_SIZE,
    });

    // The half-title layout under a synthetic measure both sides can run:
    // the shrink formula and the line rhythm are the algorithm under test,
    // the embedded face stays the renderer's business.
    let garde_mesure = |s: &str, pt: f64| s.chars().count() as f64 * pt * 0.2;
    let garde_samples: Vec<serde_json::Value> = [
        "Corse\n\nDu 27 octobre au 3 novembre 2013\nCalvi, Bastia",
        "Un titre bien trop long pour tenir sur la page de garde sans fondre\n\nLigne",
        "Seul",
    ]
    .iter()
    .map(|text| {
        let place = crate::garde::place(&g);
        let lignes: Vec<serde_json::Value> = crate::garde::mise_en_page_avec(
            text, place, garde_mesure,
        )
        .into_iter()
        .map(|l| serde_json::json!([l.texte, l.taille_pt, l.dy_mm]))
        .collect();
        serde_json::json!({ "texte": text, "place": place, "lignes": lignes })
    })
    .collect();

    // The wrapping of a free block under the same synthetic measure. It is
    // here for the reason the half-title samples are here: where a line breaks
    // is a function of the face, and the parity run has no face — so the
    // *algorithm* is what travels, replayed on both sides under a measure
    // neither of them owns. The committed scene fixture pins everything about
    // a free object that a measure cannot move (its box, its angle, its
    // alignment, its index); this pins the one thing it can.
    let libre_samples: Vec<serde_json::Value> = [
        // Ordinary wrap; a hard newline that must survive it; a blank
        // paragraph that keeps its turn; a word wider than the box; and each
        // of the three alignments, because the offset a line is set at is
        // computed from the same measure and is just as much the algorithm.
        ("un bloc droit qui revient à la ligne tout seul", 40.0, 11.0, Alignement::Gauche),
        ("deux lignes\ndont la seconde est nettement plus longue", 40.0, 11.0, Alignement::Centre),
        ("avant\n\naprès", 40.0, 11.0, Alignement::Droite),
        ("court anticonstitutionnellement court", 25.0, 9.0, Alignement::Centre),
        ("mot", 100.0, 12.0, Alignement::Droite),
    ]
    .iter()
    .map(|(texte, largeur, taille_pt, alignement)| {
        let (lignes, trop_large) =
            crate::scene::replier(texte, *largeur, *taille_pt, &garde_mesure);
        let dx: Vec<f64> = lignes
            .iter()
            .map(|l| crate::scene::decalage(*alignement, *largeur, garde_mesure(l, *taille_pt)))
            .collect();
        serde_json::json!({
            "texte": texte,
            "largeur": largeur,
            "taille_pt": taille_pt,
            "alignement": alignement,
            "lignes": lignes,
            "dx": dx,
            "trop_large": trop_large,
        })
    })
    .collect();

    // Count -> [template, capacity], for every count a spread can reach.
    let fallbacks: serde_json::Map<String, serde_json::Value> = (1..=9usize)
        .filter_map(|n| {
            template_for_count(n)
                .map(|(t, cap)| (n.to_string(), serde_json::json!([t, cap])))
        })
        .collect();

    // Crop windows over fixed samples: the manual-crop arithmetic (focal +
    // zoom) is written twice too, and a drift here silently shifts every
    // recadrage between the preview and the print.
    let crop_samples: Vec<serde_json::Value> = CROP_SAMPLES
        .iter()
        .map(|(rw, rh, iw, ih, fx, fy, zoom)| {
            let rect = Rect { x: 0.0, y: 0.0, w: *rw, h: *rh };
            let (x0, y0, vw, vh) = crop_window(&rect, *iw, *ih, [*fx, *fy], *zoom);
            serde_json::json!({
                "rect": [rw, rh], "image": [iw, ih], "focal": [fx, fy],
                "zoom": zoom, "window": [x0, y0, vw, vh],
            })
        })
        .collect();

    // The cover sheet, per supplier and per book thickness. The editor draws
    // that sheet from the same profile data, so the width of the spine it
    // shows and the width the printer receives are one arithmetic, checked
    // here rather than believed.
    let covers: Vec<serde_json::Value> = crate::printer::PrinterProfile::tous()
        .iter()
        .flat_map(|p| {
            [12usize, 48, 96].into_iter().map(move |spreads| {
                let mut a = album.clone();
                a.spreads = vec![
                    Spread {
                        template: "vide".into(),
                        slots: vec![],
                        caption: None,
                        text: None,
                        edited: false,
                        locked: false,
                        objets: Vec::new(),
                    };
                    spreads
                ];
                let c = crate::cover::geometry(&a, p);
                serde_json::json!({
                    "profil": p.id,
                    "spreads": spreads,
                    "sheet": [c.media_w, c.media_h],
                    "spine": c.spine_mm(),
                    "panels": [
                        [c.back.x, c.back.w],
                        [c.front.x, c.front.w],
                    ],
                })
            })
        })
        .collect();

    serde_json::json!({
        "trim_mm": { "w": album.trim_mm.w, "h": album.trim_mm.h },
        "bleed_mm": album.bleed_mm,
        "media": { "w": g.media_w, "h": g.media_h, "margin": g.margin, "gutter": g.gutter },
        "ordre": ordre,
        "templates": templates,
        "fallbacks": fallbacks,
        "anchors": anchors,
        "constantes": constantes,
        "garde_samples": garde_samples,
        "libre_samples": libre_samples,
        // La boîte de contenu de chaque page. L'éditeur y pose un bloc libre
        // neuf, donc il doit la lire là où elle est calculée plutôt que
        // recomposer une marge de son côté.
        "pages": [
            let_page(page_box(false, &g)),
            let_page(page_box(true, &g)),
        ],
        "crop_windows": crop_samples,
        "covers": covers,
    })
}

/// (rect w, rect h, image w, image h, focal x, focal y, zoom): the shapes a
/// real album mixes: portrait in landscape cell, pano, off-center focal,
/// zoomed, and the degenerate zoom below 1 that must clamp to the fill.
const CROP_SAMPLES: &[(f64, f64, f64, f64, f64, f64, f64)] = &[
    (100.0, 100.0, 2000.0, 1000.0, 0.5, 0.5, 1.0),
    (100.0, 100.0, 2000.0, 1000.0, 0.0, 0.42, 1.0),
    (160.0, 90.0, 1000.0, 1500.0, 0.5, 0.2, 1.0),
    (100.0, 100.0, 2000.0, 1000.0, 0.5, 0.5, 1.6),
    (60.0, 120.0, 3000.0, 2000.0, 0.8, 0.3, 2.5),
    (100.0, 50.0, 1200.0, 1200.0, 0.25, 0.9, 0.5),
];

/// A point on the spread's media box, in millimetres, origin bottom-left.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Nominal size of the caption box, used to test whether a candidate spot is
/// clear. Generous on purpose: a caption half over a photo is still unreadable.
/// Chapter captions: 9 pt.
pub const CAPTION_SIZE_PT: f64 = 9.0;

/// The ground a caption actually covers. `at` is the baseline: type rises by
/// about the cap height above it and drops by the descender below.
///
/// This used to be a margin-proportional proxy nearly three times the type
/// size. The slack cost nothing while the anchors floated in the bleed; once
/// they moved in to clear the trim, the proxy started reading a caption as
/// covered where the printed line runs clear of every photo.
fn caption_box(at: Point, g: &SpreadGeometry) -> Rect {
    let size = CAPTION_SIZE_PT / MM_TO_PT;
    Rect { x: at.x, y: at.y - size * 0.3, w: g.margin * 3.5, h: size * 1.35 }
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
    // Measured from the trimmed edge, not from the media: a caption placed
    // 5 mm from the media edge comes back from the press 5 mm minus the bleed
    // from the cut, and every supplier's safe zone rejects it. Anchored here,
    // the baseline clears 7 mm on a 210 mm page: past Cloudprinter's 5, which
    // is the supplier the album is composed for. Prodigi wants 10 and its
    // preflight says so; that is the fallback profile speaking, not a bug.
    let low = g.bleed + g.margin * CAPTION_SAFE;
    let high = g.media_h - g.bleed - g.margin * 0.75;
    let left = g.bleed + g.margin * 0.57;
    let right = half + g.gutter / 2.0;
    [
        Point { x: left, y: low },
        Point { x: right, y: low },
        Point { x: left, y: high },
        Point { x: right, y: high },
    ]
}

/// Where the chapter caption goes, driven by the template's signed caption
/// height. A declared band (positive) hangs the baseline under the lifted
/// frame, like a photo caption under its slot; a declared overlay (negative)
/// prints at the reading-order spot without hunting; zero keeps the historic
/// rule: the first spot no image covers, because a caption printed over a
/// full-bleed photo is unreadable and moving it costs nothing next to adding
/// a plaque behind it.
pub fn caption_anchor(template: &str, rects: &[Rect], g: &SpreadGeometry) -> Point {
    caption_anchor_of(crate::gabarit::spec(template), rects, g)
}

/// `caption_anchor` for a spec already in hand (tests build synthetic ones).
pub fn caption_anchor_of(
    spec: Option<&crate::gabarit::Spec>,
    rects: &[Rect],
    g: &SpreadGeometry,
) -> Point {
    let declared = spec.map_or(0.0, |s| s.legende);
    let c = caption_candidates(g);
    if declared > 0.0 {
        // The band under the frame: the interpreter lifted the slots, the
        // baseline drops under the lowest one like a photo caption does.
        let bas = rects.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
        if bas.is_finite() {
            return Point { x: c[0].x, y: bas - PHOTO_CAPTION_DROP_MM };
        }
    }
    if declared < 0.0 {
        return c[0];
    }
    caption_anchor_free(rects, g).unwrap_or(c[0])
}

/// The signed caption height of a spread, in millimetres: positive means the
/// caption sits clear of every photo (a declared band, or a free margin spot
/// found), negative means it prints over one (a declared overlay, or every
/// candidate covered). One number instead of two rules: the linter's caption
/// counters read its sign.
pub fn caption_height(template: &str, rects: &[Rect], g: &SpreadGeometry) -> f64 {
    caption_height_of(crate::gabarit::spec(template), rects, g)
}

/// `caption_height` for a spec already in hand (tests build synthetic ones).
pub fn caption_height_of(
    spec: Option<&crate::gabarit::Spec>,
    rects: &[Rect],
    g: &SpreadGeometry,
) -> f64 {
    let declared = spec.map_or(0.0, |s| s.legende);
    if declared != 0.0 {
        return declared;
    }
    // The hunt's verdict, carried as the ground the caption box covers.
    let h = CAPTION_SIZE_PT / MM_TO_PT * 1.35;
    match caption_anchor_free(rects, g) {
        Some(_) => h,
        None => -h,
    }
}

/// Share of the margin kept between a chapter caption and the trimmed edge.
/// 7 mm on a 210 mm page: clear of Cloudprinter's 5 mm safe zone, the
/// supplier the album is composed for. Stricter fallbacks (Prodigi at 10,
/// Lulu at 12.7) are flagged by their own preflight instead: composing for
/// the strictest profile of all would push every caption into the middle of
/// the page.
pub const CAPTION_SAFE: f64 = 0.5;

/// Photo captions: 7 pt, baseline this far under the slot's bottom edge.
pub const PHOTO_CAPTION_SIZE_PT: f64 = 7.0;
pub const PHOTO_CAPTION_DROP_MM: f64 = 3.4;

/// Spread captions, the line that dates a whole double page.
pub const SPREAD_CAPTION_SIZE_PT: f64 = 9.0;

/// Free-text pages: 11 pt, fixed leading, lines as typed.
pub const TEXT_SIZE_PT: f64 = 11.0;
pub const TEXT_LEADING_MM: f64 = 6.4;

/// Caption grey, and the slightly warmer, darker ink of a text page. Both
/// were picked against paper, not against a screen.
pub const INK: [f64; 3] = [0.25, 0.25, 0.25];
pub const TEXT_INK: [f64; 3] = [0.2, 0.19, 0.16];

/// Where a `texte` spread's first baseline sits: left margin of the recto
/// page, at 62 % of the height. The editor mirrors this anchor.
pub fn text_anchor(g: &SpreadGeometry) -> Point {
    Point { x: g.media_w / 2.0 + g.gutter / 2.0, y: g.media_h * 0.62 }
}

/// Where the colophon's first line sits: same left margin as a text page,
/// but low on the recto, under the whole empty page. It is the last thing in
/// the book and it should read like it: quiet, at the foot, not a statement.
pub fn colophon_anchor(g: &SpreadGeometry) -> Point {
    Point { x: g.media_w / 2.0 + g.gutter / 2.0, y: g.media_h * 0.30 }
}

/// The part of an image a cover-crop into `rect` shows, in image pixels:
/// `(x0, y0, vw, vh)`, top-left origin. Same arithmetic as the renderer;
/// the composer and the linter reason about face cuts with it. `zoom` is
/// the manual magnification past the cover fill (1.0 = exact fill, never
/// below: a gap inside a slot is not a crop, it is a hole).
///
/// `focal` is **a point of the image**, as a fraction of its width and
/// height: the window centres on it, and only the image borders may move it
/// off centre. That is what makes a crop survive a change of ratio — the
/// point is a property of the photograph, not of the cell it landed in.
/// Before schema 2 the same field meant a fraction of the leftover room,
/// which is cell-dependent and therefore destroyed manual work on a format
/// switch; `model::migrate_focal` converts one into the other.
pub fn crop_window(
    rect: &Rect,
    iw: f64,
    ih: f64,
    focal: [f64; 2],
    zoom: f64,
) -> (f64, f64, f64, f64) {
    let s = (rect.w / iw).max(rect.h / ih) * zoom.max(1.0);
    let vw = rect.w / s;
    let vh = rect.h / s;
    let x0 = (focal[0].clamp(0.0, 1.0) * iw - vw / 2.0).clamp(0.0, (iw - vw).max(0.0));
    let y0 = (focal[1].clamp(0.0, 1.0) * ih - vh / 2.0).clamp(0.0, (ih - vh).max(0.0));
    (x0, y0, vw, vh)
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// Slot rectangles for a template, on the given spread geometry.
///
/// This answers a question about a **template** — what would these cells be —
/// which is what the composer asks of a candidate it has not chosen yet, and
/// what the geometry dump asks of a catalogue with no album in hand. For what
/// a **spread** actually holds, ask [`crate::scene::Scene::of`]: it is the one
/// derivation the emitter, the print pass, the linter and the preflight
/// share.
/// The `_verso` variants mirror the layout onto the other page; alternating
/// them is what keeps a long album from reading like a spreadsheet.
pub fn slots_for(template: &str, n: usize, g: &SpreadGeometry) -> Vec<Rect> {
    match crate::gabarit::spec(template) {
        Some(spec) => crate::gabarit::slots(spec, n, g),
        // An album.json repaired by hand may name a template the catalogue
        // does not know: one margined box, the honest fallback of always.
        None => {
            let mut v = vec![page_box(false, g)];
            v.truncate(n.max(1));
            v
        }
    }
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
    // The offered list, not just the historical catalogue: a retained
    // generated template is exportable, so its real render is checked too.
    for (name, n) in crate::gabarit::offerts().iter().map(|s| (&s.nom, &s.capacite)) {
        if *n == 0 {
            continue; // photo-less templates have no cells to check
        }
        let spread = Spread {
            template: (*name).to_string(),
            slots: (0..*n)
                .map(|i| Slot::new(format!("{i}"), [0.5, 0.5]))
                .collect(),
            caption: None,
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        };
        let assets: Vec<JpegAsset> = (0..*n)
            .map(|i| solid_jpeg(SHEET_PALETTE[i], 160, 120))
            .collect::<Result<_>>()?;
        let mut writer = PdfWriter::new(album, dir);
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
    Ok(JpegAsset { data, width: w, height: h, focal: [0.5, 0.5], zoom: 1.0 })
}

pub struct PdfWriter {
    doc: Document,
    page_ids: Vec<Object>,
    pages_id: lopdf::ObjectId,
    /// Reserved at construction, written at `save`: the composite font can
    /// only be described once the last spread has been drawn and the set of
    /// glyphs is closed.
    font_id: lopdf::ObjectId,
    pub(crate) ecrivain: Ecrivain,
    /// The refusal code of a face named by the album and not found, or
    /// `None`. Carried rather than logged: the screen has to say it.
    police_defaut: Option<&'static str>,
    geom: SpreadGeometry,

    bleed_mm: f64,
    /// Goes into `/Info`, into `dc:title`, and nowhere else.
    title: String,
    /// Read once at the top of the render so every date in the file names the
    /// same instant, however long the render takes.
    stamp: chrono::DateTime<chrono::Local>,
}

/// Fill in the text face, once the document knows every glyph it drew.
///
/// A composite font: `/Type0` under Identity-H, whose descendant is a
/// `/CIDFontType2`. The code in the content stream **is** the glyph id, so
/// nothing on the way to the reader depends on an encoding table, and a
/// caption can carry any character the face can draw rather than the 224 a
/// single-byte encoding allowed.
///
/// The face goes in whole, so its name takes no subset prefix and the
/// descriptor carries no `/CIDSet`: both would announce a subset that does
/// not exist. Only `/W` and `/ToUnicode` are restricted to the glyphs drawn —
/// they describe the document, not the face.
fn embed_font(doc: &mut Document, font_id: lopdf::ObjectId, ecrivain: &Ecrivain) {
    let face = &ecrivain.face;
    let m = face.metrics();
    let nom = face.postscript().to_string();

    // Length1 is the size of the face before compression; a reader needs it to
    // unpack the stream back into a font file.
    let file_id = doc.add_object(Stream::new(
        dictionary! { "Length1" => face.octets().len() as i64 },
        face.octets().to_vec(),
    ));

    let descriptor_id = doc.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => nom.as_str(),
        // Nonsymbolic: the face draws the standard Latin set. Under
        // Identity-H the flag steers no glyph lookup — the code is the glyph
        // — but a descriptor still has to declare one.
        "Flags" => 32,
        "FontBBox" => m.bbox.iter().map(|v| Object::Integer(i64::from(*v))).collect::<Vec<_>>(),
        "ItalicAngle" => m.italic_angle,
        "Ascent" => i64::from(m.ascent),
        "Descent" => i64::from(m.descent),
        "CapHeight" => i64::from(m.cap_height),
        // Nominal stem width for a regular weight. Required by the spec,
        // consulted by no renderer that has the glyphs themselves.
        "StemV" => 80,
        "FontFile2" => Object::Reference(file_id),
    });

    let descendant_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => nom.as_str(),
        // Identity ordering: the CID is the glyph id, which is the whole
        // point of Identity-H and what makes `/CIDToGIDMap /Identity` true.
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"),
            "Supplement" => 0,
        },
        "FontDescriptor" => Object::Reference(descriptor_id),
        "DW" => 1000,
        "W" => largeurs(ecrivain),
        "CIDToGIDMap" => "Identity",
    });

    let tounicode_id = doc.add_object(Stream::new(
        dictionary! {},
        tounicode(&ecrivain.utilises).into_bytes(),
    ));

    doc.objects.insert(
        font_id,
        Object::Dictionary(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => nom.as_str(),
            "Encoding" => "Identity-H",
            "DescendantFonts" => vec![Object::Reference(descendant_id)],
            "ToUnicode" => Object::Reference(tounicode_id),
        }),
    );
}

/// `/W`, the advance of every glyph the document drew, in the 1000-unit em.
///
/// Consecutive glyphs share one run — `first [w w w]` — which is the compact
/// form and, more usefully, the stable one: the same album always yields the
/// same array, because the glyphs come out of an ordered set.
fn largeurs(ecrivain: &Ecrivain) -> Vec<Object> {
    let mut out: Vec<Object> = Vec::new();
    let mut run: Vec<Object> = Vec::new();
    let mut debut = 0u16;
    let mut precedent: Option<u16> = None;
    for (gid, _) in ecrivain.utilises.iter() {
        if precedent.is_some_and(|p| gid == p + 1) {
            run.push(Object::Integer(i64::from(ecrivain.face.avance(gid))));
        } else {
            if !run.is_empty() {
                out.push(Object::Integer(i64::from(debut)));
                out.push(Object::Array(std::mem::take(&mut run)));
            }
            debut = gid;
            run.push(Object::Integer(i64::from(ecrivain.face.avance(gid))));
        }
        precedent = Some(gid);
    }
    if !run.is_empty() {
        out.push(Object::Integer(i64::from(debut)));
        out.push(Object::Array(run));
    }
    out
}

/// The `ToUnicode` CMap: what each glyph the document drew means.
///
/// Without it a reader can show the page and copy nothing out of it, and
/// PDF/A would be a claim rather than a fact. The mapping is exact because it
/// was recorded as the text was drawn, never guessed by walking the face's
/// character map backwards.
fn tounicode(utilises: &font::Utilises) -> String {
    let mut out = String::from(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\n\
         begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n\
         /CMapType 2 def\n\
         1 begincodespacerange\n\
         <0000> <FFFF>\n\
         endcodespacerange\n",
    );
    // A bfchar block holds at most 100 entries; past that the file is not a
    // CMap any more, whatever it looks like.
    let entrees: Vec<(u16, char)> = utilises.iter().collect();
    for bloc in entrees.chunks(100) {
        out.push_str(&format!("{} beginbfchar\n", bloc.len()));
        for (gid, c) in bloc {
            let mut cible = String::new();
            // UTF-16BE, so a character outside the basic plane comes out as
            // its surrogate pair rather than as a truncated code point.
            for unite in c.encode_utf16(&mut [0u16; 2]) {
                cible.push_str(&format!("{unite:04X}"));
            }
            out.push_str(&format!("<{gid:04X}> <{cible}>\n"));
        }
        out.push_str("endbfchar\n");
    }
    out.push_str(
        "endcmap\n\
         CMapName currentdict /CMap defineresource pop\n\
         end\n\
         end\n",
    );
    out
}

impl PdfWriter {
    /// `dir` is the album's own folder — the one holding `album.json`, never
    /// the folder of photographs. That is where a chosen face was copied,
    /// and an album whose two folders happen to coincide is the only place
    /// the confusion would not show.
    pub fn new(album: &Album, dir: &Path) -> Self {
        Self::with_stamp(album, dir, pdfx::stamp())
    }

    /// The writer with its declared instant pinned. Everything else in a PDF
    /// this crate emits is a pure function of the album and its assets, so
    /// two writers given the same stamp produce the same bytes — which is how
    /// a change to the emitter proves it changed nothing.
    pub fn with_stamp(
        album: &Album,
        dir: &Path,
        stamp: chrono::DateTime<chrono::Local>,
    ) -> Self {
        let mut doc = Document::with_version(pdfx::PDF_VERSION);
        let pages_id = doc.new_object_id();
        let font_id = doc.new_object_id();
        // An album that chose no face touches no disk here: `police` absent
        // means the face this crate ships, and it is read once per process.
        let choix = font::face_album(dir, album.police.as_ref().map(|p| p.fichier.as_str()));
        Self {
            doc,
            page_ids: Vec::new(),
            pages_id,
            font_id,
            ecrivain: Ecrivain::depuis(choix.face),
            police_defaut: choix.defaut,
            geom: geometry(album),
            bleed_mm: album.bleed_mm,
            title: album.title.clone(),
            stamp,
        }
    }

    /// `Some(code)` when the album named a face this document could not use
    /// and fell back to the project's. Never a silent export: the caller
    /// says it, in words, on the screen that asked for the file.
    pub fn police_defaut(&self) -> Option<&'static str> {
        self.police_defaut
    }


    /// One spread, drawn from its scene.
    ///
    /// The emitter walks [`crate::scene::Scene`] rather than rebuilding the
    /// rectangles and re-deciding, for the fourth time in this crate, that
    /// `garde`, `texte` and `colophon` are special. The scene comes out in
    /// paint order, so this loop is a translation and nothing else: same
    /// objects, same order, same bytes.
    ///
    /// The ink stays here. What colour a caption prints in is a rendering
    /// decision, not a property of the spread, and the scene is deliberately
    /// silent about it.
    pub fn add_spread(&mut self, spread: &Spread, assets: &[JpegAsset]) -> Result<()> {
        use crate::scene::{Role, Scene};
        // Measured in this document's face: the half-title fits its title by
        // shrinking it, so the scene the emitter draws from is the one laid
        // out in the face the emitter is about to set the page in.
        let scene = {
            let face = &self.ecrivain;
            Scene::of_avec(spread, &self.geom, &|s, pt| face.largeur_mm(s, pt))
        };

        let mut content = String::new();
        let mut xobjects = dictionary! {};

        for object in &scene.objects {
            match &object.role {
                Role::Photo { cell, .. } => {
                    // A spread whose thumbnails went missing is refused by the
                    // caller, never drawn short: this only guards the index.
                    let Some(asset) = assets.get(*cell) else { continue };
                    self.draw_image(&mut content, &mut xobjects, *cell, asset, &object.rect);
                }
                // Photo captions: 7 pt under the slot's bottom edge,
                // left-aligned on the slot. Printed as typed, never
                // truncated: the editor is the place that signals overflow.
                Role::PhotoCaption { text, at, .. } => {
                    text_op(&mut content, &mut self.ecrivain, at.x, at.y, PHOTO_CAPTION_SIZE_PT, INK, text);
                }
                // The three pages of text, now one role: the half-title's two
                // sizes, the text page's regular leading and the colophon's
                // quieter one all reach here as lines already placed.
                Role::Text { at, lines } => {
                    for l in lines {
                        text_op(
                            &mut content,
                            &mut self.ecrivain,
                            at.x,
                            at.y - l.dy_mm,
                            l.size_pt,
                            TEXT_INK,
                            &l.text,
                        );
                    }
                }
                Role::ChapterCaption { text, at } => {
                    text_op(&mut content, &mut self.ecrivain, at.x, at.y, SPREAD_CAPTION_SIZE_PT, INK, text);
                }
                // A free block: laid out upright inside its box by the scene,
                // then turned once around the box's centre. The lines know
                // nothing about the angle, which is why the same three numbers
                // draw it here, on the canvas and in the DOM.
                Role::FreeText { at, lines, .. } => {
                    let centre = crate::scene::centre(&object.rect);
                    for l in lines {
                        let p = crate::scene::tourner(
                            crate::pdf::Point { x: at.x + l.dx_mm, y: at.y - l.dy_mm },
                            centre,
                            object.angle,
                        );
                        text_op_tourne(
                            &mut content,
                            &mut self.ecrivain,
                            p.x,
                            p.y,
                            object.angle,
                            l.size_pt,
                            TEXT_INK,
                            &l.text,
                        );
                    }
                }
            }
        }

        let b = self.bleed_mm;
        self.add_page(
            Boxes {
                media: [self.geom.media_w, self.geom.media_h],
                trim: [b, b, self.geom.media_w - b, self.geom.media_h - b],
            },
            content,
            xobjects,
        );
        Ok(())
    }

    /// Cover-crop one image into `rect`: scale to fill (times the manual
    /// zoom), anchor on the focal point, clip to the rectangle. The one place
    /// that arithmetic exists on the Rust side; `album.ts::cropWindow` is its
    /// port, and the parity check compares the two.
    pub(crate) fn draw_image(
        &mut self,
        content: &mut String,
        xobjects: &mut lopdf::Dictionary,
        index: usize,
        asset: &JpegAsset,
        rect: &Rect,
    ) {
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
        let name = format!("Im{index}");
        xobjects.set(name.as_bytes(), Object::Reference(img_id));

        let (x, y, w, h) = (
            rect.x * MM_TO_PT,
            rect.y * MM_TO_PT,
            rect.w * MM_TO_PT,
            rect.h * MM_TO_PT,
        );
        let iw = asset.width as f64;
        let ih = asset.height as f64;
        let s = (w / iw).max(h / ih) * asset.zoom.max(1.0);
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

    /// One page, its boxes and its content. Every page in every file Colophon
    /// writes goes through here, spread or cover: the TrimBox is not
    /// something a second code path can forget.
    pub(crate) fn add_page(&mut self, boxes: Boxes, content: String, xobjects: lopdf::Dictionary) {
        let content_id = self
            .doc
            .add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let resources = dictionary! {
            "XObject" => xobjects,
            "Font" => dictionary! { "F1" => Object::Reference(self.font_id) },
        };
        let pt = |v: f64| Object::Real((v * MM_TO_PT) as f32);
        let page_id = self.doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(self.pages_id),
            "MediaBox" => vec![0.into(), 0.into(), pt(boxes.media[0]), pt(boxes.media[1])],
            "BleedBox" => vec![0.into(), 0.into(), pt(boxes.media[0]), pt(boxes.media[1])],
            "TrimBox" => vec![pt(boxes.trim[0]), pt(boxes.trim[1]), pt(boxes.trim[2]), pt(boxes.trim[3])],
            "Resources" => resources,
            "Contents" => Object::Reference(content_id),
        });
        self.page_ids.push(Object::Reference(page_id));
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
        // The glyph set is closed now, so the face can finally describe
        // itself: `/W` and `/ToUnicode` name what this document draws.
        embed_font(&mut self.doc, self.font_id, &self.ecrivain);
        // Colour, standard, dates and identity, all at once: a file that
        // carries some of them and not the others fails a supplier's
        // preflight exactly as loudly as one that carries none.
        let d = pdfx::declare(&mut self.doc, &self.title, self.stamp)?;
        let catalog_id = self.doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(self.pages_id),
            "OutputIntents" => d.output_intents,
            "Metadata" => Object::Reference(d.metadata),
        });
        self.doc.trailer.set("Root", catalog_id);
        self.doc.trailer.set("Info", Object::Reference(d.info));
        self.doc.trailer.set("ID", d.id);
        self.doc.compress();
        // The binary comment rides in the version string because that is the
        // only thing lopdf writes above the first object. Splicing it into the
        // finished bytes instead would shift every offset the cross-reference
        // table has already recorded, and produce a file that opens nowhere.
        self.doc.version = pdfx::header_line();
        self.doc.save(out).context("write pdf")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Size, Slot};

    /// Un `focal` est un point de l'image, donc la fenêtre se centre dessus
    /// quel que soit le ratio de la cellule. C'est tout 3.1 : une bascule de
    /// format ne doit pas déplacer ce que l'œil a cadré. Deux cellules de
    /// ratios opposés, même `focal`, même centre — au zoom 2, où les deux axes
    /// ont du jeu et où rien n'est donc borné.
    #[test]
    fn la_fenetre_se_centre_sur_le_meme_point_quel_que_soit_le_ratio() {
        let (iw, ih) = (4000.0, 3000.0);
        let focal = [0.62, 0.38];
        for (w, h) in [(300.0, 200.0), (200.0, 300.0)] {
            let rect = Rect { x: 0.0, y: 0.0, w, h };
            let (x0, y0, vw, vh) = crop_window(&rect, iw, ih, focal, 2.0);
            assert!(
                (x0 + vw / 2.0 - focal[0] * iw).abs() < 0.5,
                "centre x {} attendu {}", x0 + vw / 2.0, focal[0] * iw
            );
            assert!(
                (y0 + vh / 2.0 - focal[1] * ih).abs() < 0.5,
                "centre y {} attendu {}", y0 + vh / 2.0, focal[1] * ih
            );
        }
    }

    /// Un point contre le bord ne fait pas sortir la fenêtre de l'image : elle
    /// s'ancre au bord, et le point cesse d'être au centre. Le bornage est la
    /// seule chose qui a le droit de déplacer le cadrage.
    #[test]
    fn le_point_au_bord_ancre_la_fenetre_sans_la_sortir() {
        let (iw, ih) = (4000.0, 3000.0);
        let rect = Rect { x: 0.0, y: 0.0, w: 300.0, h: 200.0 };
        for focal in [[0.0, 0.0], [1.0, 1.0]] {
            let (x0, y0, vw, vh) = crop_window(&rect, iw, ih, focal, 2.0);
            assert!(x0 >= 0.0 && x0 + vw <= iw + 0.5, "x0 {x0} vw {vw}");
            assert!(y0 >= 0.0 && y0 + vh <= ih + 0.5, "y0 {y0} vh {vh}");
        }
    }

    /// One number carries the caption verdict: declared values pass through
    /// sign first, and the zero case keeps the historic hunt (positive on a
    /// free spot, negative when every candidate is covered).
    #[test]
    fn la_hauteur_signee_porte_le_verdict() {
        let g = SpreadGeometry { media_w: 426.0, media_h: 216.0, margin: 14.0, gutter: 7.0, bleed: 3.0 };
        assert!(caption_height_of(None, &[], &g) > 0.0);
        let tout = [Rect { x: 0.0, y: 0.0, w: g.media_w, h: g.media_h }];
        assert!(caption_height_of(None, &tout, &g) < 0.0);
        let duo = crate::gabarit::spec("duo").unwrap();
        let bande = crate::gabarit::Spec { legende: 8.0, ..duo.clone() };
        let sur = crate::gabarit::Spec { legende: -6.0, ..duo.clone() };
        assert_eq!(caption_height_of(Some(&bande), &tout, &g), 8.0);
        assert_eq!(caption_height_of(Some(&sur), &[], &g), -6.0);
    }

    /// The anchor follows the sign: a band hangs the baseline under the
    /// lifted frame, an overlay takes the reading-order spot without
    /// hunting, zero still hunts.
    #[test]
    fn l_ancre_suit_le_signe() {
        let g = SpreadGeometry { media_w: 426.0, media_h: 216.0, margin: 14.0, gutter: 7.0, bleed: 3.0 };
        let duo = crate::gabarit::spec("duo").unwrap();

        let bande = crate::gabarit::Spec { legende: 8.0, ..duo.clone() };
        let rects = crate::gabarit::slots(&bande, 2, &g);
        let bas = rects.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
        let at = caption_anchor_of(Some(&bande), &rects, &g);
        assert!((at.y - (bas - PHOTO_CAPTION_DROP_MM)).abs() < 1e-9);

        // A photo over the first candidate spot: the hunt moves to the next
        // one, the declared overlay stays put.
        let bas_gauche = [Rect { x: 0.0, y: 0.0, w: g.media_w / 2.0, h: g.media_h / 2.0 }];
        let sur = crate::gabarit::Spec { legende: -6.0, ..duo.clone() };
        let chasse = caption_anchor_of(None, &bas_gauche, &g);
        let surimpression = caption_anchor_of(Some(&sur), &bas_gauche, &g);
        assert!(chasse.x > surimpression.x);
    }

    /// Write a one-spread album and hand back its bytes plus the reopened
    /// document. Everything below reads the file the writer actually
    /// produced, never the structures it held in memory: the declaration is
    /// only worth what survives serialisation.
    fn written() -> (Vec<u8>, Document) {
        written_at(chrono::Local::now())
    }

    /// The same album at a pinned instant. Splitting the stamp out is what
    /// makes byte identity testable: it is the one field of a Colophon PDF
    /// that is not a function of the album.
    fn written_at(stamp: chrono::DateTime<chrono::Local>) -> (Vec<u8>, Document) {
        let mut album = Album::new("Été & cie", std::path::Path::new("."), Size { w: 210.0, h: 210.0 });
        album.spreads.push(Spread {
            template: "duo".into(),
            slots: vec![
                Slot { src: "a.jpg".into(), focal: [0.5, 0.5], zoom: 1.0, caption: Some("la plage".into()) },
                Slot::new("b.jpg".into(), [0.5, 0.5]),
            ],
            caption: Some("Corse, 2013".into()),
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        });
        let assets = vec![
            solid_jpeg([200, 30, 40], 160, 120).unwrap(),
            solid_jpeg([30, 120, 200], 160, 120).unwrap(),
        ];
        // Un album de test ne choisit aucune face : le dossier n'est
        // jamais lu, et le writer prend celle du projet.
        let mut w = PdfWriter::with_stamp(&album, std::path::Path::new("."), stamp);
        w.add_spread(&album.spreads[0], &assets).unwrap();
        let path = std::env::temp_dir().join(format!(
            "colophon-pdfx-{}-{:?}-{}.pdf",
            std::process::id(),
            std::thread::current().id(),
            stamp.timestamp_nanos_opt().unwrap_or_default()
        ));
        w.save(&path).expect("écriture");
        let bytes = std::fs::read(&path).unwrap();
        let doc = Document::load(&path).expect("relecture");
        let _ = std::fs::remove_file(&path);
        (bytes, doc)
    }

    /// Two exports of the same album at the same instant are the same file,
    /// byte for byte. Everything a Colophon PDF holds is a pure function of
    /// the album and its assets except the declared instant, so pinning that
    /// pins the bytes.
    ///
    /// This is the measuring instrument the scene port needs: a refactor of
    /// the emitter that leaves this diff empty displaced nothing, which no
    /// count of green counters can say as plainly.
    #[test]
    fn deux_exports_au_meme_instant_sont_le_meme_fichier() {
        use chrono::{Local, TimeZone};
        let t = Local.with_ymd_and_hms(2026, 8, 20, 11, 0, 0).unwrap();
        let (a, _) = written_at(t);
        let (b, _) = written_at(t);
        assert_eq!(a.len(), b.len(), "longueurs différentes");
        assert!(a == b, "deux exports au même instant diffèrent");

        // And the test is not vacuous: move the instant, the file moves.
        let (c, _) = written_at(Local.with_ymd_and_hms(2026, 8, 20, 11, 0, 1).unwrap());
        assert!(a != c, "l'horodatage ne se lit pas dans le fichier");
    }

    /// The probe of the load-bearing claim of 6.2: **an album that carries no
    /// free object produces the PDF it produced before free objects existed**,
    /// byte for byte.
    ///
    /// It cannot be a self-contained assertion — the reference lives on the
    /// other side of the change — so it is a bench, run on both trees, and the
    /// two files are compared outside. Ignored by default for that reason, and
    /// not because it is slow.
    #[test]
    #[ignore]
    fn banc_octets_d_un_album_sans_objet_libre() {
        use chrono::{Local, TimeZone};
        let t = Local.with_ymd_and_hms(2026, 9, 4, 11, 0, 0).unwrap();
        let (bytes, _) = written_at(t);
        let out = std::env::var("COLOPHON_OCTETS")
            .unwrap_or_else(|_| "/tmp/colophon-octets.pdf".to_string());
        std::fs::write(&out, &bytes).unwrap();
        println!("{} octets écrits dans {out}", bytes.len());
    }

    /// The content stream of a spread that carries one free object, so a test
    /// can read what the emitter actually wrote.
    fn flux_avec_objet(objet: crate::model::Objet) -> String {
        let mut album =
            Album::new("t", std::path::Path::new("."), Size { w: 210.0, h: 210.0 });
        album.spreads.push(Spread {
            template: "texte".into(),
            slots: vec![],
            caption: None,
            text: None,
            edited: false,
            locked: false,
            objets: vec![objet],
        });
        let mut w = PdfWriter::new(&album, std::path::Path::new("."));
        w.add_spread(&album.spreads[0], &[]).unwrap();
        let path = std::env::temp_dir()
            .join(format!("colophon-libre-{}-{:?}.pdf", std::process::id(), std::thread::current().id()));
        w.save(&path).unwrap();
        let doc = Document::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let pages: Vec<_> = doc.get_pages().values().copied().collect();
        let contents = doc
            .get_dictionary(pages[0])
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .as_reference()
            .unwrap();
        String::from_utf8_lossy(&flux(&doc, contents)).into_owned()
    }

    fn bloc_libre(angle: f64) -> crate::model::Objet {
        crate::model::Objet {
            x: 40.0,
            y: 60.0,
            w: 80.0,
            h: 30.0,
            angle,
            contenu: crate::model::Contenu::Texte {
                texte: "un bloc".into(),
                taille_pt: 12.0,
                interligne_mm: Some(6.0),
                alignement: crate::model::Alignement::Gauche,
            },
        }
    }

    /// Decision 4 of the 6.2 prompt, read back out of the file. An upright
    /// block places its lines with `Td` — the operator every album has always
    /// been written with, which is why no existing PDF moved — and a turned
    /// one with `Tm`, the *text* matrix rather than the graphics one, so
    /// nothing outside the `BT`/`ET` pair is disturbed and there is no `q`/`Q`
    /// to balance.
    #[test]
    fn un_bloc_droit_garde_td_un_bloc_tourne_passe_a_tm() {
        let droit = flux_avec_objet(bloc_libre(0.0));
        assert!(droit.contains(" Td "), "un bloc droit doit rester en Td :\n{droit}");
        assert!(!droit.contains(" Tm "), "un bloc droit n'écrit pas de matrice");

        let tourne = flux_avec_objet(bloc_libre(30.0));
        assert!(tourne.contains(" Tm "), "un bloc tourné doit poser sa matrice :\n{tourne}");
        assert!(!tourne.contains(" Td "), "et ne pas garder le placement droit");
        // The matrix is the rotation, not decoration: cos 30° = 0.86603,
        // sin 30° = 0.5, and the anti-diagonal carries the sign.
        assert!(
            tourne.contains("0.86603 0.50000 -0.50000 0.86603"),
            "matrice inattendue :\n{tourne}"
        );
        // And the probe bites: the two files are not the same file.
        assert_ne!(droit, tourne);
    }

    /// A free object reaches the page. Stated on its own because the byte
    /// identity test next door proves the *absence* of a change, and a pair
    /// of tests where one proves nothing moved is worth little without the
    /// other proving something can.
    #[test]
    fn un_objet_libre_atteint_la_page() {
        let sans = {
            let mut album =
                Album::new("t", std::path::Path::new("."), Size { w: 210.0, h: 210.0 });
            album.spreads.push(Spread {
                template: "texte".into(),
                slots: vec![],
                caption: None,
                text: None,
                edited: false,
                locked: false,
                objets: vec![],
            });
            let mut w = PdfWriter::new(&album, std::path::Path::new("."));
            w.add_spread(&album.spreads[0], &[]).unwrap();
            let path = std::env::temp_dir().join(format!(
                "colophon-sans-{}-{:?}.pdf",
                std::process::id(),
                std::thread::current().id()
            ));
            w.save(&path).unwrap();
            let doc = Document::load(&path).unwrap();
            let _ = std::fs::remove_file(&path);
            let pages: Vec<_> = doc.get_pages().values().copied().collect();
            let c = doc.get_dictionary(pages[0]).unwrap().get(b"Contents").unwrap()
                .as_reference().unwrap();
            String::from_utf8_lossy(&flux(&doc, c)).into_owned()
        };
        assert!(sans.trim().is_empty(), "une planche vide ne dessine rien : {sans:?}");
        assert!(!flux_avec_objet(bloc_libre(0.0)).trim().is_empty());
    }

    /// The header says 1.6 and the line under it carries the binary marker.
    /// Both are structural: a validator reads them before anything else.
    #[test]
    fn the_file_opens_on_a_pdf_x_header() {
        let (bytes, _) = written();
        assert!(bytes.starts_with(b"%PDF-1.6\n"), "{:?}", &bytes[..12]);
        let second = &bytes[9..];
        assert_eq!(second[0], b'%', "pas de commentaire sous l'en-tête");
        let high = second[1..9].iter().take_while(|b| **b >= 128).count();
        assert!(high >= 4, "{high} octets hauts, il en faut quatre");
    }

    /// The colour the file was made for travels inside it: two intents, one
    /// profile, and the profile is the sRGB asset rather than a name.
    #[test]
    fn the_output_intent_carries_the_profile() {
        let (_, doc) = written();
        let catalog = doc.catalog().expect("catalogue");
        let intents = catalog.get(b"OutputIntents").unwrap().as_array().unwrap();
        let subtypes: Vec<&str> = intents
            .iter()
            .map(|o| o.as_dict().unwrap().get(b"S").unwrap().as_name_str().unwrap())
            .collect();
        assert!(subtypes.contains(&"GTS_PDFX"), "{subtypes:?}");
        assert!(subtypes.contains(&"GTS_PDFA1"), "{subtypes:?}");

        // Both point at the same stream: PDF/A refuses a file whose intents
        // disagree on the destination.
        let profiles: Vec<lopdf::ObjectId> = intents
            .iter()
            .map(|o| match o.as_dict().unwrap().get(b"DestOutputProfile").unwrap() {
                Object::Reference(id) => *id,
                other => panic!("profil non référencé : {other:?}"),
            })
            .collect();
        assert_eq!(profiles[0], profiles[1]);

        let stream = doc.get_object(profiles[0]).unwrap().as_stream().unwrap();
        assert_eq!(stream.dict.get(b"N").unwrap().as_i64().unwrap(), 3);
        let icc = stream.decompressed_content().unwrap();
        assert_eq!(icc, crate::icc::ICC_DATA, "le profil embarqué n'est pas l'asset");
    }

    /// The XMP packet is in the file, readable without unpacking a filter,
    /// and names the level the printer's preflight looks for.
    #[test]
    fn the_metadata_names_the_standard() {
        let (_, doc) = written();
        let catalog = doc.catalog().unwrap();
        let id = match catalog.get(b"Metadata").unwrap() {
            Object::Reference(id) => *id,
            other => panic!("métadonnées non référencées : {other:?}"),
        };
        let stream = doc.get_object(id).unwrap().as_stream().unwrap();
        assert!(stream.dict.get(b"Filter").is_err(), "le paquet XMP est compressé");
        let xmp = String::from_utf8(stream.content.clone()).expect("XMP en UTF-8");
        assert!(xmp.contains("<pdfxid:GTS_PDFXVersion>PDF/X-4</pdfxid:GTS_PDFXVersion>"));
        assert!(xmp.contains("<pdfaid:part>2</pdfaid:part>"));
        // The album's own title, escaped, not the file name.
        assert!(xmp.contains("Été &amp; cie"), "{xmp}");
    }

    /// The `/F1` of the first page, whatever it turns out to be.
    fn police(doc: &Document) -> lopdf::Dictionary {
        let pages = doc.get_pages();
        let (_, page_id) = pages.iter().next().expect("une page");
        let page_id = *page_id;
        let resources = doc.get_page_resources(page_id).unwrap().0.expect("des ressources");
        let fonts = resources.get(b"Font").unwrap().as_dict().unwrap();
        let id = match fonts.get(b"F1").unwrap() {
            Object::Reference(id) => *id,
            other => panic!("police non référencée : {other:?}"),
        };
        doc.get_object(id).unwrap().as_dict().unwrap().clone()
    }

    /// The bytes of a stream, whether or not the writer compressed it.
    fn flux(doc: &Document, id: lopdf::ObjectId) -> Vec<u8> {
        let stream = doc.get_object(id).unwrap().as_stream().unwrap();
        stream.decompressed_content().unwrap_or_else(|_| stream.content.clone())
    }

    /// `/W`, flattened back into the advance of each glyph it names.
    fn chasses_declarees(descendant: &lopdf::Dictionary) -> std::collections::BTreeMap<u16, i64> {
        let mut out = std::collections::BTreeMap::new();
        let w = descendant.get(b"W").unwrap().as_array().unwrap();
        let mut i = 0;
        while i < w.len() {
            let debut = w[i].as_i64().unwrap() as u16;
            let run = w[i + 1].as_array().unwrap();
            for (n, v) in run.iter().enumerate() {
                out.insert(debut + n as u16, v.as_i64().unwrap());
            }
            i += 2;
        }
        out
    }

    /// The face goes in as a composite: `/Type0` under Identity-H over a
    /// `/CIDFontType2`. The code in the content stream is the glyph id, so
    /// nothing between us and the reader depends on an encoding table — which
    /// is what lets a caption carry any character the face can draw.
    ///
    /// It also goes in whole, so it wears no subset prefix and declares no
    /// `/CIDSet`: both would announce a subset that does not exist.
    #[test]
    fn la_police_du_fichier_est_un_composite_identity_h() {
        let (_, doc) = written();
        let f = police(&doc);
        assert_eq!(f.get(b"Subtype").unwrap().as_name_str().unwrap(), "Type0");
        assert_eq!(f.get(b"Encoding").unwrap().as_name_str().unwrap(), "Identity-H");
        assert!(f.get(b"ToUnicode").is_ok(), "sans ToUnicode, la page ne se copie pas");

        let base = f.get(b"BaseFont").unwrap().as_name_str().unwrap().to_string();
        assert_eq!(base, font::FONT_NAME, "la face se nomme elle-même");
        assert!(!base.contains('+'), "préfixe de sous-ensemble sur une face entière : {base}");

        let d = match &f.get(b"DescendantFonts").unwrap().as_array().unwrap()[0] {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
            other => panic!("descendant non référencé : {other:?}"),
        };
        assert_eq!(d.get(b"Subtype").unwrap().as_name_str().unwrap(), "CIDFontType2");
        assert_eq!(d.get(b"CIDToGIDMap").unwrap().as_name_str().unwrap(), "Identity");
        let cid = d.get(b"CIDSystemInfo").unwrap().as_dict().unwrap();
        assert_eq!(cid.get(b"Ordering").unwrap().as_str().unwrap(), b"Identity");

        let descr = match d.get(b"FontDescriptor").unwrap() {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
            other => panic!("descripteur non référencé : {other:?}"),
        };
        assert!(descr.get(b"FontFile2").is_ok(), "la face n'est pas dans le fichier");
        assert!(descr.get(b"CIDSet").is_err(), "un CIDSet annonce un sous-ensemble");
    }

    /// What was measured is what is declared. The engine set the album's type
    /// on these advances, and `/W` carries the same numbers: a reader that
    /// trusts the table and an engine that trusted `hmtx` place the same
    /// glyphs in the same spots.
    #[test]
    fn les_chasses_declarees_sont_celles_qui_ont_mesure() {
        let (_, doc) = written();
        let f = police(&doc);
        let d = match &f.get(b"DescendantFonts").unwrap().as_array().unwrap()[0] {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
            other => panic!("{other:?}"),
        };
        let declarees = chasses_declarees(&d);
        let face = font::Embarquee::incorporee().expect("face ouverte");

        // La légende de la planche, mesurée d'un côté, déclarée de l'autre.
        let ligne = "la plage";
        let em: i64 = face
            .glyphes(ligne)
            .iter()
            .map(|(gid, _)| *declarees.get(gid).unwrap_or_else(|| panic!("glyphe {gid} hors /W")))
            .sum();
        let large = em as f64 / 1000.0 * PHOTO_CAPTION_SIZE_PT * 25.4 / 72.0;
        assert!(
            (large - font::text_width_mm(ligne, PHOTO_CAPTION_SIZE_PT)).abs() < 1e-9,
            "{large} contre {}",
            font::text_width_mm(ligne, PHOTO_CAPTION_SIZE_PT)
        );

        // Et `/W` ne porte que ce qui a été dessiné : la face en a deux mille.
        assert!(declarees.len() < 100, "{} glyphes déclarés", declarees.len());

        // Les tranches sortent dans l'ordre des glyphes. Ce n'est pas de
        // l'esthétique : un ensemble non ordonné rendrait un `/W` différent
        // d'un export à l'autre, et deux exports du même album doivent être
        // le même fichier. La reproductibilité l'attrape aussi — mesuré le
        // 29/08 en remplaçant la carte ordonnée par une carte de hachage,
        // les deux tests tombent — mais elle le dit par un fichier qui
        // diffère, là où ceci nomme la règle.
        let w = d.get(b"W").unwrap().as_array().unwrap();
        let debuts: Vec<i64> = w.iter().step_by(2).map(|v| v.as_i64().unwrap()).collect();
        assert!(debuts.windows(2).all(|p| p[0] < p[1]), "/W dans le désordre : {debuts:?}");
    }

    /// Every glyph the file draws can say which character it is, so the page
    /// can be read back out of the print. Without that, PDF/A would be a
    /// claim rather than a fact.
    #[test]
    fn chaque_glyphe_dessine_sait_dire_son_caractere() {
        let (_, doc) = written();
        let f = police(&doc);
        let id = match f.get(b"ToUnicode").unwrap() {
            Object::Reference(id) => *id,
            other => panic!("{other:?}"),
        };
        let cmap = String::from_utf8(flux(&doc, id)).expect("un CMap est du texte");
        assert!(cmap.contains("begincmap") && cmap.contains("endcmap"));
        assert!(cmap.contains("<0000> <FFFF>"), "l'espace de codes des deux octets");

        let face = font::Embarquee::incorporee().expect("face ouverte");
        // La légende de planche porte une accentuée : elle doit se relire.
        for (gid, c) in face.glyphes("Corse, 2013") {
            let attendu = format!("<{gid:04X}> <{:04X}>", c as u32);
            assert!(cmap.contains(&attendu), "{c} manque au ToUnicode : {attendu}");
        }
    }

    /// The wave's first visible gain: a caption the old single-byte encoding
    /// could only print as `?` now draws its own glyphs. The engine measured
    /// them, the file carries them, and the reader can copy them back.
    #[test]
    fn une_legende_hors_winansi_se_dessine_au_lieu_de_se_perdre() {
        let face = font::Embarquee::incorporee().expect("face ouverte");
        let mot = "Zażółć";
        let poses = face.glyphes(mot);
        assert_eq!(poses.len(), mot.chars().count());
        let interro = face.glyphes("?")[0].0;
        for (gid, c) in &poses {
            assert_ne!(*gid, interro, "{c} tombe encore sur le point d'interrogation");
        }
        // Et la mesure suit le dessin, sinon la coupure de ligne mentirait.
        assert!(font::text_width_mm(mot, 10.0) > font::text_width_mm("Za", 10.0));
    }

    /// The face pulled out of its file describes itself exactly like the one
    /// read from it. Session 4 will embed extracted faces; it inherits no
    /// unknown about how they enter a PDF.
    #[test]
    fn la_face_extraite_donne_le_meme_composite() {
        let sortie = font::Face::extraire(font::FONT_DATA, 0).expect("extraction");
        assert_ne!(sortie.len(), font::FONT_DATA.len(), "l'extraction n'a rien retiré");

        let depuis = |octets: Vec<u8>| {
            let mut doc = Document::with_version(pdfx::PDF_VERSION);
            let id = doc.new_object_id();
            let face = font::Embarquee::depuis(octets, 0).expect("face ouverte");
            let mut ecrivain =
                Ecrivain { face: std::borrow::Cow::Owned(face), utilises: font::Utilises::default() };
            let mut content = String::new();
            text_op(&mut content, &mut ecrivain, 0.0, 0.0, 10.0, INK, "Corse, 2013");
            embed_font(&mut doc, id, &ecrivain);
            (doc, id, content)
        };
        let (a, ia, ca) = depuis(font::FONT_DATA.to_vec());
        let (b, ib, cb) = depuis(sortie);

        assert_eq!(ca, cb, "les mêmes glyphes, aux mêmes places");
        let sans_flux = |doc: &Document, id: lopdf::ObjectId| {
            let f = doc.get_object(id).unwrap().as_dict().unwrap().clone();
            let d = match &f.get(b"DescendantFonts").unwrap().as_array().unwrap()[0] {
                Object::Reference(r) => doc.get_object(*r).unwrap().as_dict().unwrap().clone(),
                other => panic!("{other:?}"),
            };
            (
                f.get(b"Encoding").unwrap().as_name_str().unwrap().to_string(),
                f.get(b"BaseFont").unwrap().as_name_str().unwrap().to_string(),
                chasses_declarees(&d),
            )
        };
        assert_eq!(sans_flux(&a, ia), sans_flux(&b, ib));
    }

    /// The same one-spread album, in a folder of its own, set in whatever
    /// face that folder carries. `police` is what `album.json` would say.
    fn ecrit_dans(
        dir: &std::path::Path,
        police: Option<crate::model::Police>,
    ) -> (Document, Option<&'static str>) {
        let mut album =
            Album::new("Été & cie", std::path::Path::new("."), Size { w: 210.0, h: 210.0 });
        album.police = police;
        album.spreads.push(Spread {
            template: "duo".into(),
            slots: vec![
                Slot { src: "a.jpg".into(), focal: [0.5, 0.5], zoom: 1.0, caption: Some("la plage".into()) },
                Slot::new("b.jpg".into(), [0.5, 0.5]),
            ],
            caption: Some("Corse, 2013".into()),
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        });
        let assets = vec![
            solid_jpeg([200, 30, 40], 160, 120).unwrap(),
            solid_jpeg([30, 120, 200], 160, 120).unwrap(),
        ];
        let mut w = PdfWriter::new(&album, dir);
        let defaut = w.police_defaut();
        w.add_spread(&album.spreads[0], &assets).unwrap();
        let path = dir.join("album.pdf");
        w.save(&path).expect("écriture");
        (Document::load(&path).expect("relecture"), defaut)
    }

    /// **The file copied beside the album is the file inside the PDF.**
    ///
    /// Not « the same face », not « the same metrics »: the same bytes. The
    /// album's folder holds one face, the emitter embeds one face, and if
    /// those two ever diverge the book prints in something nobody chose. The
    /// name follows the same rule — the face names itself, and Source Sans 3
    /// is nowhere in the file.
    #[test]
    fn les_octets_poses_a_cote_de_lalbum_sont_ceux_du_fichier() {
        let dir = std::env::temp_dir()
            .join(format!("colophon-police-pdf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Une vraie face du fichier de test, extraite comme le fait l'app.
        let (nom, octets) =
            font::extraire_pour_album(font::FONT_DATA, 0).expect("extraction");
        std::fs::write(dir.join(nom), &octets).unwrap();
        let choix = crate::model::Police {
            fichier: nom.to_string(),
            postscript: "SourceSans3-Regular".into(),
            nom: "Source Sans 3 Regular".into(),
        };

        let (doc, defaut) = ecrit_dans(&dir, Some(choix));
        assert!(defaut.is_none(), "la face est là, rien à signaler");
        let f = police(&doc);
        let descendant = match &f.get(b"DescendantFonts").unwrap().as_array().unwrap()[0] {
            Object::Reference(r) => doc.get_object(*r).unwrap().as_dict().unwrap().clone(),
            other => panic!("{other:?}"),
        };
        let descr = match descendant.get(b"FontDescriptor").unwrap() {
            Object::Reference(r) => doc.get_object(*r).unwrap().as_dict().unwrap().clone(),
            other => panic!("{other:?}"),
        };
        let file_id = match descr.get(b"FontFile2").unwrap() {
            Object::Reference(r) => *r,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            flux(&doc, file_id),
            octets,
            "le FontFile2 n'est pas le fichier posé à côté de l'album"
        );
        // Et l'extraction a bien retiré quelque chose : sans ça l'égalité
        // ci-dessus serait vraie pour la mauvaise raison.
        assert_ne!(octets.len(), font::FONT_DATA.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A face named and gone does not fail, and does not lie.** The album
    /// comes out in the face this crate ships, and the writer carries the
    /// reason so the screen can say it. Never a silent export failure, and
    /// never a book printed in a face nobody chose without knowing.
    #[test]
    fn une_police_effacee_a_la_main_sort_quand_meme_et_se_dit() {
        let dir = std::env::temp_dir()
            .join(format!("colophon-police-absente-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (doc, defaut) = ecrit_dans(
            &dir,
            Some(crate::model::Police {
                fichier: font::POLICE_TTF.into(),
                postscript: "HelveticaNeue".into(),
                nom: "Helvetica Neue".into(),
            }),
        );
        assert_eq!(defaut, Some(font::REFUS_FICHIER_ABSENT));
        let f = police(&doc);
        assert_eq!(
            f.get(b"BaseFont").unwrap().as_name_str().unwrap(),
            font::FONT_NAME,
            "la face du projet, nommée d'après elle-même"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `/Info` answers the trapping question and dates the file, and the
    /// trailer identifies it. PDF/X refuses a file missing any of the three.

    #[test]
    fn the_information_dictionary_is_complete() {
        let (_, doc) = written();
        let info = match doc.trailer.get(b"Info").unwrap() {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap(),
            other => panic!("Info non référencé : {other:?}"),
        };
        assert_eq!(info.get(b"Trapped").unwrap().as_name_str().unwrap(), "False");
        let created = info.get(b"CreationDate").unwrap().as_str().unwrap();
        assert!(created.starts_with(b"D:"), "{:?}", String::from_utf8_lossy(created));
        assert_eq!(info.get(b"ModDate").unwrap().as_str().unwrap(), created);

        let ids = doc.trailer.get(b"ID").unwrap().as_array().unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].as_str().unwrap().len(), 16, "identifiant de 16 octets");
    }

    /// Every page says where the knife goes, and the trim never leaves the
    /// sheet. This is the one geometric rule PDF/X adds to the others, and
    /// the whole point of the bleed the composer lays down.
    #[test]
    fn every_page_marks_its_trim() {
        let (_, doc) = written();
        for (_, page_id) in doc.get_pages() {
            let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
            let nums = |k: &[u8]| -> Vec<f64> {
                page.get(k)
                    .unwrap_or_else(|_| panic!("{} absent", String::from_utf8_lossy(k)))
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|o| o.as_float().unwrap() as f64)
                    .collect()
            };
            let media = nums(b"MediaBox");
            let trim = nums(b"TrimBox");
            assert!(trim[0] > media[0] && trim[1] > media[1], "le fond perdu est nul");
            assert!(trim[2] < media[2] && trim[3] < media[3], "trim {trim:?} media {media:?}");
        }
    }

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

