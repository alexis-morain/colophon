//! HEIC/HEIF decoding through the platform's own codec, behind a trait.
//! macOS decodes with ImageIO, a system framework: nothing to ship, the
//! exact codec Photos uses. Windows will plug WIC into the same trait when
//! the Windows week comes; Linux would take libheif, dynamically only.
//! Never imazen/heic here: AGPL-3.0 would contaminate the whole binary.

use anyhow::{Context, Result};
use image::DynamicImage;
use std::path::Path;

/// One platform's system decoder. Same contract as `image::open`: pixels
/// come back unrotated, the caller applies the EXIF orientation.
pub trait HeicDecoder: Sync {
    fn decode(&self, path: &Path) -> Result<DynamicImage>;
    /// Pixel size without a full decode, orientation not applied.
    fn dimensions(&self, path: &Path) -> Result<(u32, u32)>;
}

/// The system decoder of this platform, when it has one.
pub fn system() -> Option<&'static dyn HeicDecoder> {
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
    path.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            e == "heic" || e == "heif"
        })
        .unwrap_or(false)
}

/// `image::open`, HEIC routed through the system decoder.
pub fn open(path: &Path) -> Result<DynamicImage> {
    if is_heic(path) {
        system()
            .with_context(|| {
                format!("pas de décodeur HEIC sur cette plateforme : {}", path.display())
            })?
            .decode(path)
    } else {
        image::open(path).with_context(|| format!("décodage de {}", path.display()))
    }
}

/// `image::image_dimensions`, HEIC routed through the system decoder.
/// Header reads only, no pixel decode on either path.
pub fn dimensions(path: &Path) -> Result<(u32, u32)> {
    if is_heic(path) {
        system()
            .with_context(|| {
                format!("pas de décodeur HEIC sur cette plateforme : {}", path.display())
            })?
            .dimensions(path)
    } else {
        Ok(image::image_dimensions(path)?)
    }
}

#[cfg(target_os = "macos")]
mod imageio {
    //! Minimal ImageIO + CoreGraphics FFI. The frameworks are stable C API
    //! shipped with every macOS; hand-rolled bindings keep the dependency
    //! tree empty. All calls here are documented thread-safe.

    use super::HeicDecoder;
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
        fn CFNumberGetValue(
            number: *const c_void,
            the_type: isize,
            value_ptr: *mut c_void,
        ) -> bool;
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
        fn CGImageSourceCopyPropertiesAtIndex(
            source: *const c_void,
            index: usize,
            options: *const c_void,
        ) -> *const c_void;
        static kCGImagePropertyPixelWidth: *const c_void;
        static kCGImagePropertyPixelHeight: *const c_void;
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

    pub struct ImageIo;

    impl HeicDecoder for ImageIo {
        fn decode(&self, path: &Path) -> Result<DynamicImage> {
            let source = image_source(path)?;
            unsafe {
                let img =
                    CGImageSourceCreateImageAtIndex(source.0, 0, std::ptr::null());
                if img.is_null() {
                    return Err(anyhow!("décodage HEIC échoué : {}", path.display()));
                }
                let img = Released(img, CGImageRelease);
                let (w, h) = (CGImageGetWidth(img.0), CGImageGetHeight(img.0));
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
                    img.0,
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

        fn dimensions(&self, path: &Path) -> Result<(u32, u32)> {
            let source = image_source(path)?;
            unsafe {
                let props =
                    CGImageSourceCopyPropertiesAtIndex(source.0, 0, std::ptr::null());
                if props.is_null() {
                    return Err(anyhow!("propriétés illisibles : {}", path.display()));
                }
                let props = Released(props, CFRelease);
                let read = |key: *const c_void| -> Option<u32> {
                    let num = CFDictionaryGetValue(props.0, key);
                    if num.is_null() {
                        return None;
                    }
                    let mut value: i32 = 0;
                    CFNumberGetValue(
                        num,
                        CF_NUMBER_SINT32,
                        &mut value as *mut i32 as *mut c_void,
                    )
                    .then_some(value.max(0) as u32)
                };
                let w = read(kCGImagePropertyPixelWidth);
                let h = read(kCGImagePropertyPixelHeight);
                match (w, h) {
                    (Some(w), Some(h)) if w > 0 && h > 0 => Ok((w, h)),
                    _ => Err(anyhow!("taille absente : {}", path.display())),
                }
            }
        }
    }
}
