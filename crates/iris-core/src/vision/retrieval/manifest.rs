use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};
use utoipa::ToSchema;

pub const PREPROCESSING: &str = "clip-rgb224-center-catmullrom-meanstd-l2-v1";
const CONFIG: &str = "optional/retrieval/config.json";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub provider: String,
    pub label: String,
    /// Explicit local installation record; the application never sets this for the user.
    pub license_reviewed: bool,
    pub image_sha256: String,
    pub text_sha256: String,
    pub tokenizer_sha256: String,
    pub dimensions: usize,
    pub context_length: usize,
    pub bos_token_id: u32,
    pub eos_token_id: u32,
    pub pad_token_id: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RetrievalStatus {
    pub state: String,
    pub label: Option<String>,
    pub space_key: Option<String>,
    pub dimensions: Option<usize>,
    pub reason: Option<String>,
}
pub struct VerifiedAssets {
    pub manifest: Manifest,
    pub space_key: String,
    pub image: Vec<u8>,
    pub text: Vec<u8>,
    pub tokenizer: Vec<u8>,
}
fn read(dir: &Path, file: &str, limit: u64) -> Result<Vec<u8>> {
    let path = crate::model_paths::checked_model_path(dir, file)?;
    anyhow::ensure!(
        std::fs::metadata(&path)?.len() <= limit,
        "retrieval artifact exceeds size limit"
    );
    let mut bytes = vec![];
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() as u64 <= limit,
        "retrieval artifact grew beyond limit"
    );
    Ok(bytes)
}
fn verified(dir: &Path, file: &str, hash: &str, limit: u64) -> Result<Vec<u8>> {
    anyhow::ensure!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid retrieval SHA-256"
    );
    let bytes = read(dir, file, limit)?;
    anyhow::ensure!(
        format!("{:x}", Sha256::digest(&bytes)).eq_ignore_ascii_case(hash),
        "retrieval artifact SHA-256 mismatch: {file}"
    );
    Ok(bytes)
}
impl VerifiedAssets {
    pub fn load(dir: &Path) -> Result<Self> {
        let config = read(dir, CONFIG, 65536)?;
        let manifest: Manifest =
            serde_json::from_slice(&config).context("invalid retrieval manifest")?;
        anyhow::ensure!(
            manifest.provider == "clip_dual_encoder_v1",
            "unsupported retrieval provider"
        );
        anyhow::ensure!(manifest.license_reviewed,"review model and tokenizer licenses before explicitly installing optional retrieval assets");
        anyhow::ensure!(
            (2..=2048).contains(&manifest.dimensions)
                && (4..=256).contains(&manifest.context_length),
            "retrieval dimensions or token limit unsupported"
        );
        anyhow::ensure!(
            !manifest.label.trim().is_empty() && manifest.label.len() <= 120,
            "invalid retrieval label"
        );
        let image = verified(
            dir,
            "optional/retrieval/image.onnx",
            &manifest.image_sha256,
            512 * 1024 * 1024,
        )?;
        let text = verified(
            dir,
            "optional/retrieval/text.onnx",
            &manifest.text_sha256,
            512 * 1024 * 1024,
        )?;
        let tokenizer = verified(
            dir,
            "optional/retrieval/tokenizer.json",
            &manifest.tokenizer_sha256,
            32 * 1024 * 1024,
        )?;
        let key = format!(
            "{PREPROCESSING}:{}:{}:{}:{}:{}:{}:{}:{}",
            manifest.image_sha256.to_lowercase(),
            manifest.text_sha256.to_lowercase(),
            manifest.tokenizer_sha256.to_lowercase(),
            manifest.dimensions,
            manifest.context_length,
            manifest.bos_token_id,
            manifest.eos_token_id,
            manifest.pad_token_id
        );
        Ok(Self {
            manifest,
            space_key: format!("{:x}", Sha256::digest(key.as_bytes())),
            image,
            text,
            tokenizer,
        })
    }
}
pub fn status(dir: &Path) -> RetrievalStatus {
    let installed = crate::model_paths::model_path_exists(dir, CONFIG);
    if matches!(installed, Ok(false)) {
        return RetrievalStatus {
            state: "disabled".into(),
            label: None,
            space_key: None,
            dimensions: None,
            reason: Some("Optional local image/text encoders are not installed".into()),
        };
    }
    match VerifiedAssets::load(dir) {
        Ok(assets) => RetrievalStatus {
            state: "ready".into(),
            label: Some(assets.manifest.label),
            space_key: Some(assets.space_key),
            dimensions: Some(assets.manifest.dimensions),
            reason: None,
        },
        Err(error) => RetrievalStatus {
            state: "invalid".into(),
            label: None,
            space_key: None,
            dimensions: None,
            reason: Some(error.to_string()),
        },
    }
}
