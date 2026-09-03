//! Decoding through the platform's own codec, behind a trait, for the two
//! families the `image` crate does not read: HEIC/HEIF, and camera RAW.
//! macOS decodes with ImageIO, a system framework: nothing to ship, the
//! exact codec Photos uses, and its RAW support is Apple's own (thirty RAW
//! type identifiers on a stock Mac, CR3 included). Windows will plug WIC
//! into the same trait when the Windows week comes — its RAW codec is the
//! Store's « Raw Image Extension », not preinstalled; Linux would take
//! libheif, dynamically only. Never imazen/heic here: AGPL-3.0 would
//! contaminate the whole binary. And never a RAW crate either (decided
//! 03/09): the pure-Rust ones are all LGPL-2.1 and carry a demosaic the
//! platform already has. Where there is no system decoder, `scan.rs`
//! counts and the screen says so; nothing is decoded by a second engine.
//!
//! A RAW file carries two images: the sensor data, and the JPEG preview
//! the camera rendered. The preview feeds everything up to the print
//! (`thumb.rs`), and reaches the print itself whenever it holds the
//! resolution floor (`print.rs`); the demosaic is Apple's, not the
//! camera's, so it is asked for only when more pixels are needed.

use anyhow::{Context, Result};
use image::DynamicImage;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What the system reads in a file's own metadata. Only what `meta.rs`
/// needs; `None` where the file says nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExifSysteme {
    /// `DateTimeOriginal`, the shooting date and nothing else.
    pub prise: Option<chrono::NaiveDateTime>,
    /// EXIF orientation, 1 to 8.
    pub orientation: Option<u32>,
    pub modele: Option<String>,
    /// Decimal degrees, signed.
    pub gps: Option<(f64, f64)>,
}

/// One platform's system decoder. Same contract as `image::open`: pixels
/// come back unrotated, the caller applies the EXIF orientation. The
/// preview of a RAW comes back in the same frame as its sensor data.
pub trait SystemDecoder: Sync {
    fn decode(&self, path: &Path) -> Result<DynamicImage>;
    /// Pixel size without a full decode, orientation not applied. For a
    /// RAW this is the sensor, never the preview: resolution is judged on
    /// what the file can print, not on what it shows.
    fn dimensions(&self, path: &Path) -> Result<(u32, u32)>;
    /// The preview the file embeds, at most `max_px` on its long side when
    /// given, native size otherwise. Fails when the file embeds none: the
    /// caller then decides whether a full decode is worth it.
    fn preview(&self, path: &Path, max_px: Option<u32>) -> Result<DynamicImage>;
    /// A preview computed from the pixels, whatever the file embeds. A full
    /// decode, counted as one.
    fn preview_calcule(&self, path: &Path, max_px: u32) -> Result<DynamicImage>;
    /// The file's own metadata, `None` when the codec has nothing to say.
    fn exif(&self, path: &Path) -> Option<ExifSysteme>;
}

/// The system decoder of this platform, when it has one.
pub fn system() -> Option<&'static dyn SystemDecoder> {
    #[cfg(target_os = "macos")]
    {
        Some(&imageio::ImageIo)
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub fn is_heic(path: &Path) -> bool {
    extension(path).map(|e| e == "heic" || e == "heif").unwrap_or(false)
}

/// The RAW containers the scan admits, a closed list: the codec's own list
/// changes with every OS release and is invisible on Ubuntu, where the gate
/// must still count a RAW folder. A body the codec does not know fails at
/// decode time, on the « fichier illisible » path, which is the only place
/// that can know.
pub const RAW: [&str; 13] = [
    "cr2", "cr3", "crw", "nef", "nrw", "arw", "dng", "raf", "orf", "rw2", "pef", "3fr", "fff",
];

pub fn is_raw(path: &Path) -> bool {
    extension(path).map(|e| RAW.contains(&e.as_str())).unwrap_or(false)
}

/// Whether the file can only be read by the system decoder.
pub fn systeme_requis(path: &Path) -> bool {
    is_heic(path) || is_raw(path)
}

fn extension(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
}

/// How many full decodes the system decoder has run in this process. The
/// composition of a RAW folder must not move it: everything before the
/// print reads the preview, and a test holds that promise.
static DECODAGES_PLEINS: AtomicUsize = AtomicUsize::new(0);

pub fn decodages_pleins() -> usize {
    DECODAGES_PLEINS.load(Ordering::Relaxed)
}

fn decodeur(path: &Path) -> Result<&'static dyn SystemDecoder> {
    system().with_context(|| {
        format!(
            "pas de décodeur {} sur cette plateforme : {}",
            if is_raw(path) { "RAW" } else { "HEIC" },
            path.display()
        )
    })
}

/// `image::open`, HEIC and RAW routed through the system decoder. For a
/// RAW this is the demosaic: full sensor, the platform's rendering.
pub fn open(path: &Path) -> Result<DynamicImage> {
    if systeme_requis(path) {
        DECODAGES_PLEINS.fetch_add(1, Ordering::Relaxed);
        decodeur(path)?.decode(path)
    } else {
        image::open(path).with_context(|| format!("décodage de {}", path.display()))
    }
}

/// `image::image_dimensions`, HEIC and RAW routed through the system
/// decoder. Header reads only, no pixel decode on either path.
pub fn dimensions(path: &Path) -> Result<(u32, u32)> {
    if systeme_requis(path) {
        decodeur(path)?.dimensions(path)
    } else {
        Ok(image::image_dimensions(path)?)
    }
}

/// The embedded preview of a RAW, native size. `Err` when the file has
/// none or the platform cannot read it.
pub fn apercu(path: &Path) -> Result<DynamicImage> {
    decodeur(path)?.preview(path, None)
}

/// A RAW's thumbnail, at most `max_px` on its long side, without a demosaic
/// whenever the embedded preview allows it. One invariant of `thumb.rs`
/// must survive: a thumbnail under the cap is the original's exact pixel
/// count, which is how the editor warns about resolution without reopening
/// the file. A camera whose preview is smaller than the cap while its
/// sensor is not would break it, so that one case — none of the eight
/// reference bodies — pays a decode, once, into the cache.
pub fn apercu_vignette(path: &Path, max_px: u32) -> Result<DynamicImage> {
    let d = decodeur(path)?;
    if let Ok(img) = d.preview(path, Some(max_px)) {
        let cote = img.width().max(img.height());
        let capteur = d.dimensions(path).map(|(w, h)| w.max(h)).unwrap_or(0);
        if cote >= max_px || capteur <= cote {
            return Ok(img);
        }
    }
    DECODAGES_PLEINS.fetch_add(1, Ordering::Relaxed);
    d.preview_calcule(path, max_px)
}

/// The system's reading of a RAW's metadata, `None` off macOS or when the
/// file says nothing.
pub fn exif(path: &Path) -> Option<ExifSysteme> {
    system()?.exif(path)
}

/// The same size, with the EXIF orientation applied: what every caller means
/// when it says a photo is 4000 × 3000. Tags 5 to 8 turn the picture on its
/// side, so width and height swap.
///
/// The orientation is passed in rather than read here: the composer and the
/// linter have already read the EXIF block for other reasons, and re-opening
/// the file once per photograph to learn one integer is a cost neither
/// should pay. What they must not each own is the swap — a photo judged on
/// an unswapped header reads landscape when it is portrait, which silently
/// inverts every aspect ratio downstream.
pub fn oriented_dimensions(path: &Path, orientation: u32) -> Result<(u32, u32)> {
    dimensions(path).map(|(w, h)| oriente((w, h), orientation))
}

/// The swap alone, for callers that already hold the header size.
pub fn oriente((w, h): (u32, u32), orientation: u32) -> (u32, u32) {
    if (5..=8).contains(&orientation) {
        (h, w)
    } else {
        (w, h)
    }
}

/// An EXIF date in either spelling met in the wild: the standard's
/// `2017:01:05 13:52:55`, and the dashed form the exif crate displays.
pub fn date_exif(s: &str) -> Option<chrono::NaiveDateTime> {
    let s = s.trim();
    chrono::NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
        .ok()
}

#[cfg(target_os = "macos")]
mod imageio {
    //! Minimal ImageIO + CoreGraphics FFI. The frameworks are stable C API
    //! shipped with every macOS; hand-rolled bindings keep the dependency
    //! tree empty. All calls here are documented thread-safe.
    //!
    //! Measured on the eight reference RAW files (03/09): the thumbnail
    //! call with only `CreateThumbnailFromImageIfAbsent` set, and no
    //! maximum size, returns nothing; with a maximum it returns the
    //! embedded preview scaled to it; with a large maximum and no
    //! `IfAbsent` it returns the preview at its native size. And
    //! `CreateImageAtIndex` is lazy: the demosaic runs at the draw, which
    //! is why `decode` is the only place a full decode can be timed.

    use super::{ExifSysteme, SystemDecoder};
    use anyhow::{anyhow, Context, Result};
    use image::DynamicImage;
    use std::ffi::c_void;
    use std::path::Path;

    #[repr(C)]
    struct CGRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    /// `CFDictionaryKeyCallBacks` / `CFDictionaryValueCallBacks`: a version
    /// and five function pointers. Only ever passed by address.
    #[repr(C)]
    struct CFCallBacks {
        _champs: [usize; 6],
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(
            allocator: *const c_void,
            buffer: *const u8,
            buf_len: isize,
            is_directory: bool,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFDictionaryCreate(
            allocator: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            num_values: isize,
            key_callbacks: *const CFCallBacks,
            value_callbacks: *const CFCallBacks,
        ) -> *const c_void;
        fn CFNumberCreate(
            allocator: *const c_void,
            the_type: isize,
            value_ptr: *const c_void,
        ) -> *const c_void;
        fn CFNumberGetValue(
            number: *const c_void,
            the_type: isize,
            value_ptr: *mut c_void,
        ) -> bool;
        fn CFStringGetCString(
            string: *const c_void,
            buffer: *mut u8,
            buffer_size: isize,
            encoding: u32,
        ) -> bool;
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFNumberGetTypeID() -> usize;
        static kCFTypeDictionaryKeyCallBacks: CFCallBacks;
        static kCFTypeDictionaryValueCallBacks: CFCallBacks;
        static kCFBooleanTrue: *const c_void;
    }

    #[link(name = "ImageIO", kind = "framework")]
    extern "C" {
        fn CGImageSourceCreateWithURL(
            url: *const c_void,
            options: *const c_void,
        ) -> *const c_void;
        fn CGImageSourceCreateImageAtIndex(
            source: *const c_void,
            index: usize,
            options: *const c_void,
        ) -> *const c_void;
        fn CGImageSourceCreateThumbnailAtIndex(
            source: *const c_void,
            index: usize,
            options: *const c_void,
        ) -> *const c_void;
        fn CGImageSourceCopyPropertiesAtIndex(
            source: *const c_void,
            index: usize,
            options: *const c_void,
        ) -> *const c_void;
        static kCGImagePropertyPixelWidth: *const c_void;
        static kCGImagePropertyPixelHeight: *const c_void;
        static kCGImagePropertyOrientation: *const c_void;
        static kCGImagePropertyExifDictionary: *const c_void;
        static kCGImagePropertyTIFFDictionary: *const c_void;
        static kCGImagePropertyGPSDictionary: *const c_void;
        static kCGImagePropertyExifDateTimeOriginal: *const c_void;
        static kCGImagePropertyTIFFOrientation: *const c_void;
        static kCGImagePropertyTIFFModel: *const c_void;
        static kCGImagePropertyGPSLatitude: *const c_void;
        static kCGImagePropertyGPSLatitudeRef: *const c_void;
        static kCGImagePropertyGPSLongitude: *const c_void;
        static kCGImagePropertyGPSLongitudeRef: *const c_void;
        static kCGImageSourceCreateThumbnailFromImageIfAbsent: *const c_void;
        static kCGImageSourceCreateThumbnailFromImageAlways: *const c_void;
        static kCGImageSourceThumbnailMaxPixelSize: *const c_void;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGImageGetWidth(image: *const c_void) -> usize;
        fn CGImageGetHeight(image: *const c_void) -> usize;
        fn CGImageRelease(image: *const c_void);
        fn CGColorSpaceCreateDeviceRGB() -> *const c_void;
        fn CGColorSpaceRelease(space: *const c_void);
        fn CGBitmapContextCreate(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            space: *const c_void,
            bitmap_info: u32,
        ) -> *const c_void;
        fn CGContextRelease(ctx: *const c_void);
        fn CGContextDrawImage(ctx: *const c_void, rect: CGRect, image: *const c_void);
        fn CGBitmapContextGetData(ctx: *const c_void) -> *mut c_void;
    }

    /// kCGImageAlphaNoneSkipLast: RGBX, 8 bits per component.
    const BITMAP_INFO_RGBX: u32 = 5;
    /// kCFNumberSInt32Type
    const CF_NUMBER_SINT32: isize = 3;
    /// kCFNumberDoubleType
    const CF_NUMBER_DOUBLE: isize = 13;
    /// kCFStringEncodingUTF8
    const CF_UTF8: u32 = 0x0800_0100;
    /// A maximum no camera reaches: asks for the embedded preview whole.
    const SANS_PLAFOND: i32 = 100_000;

    /// RAII wrapper so every CF/CG object is released on every path.
    struct Released(*const c_void, unsafe extern "C" fn(*const c_void));
    impl Drop for Released {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { (self.1)(self.0) }
            }
        }
    }

    fn image_source(path: &Path) -> Result<Released> {
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_os_str().as_bytes();
        unsafe {
            let url = CFURLCreateFromFileSystemRepresentation(
                std::ptr::null(),
                bytes.as_ptr(),
                bytes.len() as isize,
                false,
            );
            if url.is_null() {
                return Err(anyhow!("URL invalide : {}", path.display()));
            }
            let url = Released(url, CFRelease);
            let source = CGImageSourceCreateWithURL(url.0, std::ptr::null());
            if source.is_null() {
                return Err(anyhow!("fichier illisible : {}", path.display()));
            }
            Ok(Released(source, CFRelease))
        }
    }

    /// Draw a CGImage into an RGB buffer. For a RAW this is where the
    /// demosaic actually runs: the image object itself is lazy.
    fn pixels(img: *const c_void, path: &Path) -> Result<DynamicImage> {
        unsafe {
            let (w, h) = (CGImageGetWidth(img), CGImageGetHeight(img));
            anyhow::ensure!(w > 0 && h > 0, "image vide : {}", path.display());

            let space = Released(CGColorSpaceCreateDeviceRGB(), CGColorSpaceRelease);
            let ctx = CGBitmapContextCreate(
                std::ptr::null_mut(),
                w,
                h,
                8,
                w * 4,
                space.0,
                BITMAP_INFO_RGBX,
            );
            if ctx.is_null() {
                return Err(anyhow!("contexte bitmap refusé ({w}×{h})"));
            }
            let ctx = Released(ctx, CGContextRelease);
            CGContextDrawImage(
                ctx.0,
                CGRect { x: 0.0, y: 0.0, w: w as f64, h: h as f64 },
                img,
            );
            let data = CGBitmapContextGetData(ctx.0);
            anyhow::ensure!(!data.is_null(), "bitmap sans données");
            let rgbx = std::slice::from_raw_parts(data as *const u8, w * h * 4);
            let mut rgb = Vec::with_capacity(w * h * 3);
            for px in rgbx.chunks_exact(4) {
                rgb.extend_from_slice(&px[..3]);
            }
            let buf = image::RgbImage::from_raw(w as u32, h as u32, rgb)
                .context("tampon RGB incohérent")?;
            Ok(DynamicImage::ImageRgb8(buf))
        }
    }

    /// The options of a thumbnail request. `plafond` is the long side;
    /// `depuis_les_pixels` selects between « only if the file embeds none »
    /// and « always from the pixels ».
    fn options_vignette(plafond: i32, depuis_les_pixels: Option<bool>) -> Released {
        unsafe {
            let nombre = Released(
                CFNumberCreate(
                    std::ptr::null(),
                    CF_NUMBER_SINT32,
                    &plafond as *const i32 as *const c_void,
                ),
                CFRelease,
            );
            let mut cles: Vec<*const c_void> = vec![kCGImageSourceThumbnailMaxPixelSize];
            let mut valeurs: Vec<*const c_void> = vec![nombre.0];
            match depuis_les_pixels {
                Some(true) => {
                    cles.push(kCGImageSourceCreateThumbnailFromImageAlways);
                    valeurs.push(kCFBooleanTrue);
                }
                Some(false) => {
                    cles.push(kCGImageSourceCreateThumbnailFromImageIfAbsent);
                    valeurs.push(kCFBooleanTrue);
                }
                None => {}
            }
            let dict = CFDictionaryCreate(
                std::ptr::null(),
                cles.as_ptr(),
                valeurs.as_ptr(),
                cles.len() as isize,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            Released(dict, CFRelease)
        }
    }

    fn vignette(path: &Path, plafond: i32, depuis_les_pixels: Option<bool>) -> Result<DynamicImage> {
        let source = image_source(path)?;
        let options = options_vignette(plafond, depuis_les_pixels);
        unsafe {
            let img = CGImageSourceCreateThumbnailAtIndex(source.0, 0, options.0);
            if img.is_null() {
                return Err(anyhow!("pas d'aperçu embarqué : {}", path.display()));
            }
            let img = Released(img, CGImageRelease);
            pixels(img.0, path)
        }
    }

    /// A string value of a CF dictionary, `None` when absent or not a string.
    unsafe fn chaine(dict: *const c_void, key: *const c_void) -> Option<String> {
        let v = CFDictionaryGetValue(dict, key);
        if v.is_null() || CFGetTypeID(v) != CFStringGetTypeID() {
            return None;
        }
        let mut buf = vec![0u8; 512];
        if !CFStringGetCString(v, buf.as_mut_ptr(), buf.len() as isize, CF_UTF8) {
            return None;
        }
        let fin = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        Some(String::from_utf8_lossy(&buf[..fin]).to_string())
    }

    unsafe fn nombre(dict: *const c_void, key: *const c_void) -> Option<f64> {
        let v = CFDictionaryGetValue(dict, key);
        if v.is_null() || CFGetTypeID(v) != CFNumberGetTypeID() {
            return None;
        }
        let mut value: f64 = 0.0;
        CFNumberGetValue(v, CF_NUMBER_DOUBLE, &mut value as *mut f64 as *mut c_void)
            .then_some(value)
    }

    unsafe fn entier(dict: *const c_void, key: *const c_void) -> Option<u32> {
        let v = CFDictionaryGetValue(dict, key);
        if v.is_null() || CFGetTypeID(v) != CFNumberGetTypeID() {
            return None;
        }
        let mut value: i32 = 0;
        CFNumberGetValue(v, CF_NUMBER_SINT32, &mut value as *mut i32 as *mut c_void)
            .then_some(value.max(0) as u32)
    }

    fn proprietes(path: &Path) -> Result<Released> {
        let source = image_source(path)?;
        unsafe {
            let props = CGImageSourceCopyPropertiesAtIndex(source.0, 0, std::ptr::null());
            if props.is_null() {
                return Err(anyhow!("propriétés illisibles : {}", path.display()));
            }
            Ok(Released(props, CFRelease))
        }
    }

    pub struct ImageIo;

    impl SystemDecoder for ImageIo {
        fn decode(&self, path: &Path) -> Result<DynamicImage> {
            let source = image_source(path)?;
            unsafe {
                let img =
                    CGImageSourceCreateImageAtIndex(source.0, 0, std::ptr::null());
                if img.is_null() {
                    return Err(anyhow!("décodage refusé par ImageIO : {}", path.display()));
                }
                let img = Released(img, CGImageRelease);
                pixels(img.0, path)
            }
        }

        fn dimensions(&self, path: &Path) -> Result<(u32, u32)> {
            let props = proprietes(path)?;
            unsafe {
                let w = entier(props.0, kCGImagePropertyPixelWidth);
                let h = entier(props.0, kCGImagePropertyPixelHeight);
                match (w, h) {
                    (Some(w), Some(h)) if w > 0 && h > 0 => Ok((w, h)),
                    _ => Err(anyhow!("taille absente : {}", path.display())),
                }
            }
        }

        fn preview(&self, path: &Path, max_px: Option<u32>) -> Result<DynamicImage> {
            match max_px {
                Some(max) => vignette(path, max.min(SANS_PLAFOND as u32) as i32, Some(false)),
                None => vignette(path, SANS_PLAFOND, None),
            }
        }

        fn preview_calcule(&self, path: &Path, max_px: u32) -> Result<DynamicImage> {
            vignette(path, max_px.min(SANS_PLAFOND as u32) as i32, Some(true))
        }

        fn exif(&self, path: &Path) -> Option<ExifSysteme> {
            let props = proprietes(path).ok()?;
            unsafe {
                let exif = CFDictionaryGetValue(props.0, kCGImagePropertyExifDictionary);
                let tiff = CFDictionaryGetValue(props.0, kCGImagePropertyTIFFDictionary);
                let gps = CFDictionaryGetValue(props.0, kCGImagePropertyGPSDictionary);
                let mut lu = ExifSysteme::default();
                if !exif.is_null() {
                    lu.prise = chaine(exif, kCGImagePropertyExifDateTimeOriginal)
                        .and_then(|s| super::date_exif(&s));
                }
                // The TIFF block's orientation first, then ImageIO's own
                // reading of it: both say the same thing, the second is
                // there for containers that keep it elsewhere.
                let orientation = if tiff.is_null() {
                    None
                } else {
                    entier(tiff, kCGImagePropertyTIFFOrientation)
                }
                .or_else(|| entier(props.0, kCGImagePropertyOrientation));
                lu.orientation = orientation.filter(|o| (1..=8).contains(o));
                if !tiff.is_null() {
                    lu.modele = chaine(tiff, kCGImagePropertyTIFFModel)
                        .map(|m| m.replace('"', "").trim().to_string())
                        .filter(|m| !m.is_empty());
                }
                if !gps.is_null() {
                    let lat = nombre(gps, kCGImagePropertyGPSLatitude);
                    let lon = nombre(gps, kCGImagePropertyGPSLongitude);
                    if let (Some(lat), Some(lon)) = (lat, lon) {
                        let sud = chaine(gps, kCGImagePropertyGPSLatitudeRef)
                            .map(|r| r.contains('S'))
                            .unwrap_or(false);
                        let ouest = chaine(gps, kCGImagePropertyGPSLongitudeRef)
                            .map(|r| r.contains('W'))
                            .unwrap_or(false);
                        let lat = if sud { -lat.abs() } else { lat };
                        let lon = if ouest { -lon.abs() } else { lon };
                        if lat != 0.0 || lon != 0.0 {
                            lu.gps = Some((lat, lon));
                        }
                    }
                }
                Some(lu)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tags 5 to 8 turn the picture on its side. The composer, the linter and
    /// the bascule all read this one function, because when they each owned a
    /// copy the bascule's was simply missing: every rotated photograph then
    /// read landscape when it was portrait, and two paths that must agree
    /// disagreed on three reference sets out of three.
    #[test]
    fn une_photo_couchee_echange_ses_cotes() {
        for droit in [1, 2, 3, 4] {
            assert_eq!(oriente((4000, 3000), droit), (4000, 3000), "tag {droit}");
        }
        for couche in [5, 6, 7, 8] {
            assert_eq!(oriente((4000, 3000), couche), (3000, 4000), "tag {couche}");
        }
    }

    /// The closed list, case-insensitive, and nothing else: a `.jpg` is not
    /// a RAW and a `.CR3` is.
    #[test]
    fn un_raw_se_reconnait_a_son_extension() {
        for ext in RAW {
            assert!(is_raw(Path::new(&format!("a.{ext}"))), "{ext}");
            assert!(is_raw(Path::new(&format!("a.{}", ext.to_uppercase()))), "{ext}");
            assert!(!is_heic(Path::new(&format!("a.{ext}"))));
        }
        for ext in ["jpg", "jpeg", "png", "heic", "tif", "xmp", "json"] {
            assert!(!is_raw(Path::new(&format!("a.{ext}"))), "{ext}");
        }
        assert!(!is_raw(Path::new("sans-extension")));
    }

    /// Both spellings of an EXIF date, and the refusals.
    #[test]
    fn une_date_exif_se_lit_dans_ses_deux_orthographes() {
        let attendu = chrono::NaiveDateTime::parse_from_str(
            "2019-11-29 13:07:45",
            "%Y-%m-%d %H:%M:%S",
        )
        .unwrap();
        assert_eq!(date_exif("2019:11:29 13:07:45"), Some(attendu));
        assert_eq!(date_exif("2019-11-29 13:07:45"), Some(attendu));
        assert_eq!(date_exif("  2019:11:29 13:07:45 "), Some(attendu));
        assert_eq!(date_exif("0000:00:00 00:00:00"), None);
        assert_eq!(date_exif(""), None);
    }

    /// Off macOS there is no system decoder: a RAW is refused with its
    /// family named, never mistaken for a HEIC.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn sans_decodeur_le_refus_nomme_la_famille() {
        let err = open(Path::new("photo.nef")).unwrap_err().to_string();
        assert!(err.contains("RAW"), "{err}");
        let err = open(Path::new("photo.heic")).unwrap_err().to_string();
        assert!(err.contains("HEIC"), "{err}");
    }

    /// The eight reference RAW files, one per container, from raw.pixls.us
    /// (CC0), kept out of the repository like every test set. What is held:
    /// the sensor size is read without a decode, the system reads a date
    /// and a model in every container — including the four the exif crate
    /// refuses (ORF, RW2, RAF, CR3) —, the preview comes back without a
    /// full decode, and the demosaic is the slow path.
    /// `cargo test -p colophon-core --release banc_raw_du_mac -- --ignored --nocapture`
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore]
    fn banc_raw_du_mac() {
        let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap())
            .join("Pictures/colophon-testsets/raw");
        let mut fichiers: Vec<_> = std::fs::read_dir(&dir)
            .expect("~/Pictures/colophon-testsets/raw absent")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| is_raw(p))
            .collect();
        fichiers.sort();
        assert!(!fichiers.is_empty());
        println!(
            "{:<36} {:>11} {:>11} {:>7} {:>11} {:>7} {:>8}  date, modèle",
            "fichier", "capteur", "aperçu", "ms", "vignette", "ms", "plein ms"
        );
        let mut conteneurs_hors_tiff = 0;
        for p in &fichiers {
            let avant = decodages_pleins();
            let capteur = dimensions(p).unwrap();
            let t = std::time::Instant::now();
            let ap = apercu(p).unwrap();
            let ms_ap = t.elapsed().as_millis();
            let t = std::time::Instant::now();
            let vg = apercu_vignette(p, crate::thumb::THUMB_SIZE).unwrap();
            let ms_vg = t.elapsed().as_millis();
            assert_eq!(decodages_pleins(), avant, "l'aperçu a coûté un décodage plein : {}", p.display());
            let ex = exif(p).expect("le système lit les métadonnées");
            assert!(ex.prise.is_some(), "date absente : {}", p.display());
            assert!(ex.modele.is_some(), "modèle absent : {}", p.display());
            assert_eq!(ex.orientation, Some(1), "{}", p.display());
            let t = std::time::Instant::now();
            let plein = open(p).unwrap();
            let ms_plein = t.elapsed().as_millis();
            assert_eq!((plein.width(), plein.height()), capteur);
            assert_eq!(decodages_pleins(), avant + 1);
            let ext = p.extension().unwrap().to_string_lossy().to_lowercase();
            if ["orf", "rw2", "raf", "cr3"].contains(&ext.as_str()) {
                conteneurs_hors_tiff += 1;
            }
            println!(
                "{:<36} {:>5}×{:<5} {:>5}×{:<5} {:>7} {:>5}×{:<5} {:>7} {:>8}  {}, {}",
                p.file_name().unwrap().to_string_lossy(),
                capteur.0,
                capteur.1,
                ap.width(),
                ap.height(),
                ms_ap,
                vg.width(),
                vg.height(),
                ms_vg,
                ms_plein,
                ex.prise.unwrap(),
                ex.modele.unwrap()
            );
        }
        assert_eq!(conteneurs_hors_tiff, 4, "les quatre conteneurs que le crate exif refuse");
    }
}
