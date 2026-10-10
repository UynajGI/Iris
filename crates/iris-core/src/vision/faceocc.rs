//! Optional visible-face segmentation. Pixel mask readings are not calibrated eye visibility.
use super::{
    models::Detection, AnalysisSettings, Eye, EyeVisibility, Face, ModelAvailability,
    OcclusionProvider,
};
use anyhow::{bail, Context, Result};
use image::RgbImage;
use ort::{
    session::Session,
    tensor::TensorElementType,
    value::{Tensor, ValueType},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use utoipa::ToSchema;

const SIDE: usize = 256;
const MODEL: &str = "optional/faceocc.onnx";
const METADATA: &str = "optional/faceocc.metadata.json";
const METHOD: &str = "faceocc_visible_face_v1_corner_roi_1.2x0.5_mask_p0.5";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OcclusionModelStatus {
    pub provider: OcclusionProvider,
    pub state: ModelAvailability,
    /// Actual hash of read bytes. Available only describes artifact checks, not inference or licensing.
    pub sha256: Option<String>,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    model_id: String,
    source_url: String,
    license_url: String,
    license_note: String,
}

fn inspect(dir: &Path, settings: &AnalysisSettings) -> (OcclusionModelStatus, Option<Vec<u8>>) {
    let mut status = OcclusionModelStatus {
        provider: settings.occlusion_provider,
        state: ModelAvailability::Invalid,
        sha256: None,
        reason: None,
    };
    if settings.occlusion_provider == OcclusionProvider::None {
        status.state = ModelAvailability::Disabled;
        status.reason = Some(
            "Occlusion provider disabled; FaceOcc artifacts and inference have not been checked"
                .into(),
        );
        return (status, None);
    }
    let result = (|| -> Result<Vec<u8>> {
        settings.validate()?;
        let path = crate::model_paths::checked_model_path(dir, MODEL)?;
        if !path.exists() {
            status.state = ModelAvailability::Missing;
            bail!("FaceOcc model missing: {}", path.display());
        }
        let bytes = super::scrfd::read_bounded(&path, 128 * 1024 * 1024)?;
        let hash = format!("{:x}", Sha256::digest(&bytes));
        status.sha256 = Some(hash.clone());
        if !hash.eq_ignore_ascii_case(
            settings
                .occlusion_model_sha256
                .as_deref()
                .context("FaceOcc SHA-256 missing")?,
        ) {
            bail!("FaceOcc SHA-256 mismatch; actual {hash}");
        }
        let path = crate::model_paths::checked_model_path(dir, METADATA)?;
        if !path.exists() {
            status.state = ModelAvailability::Missing;
            bail!("FaceOcc metadata missing: {}", path.display());
        }
        let metadata: Metadata =
            serde_json::from_slice(&super::scrfd::read_bounded(&path, 64 * 1024)?)
                .context("invalid FaceOcc metadata JSON")?;
        if metadata.model_id != "faceocc_visible_face_v1"
            || !metadata.source_url.starts_with("https://")
            || metadata.source_url.len() <= 8
            || !metadata.license_url.starts_with("https://")
            || metadata.license_url.len() <= 8
            || metadata.license_note.trim().is_empty()
        {
            bail!("FaceOcc metadata requires model_id faceocc_visible_face_v1, HTTPS source_url/license_url and nonempty license_note; declarations are not authorization");
        }
        Ok(bytes)
    })();
    match result {
        Ok(bytes) => {
            status.state = ModelAvailability::Available;
            status.reason = Some("Disk bytes, declared SHA-256 and metadata checks passed only; graph/output checks occur when selected and loaded. No accuracy validation. License metadata is an operator declaration, not authorization.".into());
            (status, Some(bytes))
        }
        Err(error) => {
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            {
                status.state = ModelAvailability::Missing;
            }
            status.reason = Some(format!("{error:#}"));
            (status, None)
        }
    }
}

pub fn occlusion_model_status(dir: &Path, settings: &AnalysisSettings) -> OcclusionModelStatus {
    inspect(dir, settings).0
}
pub fn validate_occlusion_model(dir: &Path, settings: &AnalysisSettings) -> Result<()> {
    settings.validate()?;
    if settings.occlusion_provider == OcclusionProvider::None {
        return Ok(());
    }
    let (status, _) = inspect(dir, settings);
    if status.state != ModelAvailability::Available {
        bail!("FaceOcc unavailable: {}", status.reason.unwrap_or_default());
    }
    Ok(())
}

pub(super) struct FaceOcc {
    session: Session,
}
impl FaceOcc {
    pub(super) fn load(dir: &Path, settings: &AnalysisSettings) -> Result<Self> {
        let (status, bytes) = inspect(dir, settings);
        let bytes = bytes.with_context(|| {
            format!("FaceOcc unavailable: {}", status.reason.unwrap_or_default())
        })?;
        let session = super::runtime::session(&bytes, "faceocc")
            .context("FaceOcc load failed; no fallback")?;
        if session.inputs.len() != 1
            || session.outputs.len() != 1
            || session.inputs[0].name != "rgb_0_1"
            || session.outputs[0].name != "visible_face_logits"
        {
            bail!("FaceOcc requires rgb_0_1 input and visible_face_logits output; no fallback");
        }
        for (dtype, expected) in [
            (&session.inputs[0].input_type, [1, 3, 256, 256]),
            (&session.outputs[0].output_type, [1, 1, 256, 256]),
        ] {
            if !matches!(dtype, ValueType::Tensor { ty: TensorElementType::Float32, shape, .. } if &shape[..] == expected)
            {
                bail!("FaceOcc requires fixed float32 tensor shape {expected:?}, got {dtype:?}");
            }
        }
        let mut model = Self { session };
        // Always validate execution before photos, including no-face/all-withheld batches.
        model
            .run(vec![0.5; 3 * SIDE * SIDE])
            .context("FaceOcc fixed tensor validation failed; no fallback")?;
        Ok(model)
    }

    fn run(&mut self, input: Vec<f32>) -> Result<Vec<f32>> {
        if input.len() != 3 * SIDE * SIDE
            || input
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            bail!("invalid FaceOcc RGB input");
        }
        // The pinned research ONNX graph already contains ImageNet normalization.
        let output = self
            .session
            .run(ort::inputs![Tensor::from_array((
                [1usize, 3, SIDE, SIDE],
                input
            ))?])
            .context("FaceOcc inference failed; no fallback")?;
        if output.len() != 1 {
            bail!("FaceOcc must return one tensor");
        }
        let (shape, values) = output[0].try_extract_tensor::<f32>()?;
        validate_logits(&shape[..], values)?;
        Ok(values.to_vec())
    }

    pub(super) fn apply(
        &mut self,
        image: &RgbImage,
        detection: &Detection,
        face: &mut Face,
        minimum: f32,
    ) -> Result<()> {
        let crop = match Crop::new(detection) {
            Ok(crop) => crop,
            Err(error) => {
                // Unusable detector geometry is a face observation failure. The
                // selected model has already executed its startup validation.
                // Keep model load/run failures distinct from unavailable ROIs.
                let message = format!("face crop unavailable: {error}");
                apply_observation(
                    &mut face.left_eye,
                    &mut face.left_eye_unreliable_reason,
                    &mut face.left_eye_visibility,
                    Err(anyhow::anyhow!(message.clone())),
                    minimum,
                );
                apply_observation(
                    &mut face.right_eye,
                    &mut face.right_eye_unreliable_reason,
                    &mut face.right_eye_visibility,
                    Err(anyhow::anyhow!(message)),
                    minimum,
                );
                return Ok(());
            }
        };
        let logits = self.run(crop.input(image)?)?;
        let left = observe(
            &logits,
            &crop,
            image.dimensions(),
            &face.landmarks,
            [362, 263],
        );
        let right = observe(
            &logits,
            &crop,
            image.dimensions(),
            &face.landmarks,
            [33, 133],
        );
        apply_observation(
            &mut face.left_eye,
            &mut face.left_eye_unreliable_reason,
            &mut face.left_eye_visibility,
            left,
            minimum,
        );
        apply_observation(
            &mut face.right_eye,
            &mut face.right_eye_unreliable_reason,
            &mut face.right_eye_visibility,
            right,
            minimum,
        );
        Ok(())
    }
}

fn validate_logits(shape: &[i64], values: &[f32]) -> Result<()> {
    if shape != [1, 1, 256, 256]
        || values.len() != SIDE * SIDE
        || values.iter().any(|v| !v.is_finite())
    {
        bail!("FaceOcc output must be finite float32 [1,1,256,256] logits");
    }
    Ok(())
}

/// Same face-box expansion and detector-eye roll as the landmark crop; sampling uses black padding.
struct Crop {
    center: [f32; 2],
    side: f32,
    cos: f32,
    sin: f32,
}
impl Crop {
    fn new(detection: &Detection) -> Result<Self> {
        let [x, y, w, h] = detection.bbox;
        let mut a = detection.keypoints[0];
        let mut b = detection.keypoints[1];
        if [x, y, w, h, a[0], a[1], b[0], b[1]]
            .iter()
            .any(|v| !v.is_finite())
            || w <= 0.
            || h <= 0.
        {
            bail!("FaceOcc invalid face crop geometry");
        }
        if a[0] > b[0] {
            std::mem::swap(&mut a, &mut b);
        }
        if (b[0] - a[0]).hypot(b[1] - a[1]) < 1. {
            bail!("FaceOcc detector eye alignment is degenerate");
        }
        let angle = (b[1] - a[1]).atan2(b[0] - a[0]);
        let (sin, cos) = angle.sin_cos();
        Ok(Self {
            center: [x + w / 2., y + h / 2.],
            side: w.max(h) * 1.5,
            cos,
            sin,
        })
    }
    fn world(&self, p: [f32; 2]) -> [f32; 2] {
        let u = (p[0] / SIDE as f32 - 0.5) * self.side;
        let v = (p[1] / SIDE as f32 - 0.5) * self.side;
        [
            self.center[0] + u * self.cos - v * self.sin,
            self.center[1] + u * self.sin + v * self.cos,
        ]
    }
    fn local(&self, p: [f32; 2]) -> [f32; 2] {
        let x = p[0] - self.center[0];
        let y = p[1] - self.center[1];
        [
            ((x * self.cos + y * self.sin) / self.side + 0.5) * SIDE as f32,
            ((-x * self.sin + y * self.cos) / self.side + 0.5) * SIDE as f32,
        ]
    }
    fn input(&self, image: &RgbImage) -> Result<Vec<f32>> {
        if image.width() == 0 || image.height() == 0 {
            bail!("FaceOcc image is empty");
        }
        let mut input = vec![0.; 3 * SIDE * SIDE];
        for y in 0..SIDE {
            for x in 0..SIDE {
                let p = self.world([x as f32 + 0.5, y as f32 + 0.5]);
                if p[0] < 0.
                    || p[1] < 0.
                    || p[0] > image.width() as f32 - 1.
                    || p[1] > image.height() as f32 - 1.
                {
                    continue;
                }
                let pixel = super::models::sample(image, p[0], p[1]);
                for c in 0..3 {
                    // Bilinear interpolation is bounded mathematically, but f32
                    // rounding can put a white/highlight pixel just above 255.
                    input[c * SIDE * SIDE + y * SIDE + x] = (pixel[c] / 255.).clamp(0., 1.);
                }
            }
        }
        Ok(input)
    }
}

fn observe(
    logits: &[f32],
    crop: &Crop,
    size: (u32, u32),
    points: &[[f32; 3]],
    corners: [usize; 2],
) -> Result<EyeVisibility> {
    let a = points
        .get(corners[0])
        .context("eye-corner landmarks missing")?;
    let b = points
        .get(corners[1])
        .context("eye-corner landmarks missing")?;
    if a.iter().chain(b).any(|v| !v.is_finite()) {
        bail!("eye-corner landmarks non-finite");
    }
    let span = (b[0] - a[0]).hypot(b[1] - a[1]);
    if span < 8. {
        bail!("eye-corner span below 8 preview pixels");
    }
    let a = crop.local([a[0], a[1]]);
    let b = crop.local([b[0], b[1]]);
    let span = (b[0] - a[0]).hypot(b[1] - a[1]);
    let u = [(b[0] - a[0]) / span, (b[1] - a[1]) / span];
    let v = [-u[1], u[0]];
    let center = [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.];
    // Width 1.2*corner span and height 0.5*span, independent of eyelid aperture/EAR.
    let half_w = span * 0.6;
    let half_h = span * 0.25;
    let mut polygon = Vec::with_capacity(4);
    for sx in [-1., 1.] {
        for sy in [-1., 1.] {
            let p = [
                center[0] + sx * half_w * u[0] + sy * half_h * v[0],
                center[1] + sx * half_w * u[1] + sy * half_h * v[1],
            ];
            let world = crop.world(p);
            if p.iter()
                .any(|v| !v.is_finite() || *v < 0. || *v > SIDE as f32)
                || world[0] < 0.
                || world[1] < 0.
                || world[0] > size.0 as f32 - 1.
                || world[1] > size.1 as f32 - 1.
            {
                bail!("eye ROI crosses crop or image boundary");
            }
            polygon.push(p);
        }
    }
    let min_x = polygon
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min)
        .floor() as usize;
    let max_x = polygon.iter().map(|p| p[0]).fold(0., f32::max).ceil() as usize;
    let min_y = polygon
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min)
        .floor() as usize;
    let max_y = polygon.iter().map(|p| p[1]).fold(0., f32::max).ceil() as usize;
    let mut count = 0usize;
    let mut visible = 0usize;
    let mut probability = 0f64;
    for y in min_y..max_y.min(SIDE) {
        for x in min_x..max_x.min(SIDE) {
            let dx = x as f32 + 0.5 - center[0];
            let dy = y as f32 + 0.5 - center[1];
            if (dx * u[0] + dy * u[1]).abs() > half_w || (dx * v[0] + dy * v[1]).abs() > half_h {
                continue;
            }
            let logit = *logits
                .get(y * SIDE + x)
                .context("FaceOcc mask size mismatch")?;
            if !logit.is_finite() {
                bail!("FaceOcc mask non-finite");
            }
            count += 1;
            visible += usize::from(logit >= 0.);
            probability += 1. / (1. + (-(logit as f64)).exp());
        }
    }
    if count < 4 {
        bail!("eye ROI contains fewer than four model pixels");
    }
    Ok(EyeVisibility {
        visible_fraction: visible as f32 / count as f32,
        mean_probability: (probability / count as f64) as f32,
        sampled_pixels: count,
        method: METHOD.into(),
    })
}

fn apply_observation(
    eye: &mut Option<Eye>,
    reason: &mut Option<String>,
    reading: &mut Option<EyeVisibility>,
    result: Result<EyeVisibility>,
    minimum: f32,
) {
    match result {
        Ok(value) => {
            if value.visible_fraction < minimum {
                *eye = None;
                if reason.is_none() {
                    *reason=Some(format!("FaceOcc visible-face fraction {:.4} below explicit threshold {:.4}; uncalibrated mask measurement",value.visible_fraction,minimum));
                }
            }
            *reading = Some(value);
        }
        Err(error) => {
            *eye = None;
            *reading = None;
            if reason.is_none() {
                *reason = Some(format!("FaceOcc eye ROI unavailable: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::EyeState;
    use super::*;

    fn settings(hash: String) -> AnalysisSettings {
        AnalysisSettings {
            occlusion_provider: OcclusionProvider::Faceocc,
            occlusion_model_sha256: Some(hash),
            occlusion_min_visible_fraction: Some(0.5),
            ..Default::default()
        }
    }
    fn metadata(dir: &Path) {
        std::fs::write(dir.join(METADATA), br#"{"model_id":"faceocc_visible_face_v1","source_url":"https://huggingface.co/mertakin/FaceOcc","license_url":"https://huggingface.co/mertakin/FaceOcc/blob/03f229dc75fa14ae480cca9810f983912c2730ad/LICENSE","license_note":"Operator declaration for local research, not authorization proof"}"#).unwrap();
    }
    fn identity_crop() -> Crop {
        Crop {
            center: [128., 128.],
            side: 256.,
            cos: 1.,
            sin: 0.,
        }
    }
    fn points() -> Vec<[f32; 3]> {
        let mut points = vec![[128., 128., 0.]; 478];
        points[362] = [170., 128., 0.];
        points[263] = [190., 128., 0.];
        points[33] = [65., 128., 0.];
        points[133] = [85., 128., 0.];
        points
    }
    fn open_eye() -> Option<Eye> {
        Some(Eye {
            state: EyeState::Open,
            blink_score: Some(0.1),
            ear: 0.3,
        })
    }

    #[test]
    fn configuration_requires_explicit_hash_and_threshold() {
        let default = AnalysisSettings::default();
        assert_eq!(default.occlusion_provider, OcclusionProvider::None);
        let json = serde_json::to_value(&default).unwrap();
        assert!(json.get("occlusion_model_sha256").is_none());
        assert!(json.get("occlusion_min_visible_fraction").is_none());
        let mut selected = settings("a".repeat(64));
        selected.validate().unwrap();
        selected.occlusion_min_visible_fraction = None;
        assert!(selected.validate().is_err());
        for value in [0., -1., 1.01, f32::NAN, f32::INFINITY] {
            selected.occlusion_min_visible_fraction = Some(value);
            assert!(selected.validate().is_err());
        }
        selected.occlusion_min_visible_fraction = Some(1.);
        selected.validate().unwrap();
        selected.occlusion_model_sha256 = Some("z".repeat(64));
        assert!(selected.validate().is_err());
        selected.occlusion_model_sha256 = None;
        assert!(selected.validate().is_err());
    }

    #[test]
    fn artifact_checks_are_bounded_and_do_not_claim_model_execution() {
        let dir = tempfile::tempdir().unwrap();
        let default = AnalysisSettings::default();
        assert_eq!(
            occlusion_model_status(dir.path(), &default).state,
            ModelAvailability::Disabled
        );
        assert!(validate_occlusion_model(dir.path(), &default).is_ok());
        let mut selected = settings("0".repeat(64));
        assert_eq!(
            occlusion_model_status(dir.path(), &selected).state,
            ModelAvailability::Missing
        );
        std::fs::create_dir(dir.path().join("optional")).unwrap();
        std::fs::write(dir.path().join(MODEL), b"not a model").unwrap();
        let status = occlusion_model_status(dir.path(), &selected);
        assert_eq!(status.state, ModelAvailability::Invalid);
        selected.occlusion_model_sha256 = status.sha256;
        assert_eq!(
            occlusion_model_status(dir.path(), &selected).state,
            ModelAvailability::Missing
        );
        metadata(dir.path());
        let status = occlusion_model_status(dir.path(), &selected);
        assert_eq!(status.state, ModelAvailability::Available);
        assert!(status.reason.unwrap().contains("operator declaration"));
        std::fs::write(dir.path().join(METADATA), vec![b' '; 65537]).unwrap();
        assert_eq!(
            occlusion_model_status(dir.path(), &selected).state,
            ModelAvailability::Invalid
        );
        assert!(validate_occlusion_model(dir.path(), &selected).is_err());
    }

    #[test]
    fn left_and_right_measurements_are_independent_and_aperture_invariant() {
        let crop = identity_crop();
        let logits: Vec<f32> = (0..SIDE * SIDE)
            .map(|i| if i % SIDE > 128 { 2. } else { -2. })
            .collect();
        let mut landmarks = points();
        let left = observe(&logits, &crop, (256, 256), &landmarks, [362, 263]).unwrap();
        let right = observe(&logits, &crop, (256, 256), &landmarks, [33, 133]).unwrap();
        assert_eq!(left.visible_fraction, 1.);
        assert_eq!(right.visible_fraction, 0.);
        assert_eq!(left.sampled_pixels, 240);
        assert_eq!(right.sampled_pixels, 240);
        assert!((left.mean_probability - 0.8807971).abs() < 1e-6);
        // Collapse all upper/lower lid points to a line. Corner-based ROI stays identical.
        for i in [385, 387, 373, 380, 160, 158, 153, 144] {
            landmarks[i] = [180., 128., 0.];
        }
        let closed = observe(&logits, &crop, (256, 256), &landmarks, [362, 263]).unwrap();
        assert_eq!(closed.sampled_pixels, left.sampled_pixels);
        assert_eq!(closed.visible_fraction, left.visible_fraction);
        let mut eye = open_eye();
        let mut reason = None;
        let mut reading = None;
        apply_observation(&mut eye, &mut reason, &mut reading, Ok(right), 0.5);
        assert!(eye.is_none());
        assert!(reason.unwrap().contains("explicit threshold"));
        assert_eq!(reading.unwrap().visible_fraction, 0.);
        let mut eye = open_eye();
        let mut reason = None;
        let mut reading = None;
        apply_observation(&mut eye, &mut reason, &mut reading, Ok(left), 0.5);
        assert!(eye.is_some());
        assert!(reason.is_none());
    }

    #[test]
    fn missing_or_boundary_roi_has_no_fake_reading_and_never_restores_hidden_eye() {
        let crop = identity_crop();
        let logits = vec![8.; SIDE * SIDE];
        assert!(observe(&logits, &crop, (256, 256), &[], [33, 133]).is_err());
        let mut landmarks = points();
        landmarks[33] = [2., 2., 0.];
        landmarks[133] = [22., 2., 0.];
        let result = observe(&logits, &crop, (256, 256), &landmarks, [33, 133]);
        assert!(result.is_err());
        let mut eye = open_eye();
        let mut reason = None;
        let mut reading = None;
        apply_observation(&mut eye, &mut reason, &mut reading, result, 0.5);
        assert!(eye.is_none());
        assert!(reading.is_none());
        assert!(reason.unwrap().contains("boundary"));
        let mut eye = None;
        let mut reason = Some("existing low-light quality gate".into());
        let mut reading = None;
        let high = observe(&logits, &crop, (256, 256), &points(), [362, 263]);
        apply_observation(&mut eye, &mut reason, &mut reading, high, 0.5);
        assert!(eye.is_none());
        assert_eq!(reason.as_deref(), Some("existing low-light quality gate"));
        assert_eq!(reading.unwrap().visible_fraction, 1.);
    }

    #[test]
    fn crop_roll_round_trips_and_input_is_rgb_zero_one_with_black_padding() {
        let detection = Detection {
            bbox: [40., 50., 80., 60.],
            confidence: 0.9,
            keypoints: [[60., 70.], [100., 90.], [0., 0.], [0., 0.], [0., 0.]],
        };
        let crop = Crop::new(&detection).unwrap();
        for p in [[0., 0.], [125.5, 64.5], [256., 256.]] {
            let back = crop.local(crop.world(p));
            assert!((back[0] - p[0]).abs() < 1e-4 && (back[1] - p[1]).abs() < 1e-4);
        }
        let crop = Crop {
            center: [0., 0.],
            side: 256.,
            cos: 1.,
            sin: 0.,
        };
        let image = RgbImage::from_pixel(256, 256, image::Rgb([255, 128, 0]));
        let input = crop.input(&image).unwrap();
        assert_eq!(input[0], 0.);
        assert_eq!(input[SIDE * SIDE], 0.);
        let i = 200 * SIDE + 200;
        assert_eq!(input[i], 1.);
        assert_eq!(input[SIDE * SIDE + i], 128. / 255.);
        assert_eq!(input[2 * SIDE * SIDE + i], 0.);
    }

    #[test]
    fn output_contract_rejects_bad_shapes_and_nonfinite_values() {
        let mut values = vec![0.; SIDE * SIDE];
        validate_logits(&[1, 1, 256, 256], &values).unwrap();
        assert!(validate_logits(&[1, 256, 256], &values).is_err());
        values[7] = f32::NAN;
        assert!(validate_logits(&[1, 1, 256, 256], &values).is_err());
    }

    #[test]
    fn rotated_highlights_remain_in_rgb_zero_one_despite_interpolation_roundoff() {
        let image = RgbImage::from_fn(256, 256, |x, y| {
            if (x + y) % 2 == 0 {
                image::Rgb([255, 255, 255])
            } else {
                image::Rgb([255, 254, 253])
            }
        });
        for angle in [0.123f32, 0.48, 1.03] {
            let (sin, cos) = angle.sin_cos();
            let crop = Crop {
                center: [127.13, 131.77],
                side: 201.73,
                cos,
                sin,
            };
            let values = crop.input(&image).unwrap();
            assert!(values
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        }
    }

    #[test]
    #[ignore = "requires locally acquired FaceOcc research ONNX and bundled runtime; no downloads"]
    fn research_faceocc_onnx_tensor_smoke() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        #[cfg(windows)]
        ort::init_from(root.join("models/onnxruntime.dll").to_string_lossy()).commit()?;
        let dir = tempfile::tempdir()?;
        std::fs::create_dir(dir.path().join("optional"))?;
        std::fs::copy(
            root.join("artifacts/faceocc-research/faceocc-research.onnx"),
            dir.path().join(MODEL),
        )?;
        metadata(dir.path());
        let selected =
            settings("e61151ef3be24948a2d45ba870a434fa3ce6b4c1b2d2dab90eb80a4fd635ea86".into());
        let mut model = FaceOcc::load(dir.path(), &selected)?;
        for value in [0., 0.5, 1.] {
            let output = model.run(vec![value; 3 * SIDE * SIDE])?;
            assert_eq!(output.len(), SIDE * SIDE);
            assert!(output.iter().all(|v| v.is_finite()));
        }
        let mut face = Face {
            index: 0,
            bbox: [50., 50., 100., 100.],
            confidence: 0.9,
            quality: None,
            quality_unavailable_reason: None,
            head_pose: None,
            left_eye: open_eye(),
            right_eye: open_eye(),
            left_eye_unreliable_reason: None,
            right_eye_unreliable_reason: Some("previous quality gate".into()),
            left_eye_visibility: None,
            right_eye_visibility: None,
            smile_score: None,
            landmark_confidence: Some(0.99),
            landmarks: points(),
            unreliable_reason: None,
        };
        let degenerate = Detection {
            bbox: face.bbox,
            confidence: 0.9,
            keypoints: [[100., 100.]; 5],
        };
        model.apply(&RgbImage::new(256, 256), &degenerate, &mut face, 0.5)?;
        assert!(face.left_eye.is_none() && face.right_eye.is_none());
        assert!(face.left_eye_visibility.is_none() && face.right_eye_visibility.is_none());
        assert!(face
            .left_eye_unreliable_reason
            .as_deref()
            .unwrap()
            .contains("degenerate"));
        assert_eq!(
            face.right_eye_unreliable_reason.as_deref(),
            Some("previous quality gate")
        );
        let detection = Detection {
            bbox: face.bbox,
            confidence: 0.9,
            keypoints: [
                [80., 80.],
                [120., 90.],
                [100., 110.],
                [90., 130.],
                [110., 130.],
            ],
        };
        let crop = Crop::new(&detection)?;
        let highlights = RgbImage::from_pixel(256, 256, image::Rgb([255, 255, 255]));
        assert!(model.run(crop.input(&highlights)?).is_ok());
        // Correct hash for incompatible bytes is insufficient: loading must still fail.
        std::fs::write(dir.path().join(MODEL), b"not an ONNX graph")?;
        let bad = settings(format!("{:x}", Sha256::digest(b"not an ONNX graph")));
        assert!(FaceOcc::load(dir.path(), &bad).is_err());
        println!("Pinned FaceOcc ONNX executed on fixed RGB tensors; malformed graph rejected; no accuracy claim");
        Ok(())
    }
}
