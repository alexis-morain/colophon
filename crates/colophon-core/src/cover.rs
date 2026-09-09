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

#[cfg(test)]
use crate::font;

use crate::model::{Album, Cover};
use crate::pdf::{self, Boxes, PdfWriter, Rect};
use crate::printer::{Fichiers, PrinterProfile, GRAMMAGE_DEFAUT};
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
/// one hard offset instead, the same shape the editor now shows.
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
/// profile computes, plus the hinge groove on each side of it, plus what the
/// board takes on the outside.
///
/// The sheet **grows around its contents, it never moves them**. A supplier
/// who binds a soft cover carries the three case-wrap cotes at zero, and
/// then every number below is the one this function has always returned:
/// the boards are a case of the same arithmetic, not a branch beside it.
pub fn geometry(album: &Album, profil: &PrinterProfile) -> CoverGeometry {
    let pages = album.spreads.len() * 2;
    let spine_w = profil.dos_mm(pages, GRAMMAGE_DEFAUT).unwrap_or(0.0);
    let b = &profil.bleed_mm;
    let (ext, haut, bas) = (b.exterieur, b.haut, b.bas);
    let (rempli, debord, mors) = (profil.rempli_mm, profil.debord_mm, profil.mors_mm);

    // What lies between the edge of the card and the finished panel: the
    // bleed the knife takes, then the board's overhang, then the turn-in
    // that folds behind it. The same three on all four sides.
    let marge = ext + rempli + debord;
    let media_w = album.trim_mm.w * 2.0 + spine_w + mors * 2.0 + marge * 2.0;
    let media_h = album.trim_mm.h + (haut + rempli + debord) + (bas + rempli + debord);
    let panel = |x: f64, w: f64| Rect { x, y: bas + rempli + debord, w, h: album.trim_mm.h };

    let back = panel(marge, album.trim_mm.w);
    let spine = profil
        .dos_mm(pages, GRAMMAGE_DEFAUT)
        .map(|w| panel(marge + album.trim_mm.w + mors, w));
    let front = panel(marge + album.trim_mm.w + mors + spine_w + mors, album.trim_mm.w);

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

/// Render the flat cover sheet to `out`, at print resolution.
///
/// The name is the caller's, not ours: the delivery is `album-cover.pdf`
/// (`--cover`), the app's preview `album-cover.apercu.pdf`. Only the first is
/// judged by the preflight, so a function that named its own file would put
/// every preview under the delivery's rules.
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

    let mut writer = PdfWriter::new(&album, dir);
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
        // The adjustment is keyed by source, so a photo on the cover and on
        // a spread is adjusted once and prints the same on both.
        let asset = print::print_asset(
            &src,
            orientation,
            &rect,
            slot.focal,
            slot.zoom,
            album.reglages.get(&slot.src),
        )
        .with_context(|| format!("photo de couverture : {}", slot.src))?;
        writer.draw_image(&mut content, &mut xobjects, 0, &asset, &rect);
    }

    draw_text(&mut content, &mut writer.ecrivain, &g, &album, &cover);

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

/// Where the front-cover photo goes: everything to the right of the spine,
/// out to the three edges of the card.
///
/// Two numbers, and each is a decision. The **width** is what is left of the
/// sheet, not the panel plus a bleed: on a case-wrap cover the card runs on
/// past the finished page by the board's overhang and the turn-in, and a
/// photograph that stopped at the old width would leave the last centimetres
/// of the front white. On a sheet without boards the two are the same number,
/// which is what lets this stay one expression.
///
/// The **origin** is the right edge of the spine, so the image runs *through*
/// the hinge groove instead of starting at it. A case is assembled to a
/// millimetre or two, and a white line in the groove would be visible on
/// every copy; a photograph folded into the groove is not. There is nothing
/// to read there, so nothing is lost — the rule this obeys is that the mors
/// carries no text, not that it carries no ink.
pub fn photo_rect(g: &CoverGeometry) -> Rect {
    let x = g.spine.as_ref().map_or(g.front.x, |s| s.x + s.w);
    Rect {
        x,
        y: 0.0,
        w: g.media_w - x,
        h: g.media_h,
    }
}

/// Where the front photograph goes on a single leaf: the whole card. A leaf
/// has no fold to stop it at, so it bleeds on all four sides.
pub fn leaf_photo_rect(g: &CoverGeometry) -> Rect {
    Rect { x: 0.0, y: 0.0, w: g.media_w, h: g.media_h }
}

/// The rectangle the front photograph is printed into **at this supplier**,
/// whichever shape the cover takes there.
///
/// One function rather than a `match` at every caller, because the two shapes
/// are not the same surface at all: on a 210 mm book the flat case-wrap sheet
/// prints it into 239 × 258 mm and a single leaf into 216 × 216. Anything
/// that measures the cover — the preflight's resolution rule first of all —
/// reads it here, or it measures a rectangle nobody prints.
pub fn photo_rect_du_profil(album: &Album, profil: &PrinterProfile) -> Rect {
    match profil.fichiers {
        Fichiers::Deux => photo_rect(&geometry(album, profil)),
        Fichiers::Un => {
            leaf_photo_rect(&page_geometry(album, profil, Face::Premiere))
        }
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
                let rect = leaf_photo_rect(&g);
                let orientation = meta::read(&src).orientation;
                let asset = print::print_asset(
                    &src,
                    orientation,
                    &rect,
                    slot.focal,
                    slot.zoom,
                    album.reglages.get(&slot.src),
                )
                .with_context(|| format!("photo de couverture : {}", slot.src))?;
                writer.draw_image(&mut content, &mut xobjects, 0, &asset, &rect);
            }
            draw_front(&mut content, &mut writer.ecrivain, &g, album, &cover);
        }
        Face::Quatrieme => draw_back(&mut content, &mut writer.ecrivain, &g, album, &cover),
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
fn draw_text(
    content: &mut String,
    ecrivain: &mut pdf::Ecrivain,
    g: &CoverGeometry,
    album: &Album,
    cover: &Cover,
) {
    draw_front(content, ecrivain, g, album, cover);
    draw_spine(content, ecrivain, g, album, cover);
    draw_back(content, ecrivain, g, album, cover);
}

/// The title the book wears, from the album alone: the cover's when it was
/// given one of its own, the album's name otherwise. The flat sheet reads it
/// through [`cover_title`] with the cover it is rendering; everything else
/// that has to print the book's name (the half-title, [`crate::garde`]) asks
/// here, so a book called « Un été » on its cover is not called something
/// else three pages later.
pub fn titre_du_livre(album: &Album) -> &str {
    match album.cover.as_ref() {
        Some(c) if !c.title.trim().is_empty() => c.title.as_str(),
        _ => album.title.as_str(),
    }
}

/// The title of the album, whether or not the cover editor has been opened.
fn cover_title<'a>(album: &'a Album, cover: &'a Cover) -> &'a str {
    if cover.title.is_empty() { album.title.as_str() } else { cover.title.as_str() }
}

/// Front: title block, bottom left, inside the trim by the same share the
/// editor shows. Baselines stack upward from the subtitle.
fn draw_front(
    content: &mut String,
    ecrivain: &mut pdf::Ecrivain,
    g: &CoverGeometry,
    album: &Album,
    cover: &Cover,
) {
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
    plate(content, ecrivain, x, subtitle_y, subtitle_pt, over_photo, &cover.subtitle);
    plate(content, ecrivain, x, title_y, title_pt, over_photo, title);
}

/// Spine: the title along the fold, running bottom to top, centred. Only when
/// there is a surface to print on, and never on a single leaf.
fn draw_spine(
    content: &mut String,
    ecrivain: &mut pdf::Ecrivain,
    g: &CoverGeometry,
    album: &Album,
    cover: &Cover,
) {
    let scale = album.trim_mm.w / 210.0;
    if let Some(spine) = &g.spine {
        if spine.w >= SPINE_TEXT_MIN_MM {
            let title = cover_title(album, cover);
            let size = SPINE_PT_AT_210 * scale;
            let width = ecrivain.largeur_mm(title, size);

            let cx = spine.x + spine.w / 2.0 - size * PT_TO_MM * 0.35;
            let cy = spine.y + (spine.h - width) / 2.0;
            rotated(content, ecrivain, cx, cy, size, pdf::TEXT_INK, title);
        }
    }
}

/// Back: the quatrième, wrapped to the panel and centred in it, the way the
/// cover editor shows it. A dedication is a short block on a wide white page;
/// ranged left in a corner it reads like a caption.
fn draw_back(
    content: &mut String,
    ecrivain: &mut pdf::Ecrivain,
    g: &CoverGeometry,
    album: &Album,
    cover: &Cover,
) {
    let scale = album.trim_mm.w / 210.0;
    if !cover.back_text.is_empty() {
        let size = BACK_TEXT_PT_AT_210 * scale;
        let box_w = g.back.w - 2.0 * (g.back.w * TITLE_INSET);
        let leading = size * PT_TO_MM * BACK_LEADING;
        let lines = wrap(ecrivain, &cover.back_text, box_w, size);

        let block = lines.len() as f64 * leading;
        let mut y = g.back.y + (g.back.h + block) / 2.0 - leading;
        for line in &lines {
            // Centred on the panel, measured on the real advance widths.
            let x = g.back.x + (g.back.w - ecrivain.largeur_mm(line, size)) / 2.0;

            pdf::text_op(content, ecrivain, x, y, size, pdf::TEXT_INK, line);
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
fn plate(
    content: &mut String,
    ecrivain: &mut pdf::Ecrivain,
    x: f64,
    y: f64,
    size: f64,
    over_photo: bool,
    s: &str,
) {
    if s.is_empty() {
        return;
    }
    if over_photo {
        pdf::text_op(content, ecrivain, x + SHADOW_OFFSET, y - SHADOW_OFFSET, size, SHADOW, s);
        pdf::text_op(content, ecrivain, x, y, size, PAPER, s);
    } else {
        pdf::text_op(content, ecrivain, x, y, size, pdf::TEXT_INK, s);
    }
}

/// Text turned a quarter turn anticlockwise, so the spine reads bottom to
/// top. One direction had to be picked and this is the one the cover editor
/// shows; a spine printed the other way up is a reprint, so the two sides
/// agree here and the choice is written down rather than left to a default.
fn rotated(
    content: &mut String,
    ecrivain: &mut pdf::Ecrivain,
    x: f64,
    y: f64,
    size: f64,
    rgb: [f64; 3],
    s: &str,
) {
    let mut run = String::new();
    pdf::text_op(&mut run, ecrivain, 0.0, 0.0, size, rgb, s);
    let mm_to_pt = 72.0 / 25.4;
    content.push_str(&format!(
        "q 0 1 -1 0 {:.2} {:.2} cm\n{run}Q\n",
        x * mm_to_pt,
        y * mm_to_pt
    ));
}

/// Greedy wrap on the real advance widths of the face this document is set
/// in — the album's own when it chose one. Words longer than the measure are
/// left whole and overflow visibly rather than being cut in a place no
/// reader would cut them.
fn wrap(
    ecrivain: &pdf::Ecrivain,
    text: &str,
    width_mm: f64,
    size_pt: f64,
) -> Vec<String> {

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
            if ecrivain.largeur_mm(&candidate, size_pt) <= width_mm || line.is_empty() {

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
                objets: Vec::new(),
            });
        }
        a
    }

    /// The sheet is twice the page, plus the spine, plus the two hinge
    /// grooves, plus what the board takes on each outer edge. Measured
    /// against the profile's own numbers rather than against a constant, so a
    /// profile change moves the cover with it.
    #[test]
    fn the_sheet_is_two_pages_plus_the_spine_plus_the_case() {
        let a = album_de(48); // 96 pages
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let g = geometry(&a, cp);
        let spine = cp.dos_mm(96, GRAMMAGE_DEFAUT).unwrap();
        let marge = cp.bleed_mm.exterieur + cp.rempli_mm + cp.debord_mm;

        assert!(
            (g.media_w - (210.0 * 2.0 + spine + cp.mors_mm * 2.0 + marge * 2.0)).abs() < 1e-9,
            "{}",
            g.media_w
        );
        assert!((g.media_h - (210.0 + marge * 2.0)).abs() < 1e-9, "{}", g.media_h);
        // The panels no longer touch the bleed: on a case-wrap cover they sit
        // inland by the board and the turn-in, and a groove separates each of
        // them from the spine.
        assert!((g.back.x - marge).abs() < 1e-9, "{}", g.back.x);
        assert!(
            (g.spine.as_ref().unwrap().x - (g.back.x + g.back.w + cp.mors_mm)).abs() < 1e-9
        );
        assert!(
            (g.front.x - (g.spine.as_ref().unwrap().x + spine + cp.mors_mm)).abs() < 1e-9
        );
        assert!((g.front.x + g.front.w - (g.media_w - marge)).abs() < 1e-9);
        // And the numbers their own template is drawn with: 492 × 258 at a
        // 14 mm spine is what `photobook_cw_s210_s_fc_cover.pdf` measures.
        assert!((g.media_h - 258.0).abs() < 1e-9, "{}", g.media_h);
        assert!(
            (210.0 * 2.0 + 14.0 + cp.mors_mm * 2.0 + marge * 2.0 - 492.0).abs() < 1e-9
        );
    }

    /// A supplier who does not wrap boards gets the sheet this project has
    /// always produced — the panels back against the bleed, the spine
    /// between them, nothing inland. The case-wrap arithmetic has to fold
    /// back to exactly that when the three cotes are zero, or every soft
    /// cover ever exported has quietly changed size.
    #[test]
    fn the_cotes_at_zero_give_back_the_flat_sheet() {
        let a = album_de(48);
        for id in ["prodigi", "lulu", "generique"] {
            let p = PrinterProfile::par_id(id).unwrap();
            let g = geometry(&a, p);
            let spine = p.dos_mm(96, GRAMMAGE_DEFAUT).unwrap_or(0.0);
            let ext = p.bleed_mm.exterieur;
            assert!((g.media_w - (210.0 * 2.0 + spine + ext * 2.0)).abs() < 1e-9, "{id}");
            assert!(
                (g.media_h - (210.0 + p.bleed_mm.haut + p.bleed_mm.bas)).abs() < 1e-9,
                "{id}"
            );
            assert!((g.back.x - ext).abs() < 1e-9, "{id}");
            assert!((g.back.y - p.bleed_mm.bas).abs() < 1e-9, "{id}");
            assert!((g.front.x + g.front.w - (g.media_w - ext)).abs() < 1e-9, "{id}");
            // And the photograph covers what it has always covered.
            let r = photo_rect(&g);
            assert!((r.x - g.front.x).abs() < 1e-9, "{id}");
            assert!((r.w - (g.front.w + ext)).abs() < 1e-9, "{id}");
            assert!((r.h - g.media_h).abs() < 1e-9, "{id}");
        }
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

    /// The photo reaches the four edges of the card and stops at the spine.
    /// It crosses the hinge groove on the way, deliberately: a case is
    /// assembled to a millimetre or two, and a white line down the groove
    /// would show on every copy. What it must never do is reach the spine,
    /// which would fold a face in half.
    #[test]
    fn the_front_photo_crosses_the_groove_and_stops_at_the_spine() {
        let a = album_de(48);
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let g = geometry(&a, cp);
        let spine = g.spine.as_ref().unwrap();
        let r = photo_rect(&g);
        assert!((r.x - (spine.x + spine.w)).abs() < 1e-9, "la photo mord sur le dos");
        assert!(r.x < g.front.x, "elle doit entrer dans le mors, pas s'y arrêter");
        assert!((g.front.x - r.x - cp.mors_mm).abs() < 1e-9, "d'exactement un mors");
        assert_eq!(r.y, 0.0);
        assert!((r.x + r.w - g.media_w).abs() < 1e-9, "elle atteint le bord extérieur");
        assert!((r.h - g.media_h).abs() < 1e-9, "et le haut comme le bas");
    }

    /// La photographie de couverture n'a pas la même surface chez tout le
    /// monde, et c'est une seule fonction qui le sait. Le carton habillé court
    /// au-delà du livre sur les quatre bords ; le feuillet volant s'arrête au
    /// fond perdu. Un prévol qui reconstruirait le rectangle lui-même
    /// mesurerait une couverture que personne n'imprime.
    #[test]
    fn la_photo_de_couverture_a_la_surface_de_son_fournisseur() {
        let a = album_de(24);
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let pr = PrinterProfile::par_id("prodigi").unwrap();

        // Feuille à plat : de la droite du dos au bord du carton, sur toute
        // sa hauteur. C'est bien le rectangle que l'émetteur y pose.
        let boitier = photo_rect_du_profil(&a, cp);
        let attendu = photo_rect(&geometry(&a, cp));
        assert!(
            (boitier.x - attendu.x).abs() < 1e-9
                && (boitier.w - attendu.w).abs() < 1e-9
                && (boitier.h - attendu.h).abs() < 1e-9,
            "{boitier:?} ≠ {attendu:?}"
        );
        assert!((boitier.h - (210.0 + 24.0 * 2.0)).abs() < 1e-9, "{boitier:?}");

        // Feuillet volant : la page plus son fond perdu, et rien de plus.
        // Prodigi n'en demande aucun, donc c'est la page nue.
        let feuillet = photo_rect_du_profil(&a, pr);
        assert!((feuillet.w - 210.0).abs() < 1e-9, "{feuillet:?}");
        assert!((feuillet.h - 210.0).abs() < 1e-9, "{feuillet:?}");
        assert!(boitier.w * boitier.h > feuillet.w * feuillet.h * 1.3);
    }

    /// A thin spine carries no type: the rule is a measurement, not a taste.
    /// The book's name comes from the cover when the cover was given one:
    /// the flat sheet, the leaf and the half-title all print the same string.
    #[test]
    fn the_book_wears_the_covers_title_when_it_has_one() {
        let mut a = album_de(24);
        a.title = "corse-2013".into();
        assert_eq!(titre_du_livre(&a), "corse-2013");
        a.cover = Some(Cover {
            title: "Un été".into(),
            subtitle: String::new(),
            photo: None,
            back_text: String::new(),
        });
        assert_eq!(titre_du_livre(&a), "Un été");
        // A cover left blank falls back to the album, it never prints nothing.
        a.cover.as_mut().unwrap().title = "   ".into();
        assert_eq!(titre_du_livre(&a), "corse-2013");
    }

    #[test]
    fn a_thin_spine_carries_no_title() {
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        // 24 pages: 12 sheets at 0.135 plus the two boards, 7.62 mm, under the floor.
        let g = geometry(&album_de(12), cp);
        assert!(g.spine_mm() < SPINE_TEXT_MIN_MM, "{}", g.spine_mm());
        let mut content = String::new();
        let cover = Cover {
            title: "Corse".into(),
            subtitle: String::new(),
            photo: None,
            back_text: String::new(),
        };
        draw_text(&mut content, &mut pdf::Ecrivain::incorporee(), &g, &album_de(12), &cover);
        // The title is on the front and nowhere else: one occurrence. Under
        // Identity-H a string in the stream is glyph ids, so the title is
        // looked for the way it is written.
        let face = font::Embarquee::incorporee().expect("face ouverte");
        let pose: String =
            face.glyphes("Corse").iter().map(|(gid, _)| format!("{gid:04X}")).collect();
        assert_eq!(content.matches(&format!("<{pose}>")).count(), 1, "{content}");
    }

    /// The quatrième wraps on the real widths of the face, inside the panel.
    #[test]
    fn the_back_text_wraps_to_the_panel() {
        let long = "Trois semaines de septembre sur la côte est, entre les calanques et \
                    les villages de l'intérieur, avec un appareil et pas de programme.";
        let lines = wrap(&pdf::Ecrivain::incorporee(), long, 100.0, 10.5);
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
        let lines =
            wrap(&pdf::Ecrivain::incorporee(), "Pour Marie.\n\nEt pour la suite.", 100.0, 10.5);
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
        // Two pages, a spine, two grooves and the case on each outer edge:
        // the number a supplier checks against their own template.
        let marge = cp.bleed_mm.exterieur + cp.rempli_mm + cp.debord_mm;
        assert!(
            (media[2]
                - (210.0 * 2.0
                    + cp.dos_mm(96, GRAMMAGE_DEFAUT).unwrap()
                    + cp.mors_mm * 2.0
                    + marge * 2.0))
                .abs()
                < 0.01,
            "{media:?}"
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

        // Prodigi: the book block between two cover leaves. It binds page by
        // page, so four spreads make eight interior pages — the first spread's
        // recto, then both pages of the other three, then the blank verso —
        // and the file is ten pages long.
        let pr = largeurs(PrinterProfile::par_id("prodigi").unwrap(), "prodigi");
        assert_eq!(pr.len(), 10, "8 pages de livre plus les deux couvertures : {pr:?}");
        assert!((pr[0] - 210.0).abs() < 0.01, "première de couverture : {pr:?}");
        assert!((pr[9] - 210.0).abs() < 0.01, "quatrième de couverture : {pr:?}");
        // A leaf is the trimmed page, an interior page is that page plus the
        // bleed the album carries: they are close in width and the file has
        // to keep them apart, which is what the count above says.
        for (i, w) in pr[1..9].iter().enumerate() {
            assert!((w - 213.0).abs() < 0.01, "page {} : {pr:?}", i + 1);
        }

        // Cloudprinter binds two files, so its interior stays an interior.
        let cp = largeurs(PrinterProfile::par_id("cloudprinter").unwrap(), "cloudprinter");
        assert_eq!(cp.len(), 8, "l'intérieur seul, la couverture est son fichier : {cp:?}");
        // Their own inside-page template: 216 × 216, bleed on all four edges.
        assert!(cp.iter().all(|w| (w - 216.0).abs() < 0.01), "{cp:?}");

        // Lulu imposes our spreads itself, so its interior is still an
        // interior of spreads — the shape the emitter has always written, and
        // the one the whole file must keep producing unchanged.
        let lu = largeurs(PrinterProfile::par_id("lulu").unwrap(), "lulu");
        assert_eq!(lu.len(), 4, "quatre planches doubles : {lu:?}");
        assert!(lu.iter().all(|w| (w - 426.0).abs() < 0.01), "{lu:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    /// Le banc de l'œil : la feuille du boîtier, ses panneaux et ses cotes
    /// tracés par-dessus, à côté du gabarit de Cloudprinter rasterisé au même
    /// nombre de pixels par millimètre.
    ///
    /// C'est le seul contrôle qui attrape une cote posée du mauvais côté.
    /// Aucun test ne le fait : ils vérifient tous que la somme tombe juste, et
    /// une somme tombe juste aussi quand le rempli est à gauche et le débord à
    /// droite. Deux images superposables, elles, ne mentent pas.
    ///
    /// La seconde image est la vraie épreuve : nos lignes sont dessinées sur
    /// **leur** gabarit. Le dos y est forcé aux 14 mm pour lesquels leur
    /// fichier est dessiné — il annonce lui-même « 100 pages of 200 gsm
    /// machine coated gloss », dont la main 0,80 n'est plus celle du papier
    /// commandé — de sorte que tout le reste doit tomber au pixel : la marge
    /// de 24, les panneaux de 210, les deux mors de 5.
    ///
    /// ```text
    /// COLOPHON_ALBUM=.albums/corse-2013 \
    /// COLOPHON_GABARIT="$HOME/Downloads/photobook_cw_s210_s_fc_product (1)/photobook_cw_s210_s_fc_cover.pdf" \
    ///   cargo test -p colophon-core --release banc_la_feuille_du_boitier -- --ignored --nocapture
    /// ```
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore]
    fn banc_la_feuille_du_boitier() {
        let dir = PathBuf::from(
            std::env::var("COLOPHON_ALBUM").expect("COLOPHON_ALBUM = dossier d'un album composé"),
        );
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let album: Album =
            serde_json::from_str(&fs::read_to_string(dir.join("album.json")).unwrap()).unwrap();
        let g = geometry(&album, cp);
        let out = dir.join("banc-boitier");
        fs::create_dir_all(&out).unwrap();

        // 1. Notre feuille, telle que la presse la reçoit, annotée.
        let pdf = out.join("feuille.pdf");
        render_cover_pdf(&dir, cp, &pdf).expect("rendu de la couverture");
        let mut feuille = rasterise(&pdf, &out.join("feuille-brute.png"));
        let ppmm = f64::from(feuille.width()) / g.media_w;
        guides(&mut feuille, &g, cp, ppmm, g.spine_mm());
        feuille.save(out.join("feuille.png")).unwrap();

        // 2. Leur gabarit, sous nos lignes. Le dos forcé au sien.
        let gabarit = std::env::var("COLOPHON_GABARIT").ok().map(PathBuf::from);
        if let Some(src) = gabarit.filter(|p| p.is_file()) {
            let mut leur = rasterise(&src, &out.join("gabarit-brut.png"));
            const DOS_DU_GABARIT: f64 = 14.0;
            let large = album.trim_mm.w * 2.0
                + DOS_DU_GABARIT
                + cp.mors_mm * 2.0
                + (cp.bleed_mm.exterieur + cp.rempli_mm + cp.debord_mm) * 2.0;
            let ppmm = f64::from(leur.width()) / large;
            let mut gg = g.clone();
            gg.media_w = large;
            gg.spine = gg.spine.map(|r| Rect { w: DOS_DU_GABARIT, ..r });
            gg.front.x = gg.back.x + gg.back.w + cp.mors_mm + DOS_DU_GABARIT + cp.mors_mm;
            guides(&mut leur, &gg, cp, ppmm, DOS_DU_GABARIT);
            leur.save(out.join("gabarit.png")).unwrap();
            println!("gabarit  : {}", out.join("gabarit.png").display());
        } else {
            println!("gabarit  : absent (COLOPHON_GABARIT), la superposition n'est pas faite");
        }

        println!("feuille  : {}", out.join("feuille.png").display());
        println!(
            "cotes    : feuille {:.2} × {:.2} mm, dos {:.2}, rempli {}, débord {}, mors {}",
            g.media_w, g.media_h, g.spine_mm(), cp.rempli_mm, cp.debord_mm, cp.mors_mm
        );
        println!("légende  : rouge = coupe de la feuille (fond perdu)");
        println!("           vert  = panneaux finis (dos, quatrième, première)");
        println!("           bleu  = mors, la gorge où la couverture plie");
        println!("           jaune = pli du rempli, ce qui se replie derrière le carton");
    }

    /// Un PDF en pixels, par `sips`, la même porte que `scripts/pdf-png.py`.
    #[cfg(target_os = "macos")]
    fn rasterise(pdf: &Path, png: &Path) -> image::RgbImage {
        let ok = std::process::Command::new("sips")
            .args(["-s", "format", "png"])
            .arg(pdf)
            .arg("--out")
            .arg(png)
            .output()
            .expect("sips");
        assert!(ok.status.success(), "sips a refusé {}", pdf.display());
        image::open(png).expect("relecture du PNG").to_rgb8()
    }

    /// Les lignes de notre géométrie, posées sur une image déjà rasterisée.
    #[cfg(target_os = "macos")]
    fn guides(
        im: &mut image::RgbImage,
        g: &CoverGeometry,
        profil: &PrinterProfile,
        ppmm: f64,
        dos: f64,
    ) {
        const ROUGE: [u8; 3] = [220, 40, 40];
        const VERT: [u8; 3] = [40, 190, 80];
        const BLEU: [u8; 3] = [60, 120, 240];
        const JAUNE: [u8; 3] = [240, 200, 40];
        let (w, h) = (im.width(), im.height());
        fn v(im: &mut image::RgbImage, x_mm: f64, ppmm: f64, c: [u8; 3]) {
            let x = (x_mm * ppmm).round() as i64;
            if x < 0 || x >= i64::from(im.width()) {
                return;
            }
            for y in 0..im.height() {
                im.put_pixel(x as u32, y, image::Rgb(c));
            }
        }
        // L'image est en coordonnées écran, la géométrie en coordonnées PDF.
        fn hh(im: &mut image::RgbImage, y_mm: f64, media_h: f64, ppmm: f64, c: [u8; 3]) {
            let y = ((media_h - y_mm) * ppmm).round() as i64;
            if y < 0 || y >= i64::from(im.height()) {
                return;
            }
            for x in 0..im.width() {
                im.put_pixel(x, y as u32, image::Rgb(c));
            }
        }
        let _ = (w, h);
        // La coupe : ce que le massicot laisse de la feuille.
        for x in [g.bleed_ext, g.media_w - g.bleed_ext] {
            v(im, x, ppmm, ROUGE);
        }
        for y in [g.bleed_bas, g.media_h - g.bleed_haut] {
            hh(im, y, g.media_h, ppmm, ROUGE);
        }
        // Le pli du rempli : le carton commence là.
        let pli = g.bleed_ext + profil.rempli_mm;
        for x in [pli, g.media_w - pli] {
            v(im, x, ppmm, JAUNE);
        }
        for y in [g.bleed_bas + profil.rempli_mm, g.media_h - g.bleed_haut - profil.rempli_mm] {
            hh(im, y, g.media_h, ppmm, JAUNE);
        }
        // Les panneaux finis, et les deux mors entre eux.
        for x in [g.back.x, g.back.x + g.back.w, g.front.x, g.front.x + g.front.w] {
            v(im, x, ppmm, VERT);
        }
        for y in [g.back.y, g.back.y + g.back.h] {
            hh(im, y, g.media_h, ppmm, VERT);
        }
        if dos > 0.0 {
            let dos_x = g.back.x + g.back.w + profil.mors_mm;
            for x in [dos_x, dos_x + dos] {
                v(im, x, ppmm, BLEU);
            }
        }
    }
}
