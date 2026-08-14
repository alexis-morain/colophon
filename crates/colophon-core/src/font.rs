//! The album's text face, embedded in every PDF.
//!
//! PDF readers carry the base-14 fonts (Helvetica and friends) themselves, so
//! a file that names one embeds nothing. It renders anywhere and fails every
//! PDF/X preflight: what the reader substitutes is not what we set, and a
//! print run is not the place to find out. The face therefore travels inside
//! the file, glyphs and all.
//!
//! Source Sans 3, under the SIL Open Font License 1.1 (see
//! `assets/SourceSans3-LICENSE.md`). Chosen for print rather than inherited
//! from a default: it was drawn for text at small sizes, which is all an album
//! ever asks of type, and it carries the French set whole, accents, œ,
//! guillemets and all. `fsType` is 0, so embedding is unrestricted.
//!
//! The metrics come out of the TrueType tables by hand, the way `print.rs`
//! reads a JPEG's SOF marker: a font file is a handful of big-endian tables at
//! known offsets, and a parser crate would be more code to audit than this.

use anyhow::{Context, Result};

/// The face itself. ~430 kB, embedded whole rather than subset: PDF/X asks for
/// embedded, not minimal, and a print PDF runs to tens of megabytes of photos.
/// Subsetting stays open as a size optimisation, never as a conformance one.
pub const FONT_DATA: &[u8] = include_bytes!("../assets/SourceSans3-Regular.ttf");

/// PostScript name, as it goes in `/BaseFont`.
pub const FONT_NAME: &str = "SourceSans3-Regular";

/// First and last WinAnsi codes we describe. The renderer escapes everything
/// into WinAnsi, so this covers every glyph a caption can reach.
pub const FIRST_CHAR: u8 = 32;
pub const LAST_CHAR: u8 = 255;

/// What the PDF needs to know about the face, all in the 1000-unit em space
/// PDF works in.
#[derive(Debug, Clone)]
pub struct Metrics {
    /// `[xMin, yMin, xMax, yMax]`, for `/FontBBox`.
    pub bbox: [i32; 4],
    pub ascent: i32,
    pub descent: i32,
    pub cap_height: i32,
    pub italic_angle: f64,
    /// Embedding permission from OS/2. 0 means unrestricted; bit 1 set would
    /// forbid embedding outright.
    pub fs_type: u16,
    /// Advance width per WinAnsi code, from [`FIRST_CHAR`] to [`LAST_CHAR`].
    pub widths: Vec<i32>,
}

impl Metrics {
    /// True when the licence bits allow us to put the face in a file at all.
    /// Checked rather than assumed: swapping the asset must not quietly ship
    /// a font whose vendor forbade embedding.
    pub fn embeddable(&self) -> bool {
        self.fs_type & 0x0002 == 0
    }
}

/// Read the face bundled above.
pub fn metrics() -> Result<Metrics> {
    parse(FONT_DATA)
}

/// The metrics, parsed once. Wrapping a paragraph asks for the widths on
/// every candidate line, and re-reading 430 kB of TrueType each time would
/// turn a cover render into a benchmark.
static METRICS: std::sync::LazyLock<Option<Metrics>> =
    std::sync::LazyLock::new(|| metrics().ok());

/// How wide a string sets, in millimetres, at `size_pt`.
///
/// Advance widths only, no kerning: the face carries none in the tables the
/// renderer reads, and what is measured here has to be what the PDF draws.
/// A character outside WinAnsi measures as the `?` the renderer substitutes.
pub fn text_width_mm(s: &str, size_pt: f64) -> f64 {
    let Some(m) = METRICS.as_ref() else { return 0.0 };
    let width = |c: char| -> i32 {
        let code = winansi_code(c).unwrap_or(b'?');
        m.widths
            .get(usize::from(code - FIRST_CHAR))
            .copied()
            .unwrap_or(0)
    };
    let em: i32 = s.chars().map(width).sum();
    // Widths are in the 1000-unit em PDF works in.
    f64::from(em) / 1000.0 * size_pt * 25.4 / 72.0
}

/// The WinAnsi code a character is escaped to, the reverse of
/// [`winansi_char`]. `None` for anything the renderer cannot print.
pub fn winansi_code(c: char) -> Option<u8> {
    (FIRST_CHAR..=LAST_CHAR).find(|code| winansi_char(*code) == Some(c))
}

fn parse(data: &[u8]) -> Result<Metrics> {
    let head = table(data, b"head").context("table head absente")?;
    let hhea = table(data, b"hhea").context("table hhea absente")?;
    let hmtx = table(data, b"hmtx").context("table hmtx absente")?;
    let maxp = table(data, b"maxp").context("table maxp absente")?;
    let cmap = table(data, b"cmap").context("table cmap absente")?;
    let os2 = table(data, b"OS/2").context("table OS/2 absente")?;

    let upem = f64::from(u16b(head, 18).context("unitsPerEm illisible")?);
    anyhow::ensure!(upem > 0.0, "unitsPerEm nul");
    // Everything below is expressed in the font's own units; PDF wants a
    // 1000-unit em, so every measure goes through here.
    let to_em = |v: i32| -> i32 { (f64::from(v) * 1000.0 / upem).round() as i32 };

    let bbox = [
        to_em(i16b(head, 36).context("xMin")?.into()),
        to_em(i16b(head, 38).context("yMin")?.into()),
        to_em(i16b(head, 40).context("xMax")?.into()),
        to_em(i16b(head, 42).context("yMax")?.into()),
    ];

    let num_h = usize::from(u16b(hhea, 34).context("numberOfHMetrics illisible")?);
    anyhow::ensure!(num_h > 0, "police sans métrique horizontale");
    let num_glyphs = u16b(maxp, 4).context("numGlyphs illisible")?;

    // OS/2 carries the typographic ascent and descent, and from version 2 the
    // cap height. Older faces fall back to hhea, which is what readers do.
    let os2_version = u16b(os2, 0).unwrap_or(0);
    let fs_type = u16b(os2, 8).unwrap_or(0);
    let ascent = i16b(os2, 68)
        .filter(|v| *v != 0)
        .or_else(|| i16b(hhea, 4))
        .context("ascendante illisible")?;
    let descent = i16b(os2, 70)
        .filter(|v| *v != 0)
        .or_else(|| i16b(hhea, 6))
        .context("descendante illisible")?;
    let cap_height = if os2_version >= 2 {
        i16b(os2, 88).filter(|v| *v != 0).unwrap_or(ascent)
    } else {
        ascent
    };

    let italic_angle = table(data, b"post")
        .and_then(|p| i32b(p, 4))
        // post stores the angle as a 16.16 fixed-point number.
        .map(|v| f64::from(v) / 65536.0)
        .unwrap_or(0.0);

    let sub = unicode_subtable(cmap).context("aucune sous-table cmap Unicode")?;
    let widths = (FIRST_CHAR..=LAST_CHAR)
        .map(|code| {
            // A code with no WinAnsi meaning, or a glyph the face does not
            // carry, takes the .notdef advance. It can never be drawn: the
            // renderer only emits what it escaped into WinAnsi.
            let gid = winansi_char(code)
                .and_then(|c| lookup(sub, c as u32))
                .filter(|g| *g < num_glyphs)
                .unwrap_or(0);
            to_em(i32::from(advance(hmtx, num_h, gid)))
        })
        .collect();

    Ok(Metrics {
        bbox,
        ascent: to_em(ascent.into()),
        descent: to_em(descent.into()),
        cap_height: to_em(cap_height.into()),
        italic_angle,
        fs_type,
        widths,
    })
}

/// Locate a table in the sfnt directory.
fn table<'a>(data: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let count = usize::from(u16b(data, 4)?);
    for i in 0..count {
        let rec = 12 + i * 16;
        if data.get(rec..rec + 4)? == tag {
            let off = u32b(data, rec + 8)? as usize;
            let len = u32b(data, rec + 12)? as usize;
            return data.get(off..off.saturating_add(len));
        }
    }
    None
}

/// The first Unicode character map: Windows BMP, Windows full, or a plain
/// Unicode platform table, in that order of preference.
fn unicode_subtable(cmap: &[u8]) -> Option<&[u8]> {
    let count = usize::from(u16b(cmap, 2)?);
    let mut best: Option<(u8, &[u8])> = None;
    for i in 0..count {
        let rec = 4 + i * 8;
        let platform = u16b(cmap, rec)?;
        let encoding = u16b(cmap, rec + 2)?;
        let off = u32b(cmap, rec + 4)? as usize;
        let sub = cmap.get(off..)?;
        let rank = match (platform, encoding) {
            (3, 1) => 0,
            (3, 10) => 1,
            (0, _) => 2,
            _ => continue,
        };
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, sub));
        }
    }
    best.map(|(_, s)| s)
}

/// Glyph id for a character. Handles the two subtable formats a text face
/// actually uses: segmented BMP (4) and trimmed/sparse full range (12).
fn lookup(sub: &[u8], ch: u32) -> Option<u16> {
    match u16b(sub, 0)? {
        4 => lookup_segmented(sub, u16::try_from(ch).ok()?),
        12 => lookup_groups(sub, ch),
        6 => {
            let first = u32::from(u16b(sub, 6)?);
            let count = u32::from(u16b(sub, 8)?);
            let idx = ch.checked_sub(first)?;
            if idx >= count {
                return Some(0);
            }
            u16b(sub, 10 + (idx as usize) * 2)
        }
        _ => None,
    }
}

/// cmap format 4: parallel arrays of segments, each mapping a contiguous run.
fn lookup_segmented(sub: &[u8], ch: u16) -> Option<u16> {
    let seg_count = usize::from(u16b(sub, 6)?) / 2;
    let ends = 14;
    let starts = ends + seg_count * 2 + 2;
    let deltas = starts + seg_count * 2;
    let ranges = deltas + seg_count * 2;
    for i in 0..seg_count {
        if ch > u16b(sub, ends + i * 2)? {
            continue;
        }
        let start = u16b(sub, starts + i * 2)?;
        if ch < start {
            return Some(0);
        }
        let delta = i16b(sub, deltas + i * 2)?;
        let range_offset = u16b(sub, ranges + i * 2)?;
        if range_offset == 0 {
            return Some((i32::from(ch) + i32::from(delta)) as u16);
        }
        // The offset is counted from the slot it sits in, not from the array.
        let at = ranges + i * 2 + usize::from(range_offset) + usize::from(ch - start) * 2;
        let gid = u16b(sub, at)?;
        return Some(if gid == 0 {
            0
        } else {
            (i32::from(gid) + i32::from(delta)) as u16
        });
    }
    Some(0)
}

/// cmap format 12: sorted groups over the full Unicode range.
fn lookup_groups(sub: &[u8], ch: u32) -> Option<u16> {
    let count = u32b(sub, 12)? as usize;
    for i in 0..count {
        let g = 16 + i * 12;
        let start = u32b(sub, g)?;
        let end = u32b(sub, g + 4)?;
        if ch < start {
            return Some(0);
        }
        if ch <= end {
            let gid = u32b(sub, g + 8)? + (ch - start);
            return u16::try_from(gid).ok();
        }
    }
    Some(0)
}

/// Advance width of a glyph. The table stops carrying widths once they repeat:
/// every glyph past the last entry shares it.
fn advance(hmtx: &[u8], num_h: usize, gid: u16) -> u16 {
    let i = usize::from(gid).min(num_h - 1);
    u16b(hmtx, i * 4).unwrap_or(0)
}

/// The character a WinAnsi code stands for. Codes 128 to 159 are the cp1252
/// additions, which is where the typographic quotes and the œ live, and the
/// renderer's escaper emits exactly these.
fn winansi_char(code: u8) -> Option<char> {
    Some(match code {
        0x20..=0x7E => char::from(code),
        0x80 => '€',
        0x82 => '\u{201A}',
        0x83 => 'ƒ',
        0x84 => '\u{201E}',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8A => 'Š',
        0x8B => '‹',
        0x8C => 'Œ',
        0x8E => 'Ž',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201C}',
        0x94 => '\u{201D}',
        0x95 => '•',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '˜',
        0x99 => '™',
        0x9A => 'š',
        0x9B => '›',
        0x9C => 'œ',
        0x9E => 'ž',
        0x9F => 'Ÿ',
        0xA0..=0xFF => char::from(code),
        _ => return None,
    })
}

fn u16b(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

fn i16b(d: &[u8], at: usize) -> Option<i16> {
    u16b(d, at).map(|v| v as i16)
}

fn u32b(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
        *d.get(at + 2)?,
        *d.get(at + 3)?,
    ]))
}

fn i32b(d: &[u8], at: usize) -> Option<i32> {
    u32b(d, at).map(|v| v as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled face parses, and its licence bits allow embedding. If a
    /// future asset swap breaks either, it breaks here and not at a printer.
    #[test]
    fn the_bundled_face_parses_and_may_be_embedded() {
        let m = metrics().expect("police lisible");
        assert!(m.embeddable(), "fsType {} interdit l'incorporation", m.fs_type);
        assert_eq!(m.widths.len(), 224, "32 à 255 inclus");
        assert!(m.ascent > 0 && m.descent < 0, "{m:?}");
        assert!(m.cap_height > 0);
        assert_eq!(m.italic_angle, 0.0, "la romaine n'est pas inclinée");
        assert!(m.bbox[0] < m.bbox[2] && m.bbox[1] < m.bbox[3], "{:?}", m.bbox);
    }

    /// Widths are real: a space is narrow, an M is wide, and nothing a caption
    /// can print falls back to .notdef.
    #[test]
    fn widths_describe_the_actual_glyphs() {
        let m = metrics().unwrap();
        let w = |c: char| m.widths[c as usize - FIRST_CHAR as usize];
        assert!(w(' ') > 0, "l'espace a une chasse");
        assert!(w('M') > w('i'), "M {} contre i {}", w('M'), w('i'));
        assert!(w('W') > w('.'));
        // The French set, which is the whole point of picking this face.
        for c in ['é', 'è', 'ê', 'à', 'ù', 'ô', 'ç', 'œ', 'Œ', '«', '»', '\u{2019}', '…', '€'] {
            let code = winansi_code(c).unwrap_or_else(|| panic!("{c} hors WinAnsi"));
            let width = m.widths[usize::from(code) - usize::from(FIRST_CHAR)];
            assert!(width > 0, "{c} sans chasse : glyphe absent de la police");
        }
    }

    /// The cmap really is being read: a character the face carries resolves to
    /// a glyph, one it cannot resolves to none.
    #[test]
    fn cmap_resolves_glyphs() {
        let cmap = table(FONT_DATA, b"cmap").unwrap();
        let sub = unicode_subtable(cmap).unwrap();
        assert!(lookup(sub, 'A' as u32).unwrap() > 0);
        assert!(lookup(sub, 'é' as u32).unwrap() > 0);
        assert!(lookup(sub, 'œ' as u32).unwrap() > 0);
        // A CJK ideograph is outside a Latin text face.
        assert_eq!(lookup(sub, 0x4E2D).unwrap(), 0);
    }

    /// A truncated file is refused rather than read past its end.
    #[test]
    fn a_truncated_font_is_refused() {
        assert!(parse(&FONT_DATA[..200]).is_err());
        assert!(parse(b"pas une police").is_err());
    }
}
