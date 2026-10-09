//! Explicit optional-model installation. No project settings are changed here.
use crate::*;
use anyhow::{ensure, Context};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path as FsPath, time::Duration};
mod onnx;

const MODEL: &str = "dinov3_vits16.onnx";
const HASH: &str = "f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c";
const SIZE: usize = 86_474_453;
const LICENSE_HASH: &str = "aa878c2fe56729d87f735e1cab375b27079aa2ef5f9a06e85456a4ba2c89e7b8";
const BASE: &str = "https://huggingface.co/onnx-community/dinov3-vits16-pretrain-lvd1689m-ONNX/resolve/48988dfe73065df8d6f5ccc0edc7c8bcf307de41";

#[derive(Clone, Serialize, ToSchema)]
pub struct InstallProgress {
    pub model: String,
    pub state: String,
    pub completed_bytes: usize,
    pub total_bytes: usize,
    pub error: Option<String>,
}
impl Default for InstallProgress {
    fn default() -> Self {
        Self {
            model: "dinov3_vits16".into(),
            state: "idle".into(),
            completed_bytes: 0,
            total_bytes: 0,
            error: None,
        }
    }
}
#[derive(Clone, Default)]
pub struct Installer {
    progress: Arc<Mutex<InstallProgress>>,
    cancel: Arc<AtomicBool>,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InstallRequest {
    pub model: String,
    /// Omitted selects the pinned public download; otherwise read this local folder.
    pub source_folder: Option<String>,
}
#[derive(Serialize, ToSchema)]
pub struct OptionalModel {
    pub id: String,
    pub title: String,
    pub bytes: usize,
    pub sha256: String,
    pub state: String,
    pub license: String,
    pub license_url: String,
}

fn snapshot(installer: &Installer) -> Result<InstallProgress, ApiError> {
    Ok(installer
        .progress
        .lock()
        .map_err(|_| anyhow::anyhow!("installer lock poisoned"))?
        .clone())
}
fn update(installer: &Installer, operation: impl FnOnce(&mut InstallProgress)) {
    if let Ok(mut progress) = installer.progress.lock() {
        operation(&mut progress);
    }
}
fn check_cancel(installer: &Installer) -> anyhow::Result<()> {
    ensure!(
        !installer.cancel.load(Ordering::Relaxed),
        "model installation cancelled"
    );
    Ok(())
}
fn checked(bytes: Vec<u8>, size: usize, hash: &str) -> anyhow::Result<Vec<u8>> {
    ensure!(bytes.len() == size, "model artifact size mismatch");
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == hash,
        "model artifact SHA-256 mismatch"
    );
    Ok(bytes)
}
fn read(
    mut input: impl Read,
    size: usize,
    hash: &str,
    installer: &Installer,
) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(size);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        check_cancel(installer)?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        ensure!(
            bytes.len() + n <= size,
            "model artifact exceeds pinned size"
        );
        bytes.extend_from_slice(&buffer[..n]);
        update(installer, |progress| progress.completed_bytes += n);
    }
    checked(bytes, size, hash)
}
fn local(path: &FsPath, size: usize, hash: &str, installer: &Installer) -> anyhow::Result<Vec<u8>> {
    read(
        fs::File::open(path).with_context(|| format!("cannot read {}", path.display()))?,
        size,
        hash,
        installer,
    )
}
fn download(file: &str, size: usize, hash: &str, installer: &Installer) -> anyhow::Result<Vec<u8>> {
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
        let client = reqwest::Client::builder().https_only(true)
            .connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(180))
            .user_agent("IrisVision/0.1 optional-model-installer").build()?;
        let request = client.get(format!("{BASE}/{file}")).send();
        tokio::pin!(request);
        let mut response = loop {
            tokio::select! {
                result = &mut request => break result?.error_for_status()?,
                _ = tokio::time::sleep(Duration::from_millis(100)) => check_cancel(installer)?,
            }
        };
        let mut bytes = Vec::with_capacity(size);
        loop {
            check_cancel(installer)?;
            tokio::select! {
                chunk = response.chunk() => {
                    let Some(chunk) = chunk? else { break; };
                    ensure!(bytes.len() + chunk.len() <= size, "model artifact exceeds pinned size");
                    update(installer, |progress| progress.completed_bytes += chunk.len());
                    bytes.extend_from_slice(&chunk);
                },
                _ = tokio::time::sleep(Duration::from_millis(100)) => check_cancel(installer)?,
            }
        }
        checked(bytes,size,hash)
    })
}
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn install(
    model_dir: &FsPath,
    request: &InstallRequest,
    installer: &Installer,
) -> anyhow::Result<()> {
    let (model, license) = if let Some(source) = &request.source_folder {
        update(installer, |progress| {
            progress.state = "importing".into();
            progress.total_bytes = SIZE + 7502;
        });
        let source = FsPath::new(source);
        (
            local(&source.join(MODEL), SIZE, HASH, installer)?,
            local(
                &source.join("LICENSE-DINOv3.md"),
                7502,
                LICENSE_HASH,
                installer,
            )?,
        )
    } else {
        update(installer, |progress| {
            progress.state = "downloading".into();
            progress.total_bytes = 137969 + 86347776 + 7502;
        });
        let graph = download(
            "onnx/model.onnx",
            137969,
            "bb75e9e30ff382ecdbd150266445ab41272be4acc85fd3563218dd89781e36da",
            installer,
        )?;
        let weights = download(
            "onnx/model.onnx_data",
            86347776,
            "1eff0bb9f4fdef831ca61c8bc2b5c88d8cc21ef4756d3b483d9b9c15d8d5d27f",
            installer,
        )?;
        let license = download("LICENSE.md", 7502, LICENSE_HASH, installer)?;
        update(installer, |progress| progress.state = "verifying".into());
        check_cancel(installer)?;
        (
            checked(onnx::merge(&graph, &weights)?, SIZE, HASH)?,
            license,
        )
    };
    check_cancel(installer)?;
    update(installer, |progress| progress.state = "installing".into());
    let optional = model_dir.join("optional");
    fs::create_dir_all(&optional)?;
    let stage = Staging(optional.join(format!(".install-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&stage.0)?;
    // Stage all verified bytes before repairing either fixed installation filename.
    for (name, bytes) in [
        ("LICENSE-DINOv3.md", license.as_slice()),
        (MODEL, model.as_slice()),
    ] {
        fs::write(stage.0.join(name), bytes)?;
    }
    check_cancel(installer)?;
    for (name, expected) in [
        ("LICENSE-DINOv3.md", license.as_slice()),
        (MODEL, model.as_slice()),
    ] {
        let target = optional.join(name);
        let previous = stage.0.join(format!("{name}.previous"));
        if target.exists() {
            let metadata = fs::metadata(&target)?;
            ensure!(
                metadata.is_file(),
                "model installation target is not a file"
            );
            if metadata.len() == expected.len() as u64 && fs::read(&target)? == expected {
                continue;
            }
            fs::rename(&target, &previous)?;
        }
        if let Err(error) = fs::rename(stage.0.join(name), &target) {
            if previous.exists() && fs::rename(&previous, &target).is_err() {
                let recovery = stage.0.clone();
                std::mem::forget(stage);
                anyhow::bail!(
                    "installation failed: {error}; previous model retained at {}",
                    recovery.display()
                );
            }
            return Err(error.into());
        }
    }
    Ok(())
}

#[utoipa::path(get,path="/api/v1/models/optional",responses((status=200,body=Vec<OptionalModel>)))]
pub async fn catalog(State(s): State<AppState>) -> ApiResult<Vec<OptionalModel>> {
    let state = tokio::task::spawn_blocking(move || {
        let path = s.model_dir.join("optional").join(MODEL);
        if !path.exists() {
            return "missing";
        }
        if local(&path, SIZE, HASH, &Installer::default()).is_ok()
            && local(
                &s.model_dir.join("optional/LICENSE-DINOv3.md"),
                7502,
                LICENSE_HASH,
                &Installer::default(),
            )
            .is_ok()
        {
            "available"
        } else {
            "invalid"
        }
    })
    .await
    .map_err(anyhow::Error::from)?;
    Ok(Json(vec![OptionalModel {
        id: "dinov3_vits16".into(),
        title: "增强相似照片识别".into(),
        bytes: SIZE,
        sha256: HASH.into(),
        state: state.into(),
        license: "Meta DINOv3 License".into(),
        license_url: format!("{BASE}/LICENSE.md"),
    }]))
}
#[utoipa::path(get,path="/api/v1/models/install",responses((status=200,body=InstallProgress)))]
pub async fn status(State(s): State<AppState>) -> ApiResult<InstallProgress> {
    Ok(Json(snapshot(&s.installer)?))
}
#[utoipa::path(post,path="/api/v1/models/install",request_body=InstallRequest,responses((status=200,body=InstallProgress)))]
pub async fn start(
    State(s): State<AppState>,
    Json(request): Json<InstallRequest>,
) -> ApiResult<InstallProgress> {
    if request.model != "dinov3_vits16" {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "unknown optional model".into(),
        ));
    }
    let initial = {
        let mut progress = s
            .installer
            .progress
            .lock()
            .map_err(|_| anyhow::anyhow!("installer lock poisoned"))?;
        if [
            "starting",
            "downloading",
            "importing",
            "verifying",
            "installing",
        ]
        .contains(&progress.state.as_str())
        {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "model installation already active".into(),
            ));
        }
        s.installer.cancel.store(false, Ordering::Relaxed);
        *progress = InstallProgress {
            state: "starting".into(),
            ..Default::default()
        };
        progress.clone()
    };
    tokio::task::spawn_blocking(move || {
        let result = install(&s.model_dir, &request, &s.installer);
        update(&s.installer, |progress| match result {
            Ok(()) => {
                progress.state = "complete".into();
            }
            Err(error) => {
                progress.state = if s.installer.cancel.load(Ordering::Relaxed) {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                progress.error = Some(format!("{error:#}"));
            }
        });
    });
    Ok(Json(initial))
}
#[utoipa::path(post,path="/api/v1/models/install/cancel",operation_id="cancel_model_install",responses((status=200,body=InstallProgress)))]
pub async fn cancel(State(s): State<AppState>) -> ApiResult<InstallProgress> {
    s.installer.cancel.store(true, Ordering::Relaxed);
    Ok(Json(snapshot(&s.installer)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrong_bytes_and_cancel_never_install() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join(MODEL), b"invalid").unwrap();
        let target = directory.path().join("models");
        let request = InstallRequest {
            model: "dinov3_vits16".into(),
            source_folder: Some(source.to_string_lossy().into()),
        };
        assert!(install(&target, &request, &Installer::default()).is_err());
        assert!(!target.exists());
        let installer = Installer::default();
        installer.cancel.store(true, Ordering::Relaxed);
        assert!(install(&target, &request, &installer).is_err());
        assert!(!target.exists());
    }
    #[test]
    #[ignore = "explicit public 87MB model download; no inference"]
    fn pinned_public_download_produces_exact_existing_model_hash() {
        let directory = tempfile::tempdir().unwrap();
        install(
            directory.path(),
            &InstallRequest {
                model: "dinov3_vits16".into(),
                source_folder: None,
            },
            &Installer::default(),
        )
        .unwrap();
        let bytes = fs::read(directory.path().join("optional").join(MODEL)).unwrap();
        checked(bytes, SIZE, HASH).unwrap();
    }
    #[test]
    #[ignore = "explicit existing pinned source fixture; no network or inference"]
    fn pinned_native_merge_and_offline_install_match_the_existing_artifact() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/dinov3-source");
        let graph = checked(
            fs::read(source.join("model.onnx")).unwrap(),
            137969,
            "bb75e9e30ff382ecdbd150266445ab41272be4acc85fd3563218dd89781e36da",
        )
        .unwrap();
        let weights = checked(
            fs::read(source.join("model.onnx_data")).unwrap(),
            86347776,
            "1eff0bb9f4fdef831ca61c8bc2b5c88d8cc21ef4756d3b483d9b9c15d8d5d27f",
        )
        .unwrap();
        let bytes = checked(onnx::merge(&graph, &weights).unwrap(), SIZE, HASH).unwrap();
        let fixture = tempfile::tempdir().unwrap();
        fs::write(fixture.path().join(MODEL), bytes).unwrap();
        fs::copy(
            source.join("LICENSE.md"),
            fixture.path().join("LICENSE-DINOv3.md"),
        )
        .unwrap();
        let target = tempfile::tempdir().unwrap();
        install(
            target.path(),
            &InstallRequest {
                model: "dinov3_vits16".into(),
                source_folder: Some(fixture.path().to_string_lossy().into()),
            },
            &Installer::default(),
        )
        .unwrap();
        checked(
            fs::read(target.path().join("optional").join(MODEL)).unwrap(),
            SIZE,
            HASH,
        )
        .unwrap();
        // Explicit reinstall repairs only the fixed model artifacts, preserving other files.
        fs::write(target.path().join("optional").join(MODEL), b"corrupt").unwrap();
        fs::write(target.path().join("optional/unrelated.txt"), b"preserve").unwrap();
        install(
            target.path(),
            &InstallRequest {
                model: "dinov3_vits16".into(),
                source_folder: Some(fixture.path().to_string_lossy().into()),
            },
            &Installer::default(),
        )
        .unwrap();
        checked(
            fs::read(target.path().join("optional").join(MODEL)).unwrap(),
            SIZE,
            HASH,
        )
        .unwrap();
        assert_eq!(
            fs::read(target.path().join("optional/unrelated.txt")).unwrap(),
            b"preserve"
        );
    }
}
