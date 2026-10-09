//! Optional, local DINOv3 ViT-S/16 image embeddings. No download or substitute weights.
//! The pinned community ONNX conversion retains Meta's DINOv3 license.
use anyhow::{bail, Context, Result};
use image::{imageops::FilterType, RgbImage};
use ort::{
    session::Session,
    tensor::TensorElementType,
    value::{Tensor, ValueType},
};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const MODEL_FILE: &str = "optional/dinov3_vits16.onnx";
pub const MODEL_SHA256: &str = "f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c";
pub const EMBEDDING_DIM: usize = 384;
pub const PREPROCESSING_ID: &str = "dinov3_vits16_rgb224_triangle_imagenet_pooler_l2_v1";
const SIDE: usize = 224;
const MODEL_BYTES: u64 = 86_474_453;
const DIRECTML_FILE: &str = "optional/dinov3_vits16_directml.onnx";
const DIRECTML_BYTES: u64 = 86_415_189;
const DIRECTML_SHA256: &str = "cb115eebbe83bb4f592845203dbee5f98253a63292210ff9ab3e73062c46dde5";

fn verified_directml_bytes(model_dir: &Path) -> Result<Option<Vec<u8>>> {
    let path = model_dir.join(DIRECTML_FILE);
    if !path.try_exists()? {
        return Ok(None);
    }
    anyhow::ensure!(
        std::fs::metadata(&path)?.len() == DIRECTML_BYTES,
        "DINOv3 DirectML artifact size mismatch"
    );
    let bytes = super::scrfd::read_bounded(&path, DIRECTML_BYTES)?;
    anyhow::ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == DIRECTML_SHA256,
        "DINOv3 DirectML artifact SHA-256 mismatch"
    );
    Ok(Some(bytes))
}

fn verified_bytes(model_dir: &Path, expected_hash: &str) -> Result<Vec<u8>> {
    if !expected_hash.eq_ignore_ascii_case(MODEL_SHA256) {
        bail!("DINOv3 requires the pinned ViT-S/16 artifact SHA-256 {MODEL_SHA256}; no substitute model");
    }
    let path = model_dir.join(MODEL_FILE);
    let length = std::fs::metadata(&path)
        .with_context(|| {
            format!(
                "DINOv3 model missing at {}; run tools/setup-dinov3.py explicitly",
                path.display()
            )
        })?
        .len();
    if length != MODEL_BYTES {
        bail!("DINOv3 artifact size mismatch: expected {MODEL_BYTES}, found {length}");
    }
    let bytes = super::scrfd::read_bounded(&path, MODEL_BYTES)?;
    if format!("{:x}", Sha256::digest(&bytes)) != MODEL_SHA256 {
        bail!("DINOv3 artifact SHA-256 mismatch");
    }
    Ok(bytes)
}

/// Checks disk bytes only, without creating a session or connecting to a network.
pub fn validate_dinov3_model(model_dir: &Path, expected_hash: &str) -> Result<()> {
    verified_bytes(model_dir, expected_hash)?;
    // A cached vector must never bypass validation of an installed GPU artifact.
    verified_directml_bytes(model_dir)?;
    Ok(())
}

pub struct DinoV3 {
    session: Session,
}
impl DinoV3 {
    /// The host initializes the existing ONNX Runtime before loading optional models.
    pub fn load(model_dir: &Path, expected_hash: &str) -> Result<Self> {
        let bytes = verified_bytes(model_dir, expected_hash)?;
        let directml = verified_directml_bytes(model_dir)?;
        if super::runtime::directml_active() && directml.is_none() {
            super::runtime::force_cpu(
                "DirectML DINOv3 fixed-shape graph missing; CPU fallback".into(),
            );
        }
        // Verify weights before any execution provider is selected.
        let session_bytes = if super::runtime::directml_active() {
            directml.as_deref().unwrap_or(&bytes)
        } else {
            &bytes
        };
        let session = super::runtime::session(session_bytes, "dinov3")
            .context("DINOv3 ONNX load failed; no fallback")?;
        if session.inputs.len() != 1
            || session.inputs[0].name != "pixel_values"
            || session.outputs.len() != 2
            || session.outputs[0].name != "last_hidden_state"
            || session.outputs[1].name != "pooler_output"
        {
            bail!("DINOv3 requires pixel_values input and last_hidden_state/pooler_output outputs");
        }
        if !matches!(&session.inputs[0].input_type, ValueType::Tensor { ty: TensorElementType::Float32, shape, .. }
            if shape.len() == 4 && shape[1] == 3 && (shape[0] == -1 || shape[0] == 1)
                && (shape[2] == -1 || shape[2] == SIDE as i64) && (shape[3] == -1 || shape[3] == SIDE as i64))
        {
            bail!("DINOv3 requires float32 NCHW RGB compatible with [1,3,224,224]");
        }
        if !matches!(&session.outputs[1].output_type, ValueType::Tensor { ty: TensorElementType::Float32, shape, .. }
            if shape.len() == 2 && (shape[0] == -1 || shape[0] == 1) && shape[1] == EMBEDDING_DIM as i64)
        {
            bail!("DINOv3 pooler_output must be float32 [batch,384]");
        }
        let mut model = Self { session };
        if let Err(error) = model.run(vec![0.; 3 * SIDE * SIDE]) {
            if !super::runtime::directml_active() {
                return Err(error.context("DINOv3 startup inference failed"));
            }
            super::runtime::force_cpu(format!(
                "DirectML DINOv3 startup failed; CPU fallback: {error:#}"
            ));
            model.session = super::runtime::session(&bytes, "dinov3")?;
            model
                .run(vec![0.; 3 * SIDE * SIDE])
                .context("DINOv3 CPU startup failed")?;
        }
        Ok(model)
    }

    /// Input is the existing decoded RGB analysis preview, never a source path.
    /// Resize the full preview to 224x224 (bilinear/triangle, no crop), scale /255,
    /// ImageNet-normalize channels, and L2-normalize the 384-value CLS pooler.
    /// This is a versioned local preprocessing choice, not bitwise torchvision parity.
    pub fn embed(&mut self, image: &RgbImage) -> Result<Vec<f32>> {
        self.run(preprocess(image)?)
    }

    fn run(&mut self, input: Vec<f32>) -> Result<Vec<f32>> {
        let output = self
            .session
            .run(ort::inputs![Tensor::from_array((
                [1usize, 3, SIDE, SIDE],
                input
            ))?])
            .context("DINOv3 inference failed")?;
        let (shape, values) = output["pooler_output"].try_extract_tensor::<f32>()?;
        if &shape[..] != [1, EMBEDDING_DIM as i64] {
            bail!("DINOv3 pooler returned invalid shape: {shape:?}");
        }
        normalize(values)
    }
}

fn preprocess(image: &RgbImage) -> Result<Vec<f32>> {
    if image.width() == 0 || image.height() == 0 || image.width().max(image.height()) > 1280 {
        bail!("DINOv3 requires a nonempty analysis preview with longest edge <=1280");
    }
    let resized = image::imageops::resize(image, SIDE as u32, SIDE as u32, FilterType::Triangle);
    let mut input = vec![0.; 3 * SIDE * SIDE];
    let mean = [0.485f32, 0.456, 0.406];
    let std = [0.229f32, 0.224, 0.225];
    for (i, pixel) in resized.pixels().enumerate() {
        for c in 0..3 {
            input[c * SIDE * SIDE + i] = (pixel[c] as f32 / 255. - mean[c]) / std[c];
        }
    }
    Ok(input)
}

fn normalize(values: &[f32]) -> Result<Vec<f32>> {
    if values.len() != EMBEDDING_DIM || values.iter().any(|x| !x.is_finite()) {
        bail!("DINOv3 embedding must contain 384 finite values");
    }
    let norm = values
        .iter()
        .map(|&v| f64::from(v).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= 1e-12 {
        bail!("DINOv3 embedding has invalid/zero norm");
    }
    Ok(values
        .iter()
        .map(|&v| (f64::from(v) / norm) as f32)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_gpu_graph_is_optional_but_corruption_is_fatal() {
        let dir = tempfile::tempdir().unwrap();
        assert!(verified_directml_bytes(dir.path()).unwrap().is_none());
        std::fs::create_dir(dir.path().join("optional")).unwrap();
        std::fs::write(dir.path().join(DIRECTML_FILE), b"corrupt").unwrap();
        assert!(verified_directml_bytes(dir.path())
            .unwrap_err()
            .to_string()
            .contains("size mismatch"));
    }
    #[test]
    fn missing_and_unapproved_models_fail_explicitly() {
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_dinov3_model(dir.path(), MODEL_SHA256)
            .unwrap_err()
            .to_string()
            .contains("missing"));
        assert!(validate_dinov3_model(dir.path(), &"0".repeat(64))
            .unwrap_err()
            .to_string()
            .contains("pinned"));
    }
    #[test]
    fn normalization_rejects_invalid_vectors() {
        assert!(normalize(&[]).is_err());
        assert!(normalize(&vec![0.; EMBEDDING_DIM]).is_err());
        assert!(normalize(&vec![f32::NAN; EMBEDDING_DIM]).is_err());
        let unit = normalize(&vec![2.; EMBEDDING_DIM]).unwrap();
        assert!((unit.iter().map(|x| x * x).sum::<f32>() - 1.).abs() < 1e-5);
    }
    #[test]
    fn preprocessing_is_rgb_nchw_imagenet_and_preview_bounded() {
        let image = RgbImage::from_pixel(20, 10, image::Rgb([255, 0, 128]));
        let tensor = preprocess(&image).unwrap();
        assert_eq!(tensor.len(), 3 * SIDE * SIDE);
        assert!((tensor[0] - (1. - 0.485) / 0.229).abs() < 1e-6);
        assert!((tensor[SIDE * SIDE] + 0.456 / 0.224).abs() < 1e-6);
        assert!((tensor[2 * SIDE * SIDE] - (128. / 255. - 0.406) / 0.225).abs() < 1e-6);
        assert!(preprocess(&RgbImage::new(0, 0)).is_err());
        assert!(preprocess(&RgbImage::new(1281, 1)).is_err());
    }
    #[test]
    #[ignore = "requires the optional pinned DINOv3 model and ONNX Runtime"]
    fn real_dinov3_cpu_embedding() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
        #[cfg(windows)]
        ort::init_from(dir.join("onnxruntime.dll").to_string_lossy())
            .commit()
            .unwrap();
        let mut model = DinoV3::load(&dir, MODEL_SHA256).unwrap();
        let image = RgbImage::from_pixel(320, 180, image::Rgb([64, 120, 180]));
        let a = model.embed(&image).unwrap();
        let b = model.embed(&image).unwrap();
        assert_eq!(a, b);
        assert!((a.iter().map(|x| x * x).sum::<f32>() - 1.).abs() < 1e-5);
        let other = model
            .embed(&RgbImage::from_fn(224, 224, |x, y| {
                image::Rgb([(x % 256) as u8, (y % 256) as u8, 0])
            }))
            .unwrap();
        assert!(a.iter().zip(other).map(|(x, y)| x * y).sum::<f32>() < 0.999);
        if let Some(photo_dir) = std::env::var_os("IRIS_TEST_PHOTOS") {
            let mut paths = std::fs::read_dir(photo_dir)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("jpg"))
                })
                .collect::<Vec<_>>();
            paths.sort();
            assert!(
                paths.len() >= 2,
                "real-photo smoke requires at least two JPGs"
            );
            let before = paths[..2]
                .iter()
                .map(|path| format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())))
                .collect::<Vec<_>>();
            let first = super::super::decode::decode_jpeg_preview(&paths[0], 1280)
                .unwrap()
                .image;
            let second = super::super::decode::decode_jpeg_preview(&paths[1], 1280)
                .unwrap()
                .image;
            let bright = RgbImage::from_fn(first.width(), first.height(), |x, y| {
                image::Rgb(
                    first
                        .get_pixel(x, y)
                        .0
                        .map(|v| (f32::from(v) * 1.15).min(255.).round() as u8),
                )
            });
            let started = std::time::Instant::now();
            let original = model.embed(&first).unwrap();
            assert_eq!(original, model.embed(&first).unwrap());
            let exposure = model.embed(&bright).unwrap();
            let different = model.embed(&second).unwrap();
            let cosine = |v: &[f32]| original.iter().zip(v).map(|(a, b)| a * b).sum::<f32>();
            let after = paths[..2]
                .iter()
                .map(|path| format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())))
                .collect::<Vec<_>>();
            assert_eq!(before, after);
            let report = serde_json::json!({"ok":true, "model_sha256":MODEL_SHA256, "preprocessing":PREPROCESSING_ID,
                "embedding_dim":original.len(), "repeat_identical":true, "source_hashes_unchanged":true,
                "source_sha256":before, "preview_dimensions":[[first.width(),first.height()],[second.width(),second.height()]],
                "original_norm":original.iter().map(|x| x*x).sum::<f32>().sqrt(),
                "cosine_exposure_1_15":cosine(&exposure), "cosine_different_photo":cosine(&different),
                "four_embeddings_seconds":started.elapsed().as_secs_f64(), "quality_validated":false, "photos_uploaded":false});
            println!("{report}");
            if let Some(path) = std::env::var_os("IRIS_DINOV3_REPORT") {
                std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
            }
        }
    }
}
