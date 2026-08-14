//! EXIF extraction: capture time, orientation, GPS. Falls back to file mtime
//! so that photos without EXIF still sort somewhere sensible.

use chrono::NaiveDateTime;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PhotoMeta {
    pub taken: NaiveDateTime,
    /// True when `taken` came from EXIF rather than file mtime.
    pub taken_reliable: bool,
    /// EXIF orientation tag value (1..8), 1 = upright.
    pub orientation: u32,
    pub gps: Option<(f64, f64)>,
    /// Camera model from EXIF. Its absence is a strong junk signal.
    pub model: Option<String>,
}

pub fn read(path: &Path) -> PhotoMeta {
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).naive_utc())
        .unwrap_or_else(|| chrono::Utc::now().naive_utc());

    let mut meta =
        PhotoMeta { taken: mtime, taken_reliable: false, orientation: 1, gps: None, model: None };

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

    meta.gps = read_gps(&exif);
    meta
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
