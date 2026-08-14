//! Per-photo analysis on thumbnails: perceptual hashes (dHash for
//! gradients, DCT pHash for structure), sharpness (variance of Laplacian)
//! and exposure (clipped-histogram penalty).

use image::{imageops::FilterType, DynamicImage, GrayImage};

#[derive(Debug, Clone)]
pub struct Analysis {
    pub dhash: u64,
    /// DCT perceptual hash (image_hasher, MIT). Second opinion on
    /// near-duplicates: structure-based where dHash reads gradients.
    pub phash: u64,
    /// Mean RGB per quadrant (2x2 grid): coarse color fingerprint that
    /// catches same-scene shots the gradient hash misses.
    pub colorsig: [u8; 12],
    /// Higher is sharper. Comparable across photos of similar size only.
    pub sharpness: f64,
    /// 0..1, 1 = well exposed.
    pub exposure: f64,
    pub width: u32,
    pub height: u32,
}

impl Analysis {
    pub fn is_portrait(&self) -> bool {
        self.height > self.width
    }

    pub fn aspect(&self) -> f64 {
        f64::from(self.width) / f64::from(self.height).max(1.0)
    }

    /// Composite score used to rank photos inside a burst or chapter.
    pub fn score(&self) -> f64 {
        // Sharpness dominates; exposure modulates. Log keeps outliers sane.
        (1.0 + self.sharpness).ln() * (0.5 + 0.5 * self.exposure)
    }
}

pub fn analyze(img: &DynamicImage) -> Analysis {
    let gray = img.to_luma8();
    Analysis {
        dhash: dhash(&gray),
        phash: phash(img),
        colorsig: colorsig(img),
        sharpness: laplacian_variance(&gray),
        exposure: exposure_score(&gray),
        width: img.width(),
        height: img.height(),
    }
}

fn colorsig(img: &DynamicImage) -> [u8; 12] {
    let small = img.resize_exact(2, 2, FilterType::Triangle).to_rgb8();
    let mut sig = [0u8; 12];
    for (i, p) in small.pixels().enumerate() {
        sig[i * 3] = p[0];
        sig[i * 3 + 1] = p[1];
        sig[i * 3 + 2] = p[2];
    }
    sig
}

/// Mean absolute channel difference between two color signatures.
pub fn color_distance(a: &[u8; 12], b: &[u8; 12]) -> u32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| u32::from(x.abs_diff(*y)))
        .sum::<u32>()
        / 12
}

fn dhash(gray: &GrayImage) -> u64 {
    let small = image::imageops::resize(gray, 9, 8, FilterType::Triangle);
    let mut bits: u64 = 0;
    for y in 0..8 {
        for x in 0..8 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            bits <<= 1;
            if left > right {
                bits |= 1;
            }
        }
    }
    bits
}

pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Classic 64-bit pHash: DCT preprocessing over a mean hash.
fn phash(img: &DynamicImage) -> u64 {
    let hasher = image_hasher::HasherConfig::new()
        .hash_alg(image_hasher::HashAlg::Mean)
        .hash_size(8, 8)
        .preproc_dct()
        .to_hasher();
    let hash = hasher.hash_image(img);
    let mut bits: u64 = 0;
    for (i, b) in hash.as_bytes().iter().take(8).enumerate() {
        bits |= u64::from(*b) << (i * 8);
    }
    bits
}

fn laplacian_variance(gray: &GrayImage) -> f64 {
    // Work at a fixed small size so scores are comparable.
    let g = image::imageops::resize(gray, 256, 256, FilterType::Triangle);
    let (w, h) = (g.width() as i32, g.height() as i32);
    let px = |x: i32, y: i32| g.get_pixel(x as u32, y as u32)[0] as f64;
    let mut vals = Vec::with_capacity(((w - 2) * (h - 2)) as usize);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let lap = 4.0 * px(x, y) - px(x - 1, y) - px(x + 1, y) - px(x, y - 1) - px(x, y + 1);
            vals.push(lap);
        }
    }
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64
}

fn exposure_score(gray: &GrayImage) -> f64 {
    let g = image::imageops::resize(gray, 128, 128, FilterType::Triangle);
    let total = (g.width() * g.height()) as f64;
    let mut dark = 0u32;
    let mut bright = 0u32;
    let mut sum = 0u64;
    for p in g.pixels() {
        sum += u64::from(p[0]);
        match p[0] {
            0..=9 => dark += 1,
            246..=255 => bright += 1,
            _ => {}
        }
    }
    let clipped = (dark + bright) as f64 / total;
    let mut score = (1.0 - clipped * 2.0).clamp(0.0, 1.0);
    // Penalize globally dark or washed-out images even without hard clipping.
    let mean = sum as f64 / total;
    if mean < 70.0 {
        score *= mean / 70.0;
    } else if mean > 200.0 {
        score *= (255.0 - mean) / 55.0;
    }
    score.clamp(0.0, 1.0)
}
