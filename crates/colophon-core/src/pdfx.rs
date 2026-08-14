//! The conformance declaration of an exported PDF.
//!
//! Embedding the face and the colour profile puts the *content* in the file.
//! What a prepress workflow reads first is the *declaration*: an OutputIntent
//! naming the colour the file was made for, an XMP packet naming the standard
//! it claims, an `/Info` dictionary that agrees with that packet word for
//! word, a document identifier, and a trapping answer. Any one of them
//! missing and a supplier's preflight rejects the job before a human sees it.
//!
//! Everything here is written so a tool can check it afterwards, never
//! because a specification was read carefully: `scripts/pdfx.sh` runs veraPDF
//! over the real exports, and [`crate::pdf`] reopens what it just wrote.
//!
//! ## What we can measure, and what we cannot
//!
//! No free validator certifies PDF/X-4. veraPDF, the industry reference,
//! ships PDF/A and PDF/UA profiles only. So the file declares **both**
//! PDF/X-4 and PDF/A-2b, and PDF/A-2b is the one that gets measured: the two
//! standards share their whole structural core (embedded fonts, an
//! OutputIntent with a real ICC profile, well-formed XMP agreeing with
//! `/Info`, no encryption, no borrowed resources), so a green PDF/A-2b run
//! covers nearly all of what X-4 asks. The handful of rules that belong to
//! X-4 alone — the `GTS_PDFX` output intent, the version in the XMP, TrimBox
//! on every page, `/Trapped` — are checked by [`crate::pdf`]'s own tests,
//! which reopen the written file.
//!
//! What stays unmeasured here is the X-4 conformance *as a certified verdict*.
//! The supplier's own preflight is that verdict, and the spec sheet says so.

use crate::icc;
use anyhow::Result;
use chrono::{DateTime, Local};
use lopdf::{dictionary, Document, Object, ObjectId, Stream, StringFormat};

/// Whether the writer's output may call itself PDF/X.
///
/// The single switch: [`crate::prevol`] reads it rather than restating the
/// rule, and its test flips with it. It flips on a green validator run over
/// the real exports, never on a careful reading of the code below.
///
/// True since 14/08/2026: `scripts/pdfx.sh` puts 168 rendered templates (six
/// formats) and the composed albums through veraPDF as PDF/A-2b without a
/// single refusal, and the tests in [`crate::pdf`] reopen the written file
/// for the rules X-4 owns alone.
pub const EMITS_PDF_X: bool = true;

/// The PDF/X level declared in the XMP, and the string a supplier greps for.
pub const PDF_X_VERSION: &str = "PDF/X-4";

/// The PDF/A level the file also declares, and the one veraPDF measures.
pub const PDF_A_PART: &str = "2";
pub const PDF_A_CONFORMANCE: &str = "B";

/// PDF 1.6 is the version PDF/X-4:2010 is built on. PDF/A-2 allows it.
pub const PDF_VERSION: &str = "1.6";

/// `/Producer` and `xmp:CreatorTool`, identical on purpose: the same program
/// laid the album out and wrote the file.
pub fn producer() -> String {
    format!("Colophon {}", env!("CARGO_PKG_VERSION"))
}

/// The pieces a catalog and a trailer need to carry the declaration.
pub struct Declaration {
    /// `/OutputIntents`, an array of one PDF/X and one PDF/A intent sharing
    /// a single embedded profile.
    pub output_intents: Object,
    /// `/Metadata`, the XMP packet.
    pub metadata: ObjectId,
    /// `/Info`, whose every field the XMP repeats.
    pub info: ObjectId,
    /// `/ID`, the trailer's pair of identifiers.
    pub id: Object,
}

/// Put the declaration in the document and hand back what to reference.
///
/// `stamp` is passed in rather than read from the clock so a test can assert
/// that `/Info` and the XMP carry the same instant, which is a rule of the
/// format and not a nicety.
pub fn declare(doc: &mut Document, title: &str, stamp: DateTime<Local>) -> Result<Declaration> {
    let h = icc::header()?;

    // One profile stream, referenced by both intents: PDF/A-2 requires every
    // OutputIntent in a file to point at the same destination profile.
    let profile_id = doc.add_object(Stream::new(
        dictionary! { "N" => h.components()? },
        icc::ICC_DATA.to_vec(),
    ));

    let intent = |subtype: &str| {
        Object::Dictionary(dictionary! {
            "Type" => "OutputIntent",
            "S" => subtype,
            "OutputConditionIdentifier" => text(icc::CONDITION),
            "OutputCondition" => text(icc::CONDITION),
            "RegistryName" => text(icc::REGISTRY),
            "Info" => text(icc::CONDITION),
            "DestOutputProfile" => Object::Reference(profile_id),
        })
    };

    let metadata = doc.add_object(
        // The XMP packet stays uncompressed: a prepress tool is allowed to
        // read it straight out of the bytes without unpacking a filter, and
        // PDF/A asks for exactly that.
        Stream::new(
            dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
            xmp(title, stamp).into_bytes(),
        )
        .with_compression(false),
    );

    let info = doc.add_object(dictionary! {
        "Title" => text(title),
        "Producer" => text(&producer()),
        "Creator" => text(&producer()),
        "CreationDate" => Object::string_literal(pdf_date(stamp)),
        "ModDate" => Object::string_literal(pdf_date(stamp)),
        // PDF/X wants an answer, not a shrug: the album is never trapped, and
        // "Unknown" is the one value the format refuses.
        "Trapped" => "False",
    });

    let fingerprint = fingerprint(title, stamp);
    let id = Object::Array(vec![
        Object::String(fingerprint.clone(), StringFormat::Hexadecimal),
        Object::String(fingerprint, StringFormat::Hexadecimal),
    ]);

    Ok(Declaration { output_intents: Object::Array(vec![intent("GTS_PDFX"), intent("GTS_PDFA1")]), metadata, info, id })
}

/// A PDF text string. Titles carry accents, so everything goes out as
/// UTF-16BE behind its byte-order mark rather than as bytes that would read
/// differently on the two sides of the Atlantic.
fn text(s: &str) -> Object {
    let mut v = vec![0xFE, 0xFF];
    for u in s.encode_utf16() {
        v.extend_from_slice(&u.to_be_bytes());
    }
    Object::String(v, StringFormat::Literal)
}

/// `D:YYYYMMDDHHmmSSOHH'mm'`, the only date syntax `/Info` accepts.
fn pdf_date(t: DateTime<Local>) -> String {
    let base = t.format("D:%Y%m%d%H%M%S").to_string();
    let off = t.format("%z").to_string(); // +0200
    format!("{base}{}{}'{}'", &off[..1], &off[1..3], &off[3..5])
}

/// The same instant in the ISO 8601 form XMP wants. `/Info` and the packet
/// disagreeing is a conformance defect on its own.
fn xmp_date(t: DateTime<Local>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// Two identical 16-byte halves derived from the title and the stamp. The
/// pair exists because the format asks for one; the halves match because the
/// file has never been revised since it was created.
fn fingerprint(title: &str, stamp: DateTime<Local>) -> Vec<u8> {
    let seed = format!("{title}|{}", stamp.to_rfc3339());
    let mut out = Vec::with_capacity(16);
    for round in 0..2u64 {
        // FNV-1a, twice with a different offset basis. A document identifier
        // needs to differ between documents, not to resist an adversary.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ round.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for b in seed.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        out.extend_from_slice(&h.to_be_bytes());
    }
    out
}

/// Escape the five characters that would otherwise end the XML element they
/// sit in. An album called `Été & cie <2013>` is an ordinary album.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// The XMP packet: what the file claims, in the schemas a validator knows.
///
/// Two properties here are not in the schemas PDF/A predefines: `pdfxid`
/// entire, and `pdf:Trapped`, which the 2005 XMP specification never listed.
/// Both are therefore described in the extension block, which is what a
/// validator reads before deciding a property is unknown. That is not a
/// reading of the standard, it is the two rules veraPDF failed on the first
/// export: 6.6.2.3.1 tests 1 and 2, both on `pdf:Trapped`.
pub fn xmp(title: &str, stamp: DateTime<Local>) -> String {
    let t = xml_escape(title);
    let d = xmp_date(stamp);
    let p = xml_escape(&producer());
    // The packet opens on a byte-order mark, inside the attribute, as the XMP
    // specification writes it. Spelled out here rather than pasted into the
    // template below, where an invisible character would be unreviewable.
    let bom = '\u{feff}';
    format!(
        r#"<?xpacket begin="{bom}" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Colophon">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
   <dc:format>application/pdf</dc:format>
   <dc:title><rdf:Alt><rdf:li xml:lang="x-default">{t}</rdf:li></rdf:Alt></dc:title>
  </rdf:Description>
  <rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/">
   <xmp:CreatorTool>{p}</xmp:CreatorTool>
   <xmp:CreateDate>{d}</xmp:CreateDate>
   <xmp:ModifyDate>{d}</xmp:ModifyDate>
   <xmp:MetadataDate>{d}</xmp:MetadataDate>
  </rdf:Description>
  <rdf:Description rdf:about="" xmlns:pdf="http://ns.adobe.com/pdf/1.3/">
   <pdf:Producer>{p}</pdf:Producer>
   <pdf:Trapped>False</pdf:Trapped>
  </rdf:Description>
  <rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
   <pdfaid:part>{PDF_A_PART}</pdfaid:part>
   <pdfaid:conformance>{PDF_A_CONFORMANCE}</pdfaid:conformance>
  </rdf:Description>
  <rdf:Description rdf:about="" xmlns:pdfxid="http://www.npes.org/pdfx/ns/id/">
   <pdfxid:GTS_PDFXVersion>{PDF_X_VERSION}</pdfxid:GTS_PDFXVersion>
  </rdf:Description>
  <rdf:Description rdf:about=""
    xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/"
    xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#"
    xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#">
   <pdfaExtension:schemas>
    <rdf:Bag>
     <rdf:li rdf:parseType="Resource">
      <pdfaSchema:namespaceURI>http://www.npes.org/pdfx/ns/id/</pdfaSchema:namespaceURI>
      <pdfaSchema:prefix>pdfxid</pdfaSchema:prefix>
      <pdfaSchema:schema>PDF/X ID Schema</pdfaSchema:schema>
      <pdfaSchema:property>
       <rdf:Seq>
        <rdf:li rdf:parseType="Resource">
         <pdfaProperty:category>internal</pdfaProperty:category>
         <pdfaProperty:description>ID of PDF/X standard</pdfaProperty:description>
         <pdfaProperty:name>GTS_PDFXVersion</pdfaProperty:name>
         <pdfaProperty:valueType>Text</pdfaProperty:valueType>
        </rdf:li>
       </rdf:Seq>
      </pdfaSchema:property>
     </rdf:li>
     <rdf:li rdf:parseType="Resource">
      <pdfaSchema:namespaceURI>http://ns.adobe.com/pdf/1.3/</pdfaSchema:namespaceURI>
      <pdfaSchema:prefix>pdf</pdfaSchema:prefix>
      <pdfaSchema:schema>Adobe PDF Schema</pdfaSchema:schema>
      <pdfaSchema:property>
       <rdf:Seq>
        <rdf:li rdf:parseType="Resource">
         <pdfaProperty:category>internal</pdfaProperty:category>
         <pdfaProperty:description>Trapping status</pdfaProperty:description>
         <pdfaProperty:name>Trapped</pdfaProperty:name>
         <pdfaProperty:valueType>Text</pdfaProperty:valueType>
        </rdf:li>
       </rdf:Seq>
      </pdfaSchema:property>
     </rdf:li>
    </rdf:Bag>
   </pdfaExtension:schemas>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#
    )
}

/// What goes after `%PDF-` on the first line, newline and all.
///
/// The format asks for the header to be followed immediately by a comment
/// holding at least four bytes above 127: a file transfer in text mode then
/// mangles those bytes and the corruption is visible, rather than silently
/// eating the carriage returns of a 200 MB print file. Written as one string
/// with the version because the writer emits exactly one line up there, and
/// because inserting bytes afterwards would move every offset the
/// cross-reference table has already noted down.
pub fn header_line() -> String {
    // Four accented letters, eight bytes once encoded, every one of them
    // above 127.
    format!("{PDF_VERSION}\n%âãÏÓ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn stamp() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 8, 14, 17, 5, 30).unwrap()
    }

    /// `/Info` and the XMP describe the same instant. Two clocks in one file
    /// is a conformance defect, and the kind nobody notices by reading.
    #[test]
    fn the_two_dates_are_the_same_instant() {
        let t = stamp();
        let info = pdf_date(t);
        let packet = xmp("Corse", t);
        assert!(info.starts_with("D:20260814170530"), "{info}");
        assert!(packet.contains("2026-08-14T17:05:30"), "{packet}");
        // Both carry the offset, in their own syntax.
        assert!(info.contains('\''), "{info}");
        let off = t.format("%:z").to_string();
        assert!(packet.contains(&format!("2026-08-14T17:05:30{off}")));
    }

    /// The packet names both standards, and describes the schema PDF/A does
    /// not predefine. Dropping the extension block would make an otherwise
    /// valid file fail on an unknown property.
    #[test]
    fn the_packet_declares_both_standards() {
        let p = xmp("Corse", stamp());
        assert!(p.contains("<pdfaid:part>2</pdfaid:part>"));
        assert!(p.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
        assert!(p.contains("<pdfxid:GTS_PDFXVersion>PDF/X-4</pdfxid:GTS_PDFXVersion>"));
        assert!(p.contains("pdfaExtension:schemas"));
        assert!(p.contains("http://www.npes.org/pdfx/ns/id/"));
        // The packet opens with the byte-order mark the XMP spec prescribes.
        assert!(p.contains('\u{feff}'), "BOM absent du xpacket");
    }

    /// A title is data, not markup. This one would break the packet if it
    /// went in raw.
    #[test]
    fn a_title_with_markup_in_it_stays_data() {
        let p = xmp("Été & cie <2013>", stamp());
        assert!(p.contains("Été &amp; cie &lt;2013&gt;"), "{p}");
        assert!(!p.contains("<2013>"));
    }

    /// Two albums get two identifiers, and the same album twice at the same
    /// instant gets the same one.
    #[test]
    fn the_identifier_identifies() {
        let t = stamp();
        assert_eq!(fingerprint("Corse", t).len(), 16);
        assert_eq!(fingerprint("Corse", t), fingerprint("Corse", t));
        assert_ne!(fingerprint("Corse", t), fingerprint("Mauritanie", t));
    }
}
