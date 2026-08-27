//! EXIF extraction: capture time, orientation, GPS, and the rating the user
//! already gave the photo. Falls back to file mtime so that photos without
//! EXIF still sort somewhere sensible.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoMeta {
    pub taken: NaiveDateTime,
    /// True when `taken` came from EXIF rather than file mtime.
    pub taken_reliable: bool,
    /// EXIF orientation tag value (1..8), 1 = upright.
    pub orientation: u32,
    pub gps: Option<(f64, f64)>,
    /// Camera model from EXIF. Its absence is a strong junk signal.
    pub model: Option<String>,
    /// What the user already said about this photo: 1 to 5 stars, or -1 for
    /// a photo they rejected. `None` when nobody ever rated it, which is
    /// the case for the overwhelming majority of folders, and reads as a
    /// neutral score, never as a zero.
    pub rating: Option<i8>,
}

pub fn read(path: &Path) -> PhotoMeta {
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).naive_utc())
        .unwrap_or_else(|| chrono::Utc::now().naive_utc());

    let mut meta = PhotoMeta {
        taken: mtime,
        taken_reliable: false,
        orientation: 1,
        gps: None,
        model: None,
        rating: read_xmp_rating(path),
    };

    let Ok(file) = fs::File::open(path) else { return meta };
    let mut reader = std::io::BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
        return meta;
    };

    // Only DateTimeOriginal is a shooting date. Bare DateTime (0x0132) is
    // the date of the last file change, and export tools do write it: one
    // photo copied on June 14th carried « 2026:06:14 » there and dated a
    // whole chapter in the future. It still beats mtime as a sort key, so
    // it fills `taken`, but it earns no trust.
    for (tag, reliable) in
        [(exif::Tag::DateTimeOriginal, true), (exif::Tag::DateTime, false)]
    {
        if let Some(f) = exif.get_field(tag, exif::In::PRIMARY) {
            let s = f.display_value().to_string();
            if let Ok(dt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
                meta.taken = dt;
                meta.taken_reliable = reliable;
                break;
            }
        }
    }

    if let Some(f) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        if let Some(v) = f.value.get_uint(0) {
            if (1..=8).contains(&v) {
                meta.orientation = v;
            }
        }
    }

    if let Some(f) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        let m = f.display_value().to_string().replace('"', "").trim().to_string();
        if !m.is_empty() {
            meta.model = Some(m);
        }
    }

    // The Windows rating tag, written by Explorer and by a fair share of
    // cameras. XMP wins when both are there: it is what the cataloguing
    // apps rewrite, so it is the more recent statement of the two.
    if meta.rating.is_none() {
        if let Some(f) = exif.get_field(TAG_RATING, exif::In::PRIMARY) {
            meta.rating = f.value.get_uint(0).and_then(|v| clamp_rating(f64::from(v)));
        }
    }

    meta.gps = read_gps(&exif);
    meta
}

/// The Windows rating tag (0x4746 in IFD0), absent from the exif crate's
/// table. Its sibling `RatingPercent` (0x4747) says the same thing in
/// hundredths and is ignored.
const TAG_RATING: exif::Tag = exif::Tag(exif::Context::Tiff, 0x4746);

/// How far into a file the XMP packet is looked for. Every writer puts it in
/// the header: an APP1 segment for JPEG, the meta box for HEIC, an iTXt
/// chunk for PNG. Past this we would be scanning compressed pixels for
/// something that is not in them.
const XMP_SCAN_BYTES: usize = 256 * 1024;

/// The rating from `xmp:Rating`, sidecar first. Lightroom and friends write
/// a `.xmp` next to the photo and keep rewriting it as the user changes
/// their mind; the packet inside the file is whatever was true at export.
/// The sidecar therefore wins.
///
/// Apple Photos keeps its favourites in the library database and puts
/// nothing in the exported file: there is nothing to read there, and nothing
/// is invented.
fn read_xmp_rating(path: &Path) -> Option<i8> {
    for sidecar in sidecar_paths(path) {
        if let Ok(text) = fs::read_to_string(&sidecar) {
            if let Some(r) = rating_in_xmp(&text) {
                return Some(r);
            }
        }
    }
    let head = read_head(path, XMP_SCAN_BYTES)?;
    let text = String::from_utf8_lossy(&head);
    let start = text.find("<x:xmpmeta")?;
    let end = text[start..].find("</x:xmpmeta>")? + start;
    rating_in_xmp(&text[start..end])
}

/// Both spellings in the wild: `IMG_1234.xmp` (Adobe, extension replaced)
/// and `IMG_1234.jpg.xmp` (extension appended).
fn sidecar_paths(path: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![path.with_extension("xmp")];
    let appended = format!("{}.xmp", path.to_string_lossy());
    let appended = std::path::PathBuf::from(appended);
    if appended != out[0] {
        out.push(appended);
    }
    out
}

fn read_head(path: &Path, max: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.take(max as u64).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// `xmp:Rating` in either of the two RDF spellings: as an attribute of an
/// `rdf:Description`, or as a child element of one.
fn rating_in_xmp(xmp: &str) -> Option<i8> {
    let at = xmp.find("xmp:Rating")?;
    let rest = xmp[at + "xmp:Rating".len()..].trim_start();
    let value = match rest.as_bytes().first()? {
        b'=' => rest[1..].trim_start().trim_start_matches(['"', '\'']),
        b'>' => &rest[1..],
        _ => return None,
    };
    let digits: String = value
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '.')
        .collect();
    clamp_rating(digits.parse::<f64>().ok()?)
}

/// -1 is Adobe's rejected photo, 1 to 5 are stars, 0 means unrated. Halves
/// exist in the wild, so the value is rounded rather than refused.
fn clamp_rating(value: f64) -> Option<i8> {
    let r = value.round();
    if r == -1.0 {
        return Some(-1);
    }
    if (1.0..=5.0).contains(&r) {
        return Some(r as i8);
    }
    None
}

fn read_gps(exif: &exif::Exif) -> Option<(f64, f64)> {
    let lat = dms_to_deg(exif, exif::Tag::GPSLatitude)?;
    let lon = dms_to_deg(exif, exif::Tag::GPSLongitude)?;
    let lat_sign = ref_sign(exif, exif::Tag::GPSLatitudeRef, "S");
    let lon_sign = ref_sign(exif, exif::Tag::GPSLongitudeRef, "W");
    Some((lat * lat_sign, lon * lon_sign))
}

fn dms_to_deg(exif: &exif::Exif, tag: exif::Tag) -> Option<f64> {
    let field = exif.get_field(tag, exif::In::PRIMARY)?;
    if let exif::Value::Rational(v) = &field.value {
        if v.len() >= 3 {
            return Some(v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two RDF spellings, plus the values that must read as "unrated".
    #[test]
    fn xmp_rating_both_spellings() {
        let attr = r#"<rdf:Description xmp:Rating="4" xmp:Label="Jaune"/>"#;
        assert_eq!(rating_in_xmp(attr), Some(4));

        let elem = "<rdf:Description><xmp:Rating>5</xmp:Rating></rdf:Description>";
        assert_eq!(rating_in_xmp(elem), Some(5));

        // Rejected in Lightroom.
        assert_eq!(rating_in_xmp(r#"<x xmp:Rating="-1"/>"#), Some(-1));
        // Half stars are rounded, not refused.
        assert_eq!(rating_in_xmp(r#"<x xmp:Rating="2.5"/>"#), Some(3));
        // Zero is "never rated", not "worst photo of the folder".
        assert_eq!(rating_in_xmp(r#"<x xmp:Rating="0"/>"#), None);
        // A packet that says nothing about ratings says nothing.
        assert_eq!(rating_in_xmp("<rdf:Description rdf:about=\"\"/>"), None);
    }

    /// A sidecar written next to the photo is read, whichever of the two
    /// naming conventions the cataloguing app used.
    #[test]
    fn sidecar_rating_wins_over_nothing() {
        let dir = std::env::temp_dir().join(format!("colophon-meta-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let packet = |r: i32| {
            format!(
                r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF>
                   <rdf:Description xmp:Rating="{r}"/></rdf:RDF></x:xmpmeta>"#
            )
        };

        // Adobe's spelling: the extension is replaced.
        let photo = dir.join("a.jpg");
        fs::write(&photo, b"not really a jpeg").unwrap();
        fs::write(dir.join("a.xmp"), packet(3)).unwrap();
        assert_eq!(read_xmp_rating(&photo), Some(3));

        // The appended spelling, on a photo that has no `.xmp` twin.
        let other = dir.join("b.jpg");
        fs::write(&other, b"not really a jpeg").unwrap();
        fs::write(dir.join("b.jpg.xmp"), packet(-1)).unwrap();
        assert_eq!(read_xmp_rating(&other), Some(-1));

        // No sidecar, no packet, no rating: silence is not a zero.
        let bare = dir.join("c.jpg");
        fs::write(&bare, b"not really a jpeg").unwrap();
        assert_eq!(read_xmp_rating(&bare), None);

        let _ = fs::remove_dir_all(&dir);
    }

    /// The packet embedded in the file is read when no sidecar contradicts
    /// it, and only within the header window.
    #[test]
    fn embedded_packet_is_read_in_the_header_only() {
        let dir = std::env::temp_dir().join(format!("colophon-meta-emb-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let packet = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:Description xmp:Rating="5"/></x:xmpmeta>"#;

        let near = dir.join("near.jpg");
        let mut data = b"\xff\xd8\xff\xe1".to_vec();
        data.extend_from_slice(packet);
        data.extend_from_slice(&vec![0u8; 4096]);
        fs::write(&near, &data).unwrap();
        assert_eq!(read_xmp_rating(&near), Some(5));

        // Past the window the scan stops: pixels are not metadata.
        let far = dir.join("far.jpg");
        let mut data = vec![0u8; XMP_SCAN_BYTES + 16];
        data.extend_from_slice(packet);
        fs::write(&far, &data).unwrap();
        assert_eq!(read_xmp_rating(&far), None);

        let _ = fs::remove_dir_all(&dir);
    }
}

fn ref_sign(exif: &exif::Exif, tag: exif::Tag, negative: &str) -> f64 {
    exif.get_field(tag, exif::In::PRIMARY)
        .map(|f| {
            if f.display_value().to_string().contains(negative) {
                -1.0
            } else {
                1.0
            }
        })
        .unwrap_or(1.0)
}
