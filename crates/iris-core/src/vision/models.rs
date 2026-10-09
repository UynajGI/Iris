use super::{
    classify_eye, AnalysisSettings, Eye, Face, FaceDetectorProvider, HeadPose, OcclusionProvider,
};
use anyhow::{bail, Context, Result};
use image::{
    imageops::{self, FilterType},
    RgbImage,
};
use ort::{session::Session, value::Tensor};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// MediaPipe's documented 146-point blendshape input ordering (Apache-2.0).
const SUBSET: [usize; 146] = [
    0, 1, 4, 5, 6, 7, 8, 10, 13, 14, 17, 21, 33, 37, 39, 40, 46, 52, 53, 54, 55, 58, 61, 63, 65,
    66, 67, 70, 78, 80, 81, 82, 84, 87, 88, 91, 93, 95, 103, 105, 107, 109, 127, 132, 133, 136,
    144, 145, 146, 148, 149, 150, 152, 153, 154, 155, 157, 158, 159, 160, 161, 162, 163, 168, 172,
    173, 176, 178, 181, 185, 191, 195, 197, 234, 246, 249, 251, 263, 267, 269, 270, 276, 282, 283,
    284, 285, 288, 291, 293, 295, 296, 297, 300, 308, 310, 311, 312, 314, 317, 318, 321, 323, 324,
    332, 334, 336, 338, 356, 361, 362, 365, 373, 374, 375, 377, 378, 379, 380, 381, 382, 384, 385,
    386, 387, 388, 389, 390, 397, 398, 400, 402, 405, 409, 415, 454, 466, 468, 469, 470, 471, 472,
    473, 474, 475, 476, 477,
];
pub struct Models {
    yunet: Session,
    model_dir: PathBuf,
    scrfd: Option<(String, super::scrfd::ScrfdDetector)>,
    faceocc: Option<(String, super::faceocc::FaceOcc)>,
    landmarks: Session,
    blendshapes: Option<Session>,
    blend_warning: Option<String>,
}
#[derive(Clone)]
pub(super) struct Detection {
    pub(super) bbox: [f32; 4],
    pub(super) confidence: f32,
    pub(super) keypoints: [[f32; 2]; 5],
}
fn load_verified(dir: &Path, file: &str, hash: &str) -> Result<Session> {
    let path = dir.join(file);
    let bytes = std::fs::read(&path).with_context(|| {
        format!(
            "Required model {} missing; run python tools/setup-models.py",
            path.display()
        )
    })?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != hash {
        bail!("Model SHA-256 mismatch: {file}");
    }
    super::runtime::session(&bytes, file)
}
impl Models {
    pub fn load(dir: &Path) -> Result<Self> {
        super::runtime::initialize(dir)?;
        let yunet = load_verified(
            dir,
            "yunet.onnx",
            "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4",
        )?;
        let landmarks = load_verified(
            dir,
            "face_landmarks_detector.onnx",
            "9c8dbae0cffd7b8e195b7c5e3795bd2a0f206a06b27edf30b2dd6900175c652a",
        )?;
        let result = load_verified(
            dir,
            "face_blendshapes.onnx",
            "68b42f0bfefe907f0ccb18367513db13ffa737e6aac07c87cd6f422fd1621c40",
        );
        let (blendshapes, blend_warning) = match result {
            Ok(s) => (Some(s), None),
            Err(e) => (
                None,
                Some(format!(
                    "Blendshapes unavailable; EAR retained as measurement only, eye states uncertain: {e}"
                )),
            ),
        };
        Ok(Self {
            yunet,
            model_dir: dir.to_path_buf(),
            scrfd: None,
            faceocc: None,
            landmarks,
            blendshapes,
            blend_warning,
        })
    }
    pub fn prepare_detector(&mut self, settings: &AnalysisSettings) -> Result<()> {
        if settings.face_detector == FaceDetectorProvider::Scrfd500m {
            settings.validate()?;
            let hash = settings
                .scrfd_model_sha256
                .as_deref()
                .context("SCRFD hash missing")?;
            if !self
                .scrfd
                .as_ref()
                .is_some_and(|(loaded, _)| loaded.eq_ignore_ascii_case(hash))
            {
                let detector = super::scrfd::ScrfdDetector::load(&self.model_dir, settings)?;
                self.scrfd = Some((hash.to_ascii_lowercase(), detector));
            }
        }
        Ok(())
    }
    pub fn prepare_occlusion(&mut self, settings: &AnalysisSettings) -> Result<()> {
        if settings.occlusion_provider == OcclusionProvider::Faceocc {
            settings.validate()?;
            let hash = settings
                .occlusion_model_sha256
                .as_deref()
                .context("FaceOcc hash missing")?;
            if !self
                .faceocc
                .as_ref()
                .is_some_and(|(loaded, _)| loaded.eq_ignore_ascii_case(hash))
            {
                let model = super::faceocc::FaceOcc::load(&self.model_dir, settings)?;
                self.faceocc = Some((hash.to_ascii_lowercase(), model));
            }
        }
        Ok(())
    }
    pub fn faces(
        &mut self,
        image: &RgbImage,
        settings: &AnalysisSettings,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<Face>> {
        self.prepare_occlusion(settings)?;
        if settings.occlusion_provider == OcclusionProvider::Faceocc {
            warnings.push(format!("Eye visibility: FaceOcc visible-face mask; SHA-256 {}; explicit minimum visible fraction {}; no accuracy calibration", settings.occlusion_model_sha256.as_deref().unwrap_or_default(), settings.occlusion_min_visible_fraction.unwrap_or_default()));
        }
        if let Some(w) = &self.blend_warning {
            warnings.push(w.clone());
        }
        let detections = match settings.face_detector {
            FaceDetectorProvider::Yunet => self.detect(image, settings.face_confidence)?,
            FaceDetectorProvider::Scrfd500m => {
                self.prepare_detector(settings)?;
                let (hash, detector) = self.scrfd.as_mut().context("SCRFD not loaded")?;
                warnings.push(format!(
                    "Face detector: scrfd_500m_kps; SHA-256 {hash}; accuracy not validated"
                ));
                detector.detect(image, settings.face_confidence)?
            }
        };
        if detections.len() > settings.max_faces {
            warnings.push(format!(
                "Detected {} faces; only {} analyzed; omitted faces require review",
                detections.len(),
                settings.max_faces
            ));
        }
        let mut faces = Vec::new();
        for (index, d) in detections.into_iter().take(settings.max_faces).enumerate() {
            let mut face = Face {
                index,
                bbox: d.bbox,
                confidence: d.confidence,
                quality: None,
                quality_unavailable_reason: None,
                head_pose: None,
                left_eye: None,
                right_eye: None,
                left_eye_unreliable_reason: None,
                right_eye_unreliable_reason: None,
                left_eye_visibility: None,
                right_eye_visibility: None,
                smile_score: None,
                landmark_confidence: None,
                landmarks: Vec::new(),
                unreliable_reason: None,
            };
            if let Err(e) = self.landmark_face(image, &d, &mut face) {
                face.unreliable_reason = Some(format!("landmark inference failed: {e}"));
                warnings.push(format!("Face {index}: {e}"));
            }
            if [face.left_eye.as_ref(), face.right_eye.as_ref()]
                .into_iter()
                .flatten()
                .any(|eye| eye.blink_score.is_none())
            {
                warnings.push(format!(
                    "Face {index}: blendshape inference unavailable; EAR retained as measurement only, eye states uncertain"
                ));
            }
            if settings.occlusion_provider == OcclusionProvider::Faceocc {
                let (_, model) = self.faceocc.as_mut().context("FaceOcc not loaded")?;
                model.apply(
                    image,
                    &d,
                    &mut face,
                    settings
                        .occlusion_min_visible_fraction
                        .context("FaceOcc threshold missing")?,
                )?;
            }
            faces.push(face);
        }
        Ok(faces)
    }
    fn detect(&mut self, image: &RgbImage, threshold: f32) -> Result<Vec<Detection>> {
        // Letterbox preserves face geometry. YuNet expects unnormalized BGR NCHW.
        let scale = 640. / image.width().max(image.height()) as f32;
        let w = (image.width() as f32 * scale).round() as u32;
        let h = (image.height() as f32 * scale).round() as u32;
        let small = imageops::resize(image, w, h, FilterType::Triangle);
        let mut input = vec![0f32; 3 * 640 * 640];
        for (x, y, p) in small.enumerate_pixels() {
            let i = (y * 640 + x) as usize;
            for c in 0..3 {
                input[c * 640 * 640 + i] = f32::from(p[2 - c]);
            }
        }
        let outputs = self.yunet.run(ort::inputs![Tensor::from_array((
            [1usize, 3, 640, 640],
            input
        ))?])?;
        let mut candidates = Vec::new();
        for stride in [8usize, 16, 32] {
            let cls = outputs[format!("cls_{stride}").as_str()]
                .try_extract_tensor::<f32>()?
                .1;
            let obj = outputs[format!("obj_{stride}").as_str()]
                .try_extract_tensor::<f32>()?
                .1;
            let bbox = outputs[format!("bbox_{stride}").as_str()]
                .try_extract_tensor::<f32>()?
                .1;
            let kps = outputs[format!("kps_{stride}").as_str()]
                .try_extract_tensor::<f32>()?
                .1;
            let cols = 640 / stride;
            for i in 0..cols * cols {
                let score = (cls[i].clamp(0., 1.) * obj[i].clamp(0., 1.)).sqrt();
                if score < threshold {
                    continue;
                }
                let cx = (i % cols) as f32;
                let cy = (i / cols) as f32;
                let step = stride as f32 / scale;
                let bw = bbox[4 * i + 2].exp() * step;
                let bh = bbox[4 * i + 3].exp() * step;
                let x = (cx + bbox[4 * i]) * step - bw / 2.;
                let y = (cy + bbox[4 * i + 1]) * step - bh / 2.;
                if ![x, y, bw, bh].iter().all(|v| v.is_finite()) || bw < 8. || bh < 8. {
                    continue;
                }
                let x1 = x.clamp(0., image.width() as f32);
                let y1 = y.clamp(0., image.height() as f32);
                let x2 = (x + bw).clamp(0., image.width() as f32);
                let y2 = (y + bh).clamp(0., image.height() as f32);
                if x2 - x1 < 8. || y2 - y1 < 8. {
                    continue;
                }
                let mut keypoints = [[0.; 2]; 5];
                for j in 0..5 {
                    keypoints[j] = [
                        (cx + kps[i * 10 + j * 2]) * step,
                        (cy + kps[i * 10 + j * 2 + 1]) * step,
                    ];
                }
                candidates.push(Detection {
                    bbox: [x1, y1, x2 - x1, y2 - y1],
                    confidence: score,
                    keypoints,
                });
            }
        }
        candidates.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        candidates.truncate(5000);
        let mut keep: Vec<Detection> = Vec::new();
        for c in candidates {
            if keep.iter().all(|k| iou(&c.bbox, &k.bbox) < 0.3) {
                keep.push(c);
            }
        }
        Ok(keep)
    }
    fn landmark_face(&mut self, image: &RgbImage, d: &Detection, face: &mut Face) -> Result<()> {
        let [x, y, w, h] = d.bbox;
        let side = w.max(h) * 1.5;
        let cx = x + w / 2.;
        let cy = y + h / 2.;
        let mut eye_a = d.keypoints[0];
        let mut eye_b = d.keypoints[1];
        if eye_a[0] > eye_b[0] {
            std::mem::swap(&mut eye_a, &mut eye_b);
        }
        let angle = (eye_b[1] - eye_a[1]).atan2(eye_b[0] - eye_a[0]);
        let (cos, sin) = (angle.cos(), angle.sin());
        let mut input = vec![0f32; 256 * 256 * 3];
        let mut luminance = 0.;
        for row in 0..256 {
            for col in 0..256 {
                let u = (col as f32 + 0.5) / 256. * side - side / 2.;
                let v = (row as f32 + 0.5) / 256. * side - side / 2.;
                let sx = cx + u * cos - v * sin;
                let sy = cy + u * sin + v * cos;
                let p = sample(image, sx, sy);
                for c in 0..3 {
                    input[(row * 256 + col) * 3 + c] = p[c] / 255.;
                }
                luminance += 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
            }
        }
        luminance /= 65536.;
        let output = self.landmarks.run(ort::inputs![Tensor::from_array((
            [1usize, 256, 256, 3],
            input
        ))?])?;
        let points = output["Identity"].try_extract_tensor::<f32>()?.1;
        let logit = output["Identity_1"].try_extract_tensor::<f32>()?.1[0];
        let confidence = 1. / (1. + (-logit).exp());
        face.landmark_confidence = Some(confidence);
        if points.len() != 1434 || points.iter().any(|p| !p.is_finite()) {
            bail!("invalid landmark tensor");
        }
        if confidence < 0.8 {
            face.unreliable_reason = Some("low landmark presence confidence".into());
            return Ok(());
        }
        let local: Vec<[f32; 3]> = points.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
        face.landmarks = local
            .iter()
            .map(|p| {
                let u = (p[0] / 256. - 0.5) * side;
                let v = (p[1] / 256. - 0.5) * side;
                [
                    cx + u * cos - v * sin,
                    cy + u * sin + v * cos,
                    p[2] / 256. * side,
                ]
            })
            .collect();
        // A geometric screen-space proxy, not calibrated camera pose estimation.
        let horizontal = sub(local[263], local[33]);
        let vertical = sub(local[152], local[10]);
        let n = cross(horizontal, vertical);
        let yaw = n[0].atan2(n[2].abs()).to_degrees();
        let pitch = n[1].atan2(n[2].abs()).to_degrees();
        face.head_pose = Some(HeadPose {
            yaw,
            pitch,
            roll: angle.to_degrees(),
            method: "landmark_plane_proxy".into(),
        });
        let unreliable = if w.min(h) < 64. {
            Some("small face (<64 preview pixels)")
        } else if luminance < 45. {
            Some("low face luminance")
        } else if yaw.abs() > 35. || pitch.abs() > 35. || angle.to_degrees().abs() > 40. {
            Some("oblique head pose")
        } else {
            None
        };
        if let Some(reason) = unreliable {
            face.unreliable_reason = Some(reason.into());
            return Ok(());
        }
        let blend = if let Some(model) = &mut self.blendshapes {
            let mut inputs = Vec::with_capacity(292);
            for i in SUBSET {
                inputs.extend_from_slice(&face.landmarks[i][..2]);
            }
            match model.run(ort::inputs![Tensor::from_array((
                [1usize, 146, 2],
                inputs
            ))?]) {
                Ok(out) => {
                    let vals = out[0].try_extract_tensor::<f32>()?.1;
                    if vals.len() == 52 && vals.iter().all(|v| v.is_finite()) {
                        Some(vals.to_vec())
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        };
        let left_ear = ear(&local, [362, 385, 387, 263, 373, 380]);
        let right_ear = ear(&local, [33, 160, 158, 133, 153, 144]);
        let left_blink = blend.as_ref().map(|b| b[9]);
        let right_blink = blend.as_ref().map(|b| b[10]);
        // Neither landmarks nor blendshapes provide per-eye visibility. These
        // measured ROI gates suppress unobservable eyes; they do not recognize
        // general occlusions (e.g. textured or skin-coloured obstructions).
        face.left_eye_unreliable_reason = super::eye_quality::rejection_reason(
            image,
            &face.landmarks,
            [362, 385, 387, 263, 373, 380],
            left_ear,
        );
        face.right_eye_unreliable_reason = super::eye_quality::rejection_reason(
            image,
            &face.landmarks,
            [33, 160, 158, 133, 153, 144],
            right_ear,
        );
        face.left_eye = face.left_eye_unreliable_reason.is_none().then_some(Eye {
            state: classify_eye(left_blink, left_ear),
            blink_score: left_blink,
            ear: left_ear,
        });
        face.right_eye = face.right_eye_unreliable_reason.is_none().then_some(Eye {
            state: classify_eye(right_blink, right_ear),
            blink_score: right_blink,
            ear: right_ear,
        });
        face.smile_score = blend.map(|b| (b[44] + b[45]) / 2.);
        Ok(())
    }
}
pub(super) fn sample(image: &RgbImage, x: f32, y: f32) -> [f32; 3] {
    let x = x.clamp(0., image.width().saturating_sub(1) as f32);
    let y = y.clamp(0., image.height().saturating_sub(1) as f32);
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (x1, y1) = (
        (x0 + 1).min(image.width() - 1),
        (y0 + 1).min(image.height() - 1),
    );
    let (dx, dy) = (x - x0 as f32, y - y0 as f32);
    let mut p = [0.; 3];
    for c in 0..3 {
        p[c] = f32::from(image.get_pixel(x0, y0)[c]) * (1. - dx) * (1. - dy)
            + f32::from(image.get_pixel(x1, y0)[c]) * dx * (1. - dy)
            + f32::from(image.get_pixel(x0, y1)[c]) * (1. - dx) * dy
            + f32::from(image.get_pixel(x1, y1)[c]) * dx * dy;
    }
    p
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn ear(p: &[[f32; 3]], i: [usize; 6]) -> f32 {
    let distance =
        |a: usize, b: usize| ((p[a][0] - p[b][0]).powi(2) + (p[a][1] - p[b][1]).powi(2)).sqrt();
    (distance(i[1], i[5]) + distance(i[2], i[4])) / (2. * distance(i[0], i[3]).max(1e-6))
}
pub(super) fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let w = ((a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0])).max(0.);
    let h = ((a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1])).max(0.);
    let overlap = w * h;
    overlap / (a[2] * a[3] + b[2] * b[3] - overlap).max(1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nms_intersection() {
        assert!((iou(&[0., 0., 10., 10.], &[0., 0., 10., 10.]) - 1.).abs() < 1e-6);
        assert_eq!(iou(&[0., 0., 10., 10.], &[20., 0., 10., 10.]), 0.);
    }

    /// Artifact/runtime compatibility only; synthetic inputs make no accuracy claim.
    #[test]
    #[ignore = "requires bundled model assets and ONNX Runtime; run after tools/setup-models.py"]
    fn bundled_onnx_tensor_smoke() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
        let mut models = Models::load(&root)?;
        let detector_input: Vec<f32> = (0..3 * 640 * 640).map(|i| (i % 251) as f32).collect();
        let output = models.yunet.run(ort::inputs![Tensor::from_array((
            [1usize, 3, 640, 640],
            detector_input
        ))?])?;
        for stride in [8usize, 16, 32] {
            let count = (640 / stride).pow(2);
            for (prefix, channels) in [("cls", 1usize), ("obj", 1), ("bbox", 4), ("kps", 10)] {
                let name = format!("{prefix}_{stride}");
                let (shape, values) = output[name.as_str()].try_extract_tensor::<f32>()?;
                assert_eq!(
                    &shape[..],
                    &[1, count as i64, channels as i64],
                    "YuNet output {name}"
                );
                assert!(values.iter().all(|v| v.is_finite()), "YuNet output {name}");
            }
        }
        drop(output);
        let landmarks_input: Vec<f32> = (0..256 * 256 * 3)
            .map(|i| (i % 251) as f32 / 250.)
            .collect();
        let output = models.landmarks.run(ort::inputs![Tensor::from_array((
            [1usize, 256, 256, 3],
            landmarks_input
        ))?])?;
        let (shape, points) = output["Identity"].try_extract_tensor::<f32>()?;
        assert_eq!(&shape[..], &[1, 1, 1, 1434]);
        assert!(points.iter().all(|v| v.is_finite()));
        for (name, expected) in [("Identity_1", vec![1, 1, 1, 1]), ("Identity_2", vec![1, 1])] {
            let (shape, values) = output[name].try_extract_tensor::<f32>()?;
            assert_eq!(&shape[..], expected.as_slice());
            assert!(values.iter().all(|v| v.is_finite()));
        }
        let mut blend_input = Vec::with_capacity(292);
        for index in SUBSET {
            blend_input.extend_from_slice(&points[index * 3..index * 3 + 2]);
        }
        drop(output);
        let blendshapes = models
            .blendshapes
            .as_mut()
            .context("bundled blendshape session must load")?;
        let output = blendshapes.run(ort::inputs![Tensor::from_array((
            [1usize, 146, 2],
            blend_input
        ))?])?;
        let (shape, values) = output[0].try_extract_tensor::<f32>()?;
        assert_eq!(&shape[..], &[52]);
        assert!(values
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        println!("All three pinned ONNX sessions executed with expected shapes and finite outputs");
        Ok(())
    }
}
