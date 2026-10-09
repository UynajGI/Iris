//! Optional SCRFD-500M KPS adapter. No pretrained weights are distributed here.
use super::{
    models::{iou, Detection},
    AnalysisSettings, FaceDetectorProvider,
};
use anyhow::{bail, Context, Result};
use image::{imageops, RgbImage};
use ort::{session::Session, value::Tensor};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
use utoipa::ToSchema;

const MODEL: &str = "optional/scrfd_500m.onnx";
const METADATA: &str = "optional/scrfd_500m.metadata.json";
const YUNET_HASH: &str = "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4";
const SIDE: usize = 640;
const STRIDES: [usize; 3] = [8, 16, 32];
const OUTPUTS: [&str; 9] = [
    "score_8", "score_16", "score_32", "bbox_8", "bbox_16", "bbox_32", "kps_8", "kps_16", "kps_32",
];
// Original det_500m.onnx from the official InsightFace v0.7 buffalo_sc release.
// Keep its bytes intact: these names identify the same ordered stride heads.
const OFFICIAL_OUTPUTS: [&str; 9] = [
    "443", "468", "493", "446", "471", "496", "449", "474", "499",
];

fn validate_output_names(names: &[&str]) -> Result<()> {
    if names != OUTPUTS && names != OFFICIAL_OUTPUTS {
        bail!("incompatible SCRFD output schema: expected one complete ordered KPS schema; no fallback");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelAvailability {
    Disabled,
    Available,
    Missing,
    Invalid,
}

/// `available` means file/hash/metadata checks passed, not inference or accuracy validation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DetectorModelStatus {
    pub provider: FaceDetectorProvider,
    pub state: ModelAvailability,
    /// Actual SHA-256 of bytes read; null when no model bytes could be inspected.
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

pub(super) fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .with_context(|| format!("model file missing or unreadable: {}", path.display()))?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("model file exceeds size limit: {}", path.display());
    }
    Ok(bytes)
}

fn inspect(dir: &Path, settings: &AnalysisSettings) -> (DetectorModelStatus, Option<Vec<u8>>) {
    let mut status = DetectorModelStatus {
        provider: settings.face_detector,
        state: ModelAvailability::Invalid,
        sha256: None,
        reason: None,
    };
    let result = (|| -> Result<Vec<u8>> {
        settings.validate()?;
        let is_scrfd = settings.face_detector == FaceDetectorProvider::Scrfd500m;
        let path = dir.join(if is_scrfd { MODEL } else { "yunet.onnx" });
        if !path.exists() {
            status.state = ModelAvailability::Missing;
            bail!("model missing: {}", path.display());
        }
        let bytes = read_bounded(&path, 128 * 1024 * 1024)?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        status.sha256 = Some(actual.clone());
        let expected = if is_scrfd {
            settings
                .scrfd_model_sha256
                .as_deref()
                .context("SCRFD expected hash missing")?
        } else {
            YUNET_HASH
        };
        if !actual.eq_ignore_ascii_case(expected) {
            bail!("model SHA-256 mismatch; expected {expected}, actual {actual}");
        }
        if is_scrfd {
            let metadata_path = dir.join(METADATA);
            if !metadata_path.exists() {
                status.state = ModelAvailability::Missing;
                bail!("SCRFD metadata missing: {}", metadata_path.display());
            }
            let metadata: Metadata =
                serde_json::from_slice(&read_bounded(&metadata_path, 64 * 1024)?)
                    .context("invalid SCRFD metadata JSON")?;
            if metadata.model_id != "scrfd_500m_kps"
                || !metadata.source_url.starts_with("https://")
                || metadata.source_url.len() <= 8
                || !metadata.license_url.starts_with("https://")
                || metadata.license_url.len() <= 8
                || metadata.license_note.trim().is_empty()
            {
                bail!("SCRFD metadata requires model_id scrfd_500m_kps, HTTPS source_url/license_url and nonempty license_note; metadata is not authorization");
            }
        }
        Ok(bytes)
    })();
    match result {
        Ok(bytes) => {
            status.state = ModelAvailability::Available;
            status.reason = Some("Disk bytes, declared SHA-256 and metadata checks passed only; ONNX graph/output validation occurs when selected and loaded. Inference and accuracy are not established by this check. License metadata is an operator declaration, not authorization.".into());
            (status, Some(bytes))
        }
        Err(e) => {
            status.reason = Some(format!("{e:#}"));
            (status, None)
        }
    }
}

/// Does not initialize ONNX Runtime or perform inference.
pub fn detector_model_status(dir: &Path, settings: &AnalysisSettings) -> DetectorModelStatus {
    inspect(dir, settings).0
}

/// Optional-provider preflight. Default YuNet cache reuse does not require model files.
pub fn validate_detector_model(dir: &Path, settings: &AnalysisSettings) -> Result<()> {
    settings.validate()?;
    if settings.face_detector == FaceDetectorProvider::Yunet {
        return Ok(());
    }
    let (status, _) = inspect(dir, settings);
    if status.state != ModelAvailability::Available {
        bail!("SCRFD unavailable: {}", status.reason.unwrap_or_default());
    }
    Ok(())
}

pub(super) struct ScrfdDetector {
    session: Session,
}
impl ScrfdDetector {
    pub(super) fn load(dir: &Path, settings: &AnalysisSettings) -> Result<Self> {
        let (status, bytes) = inspect(dir, settings);
        let bytes = bytes
            .with_context(|| format!("SCRFD unavailable: {}", status.reason.unwrap_or_default()))?;
        // Load precisely the bytes that were hashed, not a path reopened after verification.
        let session = super::runtime::session(&bytes, "scrfd")
            .context("SCRFD ONNX load failed; no fallback")?;
        if session.inputs.len() != 1 || session.outputs.len() != 9 {
            bail!("SCRFD requires one input and nine KPS outputs; no fallback");
        }
        validate_output_names(
            &session
                .outputs
                .iter()
                .map(|output| output.name.as_str())
                .collect::<Vec<_>>(),
        )?;
        let mut detector = Self { session };
        // Bounded synthetic execution validates input dtype/shape and every output tensor.
        detector
            .detect(&RgbImage::new(640, 640), 0.99)
            .context("SCRFD tensor contract validation failed; no fallback")?;
        Ok(detector)
    }

    pub(super) fn detect(&mut self, image: &RgbImage, threshold: f32) -> Result<Vec<Detection>> {
        let (input, scale) = preprocess(image)?;
        let output = self.session.run(ort::inputs![Tensor::from_array((
            [1usize, 3, SIDE, SIDE],
            input
        ))?])?;
        if output.len() != 9 {
            bail!("SCRFD must return nine tensors");
        }
        let mut tensors = Vec::with_capacity(9);
        for index in 0..9 {
            let (shape, values) = output[index].try_extract_tensor::<f32>()?;
            tensors.push(TensorView {
                shape: &shape[..],
                values,
            });
        }
        decode(&tensors, image.width(), image.height(), scale, threshold)
    }
}

fn preprocess(image: &RgbImage) -> Result<(Vec<f32>, f32)> {
    if image.width() == 0 || image.height() == 0 {
        bail!("SCRFD input is empty");
    }
    let scale = SIDE as f32 / image.width().max(image.height()) as f32;
    let width = ((image.width() as f32 * scale) as u32).clamp(1, SIDE as u32);
    let height = ((image.height() as f32 * scale) as u32).clamp(1, SIDE as u32);
    let small = imageops::resize(image, width, height, imageops::FilterType::Triangle);
    // Official black bottom/right padding is normalized along with the RGB image.
    let mut input = vec![-127.5 / 128.; 3 * SIDE * SIDE];
    for (x, y, pixel) in small.enumerate_pixels() {
        let index = y as usize * SIDE + x as usize;
        for channel in 0..3 {
            input[channel * SIDE * SIDE + index] = (f32::from(pixel[channel]) - 127.5) / 128.;
        }
    }
    // Match official detection coordinate restoration, which uses resized height.
    Ok((input, height as f32 / image.height() as f32))
}

struct TensorView<'a> {
    shape: &'a [i64],
    values: &'a [f32],
}
fn validate_tensor(tensor: &TensorView<'_>, count: usize, channels: usize) -> Result<()> {
    let expected = [count as i64, channels as i64];
    let shape = if tensor.shape.len() == 3 && tensor.shape[0] == 1 {
        &tensor.shape[1..]
    } else {
        tensor.shape
    };
    if shape != expected || tensor.values.len() != count * channels {
        bail!(
            "incompatible SCRFD tensor shape: {:?}; expected [1?, {count}, {channels}]",
            tensor.shape
        );
    }
    if tensor.values.iter().any(|v| !v.is_finite()) {
        bail!("SCRFD tensor contains non-finite values");
    }
    Ok(())
}

fn decode(
    tensors: &[TensorView<'_>],
    width: u32,
    height: u32,
    scale: f32,
    threshold: f32,
) -> Result<Vec<Detection>> {
    if tensors.len() != 9 || !scale.is_finite() || scale <= 0. || !threshold.is_finite() {
        bail!("invalid SCRFD decode inputs");
    }
    let mut candidates = Vec::new();
    for (level, stride) in STRIDES.into_iter().enumerate() {
        let cols = SIDE / stride;
        let count = cols * cols * 2;
        let scores = &tensors[level];
        let boxes = &tensors[level + 3];
        let points = &tensors[level + 6];
        validate_tensor(scores, count, 1)?;
        validate_tensor(boxes, count, 4)?;
        validate_tensor(points, count, 10)?;
        if scores.values.iter().any(|v| !(0.0..=1.0).contains(v)) {
            bail!("SCRFD scores outside contract");
        }
        // Official regression heads can produce finite negative distances,
        // including low-score anchors on a blank image. The upstream decoder
        // permits signed distances; reject only invalid/degenerate decoded boxes.
        let step = stride as f32 / scale;
        for i in 0..count {
            let score = scores.values[i];
            if score < threshold {
                continue;
            }
            let cx = ((i / 2) % cols) as f32;
            let cy = ((i / 2) / cols) as f32;
            let b = &boxes.values[i * 4..i * 4 + 4];
            let raw = [
                (cx - b[0]) * step,
                (cy - b[1]) * step,
                (cx + b[2]) * step,
                (cy + b[3]) * step,
            ];
            if raw.iter().any(|v| !v.is_finite()) {
                bail!("SCRFD decoded box overflow");
            }
            let x1 = raw[0].clamp(0., width as f32);
            let y1 = raw[1].clamp(0., height as f32);
            let x2 = raw[2].clamp(0., width as f32);
            let y2 = raw[3].clamp(0., height as f32);
            if x2 - x1 < 8. || y2 - y1 < 8. {
                continue;
            }
            let mut keypoints = [[0.; 2]; 5];
            for (j, point) in keypoints.iter_mut().enumerate() {
                *point = [
                    (cx + points.values[i * 10 + j * 2]) * step,
                    (cy + points.values[i * 10 + j * 2 + 1]) * step,
                ];
                if point.iter().any(|v| !v.is_finite()) {
                    bail!("SCRFD decoded keypoint overflow");
                }
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
    for candidate in candidates {
        if keep
            .iter()
            .all(|other| iou(&candidate.bbox, &other.bbox) < 0.4)
        {
            keep.push(candidate);
        }
    }
    Ok(keep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_official_and_named_heads_are_supported_but_mixing_or_reordering_is_not() {
        validate_output_names(&OUTPUTS).unwrap();
        validate_output_names(&OFFICIAL_OUTPUTS).unwrap();
        let mut names = OFFICIAL_OUTPUTS;
        names.swap(0, 1);
        assert!(validate_output_names(&names).is_err());
        names = OFFICIAL_OUTPUTS;
        names[0] = OUTPUTS[0];
        assert!(validate_output_names(&names).is_err());
        assert!(validate_output_names(&OFFICIAL_OUTPUTS[..6]).is_err());
    }

    fn empty_outputs() -> (Vec<Vec<i64>>, Vec<Vec<f32>>) {
        let mut shapes = Vec::new();
        let mut values = Vec::new();
        for channels in [1, 4, 10] {
            for stride in STRIDES {
                let count = (SIDE / stride).pow(2) * 2;
                shapes.push(vec![1, count as i64, channels as i64]);
                values.push(vec![0.; count * channels]);
            }
        }
        (shapes, values)
    }

    fn views<'a>(shapes: &'a [Vec<i64>], values: &'a [Vec<f32>]) -> Vec<TensorView<'a>> {
        shapes
            .iter()
            .zip(values)
            .map(|(shape, values)| TensorView { shape, values })
            .collect()
    }

    #[test]
    fn rgb_preprocessing_normalizes_black_padding_and_preserves_channels() {
        let image = RgbImage::from_pixel(640, 320, image::Rgb([255, 128, 0]));
        let (input, scale) = preprocess(&image).unwrap();
        assert_eq!(scale, 1.);
        assert_eq!(input[0], 127.5 / 128.);
        assert_eq!(input[SIDE * SIDE], 0.5 / 128.);
        assert_eq!(input[2 * SIDE * SIDE], -127.5 / 128.);
        assert_eq!(input[320 * SIDE], -127.5 / 128.);
        assert!(preprocess(&RgbImage::new(0, 1)).is_err());
    }

    #[test]
    fn ltrb_and_keypoints_use_repeated_anchor_centers_stride_and_nms() {
        let (mut shapes, mut values) = empty_outputs();
        // Two anchors at cell (10, 12), stride 8. NMS must retain the larger score.
        let index = (12 * 80 + 10) * 2;
        for (offset, score) in [(0, 0.9), (1, 0.8)] {
            let i = index + offset;
            values[0][i] = score;
            values[3][i * 4..i * 4 + 4].copy_from_slice(&[1., 2., 3., 4.]);
            values[6][i * 10..i * 10 + 10]
                .copy_from_slice(&[-0.5, -1., 0.5, -1., 0., 0., -0.5, 1., 0.5, 1.]);
        }
        // Both published rank conventions are supported.
        shapes[0].remove(0);
        let detections = decode(&views(&shapes, &values), 320, 320, 2., 0.55).unwrap();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].bbox, [36., 40., 16., 24.]);
        assert_eq!(detections[0].keypoints[0], [38., 44.]);
        assert_eq!(detections[0].confidence, 0.9);
    }

    #[test]
    fn wrong_shape_nonfinite_or_non_kps_output_is_rejected_even_below_threshold() {
        let (mut shapes, mut values) = empty_outputs();
        assert!(decode(&views(&shapes, &values), 640, 640, 1., 0.55)
            .unwrap()
            .is_empty());
        shapes[8][2] = 5;
        assert!(decode(&views(&shapes, &values), 640, 640, 1., 0.55).is_err());
        shapes[8][2] = 10;
        values[8][0] = f32::NAN;
        assert!(decode(&views(&shapes, &values), 640, 640, 1., 0.55).is_err());
        values[8][0] = 0.;
        values[0][0] = 1.1;
        assert!(decode(&views(&shapes, &values), 640, 640, 1., 0.55).is_err());
        assert!(decode(&views(&shapes[..6], &values[..6]), 640, 640, 1., 0.55).is_err());
    }

    #[test]
    fn signed_box_regression_is_valid_but_degenerate_decoded_boxes_are_omitted() {
        let (shapes, mut values) = empty_outputs();
        // Real official weights produce negative low-confidence LTRB values.
        values[5][0] = -0.0013;
        assert!(decode(&views(&shapes, &values), 640, 640, 1., 0.55)
            .unwrap()
            .is_empty());
        let index = (12 * 80 + 10) * 2;
        values[0][index] = 0.9;
        values[3][index * 4..index * 4 + 4].copy_from_slice(&[-0.5, 1., 3., 2.]);
        let detections = decode(&views(&shapes, &values), 640, 640, 1., 0.55).unwrap();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].bbox, [84., 88., 20., 24.]);
        values[3][index * 4..index * 4 + 4].copy_from_slice(&[-4., 1., -4., 2.]);
        assert!(decode(&views(&shapes, &values), 640, 640, 1., 0.55)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn optional_artifact_checks_require_hash_and_metadata_but_never_infer_authorization() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = AnalysisSettings::default();
        assert!(validate_detector_model(dir.path(), &settings).is_ok());
        settings.face_detector = FaceDetectorProvider::Scrfd500m;
        assert!(validate_detector_model(dir.path(), &settings).is_err());
        settings.scrfd_model_sha256 = Some("0".repeat(64));
        assert_eq!(
            detector_model_status(dir.path(), &settings).state,
            ModelAvailability::Missing
        );
        std::fs::create_dir(dir.path().join("optional")).unwrap();
        std::fs::write(dir.path().join(MODEL), b"not an ONNX model").unwrap();
        let status = detector_model_status(dir.path(), &settings);
        assert_eq!(status.state, ModelAvailability::Invalid);
        let actual = status.sha256.unwrap();
        assert!(status.reason.unwrap().contains("mismatch"));
        settings.scrfd_model_sha256 = Some(actual.to_ascii_uppercase());
        assert_eq!(
            detector_model_status(dir.path(), &settings).state,
            ModelAvailability::Missing
        );
        std::fs::write(dir.path().join(METADATA), br#"{"model_id":"scrfd_500m_kps","source_url":"https://example.com/model","license_url":"https://example.com/license","license_note":"Test declaration, not authorization"}"#).unwrap();
        let status = detector_model_status(dir.path(), &settings);
        assert_eq!(status.state, ModelAvailability::Available);
        assert!(status.reason.unwrap().contains("not authorization"));
        // Availability deliberately does not claim these fake bytes are a valid ONNX graph.
        assert!(validate_detector_model(dir.path(), &settings).is_ok());
        std::fs::write(dir.path().join(METADATA), b"{}").unwrap();
        assert_eq!(
            detector_model_status(dir.path(), &settings).state,
            ModelAvailability::Invalid
        );
    }

    #[test]
    #[ignore = "requires explicitly acquired non-commercial research SCRFD and bundled runtime; no downloads"]
    fn research_official_scrfd_onnx_and_optional_geometry_capture() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        #[cfg(windows)]
        ort::init_from(root.join("models/onnxruntime.dll").to_string_lossy()).commit()?;
        let selected = AnalysisSettings {
            face_detector: FaceDetectorProvider::Scrfd500m,
            scrfd_model_sha256: Some(
                "5e4447f50245bbd7966bd6c0fa52938c61474a04ec7def48753668a9d8b4ea3a".into(),
            ),
            ..AnalysisSettings::default()
        };
        let mut detector = ScrfdDetector::load(&root.join("models"), &selected)?;
        for (w, h) in [(640, 640), (1024, 683), (683, 1024)] {
            detector.detect(
                &RgbImage::from_pixel(w, h, image::Rgb([80, 128, 170])),
                0.55,
            )?;
        }
        if let Some(fixture) = std::env::var_os("IRIS_SCRFD_GEOMETRY_PHOTO") {
            let output_path = std::env::var_os("IRIS_SCRFD_GEOMETRY_REPORT")
                .context("IRIS_SCRFD_GEOMETRY_REPORT required with fixture")?;
            let preview = super::super::decode_jpeg_preview(Path::new(&fixture), 1280)?;
            let image = preview.image;
            let (input, scale) = preprocess(&image)?;
            let outputs = detector.session.run(ort::inputs![Tensor::from_array((
                [1usize, 3, SIDE, SIDE],
                input
            ))?])?;
            let tensors = (0..9)
                .map(|i| {
                    let (shape, values) = outputs[i].try_extract_tensor::<f32>()?;
                    Ok(TensorView {
                        shape: &shape[..],
                        values,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let detections = decode(&tensors, image.width(), image.height(), scale, 0.55)?;
            let report = serde_json::json!({
                "model_sha256": selected.scrfd_model_sha256,
                "width": image.width(), "height": image.height(), "scale": scale, "threshold": 0.55,
                "output_names": OFFICIAL_OUTPUTS,
                "tensors": tensors.iter().map(|t| serde_json::json!({"shape": t.shape, "values": t.values})).collect::<Vec<_>>(),
                "detections": detections.iter().map(|d| serde_json::json!({"bbox": d.bbox, "confidence": d.confidence, "keypoints": d.keypoints})).collect::<Vec<_>>(),
                "scope": "Private local tensor/geometry compatibility capture; no independent accuracy labels"
            });
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output_path)?;
            file.write_all(&serde_json::to_vec(&report)?)?;
        }
        println!("Original official SCRFD bytes executed on square/landscape/portrait inputs; no accuracy claim");
        Ok(())
    }
}
