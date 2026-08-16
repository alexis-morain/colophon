//! The cover, as the printer receives it: one wide sheet, laid flat.
//!
//! Left to right, that is the back cover, the spine, and the front. The
//! interior is a book of double pages; the cover is a single piece of card
//! that wraps around it, so it is its own file with its own geometry and its
//! own bleed.
//!
//! Two things here belong to the printer profile and never to this code. The
//! **spine** exists only when the supplier asks us for it
//! ([`Dos::Calcule`]); Prodigi builds its own and gets a sheet without one.
//! The **bleed** is the profile's, edge by edge, and it is not the interior's:
//! a cover is trimmed on all four sides, so the spine-side value that exists
//! for an interior page has no meaning here.
//!
//! Every measurement below starts at the trim, never at the sheet edge. A
//! title placed 9 mm from the card is not 9 mm from the finished book, and
//! that difference is exactly what the preflight caught on the captions.

use crate::font;
use crate::model::{Album, Cover};
use crate::pdf::{self, Boxes, PdfWriter, Rect};
use crate::printer::{PrinterProfile, GRAMMAGE_DEFAUT};
use crate::{meta, print};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Title size on a 210 mm wide cover, scaled with the format from there.
const TITLE_PT_AT_210: f64 = 26.0;
const SUBTITLE_PT_AT_210: f64 = 12.0;
const BACK_TEXT_PT_AT_210: f64 = 10.5;
const SPINE_PT_AT_210: f64 = 11.0;

/// The title block sits this far in from the trim, as a share of the panel.
/// Same 9 % the cover editor shows, so what is on screen is what prints.
const TITLE_INSET: f64 = 0.09;

/// Below this, a spine is a fold and not a surface: nothing is printed on it.
/// Type on a 6 mm spine misses the fold by more than its own height.
pub const SPINE_TEXT_MIN_MM: f64 = 9.0;

/// Line spacing of the back-cover text, in multiples of its size.
const BACK_LEADING: f64 = 1.45;

/// White, for type over a photo, and the hard shadow under it. A blurred
/// halo is a screen effect; on paper it prints as a smudge, so the shadow is
/// one hard offset instead — the same shape the editor now shows.
const PAPER: [f64; 3] = [0.99, 0.97, 0.94];
const SHADOW: [f64; 3] = [0.09, 0.08, 0.06];
const SHADOW_OFFSET: f64 = 0.45;

/// The flat sheet, in millimetres, origin bottom-left of the card.
///
/// `back`, `spine` and `front` are **trim** rectangles: the finished panels,
/// with the bleed already excluded. The sheet around them is what the knife
/// takes off.
#[derive(Debug, Clone)]
pub struct CoverGeometry {
    pub media_w: f64,
    pub media_h: f64,
    /// Bleed actually applied, edge by edge, from the profile.
    pub bleed_ext: f64,
    pub bleed_haut: f64,
    pub bleed_bas: f64,
    pub back: Rect,
    /// Absent when the supplier builds the spine itself.
    pub spine: Option<Rect>,
    pub front: Rect,
    /// Keep-clear distance from the trim, from the profile.
    pub safe: f64,
}

impl CoverGeometry {
    /// The trim rectangle of the whole sheet: `[x0, y0, x1, y1]`.
    pub fn trim(&self) -> [f64; 4] {
        [
            self.bleed_ext,
            self.bleed_bas,
            self.media_w - self.bleed_ext,
            self.media_h - self.bleed_haut,
        ]
    }

    /// Spine width in millimetres, zero when there is none.
    pub fn spine_mm(&self) -> f64 {
        self.spine.as_ref().map_or(0.0, |r| r.w)
    }
}

/// Work out the flat sheet for an album at a supplier.
///
/// The width is the whole point of this function, and the one number the
/// definition of done asks to measure: twice the page, plus the spine the
/// profile computes, plus the bleed on both outer edges.
pub fn geometry(album: &Album, profil: &PrinterProfile) -> CoverGeometry {
    let pages = album.spreads.len() * 2;
    let spine_w = profil.dos_mm(pages, GRAMMAGE_DEFAUT).unwrap_or(0.0);
    let b = &profil.bleed_mm;
    let (ext, haut, bas) = (b.exterieur, b.haut, b.bas);

    let media_w = album.trim_mm.w * 2.0 + spine_w + ext * 2.0;
    let media_h = album.trim_mm.h + haut + bas;
    let panel = |x: f64, w: f64| Rect { x, y: bas, w, h: album.trim_mm.h };

    let back = panel(ext, album.trim_mm.w);
    let spine = profil
        .dos_mm(pages, GRAMMAGE_DEFAUT)
        .map(|w| panel(ext + album.trim_mm.w, w));
    let front = panel(ext + album.trim_mm.w + spine_w, album.trim_mm.w);

    CoverGeometry {
        media_w,
        media_h,
        bleed_ext: ext,
        bleed_haut: haut,
        bleed_bas: bas,
        back,
        spine,
        front,
        safe: profil.safe_mm,
    }
}

/// Render `album-cover.pdf` next to the album, at print resolution.
///
/// Fails loudly on a missing original, like the interior render: a cover with
/// a hole in it costs a reprint of the whole book.
pub fn render_cover_pdf(
    dir: &Path,
    profil: &'static PrinterProfile,
    out: &Path,
) -> Result<PathBuf> {
    let json = dir.join("album.json");
    let album: Album = serde_json::from_str(
        &fs::read_to_string(&json).with_context(|| format!("lecture de {}", json.display()))?,
    )
    .context("album.json illisible")?;

    let g = geometry(&album, profil);
    // An album composed before the cover editor has no cover: its title is
    // the one thing we know, and it is enough for a first sheet.
    let cover = album.cover.clone().unwrap_or(Cover {
        title: album.title.clone(),
        subtitle: String::new(),
        photo: None,
        back_text: String::new(),
    });

    let mut writer = PdfWriter::new(&album);
    let mut content = String::new();
    let mut xobjects = lopdf::dictionary! {};

    // The photo bleeds off the top, the bottom and the outer edge, and stops
    // dead at the fold: an image running onto the spine is folded in half.
    if let Some(slot) = &cover.photo {
        let root = std::path::PathBuf::from(&album.root);
        anyhow::ensure!(
            root.is_dir(),
            "dossier de photos introuvable : {} (déplacé ou disque absent ?)",
            root.display()
        );
        let src = root.join(&slot.src);
        let rect = photo_rect(&g);
        let orientation = meta::read(&src).orientation;
        let asset = print::print_asset(&src, orientation, &rect, slot.focal, slot.zoom)
            .with_context(|| format!("photo de couverture : {}", slot.src))?;
        writer.draw_image(&mut content, &mut xobjects, 0, &asset, &rect);
    }

    draw_text(&mut content, &g, &album, &cover);

    writer.add_page(
        Boxes { media: [g.media_w, g.media_h], trim: g.trim() },
        content,
        xobjects,
    );

    let tmp = out.with_extension("pdf.part");
    let saved = writer.save(&tmp);
    if saved.is_err() {
        let _ = fs::remove_file(&tmp);
        saved?;
    }
    fs::rename(&tmp, out)
        .with_context(|| format!("renommage vers {}", out.display()))?;
    Ok(out.to_path_buf())
}

/// Where the front-cover photo goes: the front panel, bled outward on three
/// sides, cut off at the fold.
pub fn photo_rect(g: &CoverGeometry) -> Rect {
    Rect {
        x: g.front.x,
        y: 0.0,
        w: g.front.w + g.bleed_ext,
        h: g.media_h,
    }
}

/// Which face of the cover a single leaf carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// Première de couverture: the photo, the title, the subtitle.
    Premiere,
    /// Quatrième de couverture: the dedication, and nothing else.
    Quatrieme,
}

/// The cover as a single leaf, for a supplier that binds one file and reads
/// its first and last page as the cover.
///
/// Not a flat sheet cut in two. A leaf is trimmed on all four sides and has
/// no fold, so there is no spine here and no spine-side edge to protect: the
/// supplier wraps the boards themselves and the width they need is theirs to
/// know, which is exactly why they ask for a single file.
pub fn page_geometry(album: &Album, profil: &PrinterProfile, face: Face) -> CoverGeometry {
    let b = &profil.bleed_mm;
    let (ext, haut, bas) = (b.exterieur, b.haut, b.bas);
    let media_w = album.trim_mm.w + ext * 2.0;
    let media_h = album.trim_mm.h + haut + bas;
    let panel = Rect { x: ext, y: bas, w: album.trim_mm.w, h: album.trim_mm.h };
    // The face this leaf does not carry is given no surface at all, so a
    // caller that draws the wrong one draws nothing rather than something in
    // the wrong corner.
    let absent = Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };
    let (back, front) = match face {
        Face::Premiere => (absent, panel),
        Face::Quatrieme => (panel, absent),
    };

    CoverGeometry {
        media_w,
        media_h,
        bleed_ext: ext,
        bleed_haut: haut,
        bleed_bas: bas,
        back,
        spine: None,
        front,
        safe: profil.safe_mm,
    }
}

/// Draw one cover face as the next page of `writer`.
///
/// The same title, photo and dedication the flat sheet carries, on a leaf
/// instead of a panel. Called by the interior render for the suppliers that
/// take a single file, so that the first page of that file is the front
/// cover and the last is the back.
pub(crate) fn add_cover_page(
    writer: &mut PdfWriter,
    album: &Album,
    profil: &PrinterProfile,
    face: Face,
) -> Result<()> {
    let g = page_geometry(album, profil, face);
    let cover = album.cover.clone().unwrap_or(Cover {
        title: album.title.clone(),
        subtitle: String::new(),
        photo: None,
        back_text: String::new(),
    });

    let mut content = String::new();
    let mut xobjects = lopdf::dictionary! {};

    match face {
        Face::Premiere => {
            // On a leaf the photo bleeds on all four sides: there is no fold
            // to stop it at.
            if let Some(slot) = &cover.photo {
                let root = std::path::PathBuf::from(&album.root);
                anyhow::ensure!(
                    root.is_dir(),
                    "dossier de photos introuvable : {} (déplacé ou disque absent ?)",
                    root.display()
                );
                let src = root.join(&slot.src);
                let rect = Rect { x: 0.0, y: 0.0, w: g.media_w, h: g.media_h };
                let orientation = meta::read(&src).orientation;
                let asset = print::print_asset(&src, orientation, &rect, slot.focal, slot.zoom)
                    .with_context(|| format!("photo de couverture : {}", slot.src))?;
                writer.draw_image(&mut content, &mut xobjects, 0, &asset, &rect);
            }
            draw_front(&mut content, &g, album, &cover);
        }
        Face::Quatrieme => draw_back(&mut content, &g, album, &cover),
    }

    writer.add_page(
        Boxes { media: [g.media_w, g.media_h], trim: g.trim() },
        content,
        xobjects,
    );
    Ok(())
}

/// Type on the three panels. Sizes scale with the page so a 30 × 30 album
/// does not wear a 21 × 21 album's title.
///
/// The three faces draw separately because they do not always share a sheet:
/// a supplier that binds a single file gets the front and the back as two
/// leaves of the interior, with no spine between them.
fn draw_text(content: &mut String, g: &CoverGeometry, album: &Album, cover: &Cover) {
    draw_front(content, g, album, cover);
    draw_spine(content, g, album, cover);
    draw_back(content, g, album, cover);
}

/// The title of the album, whether or not the cover editor has been opened.
fn cover_title<'a>(album: &'a Album, cover: &'a Cover) -> &'a str {
    if cover.title.is_empty() { album.title.as_str() } else { cover.title.as_str() }
}

/// Front: title block, bottom left, inside the trim by the same share the
/// editor shows. Baselines stack upward from the subtitle.
fn draw_front(content: &mut String, g: &CoverGeometry, album: &Album, cover: &Cover) {
    let scale = album.trim_mm.w / 210.0;
    let title_pt = TITLE_PT_AT_210 * scale;
    let subtitle_pt = SUBTITLE_PT_AT_210 * scale;
    let over_photo = cover.photo.is_some();

    let x = g.front.x + g.front.w * TITLE_INSET;
    let y = g.front.y + g.front.h * TITLE_INSET;
    let title = cover_title(album, cover);
    let (subtitle_y, title_y) = if cover.subtitle.is_empty() {
        (y, y)
    } else {
        (y, y + title_pt * PT_TO_MM * 1.25)
    };
    plate(content, x, subtitle_y, subtitle_pt, over_photo, &cover.subtitle);
    plate(content, x, title_y, title_pt, over_photo, title);
}

/// Spine: the title along the fold, running bottom to top, centred. Only when
/// there is a surface to print on, and never on a single leaf.
fn draw_spine(content: &mut String, g: &CoverGeometry, album: &Album, cover: &Cover) {
    let scale = album.trim_mm.w / 210.0;
    if let Some(spine) = &g.spine {
        if spine.w >= SPINE_TEXT_MIN_MM {
            let title = cover_title(album, cover);
            let size = SPINE_PT_AT_210 * scale;
            let width = font::text_width_mm(title, size);
            let cx = spine.x + spine.w / 2.0 - size * PT_TO_MM * 0.35;
            let cy = spine.y + (spine.h - width) / 2.0;
            rotated(content, cx, cy, size, pdf::TEXT_INK, title);
        }
    }
}

/// Back: the quatrième, wrapped to the panel and centred in it, the way the
/// cover editor shows it. A dedication is a short block on a wide white page;
/// ranged left in a corner it reads like a caption.
fn draw_back(content: &mut String, g: &CoverGeometry, album: &Album, cover: &Cover) {
    let scale = album.trim_mm.w / 210.0;
    if !cover.back_text.is_empty() {
        let size = BACK_TEXT_PT_AT_210 * scale;
        let box_w = g.back.w - 2.0 * (g.back.w * TITLE_INSET);
        let leading = size * PT_TO_MM * BACK_LEADING;
        let lines = wrap(&cover.back_text, box_w, size);
        let block = lines.len() as f64 * leading;
        let mut y = g.back.y + (g.back.h + block) / 2.0 - leading;
        for line in &lines {
            // Centred on the panel, measured on the real advance widths.
            let x = g.back.x + (g.back.w - font::text_width_mm(line, size)) / 2.0;
            pdf::text_op(content, x, y, size, pdf::TEXT_INK, line);
            y -= leading;
            if y < g.back.y + g.safe {
                break; // the editor is where an overlong quatrième is signalled
            }
        }
    }
}

const PT_TO_MM: f64 = 25.4 / 72.0;

/// A line of cover type: white over the photo with one hard shadow under it,
/// plain ink on a bare panel. Two draws rather than a blur, because a blur
/// needs transparency and transparency is what a print file argues about.
fn plate(content: &mut String, x: f64, y: f64, size: f64, over_photo: bool, s: &str) {
    if s.is_empty() {
        return;
    }
    if over_photo {
        pdf::text_op(content, x + SHADOW_OFFSET, y - SHADOW_OFFSET, size, SHADOW, s);
        pdf::text_op(content, x, y, size, PAPER, s);
    } else {
        pdf::text_op(content, x, y, size, pdf::TEXT_INK, s);
    }
}

/// Text turned a quarter turn anticlockwise, so the spine reads bottom to
/// top. One direction had to be picked and this is the one the cover editor
/// shows; a spine printed the other way up is a reprint, so the two sides
/// agree here and the choice is written down rather than left to a default.
fn rotated(content: &mut String, x: f64, y: f64, size: f64, rgb: [f64; 3], s: &str) {
    let mut run = String::new();
    pdf::text_op(&mut run, 0.0, 0.0, size, rgb, s);
    let mm_to_pt = 72.0 / 25.4;
    content.push_str(&format!(
        "q 0 1 -1 0 {:.2} {:.2} cm\n{run}Q\n",
        x * mm_to_pt,
        y * mm_to_pt
    ));
}

/// Greedy wrap on the real advance widths of the embedded face. Words longer
/// than the measure are left whole and overflow visibly rather than being cut
/// in a place no reader would cut them.
fn wrap(text: &str, width_mm: f64, size_pt: f64) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.lines() {
        if para.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate =
                if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if font::text_width_mm(&candidate, size_pt) <= width_mm || line.is_empty() {
                line = candidate;
            } else {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Size;

    fn album_de(spreads: usize) -> Album {
        let mut a = Album::new("Corse", std::path::Path::new("/photos"), Size { w: 210.0, h: 210.0 });
        for _ in 0..spreads {
            a.spreads.push(crate::model::Spread {
                template: "solo".into(),
                slots: vec![],
                caption: None,
                text: None,
                edited: false,
                locked: false,
            });
        }
        a
    }

    /// The sheet is twice the page, plus the spine, plus the bleed. Measured
    /// against the profile's own numbers rather than against a constant, so a
    /// profile change moves the cover with it.
    #[test]
    fn the_sheet_is_two_pages_plus_the_spine_plus_the_bleed() {
        let a = album_de(48); // 96 pages
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let g = geometry(&a, cp);
        let spine = cp.dos_mm(96, GRAMMAGE_DEFAUT).unwrap();

        assert!((g.media_w - (210.0 * 2.0 + spine + 3.0 * 2.0)).abs() < 1e-9, "{}", g.media_w);
        assert!((g.media_h - (210.0 + 3.0 + 3.0)).abs() < 1e-9, "{}", g.media_h);
        // And the panels tile the trim exactly, left to right, no gap.
        assert!((g.back.x - g.bleed_ext).abs() < 1e-9);
        assert!((g.spine.as_ref().unwrap().x - (g.back.x + g.back.w)).abs() < 1e-9);
        assert!((g.front.x - (g.spine.as_ref().unwrap().x + spine)).abs() < 1e-9);
        assert!((g.front.x + g.front.w - (g.media_w - g.bleed_ext)).abs() < 1e-9);
    }

    /// A supplier that builds its own spine gets a sheet without one, and the
    /// two panels then meet at the middle of the trim.
    #[test]
    fn a_supplier_that_makes_its_own_spine_gets_none() {
        let a = album_de(40);
        let g = geometry(&a, PrinterProfile::par_id("prodigi").unwrap());
        assert!(g.spine.is_none());
        assert_eq!(g.spine_mm(), 0.0);
        assert!((g.media_w - 210.0 * 2.0).abs() < 1e-9, "Prodigi ne demande aucun fond perdu");
        assert!((g.front.x - (g.back.x + g.back.w)).abs() < 1e-9);
    }

    /// The spine grows with the book. A thin album and a fat one do not get
    /// the same sheet, which is the whole reason the width is computed.
    #[test]
    fn the_sheet_grows_with_the_book() {
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let thin = geometry(&album_de(20), cp);
        let fat = geometry(&album_de(90), cp);
        // The extra width is the extra spine, no more and no less: pinning a
        // millimetre count here would only pin the coefficient of the day.
        let attendu = cp.dos_mm(180, GRAMMAGE_DEFAUT).unwrap() - cp.dos_mm(40, GRAMMAGE_DEFAUT).unwrap();
        assert!(attendu > 0.0, "un dos qui ne grossit pas ne se calcule pas");
        assert!(
            (fat.media_w - thin.media_w - attendu).abs() < 1e-9,
            "{} vs {}, écart attendu {attendu}",
            fat.media_w,
            thin.media_w
        );
        assert!((fat.media_h - thin.media_h).abs() < 1e-9, "la hauteur ne bouge pas");
    }

    /// The photo bleeds on three sides and stops at the fold. Anything else
    /// puts half a face in the binding.
    #[test]
    fn the_front_photo_stops_at_the_fold() {
        let a = album_de(48);
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let g = geometry(&a, cp);
        let r = photo_rect(&g);
        assert!((r.x - g.front.x).abs() < 1e-9, "la photo déborde sur le dos");
        assert_eq!(r.y, 0.0);
        assert!((r.x + r.w - g.media_w).abs() < 1e-9, "elle atteint le bord extérieur");
        assert!((r.h - g.media_h).abs() < 1e-9);
    }

    /// A thin spine carries no type: the rule is a measurement, not a taste.
    #[test]
    fn a_thin_spine_carries_no_title() {
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        // 24 pages: 12 sheets at 0.22 plus 1.5 mm, under the floor.
        let g = geometry(&album_de(12), cp);
        assert!(g.spine_mm() < SPINE_TEXT_MIN_MM, "{}", g.spine_mm());
        let mut content = String::new();
        let cover = Cover {
            title: "Corse".into(),
            subtitle: String::new(),
            photo: None,
            back_text: String::new(),
        };
        draw_text(&mut content, &g, &album_de(12), &cover);
        // The title is on the front and nowhere else: one occurrence.
        assert_eq!(content.matches("(Corse)").count(), 1, "{content}");
    }

    /// The quatrième wraps on the real widths of the face, inside the panel.
    #[test]
    fn the_back_text_wraps_to_the_panel() {
        let long = "Trois semaines de septembre sur la côte est, entre les calanques et \
                    les villages de l'intérieur, avec un appareil et pas de programme.";
        let lines = wrap(long, 100.0, 10.5);
        assert!(lines.len() > 1, "rien n'a été coupé : {lines:?}");
        for l in &lines {
            assert!(font::text_width_mm(l, 10.5) <= 100.0, "ligne trop large : {l}");
        }
        // Words survive whole.
        assert_eq!(lines.join(" ").split_whitespace().count(), long.split_whitespace().count());
    }

    /// Blank lines in the quatrième are kept: a dedication has paragraphs.
    #[test]
    fn the_back_text_keeps_its_paragraphs() {
        let lines = wrap("Pour Marie.\n\nEt pour la suite.", 100.0, 10.5);
        assert_eq!(lines, vec!["Pour Marie.", "", "Et pour la suite."]);
    }

    /// The sheet the printer receives is measured in the file it receives,
    /// not in the arithmetic above. Renders a real cover and reads its boxes
    /// back out: the definition of done for this part of the session.
    #[test]
    fn the_rendered_sheet_measures_what_the_geometry_promised() {
        let dir = std::env::temp_dir().join(format!("colophon-cover-{}", std::process::id()));
        let photos = dir.join("photos");
        fs::create_dir_all(&photos).unwrap();
        let img = image::RgbImage::from_pixel(2400, 1600, image::Rgb([70, 110, 160]));
        image::DynamicImage::ImageRgb8(img)
            .save_with_format(photos.join("a.jpg"), image::ImageFormat::Jpeg)
            .unwrap();

        let mut album = album_de(48);
        album.root = photos.to_string_lossy().to_string();
        album.cover = Some(Cover {
            title: "Corse".into(),
            subtitle: "septembre 2013".into(),
            photo: Some(crate::model::Slot::new("a.jpg".into(), [0.5, 0.42])),
            back_text: "Trois semaines sur la côte est.".into(),
        });
        fs::write(dir.join("album.json"), serde_json::to_string(&album).unwrap()).unwrap();

        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let out = dir.join("album-cover.pdf");
        render_cover_pdf(&dir, cp, &out).expect("rendu de la couverture");

        let g = geometry(&album, cp);
        let doc = lopdf::Document::load(&out).expect("relecture");
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 1, "la couverture est une seule feuille");
        let page = doc.get_object(*pages.values().next().unwrap()).unwrap().as_dict().unwrap();
        let mm = |k: &[u8]| -> Vec<f64> {
            page.get(k)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|o| f64::from(o.as_float().unwrap()) * 25.4 / 72.0)
                .collect()
        };
        let media = mm(b"MediaBox");
        assert!((media[2] - g.media_w).abs() < 0.01, "{media:?} contre {}", g.media_w);
        assert!((media[3] - g.media_h).abs() < 0.01, "{media:?} contre {}", g.media_h);
        // Two pages, a spine and a bleed on each side: the number a supplier
        // checks against their own template.
        assert!(
            (media[2] - (210.0 * 2.0 + cp.dos_mm(96, GRAMMAGE_DEFAUT).unwrap() + 6.0)).abs() < 0.01
        );
        let trim = mm(b"TrimBox");
        assert!((trim[0] - 3.0).abs() < 0.01, "{trim:?}");
        assert!((media[2] - trim[2] - 3.0).abs() < 0.01, "{trim:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    /// A leaf is one panel, trimmed on four sides, with no spine and no fold.
    /// The face it does not carry gets no surface, so nothing of the back
    /// cover can land on the front.
    #[test]
    fn a_cover_leaf_is_one_panel_and_no_spine() {
        let a = album_de(24);
        let pr = PrinterProfile::par_id("prodigi").unwrap();

        let devant = page_geometry(&a, pr, Face::Premiere);
        assert!(devant.spine.is_none(), "une feuille volante n'a pas de dos");
        // Prodigi builds the bleed itself: the leaf is the finished page.
        assert!((devant.media_w - 210.0).abs() < 1e-9, "{}", devant.media_w);
        assert!((devant.media_h - 210.0).abs() < 1e-9, "{}", devant.media_h);
        assert!((devant.front.w - 210.0).abs() < 1e-9);
        assert_eq!(devant.back.w, 0.0, "la quatrième n'est pas sur cette feuille");

        let derriere = page_geometry(&a, pr, Face::Quatrieme);
        assert!((derriere.back.w - 210.0).abs() < 1e-9);
        assert_eq!(derriere.front.w, 0.0, "la première n'est pas sur cette feuille");

        // A profile that asks for bleed gets it on all four edges, because a
        // leaf is cut on all four: this is not half a flat sheet.
        let gen = PrinterProfile::par_id("generique").unwrap();
        let g = page_geometry(&a, gen, Face::Premiere);
        assert!((g.media_w - (210.0 + 6.0)).abs() < 1e-9, "{}", g.media_w);
        assert!((g.media_h - (210.0 + 6.0)).abs() < 1e-9, "{}", g.media_h);
    }

    /// The one that matters: a supplier who binds a single file must find the
    /// front cover on page one and the back cover on the last page. Sending
    /// the interior alone binds the whole book one page out of place, and the
    /// file is valid, so nothing but this test catches it.
    #[test]
    fn a_single_file_supplier_gets_the_cover_inside_the_interior() {
        // Its own name: `colophon-print-` is already taken by a print.rs test
        // that removes the folder while this one is still writing into it.
        let dir = std::env::temp_dir().join(format!("colophon-couv-int-{}", std::process::id()));
        let photos = dir.join("photos");
        fs::create_dir_all(&photos).unwrap();
        let img = image::RgbImage::from_pixel(2400, 1600, image::Rgb([70, 110, 160]));
        image::DynamicImage::ImageRgb8(img)
            .save_with_format(photos.join("a.jpg"), image::ImageFormat::Jpeg)
            .unwrap();

        let mut album = album_de(4);
        album.root = photos.to_string_lossy().to_string();
        for spread in &mut album.spreads {
            spread.slots = vec![crate::model::Slot::new("a.jpg".into(), [0.5, 0.5])];
        }
        album.cover = Some(Cover {
            title: "Corse".into(),
            subtitle: "septembre 2013".into(),
            photo: Some(crate::model::Slot::new("a.jpg".into(), [0.5, 0.42])),
            back_text: "Trois semaines sur la côte est.".into(),
        });
        fs::write(dir.join("album.json"), serde_json::to_string(&album).unwrap()).unwrap();

        let largeurs = |profil: &'static PrinterProfile, nom: &str| -> Vec<f64> {
            let out = dir.join(format!("album-print-{nom}.pdf"));
            print::render_print_pdf(&dir, profil, &out, &|_| {}, &|| false).expect("rendu");
            let doc = lopdf::Document::load(&out).expect("relecture");
            let pages = doc.get_pages();
            let mut ids: Vec<_> = pages.iter().collect();
            ids.sort_by_key(|(n, _)| **n);
            ids.iter()
                .map(|(_, id)| {
                    let page = doc.get_object(**id).unwrap().as_dict().unwrap();
                    let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
                    f64::from(media[2].as_float().unwrap()) * 25.4 / 72.0
                })
                .collect()
        };

        // Prodigi: four spreads between two cover leaves. The leaves are one
        // page wide, the spreads two, and that difference is what says the
        // covers are covers and not the first and last plate of the book.
        let pr = largeurs(PrinterProfile::par_id("prodigi").unwrap(), "prodigi");
        assert_eq!(pr.len(), 6, "4 planches plus les deux couvertures : {pr:?}");
        assert!((pr[0] - 210.0).abs() < 0.01, "première de couverture : {pr:?}");
        assert!((pr[5] - 210.0).abs() < 0.01, "quatrième de couverture : {pr:?}");
        // The spreads keep the album's own bleed, which is not yet the
        // profile's: a leaf at 210 next to a spread at 426 is the file saying
        // out loud that the interior does not answer to the supplier the way
        // the cover now does.
        for (i, w) in pr[1..5].iter().enumerate() {
            assert!((w - 426.0).abs() < 0.01, "planche {} : {pr:?}", i + 1);
        }

        // Cloudprinter binds two files, so its interior stays an interior.
        let cp = largeurs(PrinterProfile::par_id("cloudprinter").unwrap(), "cloudprinter");
        assert_eq!(cp.len(), 4, "l'intérieur seul, la couverture est son fichier : {cp:?}");
        assert!(cp.iter().all(|w| (w - 426.0).abs() < 0.01), "{cp:?}");

        let _ = fs::remove_dir_all(&dir);
    }
}
