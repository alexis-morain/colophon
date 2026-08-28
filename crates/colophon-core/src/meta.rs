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

    read_exif(path, &mut meta);

    // The EXIF always wins, the Takeout sidecar only fills what it left
    // silent — which is also why a folder that never saw Google behaves to
    // the byte as before. Probed only when something is actually missing:
    // `read` has ten callers, and every candidate spelling is one syscall.
    if !meta.taken_reliable || meta.gps.is_none() {
        takeout_complete(path, &mut meta);
    }
    meta
}

fn read_exif(path: &Path, meta: &mut PhotoMeta) {
    let Ok(file) = fs::File::open(path) else { return };
    let mut reader = std::io::BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
        return;
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
}

/// The Google Takeout sidecar: `IMG.jpg.json` next to `IMG.jpg`, the same
/// gesture as the XMP sidecar on another format. It fills `taken` and `gps`
/// where the EXIF is silent, and nothing else: `favorited` exists in the
/// file but a boolean has no rank on the 1..5 `rating` scale co-tuned with
/// `rating_factor` and `split_junk`, and `geoDataExif` is the EXIF's own
/// copy, already read from the photograph itself.
fn takeout_complete(path: &Path, meta: &mut PhotoMeta) {
    let Some(sidecar) = takeout_read(path) else { return };
    if !meta.taken_reliable {
        if let Some(taken) = sidecar.taken {
            // A declared shooting date, not an export date: it earns the
            // rank of DateTimeOriginal. When Google itself guessed wrong
            // (no EXIF, upload date), an approximate date still beats the
            // ×0.25 junk penalty on a whole library.
            meta.taken = taken;
            meta.taken_reliable = true;
        }
    }
    if meta.gps.is_none() {
        meta.gps = sidecar.gps;
    }
}

/// What one sidecar declares. Only the two fields the album needs.
struct TakeoutSidecar {
    taken: Option<NaiveDateTime>,
    gps: Option<(f64, f64)>,
}

#[derive(Deserialize)]
struct TakeoutJson {
    #[serde(rename = "photoTakenTime")]
    photo_taken_time: Option<TakeoutStamp>,
    #[serde(rename = "geoData")]
    geo_data: Option<TakeoutGeo>,
}

#[derive(Deserialize)]
struct TakeoutStamp {
    timestamp: Option<String>,
}

#[derive(Deserialize)]
struct TakeoutGeo {
    latitude: Option<f64>,
    longitude: Option<f64>,
}

/// One answer per photo and per process. `read` has ten callers, and every
/// candidate spelling of a missing sidecar is one wasted syscall per photo
/// **per caller**: without this table, an ordinary EXIF-less folder paid
/// the probe ten times over (measured at ×1.7 on `banc_meta_read_1000`,
/// against the ×1.5 the session allows; ×1.1 with it). The table never
/// invalidates — nobody writes a Takeout sidecar by hand while an album
/// composes — and empties itself past the cap instead of growing forever.
static TAKEOUT_CACHE: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<std::path::PathBuf, CacheSidecar>>,
> = std::sync::OnceLock::new();
type CacheSidecar = Option<(Option<NaiveDateTime>, Option<(f64, f64)>)>;
const TAKEOUT_CACHE_MAX: usize = 100_000;

fn takeout_read(path: &Path) -> Option<TakeoutSidecar> {
    let cache = TAKEOUT_CACHE.get_or_init(Default::default);
    if let Some(reponse) = cache.read().ok().and_then(|c| c.get(path).copied()) {
        return reponse.map(|(taken, gps)| TakeoutSidecar { taken, gps });
    }
    let lu = takeout_probe(path);
    if let Ok(mut c) = cache.write() {
        if c.len() >= TAKEOUT_CACHE_MAX {
            c.clear();
        }
        c.insert(path.to_path_buf(), lu.as_ref().map(|s| (s.taken, s.gps)));
    }
    lu
}

/// First spelling found wins, silent or not: a closed list is what keeps
/// 20 000 photos linear and `IMG_1234` away from `IMG_1235`'s sidecar.
fn takeout_probe(path: &Path) -> Option<TakeoutSidecar> {
    let text = takeout_sidecar_candidates(path)
        .into_iter()
        .find_map(|p| fs::read_to_string(p).ok())?;
    let json: TakeoutJson = serde_json::from_str(&text).ok()?;
    // The timestamp is a UTC epoch and `taken` is naive: the timezone does
    // not exist anywhere in a Takeout, so UTC is assumed rather than
    // invented (never derived from the GPS — that would be a timezone
    // table, a dependency, and a second source of truth for a date). A
    // photo shot at 23:00 in Paris files under the next day; when the EXIF
    // is there its hour is local and right, and the EXIF wins anyway.
    let taken = json
        .photo_taken_time
        .and_then(|t| t.timestamp)
        .and_then(|s| s.trim().parse::<i64>().ok())
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
        .map(|dt| dt.naive_utc());
    let gps = json.geo_data.and_then(|g| match (g.latitude, g.longitude) {
        // Google writes (0, 0) for "unknown". The gulf of Guinea is a real
        // point of the ocean, and an invented coordinate would still vote
        // in `place_of` and rescue junk in `split_junk`: refused.
        (Some(lat), Some(lon)) if lat != 0.0 || lon != 0.0 => Some((lat, lon)),
        _ => None,
    });
    Some(TakeoutSidecar { taken, gps })
}

/// Google caps a sidecar's name at this many signs before `.json`.
const TAKEOUT_NAME_MAX: usize = 46;

/// The candidate spellings for `NOM.EXT`, in the order of arbitrage 7 —
/// most common first, truncations last. A closed list, never a directory
/// listing, never a string distance: fuzzy matching would be quadratic on
/// 20 000 photos and would pick the wrong photo's sidecar.
fn takeout_sidecar_candidates(path: &Path) -> Vec<std::path::PathBuf> {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return Vec::new();
    };
    let mut bases: Vec<String> = vec![
        name.clone(),
        format!("{name}.supplemental-metadata"),
    ];
    if let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) {
        if stem != name {
            bases.push(stem.clone());
        }
        // A duplicated `NOM(1).EXT` keeps its sidecar under the original
        // name, the marker migrated past the extension: `NOM.EXT(1).json`.
        if let Some((racine, marque)) = suffixe_de_doublon(&stem) {
            let ext = name.strip_prefix(&stem).unwrap_or_default();
            bases.push(format!("{racine}{ext}{marque}"));
            bases.push(format!("{racine}{ext}.supplemental-metadata{marque}"));
        }
    }
    // Truncated prefixes tried last, only where the cap actually cuts.
    let coupes: Vec<String> = bases
        .iter()
        .filter(|b| b.chars().count() > TAKEOUT_NAME_MAX)
        .map(|b| b.chars().take(TAKEOUT_NAME_MAX).collect())
        .collect();
    bases.extend(coupes);
    bases.dedup();
    let mut out: Vec<std::path::PathBuf> = bases
        .into_iter()
        .map(|b| path.with_file_name(format!("{b}.json")))
        .collect();
    // `X-edited.EXT` has no sidecar of its own: Google writes the pair's
    // metadata in `X.EXT`'s sidecar, so the base's spellings are tried
    // after the photo's own and the edited version keeps the original's
    // date — without that, swapping the original for its edited twin
    // (arbitrage 8) would trade a dated photo for a parasite.
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        if let Some(racine) = stem.strip_suffix("-edited") {
            let ext = path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            out.extend(takeout_sidecar_candidates(
                &path.with_file_name(format!("{racine}{ext}")),
            ));
        }
    }
    out
}

/// `"NOM(1)"` → `("NOM", "(1)")`, `None` when the stem carries no marker.
fn suffixe_de_doublon(stem: &str) -> Option<(&str, &str)> {
    let sans = stem.strip_suffix(')')?;
    let ouvre = sans.rfind('(')?;
    let digits = &sans[ouvre + 1..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((&stem[..ouvre], &stem[ouvre..]))
}

/// Whether `path` has a Takeout sidecar at all — the trigger of the
/// `-edited` rule in `build::preparer`, which must never fire outside a
/// Takeout: `mariage-edited.jpg` exists in ordinary folders too, and
/// setting its original aside there would delete a photo for no reason.
pub fn takeout_sidecar_exists(path: &Path) -> bool {
    takeout_sidecar_candidates(path).iter().any(|p| p.exists())
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

    /// Un dossier jetable pour les tests de sidecars Takeout.
    fn dossier(nom: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("colophon-takeout-{nom}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sidecar_json(ts: i64, lat: f64, lon: f64) -> String {
        format!(
            r#"{{"title":"x","photoTakenTime":{{"timestamp":"{ts}","formatted":"peu importe"}},"geoData":{{"latitude":{lat},"longitude":{lon},"altitude":0.0}}}}"#
        )
    }

    /// Les cinq orthographes de l'arbitrage 7, une par une, dans une liste
    /// fermée : jamais de listage de dossier, jamais d'appariement flou.
    /// Un sidecar qui appartient à une autre photo n'est jamais pris.
    #[test]
    fn les_cinq_orthographes_de_sidecar_takeout() {
        let dir = dossier("orthographes");
        // 2020-05-10 12:00:00 UTC.
        let ts = 1_589_112_000i64;
        let attendu =
            NaiveDateTime::parse_from_str("2020-05-10 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap();

        // (photo, sidecar) : chaque orthographe couverte séparément.
        let long = "un-nom-de-photo-vraiment-tres-long-comme-google-en-tronque.jpg";
        let cas: Vec<(&str, String)> = vec![
            // 1. NOM.EXT.json, la plus courante.
            ("a.jpg", "a.jpg.json".into()),
            // 2. Les exports récents.
            ("b.jpg", "b.jpg.supplemental-metadata.json".into()),
            // 3. L'extension remplacée.
            ("c.jpg", "c.json".into()),
            // 4. Le « (1) » migre après l'extension.
            ("d(1).jpg", "d.jpg(1).json".into()),
            // 5. La troncature : Google coupe le nom du sidecar à 46 signes.
            (long, format!("{}.json", &long[..46])),
        ];
        for (photo, sidecar) in &cas {
            let p = dir.join(photo);
            fs::write(&p, b"pas un jpeg").unwrap();
            fs::write(dir.join(sidecar), sidecar_json(ts, 42.3, 9.15)).unwrap();
            let meta = read(&p);
            assert_eq!(meta.taken, attendu, "orthographe {sidecar} non lue pour {photo}");
            assert!(meta.taken_reliable, "une date de sidecar est fiable ({sidecar})");
            assert_eq!(meta.gps, Some((42.3, 9.15)), "le GPS du sidecar comble ({sidecar})");
        }

        // Le sidecar du voisin n'est jamais pris : IMG_1234 ne lit pas IMG_1235.
        let p = dir.join("IMG_1234.jpg");
        fs::write(&p, b"pas un jpeg").unwrap();
        fs::write(dir.join("IMG_1235.jpg.json"), sidecar_json(ts, 42.3, 9.15)).unwrap();
        let meta = read(&p);
        assert!(!meta.taken_reliable, "le sidecar d'une autre photo a été pris");
        assert_eq!(meta.gps, None);
        let _ = fs::remove_dir_all(&dir);
    }

    /// L'arbitrage 2 : l'EXIF gagne toujours, le sidecar comble. Une photo
    /// avec DateTimeOriginal et un sidecar qui dit autre chose garde la
    /// date de l'appareil ; le GPS absent de l'EXIF vient du sidecar.
    #[test]
    fn l_exif_garde_la_main_sur_le_sidecar() {
        let dir = dossier("exif-prioritaire");
        let p = dir.join("appareil.jpg");
        fs::write(&p, jpeg_avec_date_exif("2019:07:14 18:30:00")).unwrap();
        // Le sidecar prétend une tout autre date.
        fs::write(dir.join("appareil.jpg.json"), sidecar_json(1_589_112_000, 42.3, 9.15)).unwrap();
        let meta = read(&p);
        let appareil =
            NaiveDateTime::parse_from_str("2019-07-14 18:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(meta.taken, appareil, "la parole de l'appareil a été perdue");
        assert!(meta.taken_reliable);
        // L'EXIF de ce fichier n'a pas de GPS : le sidecar comble.
        assert_eq!(meta.gps, Some((42.3, 9.15)));
        let _ = fs::remove_dir_all(&dir);
    }

    /// L'arbitrage 5 : Google écrit (0, 0) pour « inconnu », et le golfe de
    /// Guinée est un point réel de l'océan. Mesuré ici : le `MAX_KM` de
    /// l'atlas fait qu'aucune ville n'y vote aujourd'hui — mais une
    /// coordonnée inventée entrerait quand même dans les votes de
    /// `place_of` et dans `split_junk` (GPS + modèle). Le zéro est refusé à
    /// la porte, et `geoDataExif` n'est jamais lu (copie de l'EXIF, déjà lue).
    #[test]
    fn le_zero_du_golfe_de_guinee_est_refuse() {
        let dir = dossier("golfe");
        let p = dir.join("sans-lieu.jpg");
        fs::write(&p, b"pas un jpeg").unwrap();
        fs::write(dir.join("sans-lieu.jpg.json"), sidecar_json(1_589_112_000, 0.0, 0.0)).unwrap();
        let meta = read(&p);
        assert!(meta.taken_reliable, "la date, elle, reste bonne à prendre");
        assert_eq!(meta.gps, None, "(0, 0) veut dire « inconnu », pas le golfe de Guinée");

        // geoDataExif est ignoré, même quand geoData se tait.
        let q = dir.join("copie-exif.jpg");
        fs::write(&q, b"pas un jpeg").unwrap();
        fs::write(
            dir.join("copie-exif.jpg.json"),
            r#"{"photoTakenTime":{"timestamp":"1589112000"},"geoDataExif":{"latitude":42.3,"longitude":9.15}}"#,
        )
        .unwrap();
        assert_eq!(read(&q).gps, None, "geoDataExif est la copie de l'EXIF, déjà lue");
        let _ = fs::remove_dir_all(&dir);
    }

    /// L'arbitrage 6 : `favorited` n'a pas de rang dans l'échelle 1..5 de
    /// `rating`, co-réglée avec `rating_factor` et `split_junk`. Écarté.
    #[test]
    fn favorited_ne_devient_pas_une_etoile() {
        let dir = dossier("favorited");
        let p = dir.join("aimee.jpg");
        fs::write(&p, b"pas un jpeg").unwrap();
        fs::write(
            dir.join("aimee.jpg.json"),
            r#"{"photoTakenTime":{"timestamp":"1589112000"},"favorited":true}"#,
        )
        .unwrap();
        assert_eq!(read(&p).rating, None, "un booléen n'a pas de rang dans l'échelle 1..5");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Un JPEG minimal dont l'APP1 porte un vrai DateTimeOriginal : SOI,
    /// APP1 (TIFF little-endian, IFD0 → ExifIFD → 0x9003), EOI. Il ne se
    /// décode pas, mais `meta::read` n'a que ses en-têtes à lire.
    fn jpeg_avec_date_exif(date: &str) -> Vec<u8> {
        assert_eq!(date.len(), 19);
        let mut tiff: Vec<u8> = Vec::new();
        tiff.extend_from_slice(b"II*\0");
        tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 à l'offset 8
        // IFD0 : une entrée, le pointeur vers l'ExifIFD (0x8769).
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x8769u16.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes()); // LONG
        tiff.extend_from_slice(&1u32.to_le_bytes());
        let exif_ifd_offset = 8 + 2 + 12 + 4; // en-tête IFD0 + entrée + next
        tiff.extend_from_slice(&(exif_ifd_offset as u32).to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes()); // pas d'IFD suivant
        // ExifIFD : une entrée, DateTimeOriginal (0x9003, ASCII, 20 octets).
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x9003u16.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes()); // ASCII
        tiff.extend_from_slice(&20u32.to_le_bytes());
        let date_offset = exif_ifd_offset + 2 + 12 + 4;
        tiff.extend_from_slice(&(date_offset as u32).to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());
        tiff.extend_from_slice(date.as_bytes());
        tiff.push(0);

        let mut out = vec![0xFF, 0xD8]; // SOI
        out.extend_from_slice(&[0xFF, 0xE1]); // APP1
        let len = (2 + 6 + tiff.len()) as u16;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(b"Exif\0\0");
        out.extend_from_slice(&tiff);
        out.extend_from_slice(&[0xFF, 0xD9]); // EOI
        out
    }

    /// Le chrono de 5.1 : 1000 photos, dix appelants simulés par dix
    /// passes. Le facteur qui compte est celui du dossier **sans**
    /// sidecars — le cas de tout le monde — mesuré avant et après la
    /// lecture des sidecars Takeout. Deux dossiers distincts : le cache de
    /// sondes ne s'invalide jamais, un même chemin ne se mesure qu'une fois.
    /// `cargo test -p colophon-core --release banc_meta -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore]
    fn banc_meta_read_1000() {
        let base = std::env::temp_dir().join(format!("colophon-banc-meta-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let img = image::RgbImage::from_fn(80, 80, |x, y| {
            image::Rgb([(x * 3) as u8, (y * 2) as u8, 30])
        });
        let mut jpeg = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();

        let dossier = |nom: &str, sidecars: bool| {
            let dir = base.join(nom);
            fs::create_dir_all(&dir).unwrap();
            let paths: Vec<std::path::PathBuf> =
                (0..1000).map(|i| dir.join(format!("photo-{i}.jpg"))).collect();
            for (i, p) in paths.iter().enumerate() {
                fs::write(p, &jpeg).unwrap();
                if sidecars {
                    let ts = 1_600_000_000 + (i / 334) * 86_400 + (i % 334) * 60;
                    fs::write(
                        format!("{}.json", p.to_string_lossy()),
                        format!(
                            r#"{{"photoTakenTime":{{"timestamp":"{ts}"}},"geoData":{{"latitude":42.3,"longitude":9.15}}}}"#
                        ),
                    )
                    .unwrap();
                }
            }
            paths
        };
        let mesure = |label: &str, paths: &[std::path::PathBuf]| {
            let t = std::time::Instant::now();
            for _ in 0..10 {
                for p in paths {
                    std::hint::black_box(read(p));
                }
            }
            println!("{label}: {} ms (10 x 1000 lectures)", t.elapsed().as_millis());
        };
        let nus = dossier("nu", false);
        mesure("sans sidecars", &nus);
        let sides = dossier("sidecars", true);
        mesure("avec sidecars", &sides);
        let _ = fs::remove_dir_all(&base);
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
