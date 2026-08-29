//! The album's text face, and every face the machine carries.
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
//!
//! The same reader answers a second question — *which faces does this machine
//! carry, and which of them may enter a file* — and that question changes what
//! it has to survive. It no longer reads our asset, known and sound: it reads
//! whatever a system folder holds. Collections of a dozen faces, CFF outlines,
//! variable fonts, colour emoji, bitmap-only faces, and files that are not
//! fonts at all. So every offset and every length now comes out of the file
//! being read, every access goes through a bounded read that returns `None`
//! rather than through arithmetic that could overflow, and anything unreadable
//! comes back as a refusal carrying its reason — never a panic, never silence.
//!
//! Beside the reader stands a writer, and it is here rather than in a module
//! of its own so that the two cannot drift apart: it emits a face as a font
//! file of its own. Seven faces in ten on a stock machine live inside a
//! collection, a PDF's `FontFile2` carries a single-face sfnt, and no reader
//! is obliged to accept a `.ttc` — so pulling a face out of its file is what
//! lets the engine embed it at all. It is surgery on the table directory and
//! never on a glyph: tables are copied verbatim, whole, and the ones a PDF
//! does not need are dropped rather than rewritten.
//!
//! Reading and writing are all this module does. It embeds one face, the one
//! below, and knowing about the others is not yet using them.

use anyhow::{anyhow, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// The face itself. ~430 kB, embedded whole rather than subset: PDF/X asks for
/// embedded, not minimal, and a print PDF runs to tens of megabytes of photos.
/// Subsetting stays open as a size optimisation, never as a conformance one.
pub const FONT_DATA: &[u8] = include_bytes!("../assets/SourceSans3-Regular.ttf");

/// PostScript name, as it goes in `/BaseFont`.
pub const FONT_NAME: &str = "SourceSans3-Regular";

/// A face we cannot identify at all: not a font, truncated before its tables,
/// nameless, or a collection index that does not exist.
pub const REFUS_ILLISIBLE: &str = "illisible";
/// `fsType` bit 0x0002: the vendor forbids embedding outright.
pub const REFUS_EMBARQUEMENT_INTERDIT: &str = "embarquement_interdit";
/// Nothing to embed but bitmaps: either the licence says so (`fsType` bit
/// 0x0200), or the face carries no outline table at all, which is how colour
/// emoji and bitmap faces come out.
pub const REFUS_BITMAP_SEULEMENT: &str = "bitmap_seulement";
/// A character map in no format we read, or none at all. The face may be
/// perfectly good; we simply cannot say how wide a caption sets in it.
pub const REFUS_CMAP_ILLISIBLE: &str = "cmap_illisible";
/// Outlines in a shape no PDF can carry. `CFF2` is the only one: the format
/// defines no embedding for it, so a face that has nothing else can be read,
/// named and measured, and still never enter a file.
pub const REFUS_FORMAT_NON_EMBARQUABLE: &str = "format_non_embarquable";
/// The album names a face, and the copy that should sit beside `album.json`
/// is gone, unreadable, or named something we never write. Not a verdict on
/// a face — nobody read one — but the one thing the screen must say out
/// loud, because the book then prints in a face nobody chose.
pub const REFUS_FICHIER_ABSENT: &str = "fichier_absent";

/// The two names a face copied beside `album.json` goes by, and the whole
/// vocabulary of [`crate::model::Police::fichier`]: quadratic outlines in a
/// `.ttf`, CFF ones in an `.otf`, which is what a reader expects of each.
/// Fixed rather than derived from the original file name — a face pulled out
/// of `HelveticaNeue.ttc` is not `HelveticaNeue.ttc`, and a name coming back
/// out of a hand-repaired `album.json` must never be able to be a path.
pub const POLICE_TTF: &str = "police.ttf";
/// The CFF half of [`POLICE_TTF`].
pub const POLICE_OTF: &str = "police.otf";


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
}

impl Metrics {
    /// True when the licence bits allow us to put the face in a file at all.
    /// Checked rather than assumed: swapping the asset must not quietly ship
    /// a font whose vendor forbade embedding. The same rule the coded verdict
    /// uses, read as the yes-or-no the emitter asks for.
    pub fn embeddable(&self) -> bool {
        verdict_fs_type(self.fs_type).is_none()
    }
}

/// What a face's outlines are made of. Read from the tables the file
/// declares, never from its extension: a `.ttf` may hold CFF outlines and
/// an `.otf` may hold quadratic ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    Glyf,
    Cff,
    /// The shape a variable face gives CFF outlines. Kept apart from `Cff`
    /// rather than folded into it because PDF defines no embedding for it:
    /// the difference is the whole of [`REFUS_FORMAT_NON_EMBARQUABLE`].
    Cff2,
}

impl Genre {
    /// The code the engine speaks. Like the refusals, a screen turns it into
    /// a word; the engine never carries the word.
    pub fn code(self) -> &'static str {
        match self {
            Genre::Glyf => "glyf",
            Genre::Cff => "cff",
            Genre::Cff2 => "cff2",
        }
    }
}

/// One named design inside a font file. `Helvetica.ttc` is a file; it carries
/// several faces, each with its own name, its own metrics, and its own right
/// to enter a PDF.
#[derive(Debug, Clone)]
pub struct Face {
    /// Rank in the file: zero for a lone face, the collection's index in a
    /// `.ttc`. With the path, it is the face's address.
    pub index: u32,
    /// PostScript name (name ID 6), what would go in `/BaseFont`.
    pub postscript: String,
    /// Readable name: family (ID 1) and style (ID 2), joined. Trimming a
    /// trailing "Regular" out of it is a screen's decision, not the engine's.
    pub nom: String,
    /// Family alone (name ID 1), empty when the file declares none. Carried
    /// beside [`Face::nom`] rather than cut back out of it: a picker groups
    /// eight hundred faces by family, and splitting « MuktaMahee Medium
    /// Regular » back into its two halves from the outside is guesswork on
    /// exactly the names that need it least.
    pub famille: String,

    /// What the outlines are made of; `None` when there are none, which is a
    /// refusal rather than a kind.
    pub genre: Option<Genre>,
    /// Carries an `fvar` table. The metrics below are the default instance,
    /// which is exactly what the shared tables describe, so they need no
    /// correction. The axes are deliberately not exposed: that is interface.
    pub variable: bool,
    /// `fsType` bit 0x0100: the face may be embedded, whole only. The engine
    /// embeds faces whole anyway, so this is remembered, never refused.
    pub sous_ensemblage_interdit: bool,
    /// The metrics, in the 1000-unit em. `None` only when the character map
    /// could not be read, since no width can be measured without one.
    pub metrics: Option<Metrics>,
    /// `None` when the face may be embedded, the refusal code otherwise. A
    /// code, never a sentence: the wording belongs to the screen, the way
    /// `curation.json` carries `illisible` and the app writes the line.
    pub refus: Option<&'static str>,
}

impl Face {
    /// Read face `index` out of a font file.
    ///
    /// `Err` is [`REFUS_ILLISIBLE`] and means the bytes name no face at all.
    /// Every other refusal comes back as a `Face` that still names itself and
    /// carries its code: a screen has to say *which* face it is refusing, and
    /// it must never have to work the reason out a second time.
    pub fn parse(data: &[u8], index: u32) -> std::result::Result<Self, &'static str> {
        let dir = sfnt_dir(data, index).ok_or(REFUS_ILLISIBLE)?;
        let name = table(data, dir, b"name").ok_or(REFUS_ILLISIBLE)?;
        let postscript = name_string(name, 6);
        let famille = name_string(name, 1);
        if postscript.is_none() && famille.is_none() {
            return Err(REFUS_ILLISIBLE);
        }
        let nom = match (&famille, name_string(name, 2)) {
            (Some(f), Some(s)) if !s.is_empty() => format!("{f} {s}"),
            (Some(f), _) => f.clone(),
            // Nothing readable but the PostScript name. Better than a blank
            // line; pulling a style back out of it would be guesswork.
            (None, _) => postscript.clone().unwrap_or_default(),
        };

        let postscript = postscript.unwrap_or_else(|| nom.replace(' ', ""));

        // Presence comes from the directory, not from the bytes: the walk
        // never pulls an outline table off the disk, so asking for its
        // content here would read as "absent". `CFF2` is a kind of its own,
        // not a flavour of CFF: PDF defines no embedding for it, and a face
        // we will never be able to embed has to say so when it is read, not
        // when someone picks it. Nine faces of a stock macOS come out that
        // way, every one an internal `.SF` nobody chooses.
        //
        // No outline table is the only bitmap rule, and it is deliberate.
        // Refusing on a bitmap strike instead — `sbix`, `CBDT`, `EBDT`,
        // `bdat` — was measured against a stock macOS on 28/08 and would
        // have refused Courier New, Monaco, Geneva, Cochin, PT Sans and
        // Euphemia, every one of them a text face that merely ships small
        // sizes as bitmaps beside full outlines. Apple Color Emoji is no
        // exception either: 3811 of its 3844 glyphs carry real contours
        // under the colour. One face on that machine has none at all —
        // `NISC18030.ttf`, `bdat` and no `glyf` — and this is what catches it.
        let genre = if table_range(data, dir, b"glyf").is_some() {
            Some(Genre::Glyf)
        } else if table_range(data, dir, b"CFF ").is_some() {
            Some(Genre::Cff)
        } else if table_range(data, dir, b"CFF2").is_some() {
            Some(Genre::Cff2)
        } else {
            None
        };

        // A face whose shared tables will not read is unreadable — unless it
        // has no outlines at all, which is a refusal of its own and the more
        // useful thing to say. Apple's bitmap faces carry no `head`, only a
        // `bhed`, and would otherwise come back as unnameable rubble.
        let communes = Communes::lire(data, dir);
        if communes.is_none() && genre.is_some() {
            return Err(REFUS_ILLISIBLE);
        }
        let fs_type = communes.as_ref().map_or(0, |c| c.fs_type);
        let metrics = communes
            .as_ref()
            .and_then(|c| table(data, dir, b"cmap").and_then(|m| c.metrics(m)));

        // Licence first: what the vendor forbids outranks what the file
        // happens to carry. Then the outlines, whose absence or whose shape
        // settles the matter before any question of measurement: a face we
        // could not embed if we wanted to is refused for that, not for a
        // character map it happens to lack as well.
        let refus = verdict_fs_type(fs_type)
            .or(genre.is_none().then_some(REFUS_BITMAP_SEULEMENT))
            .or((genre == Some(Genre::Cff2)).then_some(REFUS_FORMAT_NON_EMBARQUABLE))
            .or(metrics.is_none().then_some(REFUS_CMAP_ILLISIBLE));

        Ok(Face {
            index,
            postscript,
            famille: famille.unwrap_or_default(),
            nom,
            genre,

            variable: table_range(data, dir, b"fvar").is_some(),
            sous_ensemblage_interdit: fs_type & 0x0100 != 0,
            metrics,
            refus,
        })
    }

    /// True when this face may go in a file.
    pub fn embeddable(&self) -> bool {
        self.refus.is_none()
    }

    /// Pull face `index` out of `data` as a font file of its own: one sfnt,
    /// one face, carrying only the tables a PDF asks of it.
    ///
    /// The bytes come back rather than a path: what is written and where is
    /// not this reader's business. What comes back is a real font file — a
    /// directory, checksums and all — not one our own tests would accept.
    ///
    /// `data` must be **the whole file**. The buffer [`lire_tables`] hands
    /// the walk is not: outlines were never read into it, so extracting from
    /// it yields a face that is sound in structure and empty of drawing, and
    /// no metric on earth would notice. Discovery reads little, extraction
    /// reads everything; that is deliberate, and it is the sharpest edge in
    /// this module.
    ///
    /// `Err` is the refusal code the face carries, so a screen says the same
    /// thing whether it read the face or tried to take it: a face the vendor
    /// forbids, or whose outlines are `CFF2`, is refused here too.
    pub fn extraire(data: &[u8], index: u32) -> std::result::Result<Vec<u8>, &'static str> {
        let face = Self::parse(data, index)?;
        if let Some(code) = face.refus {
            return Err(code);
        }
        let dir = sfnt_dir(data, index).ok_or(REFUS_ILLISIBLE)?;
        // The face's own signature, never one we picked: a `.ttc` holds
        // `OTTO` faces and TrueType ones side by side, and the file we emit
        // has to say which it carries.
        let version = data.get(dir..dir.saturating_add(4)).ok_or(REFUS_ILLISIBLE)?.to_vec();

        let (par_genre, requises_du_genre): (&[&[u8; 4]], &[&[u8; 4]]) = match face.genre {
            Some(Genre::Glyf) => (&TABLES_GARDEES_GLYF, &[b"glyf", b"loca"]),
            Some(Genre::Cff) => (&TABLES_GARDEES_CFF, &TABLES_GARDEES_CFF),
            // Unreachable: `parse` refuses a face with no outlines and a
            // `CFF2` one above. Written as a refusal rather than an
            // `unreachable!` because a panic here would be a crash on a
            // font file, which this module does not do.
            _ => return Err(REFUS_FORMAT_NON_EMBARQUABLE),
        };

        let mut tables: Vec<(&[u8; 4], &[u8])> = Vec::new();
        for tag in TABLES_GARDEES.iter().chain(par_genre) {
            match table(data, dir, tag) {
                Some(bytes) => tables.push((tag, bytes)),
                // Absent, or long enough to run off the end of the file —
                // the same thing to us. Dropped, unless the face would not
                // be a face without it.
                None if TABLES_REQUISES.contains(tag) || requises_du_genre.contains(tag) => {
                    return Err(REFUS_ILLISIBLE)
                }
                None => {}
            }
        }
        tables.sort_by_key(|(tag, _)| **tag);

        let n = u16::try_from(tables.len()).map_err(|_| REFUS_ILLISIBLE)?;
        let mut out = vec![0u8; 12 + tables.len() * 16];
        out[0..4].copy_from_slice(&version);
        out[4..6].copy_from_slice(&n.to_be_bytes());
        // The binary-search hints of the directory: the largest power of two
        // that fits, times sixteen. Nothing in this file reads them back; a
        // font file carries them, and one that carries nonsense is one a
        // validator has an opinion about.
        let selector = 15u16.saturating_sub(n.leading_zeros() as u16);
        let range = 16u16 << selector;
        out[6..8].copy_from_slice(&range.to_be_bytes());
        out[8..10].copy_from_slice(&selector.to_be_bytes());
        out[10..12].copy_from_slice(&(n * 16 - range).to_be_bytes());

        // Tables verbatim, in directory order, each starting on a four-byte
        // boundary. Nothing is renumbered: `loca`, `glyf`, `maxp` and `head`
        // travel together and stay consistent because none of them is
        // touched. Rebuild one without the others and every glyph shifts.
        let mut head_at = None;
        for (i, (tag, bytes)) in tables.iter().enumerate() {
            while out.len() % 4 != 0 {
                out.push(0);
            }
            let off = out.len();
            if *tag == b"head" {
                head_at = Some(off);
            }
            out.extend_from_slice(bytes);
            let rec = 12 + i * 16;
            out[rec..rec + 4].copy_from_slice(*tag);
            out[rec + 8..rec + 12]
                .copy_from_slice(&u32::try_from(off).map_err(|_| REFUS_ILLISIBLE)?.to_be_bytes());
            out[rec + 12..rec + 16].copy_from_slice(
                &u32::try_from(bytes.len()).map_err(|_| REFUS_ILLISIBLE)?.to_be_bytes(),
            );
        }
        while out.len() % 4 != 0 {
            out.push(0);
        }

        // `head.checkSumAdjustment` is the one field an extraction rewrites,
        // and it is zero while every checksum is computed — its own table's
        // included, which is how the format defines it. Written last, over
        // a sum taken on the finished file.
        let head_at = head_at.ok_or(REFUS_ILLISIBLE)?;
        if out.len() < head_at + 12 {
            return Err(REFUS_ILLISIBLE);
        }
        out[head_at + 8..head_at + 12].copy_from_slice(&0u32.to_be_bytes());
        for i in 0..tables.len() {
            let rec = 12 + i * 16;
            let off = usize::try_from(u32b(&out, rec + 8).ok_or(REFUS_ILLISIBLE)?).unwrap_or(0);
            let len = usize::try_from(u32b(&out, rec + 12).ok_or(REFUS_ILLISIBLE)?).unwrap_or(0);
            let fin = off.saturating_add(len.next_multiple_of(4)).min(out.len());
            let somme = somme_sfnt(out.get(off..fin).ok_or(REFUS_ILLISIBLE)?);
            out[rec + 4..rec + 8].copy_from_slice(&somme.to_be_bytes());
        }
        let ajustement = 0xB1B0_AFBAu32.wrapping_sub(somme_sfnt(&out));
        out[head_at + 8..head_at + 12].copy_from_slice(&ajustement.to_be_bytes());
        Ok(out)
    }

    /// A face we could not read, named by nothing but its rank. The walk
    /// still lists it: a file that will not open is a refusal a screen can
    /// show, where a silently skipped file is a face the reader will hunt for.
    fn refusee(index: u32, code: &'static str) -> Self {
        Face {
            index,
            postscript: String::new(),
            nom: String::new(),
            famille: String::new(),
            genre: None,

            variable: false,
            sous_ensemblage_interdit: false,
            metrics: None,
            refus: Some(code),
        }
    }
}

/// What the licence bits alone forbid.
///
/// 0x0004 (Preview & Print) is accepted: an album's PDF is exactly viewing
/// and printing, which is what that bit licenses. 0x0100 (no subsetting) is
/// accepted too and remembered on the face, the engine embedding faces whole.
fn verdict_fs_type(fs_type: u16) -> Option<&'static str> {
    if fs_type & 0x0002 != 0 {
        Some(REFUS_EMBARQUEMENT_INTERDIT)
    } else if fs_type & 0x0200 != 0 {
        Some(REFUS_BITMAP_SEULEMENT)
    } else {
        None
    }
}

/// Read the face bundled above. One reader for every face, and Source Sans 3
/// is the reader's first integration test.
pub fn metrics() -> Result<Metrics> {
    let face = Face::parse(FONT_DATA, 0)
        .map_err(|code| anyhow!("police incorporée illisible : {code}"))?;
    face.metrics
        .ok_or_else(|| anyhow!("police incorporée refusée : {}", face.refus.unwrap_or("?")))
}

// --- La face qu'on écrit ----------------------------------------------------

/// A face opened for writing: it keeps its bytes.
///
/// [`Face`] reads a file and lets it go, which is what discovery wants — 787
/// faces named without holding a byte. Setting a line is the opposite need:
/// every character asks the character map for a glyph and `hmtx` for its
/// advance, and the PDF asks for the file itself. So this one holds on.
///
/// It is the only place a string turns into glyphs. [`text_width_mm`] and the
/// emitter both come through here, or the album would be measured against one
/// set of advances and drawn with another — the two-geometries trap, applied
/// to type.
#[derive(Debug, Clone)]
pub struct Embarquee {
    /// The whole font file, as it will go into `FontFile2`.
    data: Vec<u8>,
    /// The Unicode character map's subtable, copied out of the file. Copied
    /// rather than borrowed: a struct that holds a slice of its own field is
    /// a fight with the borrow checker that buys nothing here.
    sous_table: Vec<u8>,
    /// Advance per glyph id, in the 1000-unit em. Its length is the face's
    /// glyph count, so it is the bounds check as well.
    avances: Vec<i32>,
    /// The glyph a character the face cannot draw is replaced by. `None` when
    /// the face cannot even draw that, and then such characters are dropped.
    interro: Option<u16>,
    metrics: Metrics,
    postscript: String,
}

impl Embarquee {
    /// Open a face for writing. `Err` carries the face's refusal code: a face
    /// the vendor forbids, or one whose outlines PDF cannot embed, never gets
    /// this far.
    ///
    /// `data` must be **the whole file** — the same rule as [`Face::extraire`],
    /// and for the same reason: the buffer discovery walks with has zeros
    /// where the outlines are.
    pub fn depuis(data: Vec<u8>, index: u32) -> std::result::Result<Self, &'static str> {
        let face = Face::parse(&data, index)?;
        if let Some(code) = face.refus {
            return Err(code);
        }
        let metrics = face.metrics.clone().ok_or(REFUS_CMAP_ILLISIBLE)?;
        let dir = sfnt_dir(&data, index).ok_or(REFUS_ILLISIBLE)?;
        let communes = Communes::lire(&data, dir).ok_or(REFUS_ILLISIBLE)?;
        let cmap = table(&data, dir, b"cmap").ok_or(REFUS_CMAP_ILLISIBLE)?;
        let sous_table = unicode_subtable(cmap).ok_or(REFUS_CMAP_ILLISIBLE)?.to_vec();
        let avances = communes.avances();
        let mut face = Embarquee {
            data,
            sous_table,
            avances,
            interro: None,
            metrics,
            postscript: face.postscript,
        };
        face.interro = face.glyphe('?');
        Ok(face)
    }

    /// The face this crate ships, opened once. Wrapping a paragraph asks for
    /// the advances on every candidate line, and re-reading 430 kB of
    /// TrueType each time would turn a cover render into a benchmark.
    pub fn incorporee() -> Option<&'static Embarquee> {
        static INCORPOREE: std::sync::LazyLock<Option<Embarquee>> =
            std::sync::LazyLock::new(|| Embarquee::depuis(FONT_DATA.to_vec(), 0).ok());
        INCORPOREE.as_ref()
    }

    /// The glyphs a string sets to, each with the character it draws.
    ///
    /// The character comes back because `ToUnicode` needs it, and taking it
    /// from here rather than walking the character map backwards makes that
    /// table exact by construction: what the reader is told a glyph means is
    /// what we asked the face to draw.
    ///
    /// A character the face has no glyph for is replaced by `?` — never the
    /// `.notdef` box, which prints as a hollow rectangle nobody sees before
    /// the guillotine. A face that cannot draw `?` either drops it.
    pub fn glyphes(&self, s: &str) -> Vec<(u16, char)> {
        let mut out = Vec::with_capacity(s.len());
        for c in s.chars() {
            match self.glyphe(c) {
                Some(gid) => out.push((gid, c)),
                None => out.extend(self.interro.map(|gid| (gid, '?'))),
            }
        }
        out
    }

    /// The glyph a character maps to, or `None` when the face has none. Glyph
    /// zero is `.notdef` and counts as none: some maps say "missing" that way
    /// rather than by leaving the character out.
    fn glyphe(&self, c: char) -> Option<u16> {
        lookup(&self.sous_table, c as u32)
            .filter(|gid| *gid != 0 && usize::from(*gid) < self.avances.len())
    }

    /// A glyph's advance, in the 1000-unit em PDF works in. This is the
    /// number `/W` carries, so a width measured here and a width declared
    /// there cannot drift apart.
    pub fn avance(&self, gid: u16) -> i32 {
        self.avances.get(usize::from(gid)).copied().unwrap_or(0)
    }

    /// How wide a string sets, in millimetres, at `size_pt`.
    ///
    /// Advance widths only, no kerning: the face's own kerning lives in
    /// tables this reader does not walk, and what is measured here has to be
    /// what the PDF draws.
    pub fn largeur_mm(&self, s: &str, size_pt: f64) -> f64 {
        let em: i32 = self.glyphes(s).iter().map(|(gid, _)| self.avance(*gid)).sum();
        f64::from(em) / 1000.0 * size_pt * 25.4 / 72.0
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// The bytes, for `FontFile2` and its `Length1`.
    pub fn octets(&self) -> &[u8] {
        &self.data
    }

    /// The name that goes in `/BaseFont`, read from the face rather than
    /// assumed: the day a face other than ours is embedded, it names itself.
    pub fn postscript(&self) -> &str {
        &self.postscript
    }
}

/// The glyphs a document actually draws, and what each one means.
///
/// `/W` and `/ToUnicode` are written from this and nothing else. Listing the
/// whole face instead costs little on the 2 000 glyphs of Source Sans 3 and
/// becomes absurd on a CJK face, where a three-word caption would drag tens of
/// thousands of entries behind it.
///
/// Ordered, and that is not a detail: two exports of the same album have to be
/// identical to the byte, and a hash map would shuffle `/W` between runs.
#[derive(Debug, Clone, Default)]
pub struct Utilises(std::collections::BTreeMap<u16, char>);

impl Utilises {
    /// Record a glyph as drawn. When two characters share a glyph, the
    /// smallest code point wins — any rule would do, as long as it is a rule
    /// and not the order the album happens to be written in.
    pub fn noter(&mut self, gid: u16, c: char) {
        self.0
            .entry(gid)
            .and_modify(|garde| {
                if c < *garde {
                    *garde = c;
                }
            })
            .or_insert(c);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Glyph and meaning, in glyph order.
    pub fn iter(&self) -> impl Iterator<Item = (u16, char)> + '_ {
        self.0.iter().map(|(gid, c)| (*gid, *c))
    }
}

/// How wide a string sets in the album's face, in millimetres, at `size_pt`.
///
/// A character the face cannot draw measures as the `?` that will be drawn in
/// its place, because measuring and drawing are the same walk.
pub fn text_width_mm(s: &str, size_pt: f64) -> f64 {
    Embarquee::incorporee().map_or(0.0, |face| face.largeur_mm(s, size_pt))
}

// --- La face de l'album -----------------------------------------------------

/// Which of the two names an extracted face is written under. Read off the
/// outlines, never off the file it came from: a `.ttc` holds both kinds.
pub fn fichier_pour(genre: Genre) -> &'static str {
    match genre {
        Genre::Cff => POLICE_OTF,
        // `Cff2` never gets here — it is refused at reading — and calling it
        // a `.ttf` would be the least of that file's problems.
        _ => POLICE_TTF,
    }
}

/// Pull face `index` out of `data` and say what to call it: the bytes to
/// write beside `album.json`, and the one of the two names they go under.
///
/// `Err` is the face's refusal code, [`Face::extraire`]'s exactly.
pub fn extraire_pour_album(
    data: &[u8],
    index: u32,
) -> std::result::Result<(&'static str, Vec<u8>), &'static str> {
    let genre = Face::parse(data, index)?.genre.ok_or(REFUS_BITMAP_SEULEMENT)?;
    Ok((fichier_pour(genre), Face::extraire(data, index)?))
}

/// The face an album is set in, resolved from its own folder.
///
/// The one place the question is asked, because it has two answers and only
/// one of them is the happy one: the album named a face and its copy is
/// there, or it did not name one — and then, third, it named one whose copy
/// is gone. That last case does not fail. The book comes out in the face
/// this crate ships and `defaut` carries the reason, because an export that
/// dies on a missing file would cost somebody their evening, and one that
/// silently prints in another face would cost them a print run.
pub struct FaceAlbum {
    /// Borrowed for the project's face, owned for the album's own: exactly
    /// the two arms [`crate::pdf::Ecrivain`] was written around.
    pub face: std::borrow::Cow<'static, Embarquee>,
    /// `Some(code)` when a face was named and could not be used. The code is
    /// a refusal code like any other, and the screen words it.
    pub defaut: Option<&'static str>,
}

/// Read the album's face out of `dir`, beside its `album.json`.
///
/// `fichier` is what `album.json` says, and it is checked against the two
/// names this crate writes rather than joined to `dir` on trust: the file is
/// hand-editable, and `dir.join(n'importe quoi)` is how a project acquires a
/// path traversal. A name we never write is a missing file.
///
/// The bytes read are the whole file, which is what [`Embarquee::depuis`]
/// requires — and here it costs nothing, an extracted face being one face.
pub fn face_album(dir: &Path, fichier: Option<&str>) -> FaceAlbum {
    let Some(nom) = fichier else {
        return FaceAlbum { face: face_projet(), defaut: None };
    };
    if nom != POLICE_TTF && nom != POLICE_OTF {
        return FaceAlbum { face: face_projet(), defaut: Some(REFUS_FICHIER_ABSENT) };
    }
    let Ok(data) = std::fs::read(dir.join(nom)) else {
        return FaceAlbum { face: face_projet(), defaut: Some(REFUS_FICHIER_ABSENT) };
    };

    // An extracted face is one face, so index zero — and if these bytes are
    // not one, the face carries its own reason for it.
    match Embarquee::depuis(data, 0) {
        Ok(face) => FaceAlbum { face: std::borrow::Cow::Owned(face), defaut: None },
        Err(code) => FaceAlbum { face: face_projet(), defaut: Some(code) },
    }
}

/// The face this crate ships, ready to write with, and the one place that
/// panics when it is not readable. That is a broken build artifact, not a
/// user error: falling back to a non-embedded base-14 would be the silent
/// export failure this project refuses.
pub fn face_projet() -> std::borrow::Cow<'static, Embarquee> {
    std::borrow::Cow::Borrowed(
        Embarquee::incorporee()
            .expect("police incorporée illisible ou refusée : asset corrompu"),
    )
}



// --- Les faces installées ---------------------------------------------------

/// A face this machine carries, and the file it lives in. Path and index are
/// the address; everything else the face knows about itself.
#[derive(Debug, Clone)]
pub struct Installee {
    pub chemin: PathBuf,
    pub face: Face,
}

/// The extensions a font file goes by. A file named otherwise is not opened:
/// this is an enumeration, not a sniffing pass over every system file.
const EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];

/// The tables a reading needs — and, just as much, the ones it does not.
/// `glyf` and `CFF ` are absent on purpose: their presence is read off the
/// directory, their bytes never leave the disk. Over six hundred system faces
/// that is the difference between an enumeration and a load.
const TABLES_LUES: [&[u8; 4]; 8] = [
    b"head", b"hhea", b"hmtx", b"maxp", b"cmap", b"OS/2", b"post", b"name",
];

/// Every face the platform's font folders carry.
pub fn installed() -> Vec<Installee> {
    installed_in(&dossiers_systeme())
}

/// Every face `dirs` carry, refused ones included, sorted by path then index.
///
/// The manners are `scan.rs`'s: symlinks are not followed, hidden files are
/// skipped, sub-folders are walked, and the order is ours rather than the file
/// system's — two runs on one machine must list the same thing in the same
/// order. A file that will not parse is one refused face rather than a
/// silence, because a screen listing faces has to be able to say why one is
/// missing, and it reads the code from here rather than working it out again.
pub fn installed_in(dirs: &[PathBuf]) -> Vec<Installee> {
    let mut out = Vec::new();
    for dir in dirs {
        for entry in WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let ext = entry
                .path()
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let chemin = entry.into_path();
            let Some((data, _)) = lire_tables(&chemin) else {
                out.push(Installee { chemin, face: Face::refusee(0, REFUS_ILLISIBLE) });
                continue;
            };
            for index in 0..face_count(&data).max(1) {
                let face = Face::parse(&data, index).unwrap_or_else(|c| Face::refusee(index, c));
                out.push(Installee { chemin: chemin.clone(), face });
            }
        }
    }
    out.sort_by(|a, b| (&a.chemin, a.face.index).cmp(&(&b.chemin, b.face.index)));
    out
}

/// Where the platform keeps its fonts. Folders that do not exist are dropped
/// rather than walked: a machine without a user font folder is normal, and so
/// is a Linux box with no system fonts at all.
pub fn dossiers_systeme() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
        if let Some(h) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(h).join("Library/Fonts"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let racine = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        dirs.push(PathBuf::from(racine).join("Fonts"));
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(l).join("Microsoft").join("Windows").join("Fonts"));
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(h) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(h).join(".local/share/fonts"));
        }
    }
    dirs.retain(|d| d.is_dir());
    dirs
}

/// The most faces a collection is believed on. Measured on a stock macOS on
/// 28/08: the fattest `.ttc` on the machine carries 46 faces, the next 18.
/// The format lets the header claim any 32-bit count, and nothing but this
/// stands between a corrupt one and a walk that tries to read millions of
/// faces out of a file that has two — measured before the cap: a 20 MB file
/// claiming 0xFFFFFFFF produced five million entries and a gigabyte in six
/// seconds. A file that claims more than this is corrupt, not a font with a
/// thousand designs, and is read for its first faces and no further.
const FACES_MAX: u32 = 256;

/// How many faces a file carries: a collection says so, anything else is one
/// face — including a file that is no font at all, which then comes back as
/// one refusal rather than as nothing.
pub fn face_count(data: &[u8]) -> u32 {
    if data.get(0..4) != Some(&b"ttcf"[..]) {
        return 1;
    }
    // Two ceilings, both against a header that lies: what the file is
    // physically long enough to hold, and what a real collection ever is.
    let plafond = u32::try_from(data.len().saturating_sub(12) / 4).unwrap_or(u32::MAX);
    u32b(data, 8).unwrap_or(0).min(plafond).min(FACES_MAX)
}

/// Pull off the disk exactly the tables a reading needs, and nothing else.
///
/// What comes back holds every table read at its own file offset, so the
/// reader below is the very one the tests feed a whole file — table offsets
/// being absolute is what makes that possible. Everything not read stays
/// zero. The second value is how many bytes actually came off the disk: what
/// the bench measures, and the reason a font's outlines never do.
///
/// The cost of absolute offsets is that the buffer runs to the far end of the
/// last table read, zeros and all. Measured on a stock macOS on 28/08: 370
/// files, 777 MB on disk, 32 MB actually read (4.2 %), the largest buffer
/// 63 MB and transient, 787 faces enumerated in 29 ms.
fn lire_tables(path: &Path) -> Option<(Vec<u8>, u64)> {
    let mut f = std::fs::File::open(path).ok()?;
    let taille = usize::try_from(f.metadata().ok()?.len()).ok()?;
    let mut buf: Vec<u8> = Vec::new();
    let mut lus = 0u64;
    prendre(&mut f, &mut buf, &mut lus, taille, 0, 12)?;

    // A collection lists its faces' directories behind a header of its own.
    // Read enough of it for the most faces a collection is believed on, then
    // let `face_count` say how many there are: one ceiling, in one place.
    if buf.get(0..4) == Some(&b"ttcf"[..]) {
        let entetes = 12usize.saturating_add(FACES_MAX as usize * 4);
        let _ = prendre(&mut f, &mut buf, &mut lus, taille, 0, entetes);
    }
    let faces = face_count(&buf);

    for index in 0..faces {
        let Some(dir) = dir_offset(&buf, index) else { continue };
        if prendre(&mut f, &mut buf, &mut lus, taille, dir, 12).is_none() {
            continue;
        }
        let Some(count) = u16b(&buf, dir.saturating_add(4)) else { continue };
        let _ = prendre(
            &mut f,
            &mut buf,
            &mut lus,
            taille,
            dir.saturating_add(12),
            usize::from(count).saturating_mul(16),
        );
        for tag in TABLES_LUES {
            if let Some((off, len)) = table_range(&buf, dir, tag) {
                let _ = prendre(&mut f, &mut buf, &mut lus, taille, off, len);
            }
        }
    }
    Some((buf, lus))
}

/// Read `len` bytes at `off` into `buf`, at that same offset, growing the
/// buffer with zeros for everything not read. Every one of these numbers came
/// out of the file being read, so a length that runs off the end is clamped
/// rather than trusted. `None` when nothing, or not all of it, could be read.
fn prendre(
    f: &mut std::fs::File,
    buf: &mut Vec<u8>,
    lus: &mut u64,
    taille: usize,
    off: usize,
    len: usize,
) -> Option<()> {
    let fin = off.saturating_add(len).min(taille);
    if off >= fin {
        return None;
    }
    if buf.len() < fin {
        buf.resize(fin, 0);
    }
    f.seek(SeekFrom::Start(off as u64)).ok()?;
    let mut at = off;
    while at < fin {
        match f.read(&mut buf[at..fin]) {
            Ok(0) | Err(_) => break,
            Ok(n) => at += n,
        }
    }
    *lus += (at - off) as u64;
    (at == fin).then_some(())
}

// --- Le lecteur -------------------------------------------------------------

/// What the tables every face shares say about it — everything but the
/// widths, which need a character map. A face can lack one and still be a
/// face we can name, which is why that reading is separate.
struct Communes<'a> {
    hmtx: &'a [u8],
    num_h: usize,
    num_glyphs: u16,
    upem: f64,
    fs_type: u16,
    bbox: [i32; 4],
    ascent: i32,
    descent: i32,
    cap_height: i32,
    italic_angle: f64,
}

impl<'a> Communes<'a> {
    /// `None` when a table no face can do without is missing or truncated.
    /// `OS/2` and `post` are not among them: plenty of older faces carry
    /// neither, and a reader falls back the same way we do here.
    fn lire(data: &'a [u8], dir: usize) -> Option<Self> {
        let head = table(data, dir, b"head")?;
        let hhea = table(data, dir, b"hhea")?;
        let hmtx = table(data, dir, b"hmtx")?;
        let maxp = table(data, dir, b"maxp")?;
        let os2 = table(data, dir, b"OS/2");

        let upem = f64::from(u16b(head, 18)?);
        if upem <= 0.0 {
            return None;
        }
        // Everything below is expressed in the font's own units; PDF wants a
        // 1000-unit em, so every measure goes through here.
        let to_em = |v: i32| -> i32 { (f64::from(v) * 1000.0 / upem).round() as i32 };

        let num_h = usize::from(u16b(hhea, 34)?);
        if num_h == 0 {
            return None;
        }

        // OS/2 carries the typographic ascent and descent, and from version 2
        // the cap height. Older faces fall back to hhea, which readers do too.
        let os2_version = os2.and_then(|o| u16b(o, 0)).unwrap_or(0);
        let ascent = os2
            .and_then(|o| i16b(o, 68))
            .filter(|v| *v != 0)
            .or_else(|| i16b(hhea, 4))?;
        let descent = os2
            .and_then(|o| i16b(o, 70))
            .filter(|v| *v != 0)
            .or_else(|| i16b(hhea, 6))?;
        let cap_height = if os2_version >= 2 {
            os2.and_then(|o| i16b(o, 88)).filter(|v| *v != 0).unwrap_or(ascent)
        } else {
            ascent
        };

        Some(Communes {
            hmtx,
            num_h,
            num_glyphs: u16b(maxp, 4)?,
            upem,
            fs_type: os2.and_then(|o| u16b(o, 8)).unwrap_or(0),
            bbox: [
                to_em(i16b(head, 36)?.into()),
                to_em(i16b(head, 38)?.into()),
                to_em(i16b(head, 40)?.into()),
                to_em(i16b(head, 42)?.into()),
            ],
            ascent: to_em(ascent.into()),
            descent: to_em(descent.into()),
            cap_height: to_em(cap_height.into()),
            italic_angle: table(data, dir, b"post")
                .and_then(|p| i32b(p, 4))
                // post stores the angle as a 16.16 fixed-point number.
                .map(|v| f64::from(v) / 65536.0)
                .unwrap_or(0.0),
        })
    }

    /// The whole of [`Metrics`]. `None` when the character map is unusable —
    /// no Unicode subtable, or one in a format this reader does not know —
    /// which is the face's one refusal that leaves it named everywhere else.
    ///
    /// The map is read here and thrown away: what a face measures is the
    /// business of [`Embarquee`], which keeps the bytes. This only decides
    /// whether the face has a usable one at all.
    fn metrics(&self, cmap: &[u8]) -> Option<Metrics> {
        let sub = unicode_subtable(cmap)?;
        // A format we cannot walk is a character map we cannot read, not a
        // face whose glyphs are all missing.
        lookup(sub, u32::from('A'))?;
        Some(Metrics {
            bbox: self.bbox,
            ascent: self.ascent,
            descent: self.descent,
            cap_height: self.cap_height,
            italic_angle: self.italic_angle,
            fs_type: self.fs_type,
        })
    }

    /// Every glyph's advance, in the 1000-unit em, indexed by glyph id. Read
    /// once when a face is opened for writing: measuring a caption asks for
    /// the same advances a hundred times, and `hmtx` is a table of pairs
    /// nobody should walk per character.
    fn avances(&self) -> Vec<i32> {
        let to_em = |v: i32| -> i32 { (f64::from(v) * 1000.0 / self.upem).round() as i32 };
        (0..self.num_glyphs)
            .map(|gid| to_em(i32::from(advance(self.hmtx, self.num_h, gid))))
            .collect()
    }
}

/// Where face `index`'s table directory starts.
///
/// A lone file has one directory, at zero. A collection (`ttcf`) lists one per
/// face — **and its tables are still addressed from the start of the file**,
/// never from the directory. Reading those offsets as relative is the classic
/// way to get a reader that works on a `.ttf` and returns nonsense on a `.ttc`.
fn sfnt_dir(data: &[u8], index: u32) -> Option<usize> {
    let dir = dir_offset(data, index)?;
    // The directory has to start with a signature, in a collection as much as
    // in a lone file. Without this, an offset out of a corrupt header walks
    // the reader straight into the outlines and asks them for a name.
    match data.get(dir..dir.saturating_add(4))? {
        b"\x00\x01\x00\x00" | b"true" | b"OTTO" | b"typ1" => Some(dir),
        _ => None,
    }
}

/// Where face `index`'s directory is *said* to start, signature unchecked.
/// [`lire_tables`] needs this before it can read the bytes that signature is
/// written in; everything else goes through [`sfnt_dir`].
fn dir_offset(data: &[u8], index: u32) -> Option<usize> {
    if data.get(0..4) == Some(&b"ttcf"[..]) {
        if index >= face_count(data) {
            return None;
        }
        let at = 12usize.saturating_add(usize::try_from(index).ok()?.saturating_mul(4));
        return usize::try_from(u32b(data, at)?).ok();
    }
    (index == 0).then_some(0)
}

/// Locate a table in the sfnt directory at `dir`.
fn table<'a>(data: &'a [u8], dir: usize, tag: &[u8; 4]) -> Option<&'a [u8]> {
    let (off, len) = table_range(data, dir, tag)?;
    data.get(off..off.saturating_add(len))
}

/// Where a table says it is, before any bounds check: what [`lire_tables`]
/// needs to pull exactly those bytes, and what tells a face's kind apart
/// without reading a single outline.
fn table_range(data: &[u8], dir: usize, tag: &[u8; 4]) -> Option<(usize, usize)> {
    let count = usize::from(u16b(data, dir.saturating_add(4))?);
    for i in 0..count {
        let rec = dir.saturating_add(12).saturating_add(i.saturating_mul(16));
        if data.get(rec..rec.saturating_add(4))? == tag {
            let off = usize::try_from(u32b(data, rec + 8)?).ok()?;
            let len = usize::try_from(u32b(data, rec + 12)?).ok()?;
            return Some((off, len));
        }
    }
    None
}

/// A string of the `name` table, by name id, decoded from the platform that
/// carries it. Windows (platform 3) stores UTF-16BE and is preferred;
/// Macintosh (platform 1) stores MacRoman and is often all an older face
/// carries. Reading only one of the two ends in a name that is either empty
/// or a row of alternating NULs — the classic symptom.
fn name_string(name: &[u8], id: u16) -> Option<String> {
    let count = usize::from(u16b(name, 2)?);
    let storage = usize::from(u16b(name, 4)?);
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let rec = 6usize.saturating_add(i.saturating_mul(12));
        // A malformed record is skipped, never fatal: one bad row must not
        // cost the face its name.
        let lu = || -> Option<(u8, String)> {
            if u16b(name, rec.saturating_add(6))? != id {
                return None;
            }
            let len = usize::from(u16b(name, rec.saturating_add(8))?);
            let off = usize::from(u16b(name, rec.saturating_add(10))?);
            let at = storage.saturating_add(off);
            let bytes = name.get(at..at.saturating_add(len))?;
            let plateforme = u16b(name, rec)?;
            // English first, platform second. A face that names itself in six
            // languages — every stock macOS one does — must not come back
            // named in whichever it happened to list first: "Times 標準體" is
            // a real reading of a real file, and a useless name.
            let rang = match (plateforme, u16b(name, rec.saturating_add(4))?) {
                (3, 0x0409) => 0u8,
                (1, 0) => 1,
                // The Unicode platform declares no language of its own.
                (0, _) => 2,
                (3, _) => 3,
                (1, _) => 4,
                _ => return None,
            };
            let texte = if plateforme == 1 {
                macroman(bytes)
            } else {
                utf16be(bytes)?
            };
            Some((rang, texte))
        };
        if let Some((rang, s)) = lu() {
            if !s.is_empty() && best.as_ref().is_none_or(|(r, _)| rang < *r) {
                best = Some((rang, s));
            }
        }
    }
    best.map(|(_, s)| s)
}

/// UTF-16BE, as platforms 3 and 0 store their names. An odd length or a
/// broken surrogate pair is a name we do not have.
fn utf16be(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

/// The upper half of MacRoman, which is what a Macintosh-platform `name`
/// record is written in. The lower half is ASCII. 0xF0 is Apple's own logo,
/// which Unicode leaves in the private use area.
const MACROMAN_HAUT: &str = "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»…\u{00a0}ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{f8ff}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ";

fn macroman(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| {
            if *b < 0x80 {
                char::from(*b)
            } else {
                MACROMAN_HAUT.chars().nth(usize::from(*b - 0x80)).unwrap_or('\u{fffd}')
            }
        })
        .collect()
}

/// The first Unicode character map: Windows BMP, Windows full, or a plain
/// Unicode platform table, in that order of preference. A symbol table (3, 0)
/// is not one: it maps a private range no caption reaches, and a face that
/// carries nothing else is refused rather than measured wrong.
fn unicode_subtable(cmap: &[u8]) -> Option<&[u8]> {
    let count = usize::from(u16b(cmap, 2)?);
    let mut best: Option<(u8, &[u8])> = None;
    for i in 0..count {
        let rec = 4usize.saturating_add(i.saturating_mul(8));
        let Some(platform) = u16b(cmap, rec) else { continue };
        let Some(encoding) = u16b(cmap, rec + 2) else { continue };
        let Some(off) = u32b(cmap, rec + 4).and_then(|o| usize::try_from(o).ok()) else {
            continue;
        };
        let Some(sub) = cmap.get(off..) else { continue };
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
/// actually uses: segmented BMP (4) and trimmed/sparse full range (12), plus
/// the trimmed byte map (6) some older faces still carry. `None` says the
/// format is one this reader does not walk, which refuses the face.
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
    let count = usize::try_from(u32b(sub, 12)?).ok()?;
    for i in 0..count {
        let g = 16usize.saturating_add(i.saturating_mul(12));
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

fn u16b(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at.checked_add(1)?)?]))
}

fn i16b(d: &[u8], at: usize) -> Option<i16> {
    u16b(d, at).map(|v| v as i16)
}

fn u32b(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(at)?,
        *d.get(at.checked_add(1)?)?,
        *d.get(at.checked_add(2)?)?,
        *d.get(at.checked_add(3)?)?,
    ]))
}

fn i32b(d: &[u8], at: usize) -> Option<i32> {
    u32b(d, at).map(|v| v as i32)
}

// --- L'écrivain -------------------------------------------------------------

/// The tables an extracted face keeps, whatever its outlines are made of.
///
/// A closed list, never "everything but": a table we kept because we did not
/// recognise it is a byte in the file we cannot account for. What falls, and
/// it is most of the weight, is every layout table (`GSUB`, `GPOS`, `GDEF`,
/// `morx`, `kern`, `feat`, `trak`), every bitmap strike (`sbix`, `CBDT`,
/// `CBLC`, `EBDT`, `EBLC`, `bdat`, `bloc`), the vendor's signature (`DSIG`,
/// which would no longer be worth anything over bytes we rearranged), the
/// variation tables (`fvar`, `gvar`, `HVAR`, `MVAR`, `STAT`, `avar`) and
/// everything else. The face that comes out is the default instance, which
/// is what the tables here already describe, and it is static.
const TABLES_GARDEES: [&[u8; 4]; 8] =
    [b"head", b"hhea", b"hmtx", b"maxp", b"cmap", b"name", b"OS/2", b"post"];

/// Kept as well when the outlines are quadratic. `loca` travels with `glyf`
/// or the glyphs shift; `cvt `, `fpgm` and `prep` are the hinting, which a
/// print PDF has no use for and which weighs nothing beside the contours.
const TABLES_GARDEES_GLYF: [&[u8; 4]; 5] = [b"glyf", b"loca", b"cvt ", b"fpgm", b"prep"];

/// And when they are CFF, where the one table holds everything.
const TABLES_GARDEES_CFF: [&[u8; 4]; 1] = [b"CFF "];

/// Without one of these there is no face to emit, whatever the outlines are.
/// `OS/2` and `post` are deliberately not here: plenty of faces carry
/// neither, and the reader falls back the same way on the other side.
const TABLES_REQUISES: [&[u8; 4]; 6] =
    [b"head", b"hhea", b"hmtx", b"maxp", b"cmap", b"name"];

/// The sfnt checksum: big-endian 32-bit words, summed and allowed to wrap.
/// Every table is written padded to four bytes, and so is the whole file, so
/// there is never a tail to worry about.
fn somme_sfnt(data: &[u8]) -> u32 {
    data.chunks_exact(4)
        .fold(0u32, |s, w| s.wrapping_add(u32::from_be_bytes([w[0], w[1], w[2], w[3]])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// The advance a fixture gives its glyphs, read the way the emitter reads
    /// it: through the character map, then `hmtx`. The fixtures give every
    /// glyph the same advance, so one glyph tells the whole story — and it is
    /// the number `/W` would carry.
    fn chasse(data: &[u8], index: u32) -> i32 {
        let face = Embarquee::depuis(data.to_vec(), index).expect("face ouverte pour écrire");
        let poses = face.glyphes("A");
        assert_eq!(poses.len(), 1);
        face.avance(poses[0].0)
    }

    /// The bundled face parses, and its licence bits allow embedding. If a
    /// future asset swap breaks either, it breaks here and not at a printer.
    #[test]
    fn the_bundled_face_parses_and_may_be_embedded() {
        let m = metrics().expect("police lisible");
        assert!(m.embeddable(), "fsType {} interdit l'incorporation", m.fs_type);
        assert!(m.ascent > 0 && m.descent < 0, "{m:?}");
        assert!(m.cap_height > 0);
        assert_eq!(m.italic_angle, 0.0, "la romaine n'est pas inclinée");
        assert!(m.bbox[0] < m.bbox[2] && m.bbox[1] < m.bbox[3], "{:?}", m.bbox);
    }

    /// The face travels through the general reader, and names itself the way
    /// the PDF names it. One reader for every face on earth, and this one is
    /// its first integration test.
    #[test]
    fn la_face_embarquee_passe_par_le_lecteur_general() {
        let f = Face::parse(FONT_DATA, 0).expect("face lisible");
        assert_eq!(f.postscript, FONT_NAME, "le /BaseFont du PDF");
        assert_eq!(f.index, 0);
        assert_eq!(f.genre, Some(Genre::Glyf));
        assert!(!f.variable);
        assert!(f.embeddable() && !f.sous_ensemblage_interdit);
        assert_eq!(face_count(FONT_DATA), 1, "un fichier, une face");
        // Le même objet que `metrics()` rend, au chiffre près.
        let a = f.metrics.expect("métriques");
        let b = metrics().unwrap();
        assert_eq!((a.bbox, a.ascent, a.descent, a.cap_height), (b.bbox, b.ascent, b.descent, b.cap_height));
        assert_eq!(a.fs_type, b.fs_type);
    }

    /// Widths are real: a space is narrow, an M is wide, and nothing a caption
    /// can print falls back to .notdef.
    #[test]
    fn widths_describe_the_actual_glyphs() {
        let face = Embarquee::incorporee().expect("face ouverte");
        let w = |c: char| {
            let g = face.glyphes(&c.to_string());
            assert_eq!(g.len(), 1, "{c} ne se pose pas en un glyphe");
            assert_eq!(g[0].1, c, "{c} est remplacé alors que la face le porte");
            face.avance(g[0].0)
        };
        assert!(w(' ') > 0, "l'espace a une chasse");
        assert!(w('M') > w('i'), "M {} contre i {}", w('M'), w('i'));
        assert!(w('W') > w('.'));
        // The French set, which is the whole point of picking this face.
        for c in ['é', 'è', 'ê', 'à', 'ù', 'ô', 'ç', 'œ', 'Œ', '«', '»', '\u{2019}', '…', '€'] {
            assert!(w(c) > 0, "{c} sans chasse : glyphe absent de la police");
        }
        // Et ce que l'encodage simple ne pouvait pas atteindre : la face les
        // porte, le composite les dessine, la mesure les voit.
        for c in ['ł', 'ș', 'ğ', 'Ω', 'д'] {
            assert!(w(c) > 0, "{c} hors WinAnsi, mais la face le dessine");
        }
    }

    /// The substitution rule, both halves. A character the face cannot draw
    /// becomes the question mark that will be printed in its place — never
    /// the `.notdef` box, which prints as a hollow rectangle nobody sees
    /// before the guillotine. A face that cannot draw that either drops it,
    /// which is the one case where measuring and drawing agree on nothing.
    #[test]
    fn un_caractere_absent_devient_un_point_dinterrogation_ou_rien() {
        let face = Embarquee::depuis(fichier(&[Fonte::neuve()]), 0).expect("face ouverte");
        let interro = face.glyphes("?");
        assert_eq!(interro.len(), 1);
        // La fixture couvre 0x20..0xFF : au-delà, la face ne sait rien.
        let absent = face.glyphes("→");
        assert_eq!(absent, interro, "un caractère absent se dessine en ?");
        assert_eq!(absent[0].1, '?', "et c'est ce que le ToUnicode dira");
        assert_eq!(face.avance(absent[0].0), face.avance(interro[0].0));

        let mut sans = Fonte::neuve();
        sans.cmap = Cmap::SansInterro;
        let face = Embarquee::depuis(fichier(&[sans]), 0).expect("face ouverte");
        assert!(face.glyphes("?").is_empty(), "rien à dessiner, rien de dessiné");
        assert_eq!(face.glyphes("A?B").len(), 2, "seul le caractère perdu tombe");
    }

    /// The cmap really is being read: a character the face carries resolves to
    /// a glyph, one it cannot resolves to none.
    #[test]
    fn cmap_resolves_glyphs() {
        let cmap = table(FONT_DATA, 0, b"cmap").unwrap();
        let sub = unicode_subtable(cmap).unwrap();
        assert!(lookup(sub, 'A' as u32).unwrap() > 0);
        assert!(lookup(sub, 'é' as u32).unwrap() > 0);
        assert!(lookup(sub, 'œ' as u32).unwrap() > 0);
        // A CJK ideograph is outside a Latin text face.
        assert_eq!(lookup(sub, 0x4E2D).unwrap(), 0);
    }

    // --- Des polices écrites à la main ------------------------------------

    /// The character map a synthetic face carries.
    #[derive(Clone, Copy)]
    enum Cmap {
        /// Segmented BMP under (3, 1): what a text face carries.
        Format4,
        /// Sparse groups under (3, 10).
        Format12,
        /// A symbol table (3, 0) and nothing else: unreadable on purpose.
        Symbole,
        /// A byte map under (3, 1): a Unicode table in a format we refuse.
        Format0,
        /// Segmented, but starting at `A`: a face that cannot draw the
        /// question mark it would be replaced by. Exotic, and the only way to
        /// exercise the second half of the substitution rule.
        SansInterro,
        Aucune,
    }

    /// A font file written byte by byte, the way `meta.rs` writes its EXIF and
    /// `build.rs` its JPEG. A font is a directory of big-endian tables at
    /// known offsets; a test that needs a face with one exact property is
    /// better served by this than by an asset nobody can read.
    struct Fonte {
        tag: [u8; 4],
        upem: u16,
        fs_type: u16,
        famille: String,
        style: String,
        postscript: Option<String>,
        /// Which `name` platforms carry the strings: 3 Windows, 1 Macintosh.
        plateformes: &'static [u16],
        cmap: Cmap,
        /// The outline table declared, if any. `None` is a bitmap face.
        contours: Option<[u8; 4]>,
        variable: bool,
        /// An Apple bitmap face: `bdat` and `bloc`, no outlines, and no
        /// `head` either — those faces carry a `bhed` instead, which is why
        /// they read as rubble unless the refusal is decided on the outlines.
        bitmap: bool,
        /// A second `name` record for family, in another language, written
        /// *before* the English one. Every stock macOS face carries six.
        autre_langue: Option<(u16, String)>,
        /// Filler declared under the outline tag: the bytes the walk must
        /// never read, and the only reason a real font file is large.
        gras: usize,
        /// Tables an extraction must leave behind, by tag and by weight:
        /// layout, strikes, signature. Every one of them is real, and on a
        /// stock machine they are most of what a font file weighs.
        bagage: Vec<([u8; 4], usize)>,
    }

    const NUM_GLYPHS: u16 = 256;
    const CHASSE: u16 = 1024;

    impl Fonte {
        fn neuve() -> Self {
            Fonte {
                tag: *b"\x00\x01\x00\x00",
                // Deliberately not 1000: every measure below has to travel
                // through the em conversion, and a conversion that vanished
                // would go unnoticed against a 1000-unit em.
                upem: 2048,
                fs_type: 0,
                famille: "Colophon Test".into(),
                style: "Regular".into(),
                postscript: Some("ColophonTest-Regular".into()),
                plateformes: &[3],
                cmap: Cmap::Format4,
                contours: Some(*b"glyf"),
                variable: false,
                bitmap: false,
                autre_langue: None,
                gras: 64,
                bagage: Vec::new(),
            }
        }

        /// `loca`, which a `glyf` face cannot be read without: `numGlyphs + 1`
        /// offsets in the short format `head` declares, the last one closing
        /// on the end of the outlines.
        fn loca(&self) -> Vec<u8> {
            let mut v = vec![0u8; usize::from(NUM_GLYPHS) * 2];
            v.extend_from_slice(&((self.gras / 2) as u16).to_be_bytes());
            v
        }

        fn head(&self) -> Vec<u8> {
            let mut v = vec![0u8; 54];
            v[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
            v[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
            v[18..20].copy_from_slice(&self.upem.to_be_bytes());
            v[36..38].copy_from_slice(&(-100i16).to_be_bytes());
            v[38..40].copy_from_slice(&(-500i16).to_be_bytes());
            v[40..42].copy_from_slice(&2000i16.to_be_bytes());
            v[42..44].copy_from_slice(&1800i16.to_be_bytes());
            v
        }

        /// hhea's ascent and descent differ from OS/2's on purpose: a reader
        /// that stopped preferring OS/2 would show it in the numbers.
        fn hhea(&self) -> Vec<u8> {
            let mut v = vec![0u8; 36];
            v[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
            v[4..6].copy_from_slice(&1500i16.to_be_bytes());
            v[6..8].copy_from_slice(&(-300i16).to_be_bytes());
            v[34..36].copy_from_slice(&NUM_GLYPHS.to_be_bytes());
            v
        }

        fn hmtx(&self) -> Vec<u8> {
            let mut v = Vec::new();
            for _ in 0..NUM_GLYPHS {
                v.extend_from_slice(&CHASSE.to_be_bytes());
                v.extend_from_slice(&0i16.to_be_bytes());
            }
            v
        }

        fn maxp(&self) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(&0x0000_5000u32.to_be_bytes());
            v.extend_from_slice(&NUM_GLYPHS.to_be_bytes());
            v
        }

        fn os2(&self) -> Vec<u8> {
            let mut v = vec![0u8; 96];
            v[0..2].copy_from_slice(&2u16.to_be_bytes());
            v[8..10].copy_from_slice(&self.fs_type.to_be_bytes());
            v[68..70].copy_from_slice(&1600i16.to_be_bytes());
            v[70..72].copy_from_slice(&(-400i16).to_be_bytes());
            v[88..90].copy_from_slice(&1400i16.to_be_bytes());
            v
        }

        fn post(&self) -> Vec<u8> {
            let mut v = vec![0u8; 32];
            v[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
            v
        }

        fn name(&self) -> Vec<u8> {
            // (plateforme, langue, nameID, octets)
            let mut entrees: Vec<(u16, u16, u16, Vec<u8>)> = Vec::new();
            for &p in self.plateformes {
                let encode = |s: &str| -> Vec<u8> {
                    if p == 1 {
                        s.chars()
                            .map(|c| {
                                if (c as u32) < 0x80 {
                                    c as u8
                                } else {
                                    MACROMAN_HAUT
                                        .chars()
                                        .position(|m| m == c)
                                        .map(|i| (i + 0x80) as u8)
                                        .unwrap_or(b'?')
                                }
                            })
                            .collect()
                    } else {
                        s.encode_utf16().flat_map(u16::to_be_bytes).collect()
                    }
                };
                let anglais = if p == 1 { 0 } else { 0x0409 };
                if let Some((langue, autre)) = &self.autre_langue {
                    entrees.push((p, *langue, 1, encode(autre)));
                }
                entrees.push((p, anglais, 1, encode(&self.famille)));
                entrees.push((p, anglais, 2, encode(&self.style)));
                if let Some(ps) = &self.postscript {
                    entrees.push((p, anglais, 6, encode(ps)));
                }
            }
            let debut = 6 + entrees.len() * 12;
            let (mut records, mut storage) = (Vec::new(), Vec::new());
            for (p, langue, id, bytes) in &entrees {
                let off = storage.len();
                storage.extend_from_slice(bytes);
                records.extend_from_slice(&p.to_be_bytes());
                records.extend_from_slice(&(if *p == 1 { 0u16 } else { 1 }).to_be_bytes());
                records.extend_from_slice(&langue.to_be_bytes());
                records.extend_from_slice(&id.to_be_bytes());
                records.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
                records.extend_from_slice(&(off as u16).to_be_bytes());
            }
            let mut v = Vec::new();
            v.extend_from_slice(&0u16.to_be_bytes());
            v.extend_from_slice(&(entrees.len() as u16).to_be_bytes());
            v.extend_from_slice(&(debut as u16).to_be_bytes());
            v.extend_from_slice(&records);
            v.extend_from_slice(&storage);
            v
        }

        fn cmap(&self) -> Option<Vec<u8>> {
            let (plateforme, encodage, sub) = match self.cmap {
                Cmap::Aucune => return None,
                Cmap::Format4 => (3u16, 1u16, sous_table_4()),
                Cmap::Format12 => (3, 10, sous_table_12()),
                Cmap::Symbole => (3, 0, sous_table_4()),
                Cmap::Format0 => (3, 1, sous_table_0()),
                Cmap::SansInterro => (3, 1, sous_table_4_depuis(0x0041)),
            };
            let mut v = Vec::new();
            v.extend_from_slice(&0u16.to_be_bytes());
            v.extend_from_slice(&1u16.to_be_bytes());
            v.extend_from_slice(&plateforme.to_be_bytes());
            v.extend_from_slice(&encodage.to_be_bytes());
            v.extend_from_slice(&12u32.to_be_bytes());
            v.extend_from_slice(&sub);
            Some(v)
        }

        /// Every table, in tag order the way a real file lays them out.
        fn tables(&self) -> Vec<([u8; 4], Vec<u8>)> {
            let mut t: Vec<([u8; 4], Vec<u8>)> =
                vec![(*b"name", self.name()), (*b"OS/2", self.os2()), (*b"post", self.post())];
            if self.bitmap {
                // Ni head ni hhea : la variante bitmap range les siennes dans
                // un `bhed`, que rien ici ne lit.
                t.push((*b"bhed", self.head()));
                t.push((*b"bdat", vec![0x2a; self.gras]));
                t.push((*b"bloc", vec![0u8; 16]));
            } else {
                t.push((*b"head", self.head()));
                t.push((*b"hhea", self.hhea()));
                t.push((*b"hmtx", self.hmtx()));
                t.push((*b"maxp", self.maxp()));
            }
            if let Some(c) = self.cmap() {
                t.push((*b"cmap", c));
            }
            if let Some(tag) = self.contours.filter(|_| !self.bitmap) {
                t.push((tag, vec![0x2a; self.gras]));
                if &tag == b"glyf" {
                    t.push((*b"loca", self.loca()));
                }
            }
            if self.variable {
                t.push((*b"fvar", vec![0u8; 16]));
            }
            for (tag, poids) in &self.bagage {
                t.push((*tag, vec![0x5a; *poids]));
            }
            t.sort_by_key(|(tag, _)| *tag);
            t
        }
    }

    /// cmap format 4, two segments: 0x20..0xFF onto glyphs 1.., then the
    /// mandatory 0xFFFF terminator.
    fn sous_table_4() -> Vec<u8> {
        sous_table_4_depuis(0x0020)
    }

    /// A segmented table covering `debut`..=0x00FF, first character on glyph
    /// one. Where it starts is a parameter because a face that does not carry
    /// the question mark is the only way to see what happens when the
    /// substitute itself is missing.
    fn sous_table_4_depuis(debut: u16) -> Vec<u8> {
        let mut v = Vec::new();
        for n in [4u16, 32, 0, 4, 4, 1, 0] {
            v.extend_from_slice(&n.to_be_bytes());
        }
        v.extend_from_slice(&0x00FFu16.to_be_bytes()); // endCode
        v.extend_from_slice(&0xFFFFu16.to_be_bytes());
        v.extend_from_slice(&0u16.to_be_bytes()); // reservedPad
        v.extend_from_slice(&debut.to_be_bytes()); // startCode
        v.extend_from_slice(&0xFFFFu16.to_be_bytes());
        v.extend_from_slice(&(1i32 - i32::from(debut)) .to_be_bytes()[2..]); // idDelta : debut → 1
        v.extend_from_slice(&1i16.to_be_bytes()); // 0xFFFF → 0
        v.extend_from_slice(&0u16.to_be_bytes()); // idRangeOffset
        v.extend_from_slice(&0u16.to_be_bytes());
        v
    }

    /// cmap format 12, one group over the same range.
    fn sous_table_12() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&12u16.to_be_bytes());
        v.extend_from_slice(&0u16.to_be_bytes());
        for n in [28u32, 0, 1, 0x20, 0xFF, 1] {
            v.extend_from_slice(&n.to_be_bytes());
        }
        v
    }

    /// cmap format 0: a byte map. Legal, Unicode-declared, and a format this
    /// reader does not walk — which is a refusal, not a face without glyphs.
    fn sous_table_0() -> Vec<u8> {
        let mut v = vec![0u8; 262];
        v[2..4].copy_from_slice(&262u16.to_be_bytes());
        for (i, slot) in v[6..262].iter_mut().enumerate() {
            *slot = i as u8;
        }
        v
    }

    /// Lay faces out as one file: a lone face at offset zero, or a collection
    /// whose per-face directories sit behind a `ttcf` header — **and whose
    /// table offsets stay absolute in the file**, which is the trap this
    /// fixture exists to spring.
    fn fichier(faces: &[Fonte]) -> Vec<u8> {
        let jeux: Vec<Vec<([u8; 4], Vec<u8>)>> = faces.iter().map(Fonte::tables).collect();
        let ttc = faces.len() > 1;
        let mut curseur = if ttc { 12 + faces.len() * 4 } else { 0 };
        let mut dirs = Vec::new();
        for tables in &jeux {
            dirs.push(curseur);
            curseur += 12 + tables.len() * 16;
        }
        let mut out = vec![0u8; curseur];
        if ttc {
            out[0..4].copy_from_slice(b"ttcf");
            out[4..8].copy_from_slice(&0x0002_0000u32.to_be_bytes());
            out[8..12].copy_from_slice(&(faces.len() as u32).to_be_bytes());
            for (i, d) in dirs.iter().enumerate() {
                out[12 + i * 4..16 + i * 4].copy_from_slice(&(*d as u32).to_be_bytes());
            }
        }
        for (fi, tables) in jeux.iter().enumerate() {
            let dir = dirs[fi];
            out[dir..dir + 4].copy_from_slice(&faces[fi].tag);
            out[dir + 4..dir + 6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
            for (ti, (tag, data)) in tables.iter().enumerate() {
                while out.len() % 4 != 0 {
                    out.push(0);
                }
                let off = out.len();
                out.extend_from_slice(data);
                let rec = dir + 12 + ti * 16;
                out[rec..rec + 4].copy_from_slice(tag);
                // The checksum is never written and never read.
                out[rec + 8..rec + 12].copy_from_slice(&(off as u32).to_be_bytes());
                out[rec + 12..rec + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
            }
        }
        out
    }

    /// The directory record of one table, so a test can corrupt exactly it.
    fn record(data: &[u8], dir: usize, tag: &[u8; 4]) -> usize {
        let count = usize::from(u16b(data, dir + 4).unwrap());
        (0..count)
            .map(|i| dir + 12 + i * 16)
            .find(|r| &data[*r..*r + 4] == tag)
            .expect("table présente")
    }

    fn dossier(nom: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("colophon-font-{nom}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // --- Ce que le lecteur doit savoir faire ------------------------------

    /// A face written here, read back whole: the names off both name ids, the
    /// metrics converted out of a 2048-unit em, the kind off the tables.
    #[test]
    fn une_ttf_synthetique_se_lit_en_entier() {
        let data = fichier(&[Fonte::neuve()]);
        let f = Face::parse(&data, 0).expect("face lisible");
        assert_eq!(f.postscript, "ColophonTest-Regular");
        assert_eq!(f.nom, "Colophon Test Regular");
        assert_eq!(f.famille, "Colophon Test", "la famille seule, pour le sélecteur");

        assert_eq!(f.genre, Some(Genre::Glyf));
        assert!(!f.variable);
        assert!(f.embeddable());
        assert!(!f.sous_ensemblage_interdit);

        let m = f.metrics.expect("métriques");
        // 1024 sur un em de 2048, soit la moitié de l'em de mille.
        assert_eq!(chasse(&data, 0), 500);
        assert_eq!(m.bbox, [-49, -244, 977, 879]);
        // OS/2 l'emporte sur hhea, qui dirait 1500 et -300.
        assert_eq!((m.ascent, m.descent, m.cap_height), (781, -195, 684));

        // Un offset de table pointé n'importe où rend une face illisible,
        // jamais un panic.
        let r = record(&data, 0, b"head");
        let mut casse = data.clone();
        casse[r + 8..r + 12].copy_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
        assert_eq!(Face::parse(&casse, 0).err(), Some(REFUS_ILLISIBLE));

        // Et une longueur qui déborde du fichier est refusée plutôt que
        // rognée : une lecture courte rendrait des mesures inventées là où
        // le fichier ne dit rien. C'est le mordant de la lecture bornée —
        // rogner au lieu de refuser fait tomber cette ligne.
        let r = record(&data, 0, b"hmtx");
        let mut deborde = data.clone();
        deborde[r + 12..r + 16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(table(&deborde, 0, b"hmtx").is_none());
        assert_eq!(Face::parse(&deborde, 0).err(), Some(REFUS_ILLISIBLE));
    }

    /// A collection carries two faces, each with its own name and its own
    /// numbers — and its tables addressed from the start of the file. A
    /// reader that took those offsets as relative to the directory would read
    /// twenty bytes beside every table and name neither face.
    #[test]
    fn une_collection_rend_ses_deux_faces() {
        let mut une = Fonte::neuve();
        une.famille = "Colophon Une".into();
        une.postscript = Some("ColophonUne-Regular".into());
        let mut deux = Fonte::neuve();
        deux.famille = "Colophon Deux".into();
        deux.style = "Italic".into();
        deux.postscript = Some("ColophonDeux-Italic".into());
        // Un em différent : lire la mauvaise face se verrait au chiffre.
        deux.upem = 1000;

        let data = fichier(&[une, deux]);
        assert_eq!(face_count(&data), 2);
        assert!(sfnt_dir(&data, 0).unwrap() > 0, "le répertoire suit l'en-tête ttcf");

        let f0 = Face::parse(&data, 0).expect("face 0");
        let f1 = Face::parse(&data, 1).expect("face 1");
        assert_eq!((f0.index, f1.index), (0, 1));
        assert_eq!(f0.nom, "Colophon Une Regular");
        assert_eq!(f1.nom, "Colophon Deux Italic");
        assert_eq!(f0.postscript, "ColophonUne-Regular");
        assert_eq!(f1.postscript, "ColophonDeux-Italic");
        assert!(f0.metrics.is_some() && f1.metrics.is_some());
        assert_eq!((chasse(&data, 0), chasse(&data, 1)), (500, 1024));
        assert_eq!(Face::parse(&data, 2).err(), Some(REFUS_ILLISIBLE), "il n'y a pas de face 2");
    }

    /// The four verdicts of the arbitration, one fixture per bit, plus the
    /// two character maps and the two files that name no face at all. A
    /// refused face still names itself: the screen has to say which one.
    #[test]
    fn les_verdicts_d_embarquement_sortent_leurs_codes() {
        let verdict = |fs_type: u16| {
            let mut f = Fonte::neuve();
            f.fs_type = fs_type;
            Face::parse(&fichier(&[f]), 0).expect("face lisible")
        };
        // 0x0002 : le vendeur interdit l'incorporation, tout court.
        assert_eq!(verdict(0x0002).refus, Some(REFUS_EMBARQUEMENT_INTERDIT));
        // 0x0200 : rien à embarquer que des bitmaps.
        assert_eq!(verdict(0x0200).refus, Some(REFUS_BITMAP_SEULEMENT));
        // 0x0004 : Preview & Print, ce qu'est exactement le PDF d'un album.
        assert!(verdict(0x0004).embeddable());
        assert!(!verdict(0x0004).sous_ensemblage_interdit);
        // 0x0100 : pas de sous-ensemblage. Accepté, et retenu — le moteur
        // embarque les faces entières de toute façon.
        let entiere = verdict(0x0100);
        assert!(entiere.embeddable());
        assert!(entiere.sous_ensemblage_interdit);

        // Une table symbole (3, 0) ne mesure rien, et la face se nomme quand
        // même : c'est tout l'intérêt d'un refus qui n'est pas une absence.
        let mut symbole = Fonte::neuve();
        symbole.cmap = Cmap::Symbole;
        let s = Face::parse(&fichier(&[symbole]), 0).unwrap();
        assert_eq!(s.refus, Some(REFUS_CMAP_ILLISIBLE));
        assert!(s.metrics.is_none());
        assert_eq!(s.nom, "Colophon Test Regular");

        // Un format que le lecteur ne parcourt pas, et pas de cmap du tout.
        for cmap in [Cmap::Format0, Cmap::Aucune] {
            let mut f = Fonte::neuve();
            f.cmap = cmap;
            let lue = Face::parse(&fichier(&[f]), 0).unwrap();
            assert_eq!(lue.refus, Some(REFUS_CMAP_ILLISIBLE));
        }

        // Pas de contours du tout : l'emoji couleur et les polices bitmap
        // sortent par là, en refus propre plutôt qu'en face sans dessin.
        let mut bitmap = Fonte::neuve();
        bitmap.contours = None;
        let bm = Face::parse(&fichier(&[bitmap]), 0).unwrap();
        assert_eq!(bm.refus, Some(REFUS_BITMAP_SEULEMENT));
        assert_eq!(bm.genre, None);

        // Une police bitmap d'Apple : `bdat` et `bloc`, pas de `head` du
        // tout. Sans contours, le refus se décide avant les tables communes,
        // sinon la face sortirait en `illisible` — vrai, mais moins utile,
        // et sans son nom.
        let mut apple = Fonte::neuve();
        apple.bitmap = true;
        let a = Face::parse(&fichier(&[apple]), 0).unwrap();
        assert_eq!(a.refus, Some(REFUS_BITMAP_SEULEMENT));
        assert_eq!(a.nom, "Colophon Test Regular");
        assert!(a.metrics.is_none());

        // Un fichier tronqué, un fichier qui n'est pas une police.
        let entier = fichier(&[Fonte::neuve()]);
        assert_eq!(Face::parse(&entier[..40], 0).err(), Some(REFUS_ILLISIBLE));
        assert_eq!(Face::parse(b"pas une police", 0).err(), Some(REFUS_ILLISIBLE));
        assert_eq!(Face::parse(&[], 0).err(), Some(REFUS_ILLISIBLE));

        // Mordant : inverser le test du bit 0x0002 dans `verdict_fs_type`
        // fait tomber la première ligne et l'avant-dernier bloc. (Vérifié.)
        assert!(verdict(0x0000).embeddable());
    }

    /// CFF outlines read through the same shared tables, and an `fvar` marks
    /// the face variable without exposing a single axis: the default instance
    /// is what those tables already describe.
    #[test]
    fn otto_se_lit_et_fvar_marque_la_variable() {
        let mut otto = Fonte::neuve();
        otto.tag = *b"OTTO";
        otto.contours = Some(*b"CFF ");
        let f = Face::parse(&fichier(&[otto]), 0).expect("OTTO lisible");
        assert_eq!(f.genre, Some(Genre::Cff));
        assert_eq!(f.genre.unwrap().code(), "cff");
        assert!(f.embeddable());
        assert!(f.metrics.is_some(), "métriques par les tables communes");

        let mut var = Fonte::neuve();
        var.variable = true;
        var.cmap = Cmap::Format12;
        let v = Face::parse(&fichier(&[var]), 0).expect("variable lisible");
        assert!(v.variable);
        assert!(v.embeddable());
        assert!(v.metrics.is_some(), "l'instance par défaut se mesure");
    }

    /// The PostScript name lives on the Windows platform, or the Macintosh
    /// one, or both. Reading one alone leaves a face nameless; reading the
    /// Windows one as bytes leaves a row of NULs.
    #[test]
    fn le_nom_se_lit_des_deux_plateformes() {
        assert_eq!(MACROMAN_HAUT.chars().count(), 128, "la moitié haute de MacRoman");

        let mut windows = Fonte::neuve();
        windows.plateformes = &[3];
        let f = Face::parse(&fichier(&[windows]), 0).unwrap();
        assert_eq!(f.postscript, "ColophonTest-Regular");
        assert!(!f.nom.contains('\0'), "UTF-16BE décodé, pas recopié : {:?}", f.nom);

        // Macintosh seul, MacRoman, accents compris.
        let mut mac = Fonte::neuve();
        mac.plateformes = &[1];
        mac.famille = "Colophon Été".into();
        let f = Face::parse(&fichier(&[mac]), 0).unwrap();
        assert_eq!(f.nom, "Colophon Été Regular");
        assert_eq!(f.postscript, "ColophonTest-Regular");

        // Six langues dans le `name`, l'anglais listé après : le nom rendu
        // est l'anglais, pas le premier venu. Sans la règle, la face
        // s'appellerait « Colophon 標準體 », ce que rend vraiment Times.ttc.
        let mut polyglotte = Fonte::neuve();
        polyglotte.autre_langue = Some((0x0404, "Colophon 標準體".into()));
        let f = Face::parse(&fichier(&[polyglotte]), 0).unwrap();
        assert_eq!(f.nom, "Colophon Test Regular");

        // Les deux : Windows préféré, et rien de perdu.
        let mut deux = Fonte::neuve();
        deux.plateformes = &[3, 1];
        let f = Face::parse(&fichier(&[deux]), 0).unwrap();
        assert_eq!(f.nom, "Colophon Test Regular");

        // Sans aucun nom lisible, il n'y a pas de face à montrer.
        let mut muette = Fonte::neuve();
        muette.plateformes = &[];
        assert_eq!(Face::parse(&fichier(&[muette]), 0).err(), Some(REFUS_ILLISIBLE));
    }

    // --- Ce que l'écrivain doit savoir faire -------------------------------

    /// The tags of a font file's directory, in the order it lists them.
    fn tags(data: &[u8]) -> Vec<String> {
        let count = usize::from(u16b(data, 4).unwrap());
        (0..count)
            .map(|i| String::from_utf8_lossy(&data[12 + i * 16..16 + i * 16]).into_owned())
            .collect()
    }

    /// Every field of the metrics, so an extraction that loses one is seen
    /// here rather than at a printer.
    fn identiques(a: &Metrics, b: &Metrics) {
        assert_eq!(
            (a.bbox, a.ascent, a.descent, a.cap_height, a.italic_angle, a.fs_type),
            (b.bbox, b.ascent, b.descent, b.cap_height, b.italic_angle, b.fs_type)
        );
    }

    /// The round trip: a face written here, extracted, and read back. Same
    /// name, same numbers, one face in the file. Nothing else proves an
    /// extraction, since nothing else is what a PDF will ask of it.
    #[test]
    fn une_face_extraite_se_relit_a_l_identique() {
        let data = fichier(&[Fonte::neuve()]);
        let avant = Face::parse(&data, 0).expect("face lisible");
        let sortie = Face::extraire(&data, 0).expect("extraction");
        let apres = Face::parse(&sortie, 0).expect("la face extraite se relit");

        assert_eq!(face_count(&sortie), 1, "un fichier, une face");
        assert_eq!(&sortie[0..4], b"\x00\x01\x00\x00", "la signature de la face");
        assert_eq!((apres.nom, apres.postscript), (avant.nom, avant.postscript));
        assert_eq!(apres.genre, Some(Genre::Glyf));
        identiques(&avant.metrics.unwrap(), &apres.metrics.unwrap());
        // Les chasses, glyphe par glyphe et non plus code par code : c'est
        // ce que `/W` portera, donc c'est ce qui doit survivre au voyage.
        let ouverte = |d: &[u8]| Embarquee::depuis(d.to_vec(), 0).expect("face ouverte");
        assert_eq!(ouverte(&data).avances, ouverte(&sortie).avances, "les chasses");

        // Mordant : la liste fermée, tag par tag. Oublier une table la fait
        // tomber ici — y compris `loca`, que pas une métrique ne consulte et
        // dont l'absence décalerait tous les glyphes d'un cran.
        assert_eq!(
            tags(&sortie),
            ["OS/2", "cmap", "glyf", "head", "hhea", "hmtx", "loca", "maxp", "name", "post"],
            "trié par tag, et rien d'autre"
        );
    }

    /// A face of a collection comes out alone, with its own numbers. Seven
    /// faces in ten on a stock macOS live like that, and a `FontFile2`
    /// carries one face: this is what the whole wave rests on.
    #[test]
    fn une_face_de_collection_sort_seule() {
        let mut une = Fonte::neuve();
        une.famille = "Colophon Une".into();
        let mut deux = Fonte::neuve();
        deux.famille = "Colophon Deux".into();
        deux.style = "Italic".into();
        deux.postscript = Some("ColophonDeux-Italic".into());
        // Un em différent : lire la mauvaise face se verrait au chiffre.
        deux.upem = 1000;

        let data = fichier(&[une, deux]);
        let sortie = Face::extraire(&data, 1).expect("la face 1 sort de sa collection");
        assert_ne!(&sortie[0..4], b"ttcf", "un sfnt, plus une collection");
        assert_eq!(face_count(&sortie), 1);

        let f = Face::parse(&sortie, 0).expect("relue");
        assert_eq!(f.nom, "Colophon Deux Italic");
        assert_eq!(f.postscript, "ColophonDeux-Italic");
        // Mordant : ses métriques à elle. Extraire la face 0 rendrait 500.
        assert!(f.metrics.is_some());
        assert_eq!(chasse(&sortie, 0), 1024);
        assert_eq!(
            Face::parse(&sortie, 1).err(),
            Some(REFUS_ILLISIBLE),
            "il n'y a plus de face 1 dans ce fichier"
        );
    }

    /// The bundled face makes the round trip. The reader's integration test
    /// was this face; the writer's is the same one, on a real font file
    /// nobody wrote for a test.
    #[test]
    fn la_face_embarquee_fait_l_aller_retour() {
        let sortie = Face::extraire(FONT_DATA, 0).expect("Source Sans 3 s'extrait");
        let f = Face::parse(&sortie, 0).expect("relue");
        assert_eq!(f.postscript, FONT_NAME, "le /BaseFont du PDF");
        assert_eq!(f.genre, Some(Genre::Glyf));
        assert_eq!(face_count(&sortie), 1);
        identiques(&metrics().unwrap(), &f.metrics.as_ref().unwrap());
        // Une face déjà seule dans son fichier rétrécit quand même : ce qui
        // part, c'est la composition, pas un glyphe.
        assert!(sortie.len() < FONT_DATA.len(), "{} contre {}", sortie.len(), FONT_DATA.len());
    }

    // --- La face de l'album ----------------------------------------------

    /// The three answers of the resolution, on a real album folder.
    ///
    /// The face beside `album.json` is read and measured; an album that
    /// named none gets the project's without touching the disk; and an album
    /// whose file has been deleted by hand still opens, in the project's
    /// face, **saying so**. The last one is the whole point: an export that
    /// died there would cost somebody their evening, and one that printed
    /// quietly in another face would cost them a print run.
    #[test]
    fn la_face_de_lalbum_se_lit_a_cote_de_lui_ou_se_dit_absente() {
        let dir = dossier("album");
        let (nom, octets) =
            extraire_pour_album(&fichier(&[Fonte::neuve()]), 0).expect("extraction");
        assert_eq!(nom, POLICE_TTF, "des contours quadratiques vont en .ttf");
        fs::write(dir.join(nom), &octets).unwrap();

        // 1. La face choisie, lue depuis le dossier de l'album.
        let choisie = face_album(&dir, Some(nom));
        assert!(choisie.defaut.is_none());
        assert_eq!(choisie.face.postscript(), "ColophonTest-Regular");
        // La fixture donne 500 à chaque glyphe sur un em de mille : deux
        // caractères font un em, soit la taille en points, en millimètres.
        assert!((choisie.face.largeur_mm("AA", 72.0) - 25.4).abs() < 1e-9);

        // 2. Aucune police choisie : celle du projet, et rien n'est lu.
        let sans = face_album(&dir, None);
        assert!(sans.defaut.is_none());
        assert_eq!(sans.face.postscript(), FONT_NAME);

        // 3. Le fichier a disparu : l'album sort quand même, et le dit.
        fs::remove_file(dir.join(nom)).unwrap();
        let perdue = face_album(&dir, Some(nom));
        assert_eq!(perdue.defaut, Some(REFUS_FICHIER_ABSENT));
        assert_eq!(perdue.face.postscript(), FONT_NAME);
        let _ = fs::remove_dir_all(&dir);
    }

    /// `album.json` is hand-repairable, so its file name is data like any
    /// other: only the two names this crate writes are ever joined to the
    /// album's folder. Anything else is a missing file, never a path.
    #[test]
    fn le_nom_de_fichier_ne_peut_jamais_etre_un_chemin() {
        let dir = dossier("chemin");
        let (_, octets) = extraire_pour_album(FONT_DATA, 0).expect("extraction");
        fs::write(dir.join(POLICE_TTF), &octets).unwrap();
        // Le fichier existe, sous son vrai nom : seul le nom écrit dans
        // album.json change, et c'est lui qu'on refuse.
        for menteur in [
            "../police.ttf",
            "/etc/passwd",
            "..\\police.ttf",
            "police.ttf.tmp",
            "SourceSans3-Regular.ttf",
            "",
        ] {
            let r = face_album(&dir, Some(menteur));
            assert_eq!(r.defaut, Some(REFUS_FICHIER_ABSENT), "{menteur:?}");
            assert_eq!(r.face.postscript(), FONT_NAME, "{menteur:?}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// A face refused at reading is refused at extraction too, so nothing
    /// unusable is ever copied beside an album. The screen names the same
    /// reason whichever side it asked from.
    #[test]
    fn une_face_refusee_ne_se_pose_pas_a_cote_dun_album() {
        let mut interdite = Fonte::neuve();
        interdite.fs_type = 0x0002;
        assert_eq!(
            extraire_pour_album(&fichier(&[interdite]), 0).err(),
            Some(REFUS_EMBARQUEMENT_INTERDIT)
        );
    }

    /// CFF outlines go in under the other of the two names, because that is
    /// what a reader expects of each, and the album records which.
    #[test]
    fn une_face_cff_sappelle_autrement() {
        let mut cff = Fonte::neuve();
        cff.tag = *b"OTTO";
        cff.contours = Some(*b"CFF ");
        let data = fichier(&[cff]);
        assert_eq!(Face::parse(&data, 0).unwrap().genre, Some(Genre::Cff));
        assert_eq!(extraire_pour_album(&data, 0).expect("extraction").0, POLICE_OTF);
    }

    /// The closed list, from the other side: layout, strikes, signature and
    /// axes are named, and none of them is in the file that comes out.
    #[test]
    fn les_tables_ecartees_le_sont_vraiment() {
        let mut lourde = Fonte::neuve();
        lourde.bagage = vec![
            (*b"GSUB", 4096),
            (*b"GPOS", 4096),
            (*b"DSIG", 2048),
            (*b"sbix", 8192),
            (*b"kern", 512),
            (*b"morx", 512),
        ];
        let data = fichier(&[lourde]);
        let sortie = Face::extraire(&data, 0).expect("extraction");
        for tag in ["GSUB", "GPOS", "DSIG", "sbix", "kern", "morx"] {
            assert!(!tags(&sortie).contains(&tag.to_string()), "{tag} a suivi la face");
        }
        // Et le fichier a maigri d'autant : c'est le gain que le report du
        // sous-ensemblage met en banque.
        assert!(
            data.len() - sortie.len() > 19_000,
            "{} octets de moins seulement",
            data.len() - sortie.len()
        );
        // Une strike n'a pas fait passer la face pour une bitmap au passage.
        assert_eq!(Face::parse(&sortie, 0).unwrap().genre, Some(Genre::Glyf));

        // Arbitrage 6 : une face variable s'extrait à son instance par
        // défaut. `fvar` ne part pas, donc la face qui sort est statique —
        // et ses mesures sont celles que le lecteur rendait déjà.
        let mut var = Fonte::neuve();
        var.variable = true;
        let variable = fichier(&[var]);
        let sortie = Face::extraire(&variable, 0).expect("extraction");
        let f = Face::parse(&sortie, 0).expect("relue");
        assert!(!f.variable, "les axes ne suivent pas");
        identiques(
            &Face::parse(&variable, 0).unwrap().metrics.unwrap(),
            &f.metrics.unwrap(),
        );
    }

    /// What comes out is a font file, not a file our own reader happens to
    /// accept: checksums recomputed here and compared, the file-wide
    /// adjustment redone by hand, directory sorted, tables aligned.
    #[test]
    fn le_fichier_emis_est_un_vrai_fichier_de_police() {
        let data = fichier(&[Fonte::neuve()]);
        let out = Face::extraire(&data, 0).expect("extraction");

        let n = usize::from(u16b(&out, 4).unwrap());
        let mut ordonnes = tags(&out);
        ordonnes.sort();
        assert_eq!(tags(&out), ordonnes, "le répertoire est trié par tag");
        // Les hints de recherche binaire : la puissance de deux qui tient.
        let selector = usize::from(u16b(&out, 8).unwrap());
        assert!(1 << selector <= n && 1 << (selector + 1) > n, "{n} tables, sélecteur {selector}");
        assert_eq!(u16b(&out, 6).unwrap(), (16 << selector) as u16);
        assert_eq!(u16b(&out, 10).unwrap(), (n * 16) as u16 - u16b(&out, 6).unwrap());

        assert_eq!(out.len() % 4, 0, "le fichier entier est aligné");
        // Toutes les sommes se prennent sur le fichier dont
        // `head.checkSumAdjustment` est mis à zéro, celle de `head`
        // comprise : le champ ne peut pas entrer dans un calcul dont il est
        // le résultat. Vérifier la somme de `head` sur le fichier fini est
        // le mordant de cette phrase — il est tombé ici avant d'être écrit.
        let head = table_range(&out, 0, b"head").unwrap().0;
        let mut zero = out.clone();
        zero[head + 8..head + 12].copy_from_slice(&0u32.to_be_bytes());
        for i in 0..n {
            let rec = 12 + i * 16;
            let off = u32b(&out, rec + 8).unwrap() as usize;
            let len = u32b(&out, rec + 12).unwrap() as usize;
            assert_eq!(off % 4, 0, "{} n'est pas alignée", tags(&out)[i]);
            assert!(off + len <= out.len(), "{} déborde", tags(&out)[i]);
            assert_eq!(
                u32b(&out, rec + 4).unwrap(),
                somme_sfnt(&zero[off..off + len.next_multiple_of(4)]),
                "somme de contrôle de {}",
                tags(&out)[i]
            );
        }

        // head.checkSumAdjustment : sur le fichier entier, ce champ mis à
        // zéro, une fois tout le reste en place. La calculer avant, ou
        // l'oublier, donne un fichier que nos tests acceptent et qu'un
        // validateur refuse — donc on la refait ici, à la main.
        assert_eq!(
            u32b(&out, head + 8).unwrap(),
            0xB1B0_AFBAu32.wrapping_sub(somme_sfnt(&zero)),
            "l'ajustement du fichier entier"
        );
        assert_ne!(u32b(&out, head + 8).unwrap(), 0, "elle est calculée, pas laissée vide");
    }

    /// CFF2 is refused when it is read, not when it is picked — and the
    /// extraction says the same word. Plain CFF, next to it, comes out fine
    /// and comes out `OTTO`.
    #[test]
    fn le_cff2_se_refuse_a_la_lecture_comme_a_l_extraction() {
        let mut cff2 = Fonte::neuve();
        cff2.tag = *b"OTTO";
        cff2.contours = Some(*b"CFF2");
        cff2.variable = true;
        let data = fichier(&[cff2]);
        let f = Face::parse(&data, 0).expect("elle se lit, et se nomme");
        assert_eq!(f.genre, Some(Genre::Cff2));
        assert_eq!(f.genre.unwrap().code(), "cff2");
        assert_eq!(f.refus, Some(REFUS_FORMAT_NON_EMBARQUABLE));
        assert!(!f.embeddable());
        assert_eq!(f.nom, "Colophon Test Regular", "refusée, et nommée quand même");
        assert_eq!(Face::extraire(&data, 0).err(), Some(REFUS_FORMAT_NON_EMBARQUABLE));

        let mut cff = Fonte::neuve();
        cff.tag = *b"OTTO";
        cff.contours = Some(*b"CFF ");
        let data = fichier(&[cff]);
        let sortie = Face::extraire(&data, 0).expect("un OTTO s'extrait");
        assert_eq!(&sortie[0..4], b"OTTO", "la signature de la face, jamais la nôtre");
        assert!(tags(&sortie).contains(&"CFF ".to_string()));
        assert!(!tags(&sortie).contains(&"loca".to_string()), "un CFF n'a pas de loca");
        assert_eq!(Face::parse(&sortie, 0).unwrap().genre, Some(Genre::Cff));

        // Et un refus de licence se dit pareil des deux côtés.
        let mut interdite = Fonte::neuve();
        interdite.fs_type = 0x0002;
        let data = fichier(&[interdite]);
        assert_eq!(Face::extraire(&data, 0).err(), Some(REFUS_EMBARQUEMENT_INTERDIT));
    }

    /// The silent trap, made loud. The walk's buffer holds zeros where the
    /// outlines are — they were never read — so extracting from it emits a
    /// font that is sound in structure and empty of drawing, with metrics
    /// identical to the real one's. No assertion about widths would ever see
    /// it. `Face::extraire` takes the whole file, and here is why.
    #[test]
    fn l_extraction_lit_le_fichier_entier_jamais_le_tampon() {
        let dir = dossier("extraction");
        let mut grasse = Fonte::neuve();
        grasse.gras = 20_000;
        let chemin = dir.join("grasse.ttf");
        fs::write(&chemin, fichier(&[grasse])).unwrap();

        let entier = fs::read(&chemin).unwrap();
        let sortie = Face::extraire(&entier, 0).expect("extraction");
        let (off, len) = table_range(&sortie, 0, b"glyf").unwrap();
        assert_eq!(len, 20_000);
        assert!(sortie[off..off + len].iter().all(|b| *b == 0x2a), "les contours du fichier");

        let (tampon, lus) = lire_tables(&chemin).expect("tables lues");
        assert!(lus < 4096, "{lus} octets : la découverte lit peu, c'est le sujet");
        let creuse = Face::extraire(&tampon, 0).expect("elle s'extrait quand même, et c'est ça");
        let (off, len) = table_range(&creuse, 0, b"glyf").unwrap();
        assert_eq!(len, 20_000);
        assert!(creuse[off..off + len].iter().all(|b| *b == 0), "des zéros, pas un contour");
        // Les deux polices se lisent pareil : la métrique ne voit rien.
        identiques(
            &Face::parse(&creuse, 0).unwrap().metrics.unwrap(),
            &Face::parse(&sortie, 0).unwrap().metrics.unwrap(),
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// The walk: whole, sorted, refusals included, sub-folders in, hidden
    /// files and symlinks out, and the same list twice running.
    #[test]
    fn la_decouverte_marche_le_dossier_et_le_trie() {
        let dir = dossier("decouverte");
        let sous = dir.join("sous-dossier");
        fs::create_dir_all(&sous).unwrap();

        let mut une = Fonte::neuve();
        une.famille = "Colophon Une".into();
        let mut deux = Fonte::neuve();
        deux.famille = "Colophon Deux".into();
        let mut otto = Fonte::neuve();
        otto.tag = *b"OTTO";
        otto.contours = Some(*b"CFF ");

        fs::write(dir.join("b.ttf"), fichier(&[Fonte::neuve()])).unwrap();
        fs::write(dir.join("a.otf"), fichier(&[otto])).unwrap();
        fs::write(sous.join("c.ttc"), fichier(&[une, deux])).unwrap();
        fs::write(dir.join("cassee.ttf"), b"pas une police du tout").unwrap();
        fs::write(dir.join(".cachee.ttf"), fichier(&[Fonte::neuve()])).unwrap();
        fs::write(dir.join("notes.txt"), b"ni police ni photo").unwrap();
        let dehors = dossier("decouverte-dehors");
        fs::write(dehors.join("d.ttf"), fichier(&[Fonte::neuve()])).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dehors.join("d.ttf"), dir.join("lien.ttf")).unwrap();

        let adresses = |liste: &[Installee]| -> Vec<String> {
            liste
                .iter()
                .map(|i| {
                    format!(
                        "{}#{}",
                        i.chemin.file_name().unwrap().to_string_lossy(),
                        i.face.index
                    )
                })
                .collect()
        };
        let liste = installed_in(&[dir.clone()]);
        assert_eq!(
            adresses(&liste),
            ["a.otf#0", "b.ttf#0", "cassee.ttf#0", "c.ttc#0", "c.ttc#1"],
            "trié par chemin puis rang, jamais l'ordre du système de fichiers"
        );
        // Le fichier illisible est listé, avec son code : la session 4
        // l'affichera, elle ne le recalculera pas.
        let cassee = liste.iter().find(|i| i.chemin.ends_with("cassee.ttf")).unwrap();
        assert_eq!(cassee.face.refus, Some(REFUS_ILLISIBLE));
        // Aucune face sans verdict.
        assert!(liste.iter().all(|i| i.face.refus.is_some() || i.face.metrics.is_some()));
        assert_eq!(liste.iter().filter(|i| i.face.embeddable()).count(), 4);
        // Le sous-dossier est marché, le caché et le lien ne le sont pas.
        assert!(liste.iter().any(|i| i.chemin.ends_with("sous-dossier/c.ttc") || i.chemin.ends_with("sous-dossier\\c.ttc")));

        assert_eq!(adresses(&installed_in(&[dir.clone()])), adresses(&liste), "deux appels, une liste");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dehors);
    }

    /// The poor man's fuzz: the fixture cut at every length, then one byte
    /// flipped every seven. Nothing here asserts a result — only that no
    /// reading of a hostile file ever panics or runs away.
    #[test]
    fn un_fichier_hostile_ne_fait_jamais_paniquer() {
        let data = fichier(&[Fonte::neuve()]);
        for n in 0..=data.len() {
            let _ = Face::parse(&data[..n], 0);
            let _ = Face::extraire(&data[..n], 0);
            let _ = face_count(&data[..n]);
        }
        for i in (0..data.len()).step_by(7) {
            let mut mute = data.clone();
            mute[i] ^= 0xFF;
            let _ = Face::parse(&mute, 0);
            let _ = Face::parse(&mute, 1);
            // L'écrivain lit les mêmes octets hostiles que le lecteur, et
            // écrit à partir d'eux : une longueur inventée doit sortir en
            // refus, jamais en allocation ni en panic.
            let _ = Face::extraire(&mute, 0);
            let _ = Face::extraire(&mute, 1);
        }
        // Une collection dont l'en-tête ment sur le nombre de faces.
        let mut ttc = fichier(&[Fonte::neuve(), Fonte::neuve()]);
        ttc[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(face_count(&ttc) as usize <= ttc.len() / 4);
        for i in 0..face_count(&ttc).min(8) {
            let _ = Face::parse(&ttc, i);
        }
    }

    /// The bench of arbitration 8: enumerating is not loading. Five hundred
    /// files whose bulk is outline data, walked in under a second, and not
    /// one byte of `glyf` read — which is the measure, not the intent.
    #[test]
    fn le_banc_enumere_sans_charger() {
        let dir = dossier("banc");
        let mut grasse = Fonte::neuve();
        grasse.gras = 20_000;
        let data = fichier(&[grasse]);
        assert!(data.len() > 20_000, "le fichier est du contour, comme une vraie police");
        for i in 0..500 {
            fs::write(dir.join(format!("f{i:03}.ttf")), &data).unwrap();
        }

        let t = std::time::Instant::now();
        let liste = installed_in(&[dir.clone()]);
        let ecoule = t.elapsed();
        assert_eq!(liste.len(), 500);
        assert!(liste.iter().all(|i| i.face.embeddable()));
        assert!(ecoule.as_secs_f64() < 1.0, "500 faces énumérées en {ecoule:?}");

        let (_, lus) = lire_tables(&dir.join("f000.ttf")).expect("tables lues");
        assert!(
            lus < 4096,
            "{lus} octets lus sur {} : un contour est passé sur le fil",
            data.len()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// The smoke test: the platform's real folders, zoo and all. It asserts
    /// nothing about what it finds — a CI Ubuntu may carry no font at all —
    /// and everything about the manner: no panic, and not one face without a
    /// verdict.
    #[test]
    fn le_zoo_du_systeme_ne_fait_pas_paniquer_la_lecture() {
        let connus = [
            REFUS_ILLISIBLE,
            REFUS_EMBARQUEMENT_INTERDIT,
            REFUS_BITMAP_SEULEMENT,
            REFUS_CMAP_ILLISIBLE,
            REFUS_FORMAT_NON_EMBARQUABLE,
        ];
        for i in &installed() {
            match i.face.refus {
                Some(code) => assert!(
                    connus.contains(&code),
                    "code inconnu {code} pour {}",
                    i.chemin.display()
                ),
                None => {
                    assert!(i.face.metrics.is_some(), "une face acceptée est mesurée");
                    assert!(i.face.genre.is_some(), "{}", i.chemin.display());
                    assert!(!i.face.postscript.is_empty(), "{} sans nom", i.chemin.display());
                }
            }
        }
    }

    /// A collection that lies about its face count is bounded, not believed.
    /// Before the ceiling, a 20 MB file claiming 0xFFFFFFFF faces produced
    /// five million entries and a gigabyte of them in six seconds — no
    /// panic, and no way back either.
    #[test]
    fn une_collection_menteuse_est_plafonnee() {
        let dir = dossier("menteuse");
        let mut data = fichier(&[Fonte::neuve(), Fonte::neuve()]);
        // Du remplissage, pour que le plafond physique ne suffise pas.
        data.resize(data.len() + 2_000_000, 0);
        data[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        fs::write(dir.join("menteuse.ttc"), &data).unwrap();

        assert_eq!(face_count(&data), FACES_MAX);
        let t = std::time::Instant::now();
        let liste = installed_in(&[dir.clone()]);
        assert!(liste.len() <= FACES_MAX as usize, "{} entrées", liste.len());
        assert!(t.elapsed().as_secs_f64() < 1.0);
        // Les deux vraies faces se lisent, les autres offsets ne mènent à
        // aucune signature sfnt et sont refusés plutôt qu'inventés.
        assert_eq!(liste.iter().filter(|i| i.face.embeddable()).count(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Le coup d'œil humain de la session : les faces de cette machine, avec
    /// leur verdict. Les noms sont-ils sensés, les refus rares et
    /// explicables ?
    /// `cargo test -p colophon-core --release banc_polices_du_mac -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn banc_polices_du_mac() {
        let t = std::time::Instant::now();
        let liste = installed();
        let ecoule = t.elapsed();
        let mut refusees = 0usize;
        let (mut fichiers, mut lus, mut tampon, mut sur_disque) = (0usize, 0u64, 0usize, 0u64);
        let mut vus: Vec<&PathBuf> = liste.iter().map(|i| &i.chemin).collect();
        vus.dedup();
        for chemin in vus {
            if let Some((buf, n)) = lire_tables(chemin) {
                fichiers += 1;
                lus += n;
                tampon = tampon.max(buf.len());
                sur_disque += fs::metadata(chemin).map(|m| m.len()).unwrap_or(0);
            }
        }
        for i in &liste {
            let verdict = match i.face.refus {
                None => format!(
                    "ok   {:<6}{}",
                    i.face.genre.map(Genre::code).unwrap_or("?"),
                    if i.face.variable { " variable" } else { "" }
                ),
                Some(code) => {
                    refusees += 1;
                    format!("REFUS {code}")
                }
            };
            println!(
                "{verdict:<24} {:<40} {}#{}",
                i.face.nom,
                i.chemin.file_name().unwrap_or_default().to_string_lossy(),
                i.face.index
            );
        }
        println!(
            "\n{} faces, {refusees} refusées, {} dossiers, {} ms",
            liste.len(),
            dossiers_systeme().len(),
            ecoule.as_millis()
        );
        println!(
            "{fichiers} fichiers, {} Mo sur le disque, {} Ko lus ({:.2} %), plus grand tampon {} Mo",
            sur_disque / 1_048_576,
            lus / 1024,
            100.0 * lus as f64 / sur_disque.max(1) as f64,
            tampon / 1_048_576
        );
    }

    /// La moitié moteur de la parité écran/papier : les largeurs de
    /// référence, dans la face qu'un album porte à côté de lui.
    ///
    /// L'autre moitié est `scripts/police-cdp.mjs`, qui mesure les mêmes
    /// chaînes dans le navigateur, sur les mêmes octets, et compare. La
    /// tolérance n'est pas un facteur de confort : c'est l'arrondi que le
    /// format impose, un demi-millième d'em par glyphe, l'em du PDF valant
    /// mille et celui d'une face du Mac valant souvent 2048.
    ///
    /// `COLOPHON_POLICE=<dossier d'album> cargo test -p colophon-core --release
    /// banc_parite_ecran_papier -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn banc_parite_ecran_papier() {
        let Some(dir) = std::env::var_os("COLOPHON_POLICE") else {
            println!("COLOPHON_POLICE=<dossier d'album> attendu");
            return;
        };
        let dir = PathBuf::from(dir);
        // Le nom que porte le fichier posé, décidé par le moteur : on
        // regarde les deux, l'album n'en a jamais qu'un.
        let fichier = [POLICE_TTF, POLICE_OTF]
            .into_iter()
            .find(|n| dir.join(n).is_file());
        let choix = face_album(&dir, fichier);
        assert!(choix.defaut.is_none(), "face illisible : {:?}", choix.defaut);

        // Ce qu'un album met vraiment sur une page : une légende, un titre,
        // des accents, des guillemets, et le polonais du 29/08 — celui qui
        // s'imprimait « Za?ó?? » avant le composite.
        let epreuves: Vec<(&str, f64)> = vec![
            ("Corse, 2013", 8.0),
            ("Porto-Vecchio, Bonifacio", 10.5),
            ("Zażółć gęślą jaźń", 10.5),
            ("Un été à l'Île-Rousse — œuvres, «guillemets»", 10.5),
            ("AVWA Ta To Yo fi fl ffi", 24.0),
            ("1234567890", 36.0),
        ];
        let mesures: Vec<String> = epreuves
            .iter()
            .map(|(texte, pt)| {
                let n = choix.face.glyphes(texte).len();
                format!(
                    "  {{ \"texte\": {}, \"pt\": {pt}, \"mm\": {}, \"glyphes\": {n} }}",
                    serde_json::to_string(texte).unwrap(),
                    choix.face.largeur_mm(texte, *pt)
                )
            })
            .collect();
        println!(
            "{{\n \"fichier\": {:?},\n \"postscript\": {:?},\n \"octets\": {},\n \"mesures\": [\n{}\n ]\n}}",
            fichier.unwrap_or("(aucune)"),
            choix.face.postscript(),
            choix.face.octets().len(),
            mesures.join(",\n")
        );
    }

    /// Le coup d'œil humain de cette session : de combien chaque face
    /// rétrécit en sortant de son fichier, et l'aller-retour tenu sur

    /// chacune. C'est le chiffre sur lequel repose le report du
    /// sous-ensemblage — et il doit être honnête sur le `glyf` partagé, que
    /// deux faces d'une même collection se repassent : sortir l'une copie
    /// l'union des dessins des deux, donc une collection rétrécit bien moins
    /// que son nombre de faces ne le laisse croire.
    /// `cargo test -p colophon-core --release banc_extraction_du_mac -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn banc_extraction_du_mac() {
        // Un fichier à la fois : la liste est triée par chemin, et certains
        // pèsent 187 Mo.
        let mut ouvert: Option<(PathBuf, Vec<u8>)> = None;
        let mut par_genre: std::collections::BTreeMap<&str, (usize, u64, u64)> =
            std::collections::BTreeMap::new();
        let (mut seules, mut collections) = ((0usize, 0f64), (0usize, 0f64));
        let (mut refusees, mut cassees) = (0usize, 0usize);
        let t = std::time::Instant::now();

        for i in installed() {
            if !i.face.embeddable() {
                refusees += 1;
                continue;
            }
            if ouvert.as_ref().is_none_or(|(c, _)| *c != i.chemin) {
                let Ok(octets) = fs::read(&i.chemin) else { continue };
                ouvert = Some((i.chemin.clone(), octets));
            }
            let (_, octets) = ouvert.as_ref().unwrap();
            let avant = octets.len() as u64;
            let sortie = match Face::extraire(octets, i.face.index) {
                Ok(s) => s,
                Err(code) => {
                    cassees += 1;
                    println!("EXTRACTION {code:<24} {}#{}", i.face.nom, i.face.index);
                    continue;
                }
            };
            // L'aller-retour, sur chacune : le nom, le rang, les mesures.
            let relue = match Face::parse(&sortie, 0) {
                Ok(f) => f,
                Err(code) => {
                    cassees += 1;
                    println!("RELECTURE  {code:<24} {}#{}", i.face.nom, i.face.index);
                    continue;
                }
            };
            let mut avaries = Vec::new();
            if relue.postscript != i.face.postscript {
                avaries.push(format!("nom {} → {}", i.face.postscript, relue.postscript));
            }
            if face_count(&sortie) != 1 {
                avaries.push(format!("{} faces dans le fichier sorti", face_count(&sortie)));
            }
            match (&i.face.metrics, &relue.metrics) {
                (Some(a), Some(b)) => {
                    if (a.bbox, a.ascent, a.descent, a.cap_height) != (b.bbox, b.ascent, b.descent, b.cap_height) {
                        avaries.push("mesures".into());
                    }
                }
                _ => avaries.push("métriques perdues".into()),
            }
            // Les chasses, glyphe par glyphe : ce que `/W` portera. Une face
            // dont l'extraction décale `hmtx` d'un cran se voit exactement là,
            // et nulle part ailleurs.
            match (
                Embarquee::depuis(octets.clone(), i.face.index),
                Embarquee::depuis(sortie.clone(), 0),
            ) {
                (Ok(a), Ok(b)) if a.avances == b.avances => {}
                (Ok(_), Ok(_)) => avaries.push("chasses".into()),
                _ => avaries.push("face non ouvrable pour écrire".into()),
            }
            if !avaries.is_empty() {
                cassees += 1;
            }

            let apres = sortie.len() as u64;
            let part = apres as f64 / avant.max(1) as f64;
            let e = par_genre.entry(i.face.genre.map(Genre::code).unwrap_or("?")).or_default();
            *e = (e.0 + 1, e.1 + avant, e.2 + apres);
            let dans_une_collection = face_count(octets) > 1;
            if dans_une_collection {
                collections = (collections.0 + 1, collections.1 + part);
            } else {
                seules = (seules.0 + 1, seules.1 + part);
            }
            println!(
                "{:>9} → {:>9}  {:>5.1} %  {:<5}{:<4} {:<38} {}#{}",
                avant,
                apres,
                100.0 * part,
                i.face.genre.map(Genre::code).unwrap_or("?"),
                if dans_une_collection { "ttc" } else { "" },
                i.face.nom,
                i.chemin.file_name().unwrap_or_default().to_string_lossy(),
                i.face.index
            );
            if !avaries.is_empty() {
                println!("            ALLER-RETOUR CASSÉ : {}", avaries.join(", "));
            }
        }

        println!("\n{refusees} faces refusées, {cassees} aller-retours cassés, {:?}", t.elapsed());
        for (genre, (n, avant, apres)) in &par_genre {
            println!(
                "{genre:<6} {n:>4} faces, {} Mo de fichiers → {} Mo extraits ({:.1} %)",
                avant / 1_048_576,
                apres / 1_048_576,
                100.0 * *apres as f64 / (*avant).max(1) as f64
            );
        }
        // Le piège du `glyf` partagé se lit dans l'écart entre ces deux
        // lignes : une face seule paye peu, une face de collection devrait
        // beaucoup payer — et paye moins que son rang ne le promet.
        println!(
            "face seule       : {} faces, {:.1} % du fichier en moyenne",
            seules.0,
            100.0 * seules.1 / seules.0.max(1) as f64
        );
        println!(
            "face de ttc      : {} faces, {:.1} % du fichier en moyenne",
            collections.0,
            100.0 * collections.1 / collections.0.max(1) as f64
        );
        assert_eq!(cassees, 0, "un aller-retour cassé sur une police du système");
    }
}
