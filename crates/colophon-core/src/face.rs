//! Face detection for crop focal points. SeetaFace via rustface: pure Rust,
//! 1.2 MB embedded model, no native runtime to ship. We only need one good
//! anchor point per photo, not exhaustive recall.

use image::DynamicImage;
use rustface::{Detector, FaceInfo, ImageData};

const MODEL: &[u8] = include_bytes!("../models/seeta_fd_frontal_v1.0.bin");
/// Detection runs on a downscaled gray image for speed.
const DETECT_SIZE: u32 = 640;

pub fn new_detector() -> Box<dyn Detector> {
    let model = rustface::model::read_model(std::io::Cursor::new(MODEL))
        .expect("embedded face model is valid");
    let mut det = rustface::create_detector_with_model(model);
    det.set_min_face_size(24);
    det.set_score_thresh(2.0);
    det.set_pyramid_scale_factor(0.8);
    det.set_slide_window_step(4, 4);
    det
}

/// Focal point in [0,1]x[0,1] anchored on the faces, or None when no face.
/// Multiple faces: weighted centroid (bigger faces weigh more), so a group
/// shot anchors between people rather than on one head.
pub fn focal_point(det: &mut dyn Detector, img: &DynamicImage) -> Option<[f64; 2]> {
    let small = img.resize(DETECT_SIZE, DETECT_SIZE, image::imageops::FilterType::Triangle);
    let gray = small.to_luma8();
    let (w, h) = (gray.width(), gray.height());
    let data = ImageData::new(gray.as_raw(), w, h);
    let faces: Vec<FaceInfo> = det.detect(&data);
    if faces.is_empty() {
        return None;
    }
    let mut sx = 0.0f64;
    let mut sy = 0.0f64;
    let mut sw = 0.0f64;
    for f in &faces {
        let b = f.bbox();
        let weight = f64::from(b.width() * b.height());
        let cx = f64::from(b.x()) + f64::from(b.width()) / 2.0;
        // Anchor slightly above the face centre: keeps hair and some headroom.
        let cy = f64::from(b.y()) + f64::from(b.height()) * 0.40;
        sx += cx * weight;
        sy += cy * weight;
        sw += weight;
    }
    Some([(sx / sw / f64::from(w)).clamp(0.0, 1.0), (sy / sw / f64::from(h)).clamp(0.0, 1.0)])
}
