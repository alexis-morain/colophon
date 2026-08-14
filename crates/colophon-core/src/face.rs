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

/// Detected face boxes `[x, y, w, h]`, normalized to [0,1] of the image,
/// (0,0) = top-left. The composer keeps them clear of crop edges and the
/// linter counts the ones that end up cut anyway.
pub fn face_boxes(det: &mut dyn Detector, img: &DynamicImage) -> Vec<[f64; 4]> {
    let small = img.resize(DETECT_SIZE, DETECT_SIZE, image::imageops::FilterType::Triangle);
    let gray = small.to_luma8();
    let (w, h) = (gray.width(), gray.height());
    let data = ImageData::new(gray.as_raw(), w, h);
    let faces: Vec<FaceInfo> = det.detect(&data);
    faces
        .iter()
        .map(|f| {
            let b = f.bbox();
            let x = (f64::from(b.x()) / f64::from(w)).clamp(0.0, 1.0);
            let y = (f64::from(b.y()) / f64::from(h)).clamp(0.0, 1.0);
            let bw = (f64::from(b.width()) / f64::from(w)).min(1.0 - x);
            let bh = (f64::from(b.height()) / f64::from(h)).min(1.0 - y);
            [x, y, bw, bh]
        })
        .collect()
}

/// Focal point in [0,1]x[0,1] anchored on the faces, or None when no face.
/// Multiple faces: weighted centroid (bigger faces weigh more), so a group
/// shot anchors between people rather than on one head.
pub fn focal_from_boxes(faces: &[[f64; 4]]) -> Option<[f64; 2]> {
    if faces.is_empty() {
        return None;
    }
    let mut sx = 0.0f64;
    let mut sy = 0.0f64;
    let mut sw = 0.0f64;
    for b in faces {
        let weight = b[2] * b[3];
        let cx = b[0] + b[2] / 2.0;
        // Anchor slightly above the face centre: keeps hair and some headroom.
        let cy = b[1] + b[3] * 0.40;
        sx += cx * weight;
        sy += cy * weight;
        sw += weight;
    }
    if sw <= 0.0 {
        return None;
    }
    Some([(sx / sw).clamp(0.0, 1.0), (sy / sw).clamp(0.0, 1.0)])
}
