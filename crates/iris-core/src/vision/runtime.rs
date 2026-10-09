//! Per-worker ONNX execution policy. GPU failures are observable CPU fallbacks.
use anyhow::{Context, Result};
use ort::{
    execution_providers::DirectMLExecutionProvider,
    session::{builder::SessionBuilder, Session},
};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex, OnceLock,
    },
};

static CPU_ONLY: AtomicBool = AtomicBool::new(false);
static WARNINGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

fn requested() -> bool {
    std::env::var("IRIS_WORKER_EXECUTION_PROVIDER").as_deref() == Ok("directml")
}
pub fn directml_active() -> bool {
    requested() && !CPU_ONLY.load(Ordering::Relaxed)
}
fn note(message: String) {
    if let Ok(mut entries) = WARNINGS.get_or_init(Default::default).lock() {
        if !entries.contains(&message) {
            entries.push(message);
        }
    }
}
pub fn force_cpu(reason: String) {
    CPU_ONLY.store(true, Ordering::Relaxed);
    note(reason);
}
pub fn warnings() -> Vec<String> {
    WARNINGS
        .get_or_init(Default::default)
        .lock()
        .map(|x| x.clone())
        .unwrap_or_default()
}

pub fn initialize(dir: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        static INITIALIZED: OnceLock<()> = OnceLock::new();
        if INITIALIZED.get().is_some() {
            return Ok(());
        }
        let mut runtime = std::env::var_os("ORT_DYLIB_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| dir.join("onnxruntime.dll"));
        if requested() {
            let gpu = dir.join("directml");
            if gpu.join("onnxruntime.dll").is_file() && gpu.join("DirectML.dll").is_file() {
                for (file, expected) in [
                    (
                        "onnxruntime.dll",
                        "95366724919f4e95ecc60010912ed538ad9804b6683fbd0aad389749102834b9",
                    ),
                    (
                        "onnxruntime_providers_shared.dll",
                        "dea79756b1ef0deb317115aa5da45eca8946eafcf1be2dbd9fa3b309551faae5",
                    ),
                    (
                        "DirectML.dll",
                        "9c9e6d822561c6c41b90e6994b3e8857cf1d66dbfb1e0c4c799c7c89b4e92da1",
                    ),
                ] {
                    let bytes = std::fs::read(gpu.join(file))
                        .with_context(|| format!("DirectML runtime file missing: {file}"))?;
                    anyhow::ensure!(
                        format!("{:x}", Sha256::digest(&bytes)) == expected,
                        "DirectML runtime SHA-256 mismatch: {file}"
                    );
                }
                // Keep dependencies alive, using the explicit directory/system search.
                for file in ["DirectML.dll", "onnxruntime_providers_shared.dll"] {
                    let library = unsafe {
                        libloading::os::windows::Library::load_with_flags(
                            gpu.join(file),
                            0x00000100 | 0x00001000,
                        )
                    };
                    match library {
                        Ok(library) => {
                            let _ = library.into_raw();
                        }
                        Err(error) => {
                            force_cpu(format!(
                                "DirectML runtime unavailable; CPU fallback: {error}"
                            ));
                            break;
                        }
                    }
                }
                if directml_active() {
                    runtime = gpu.join("onnxruntime.dll");
                }
            } else {
                force_cpu("DirectML runtime missing; CPU fallback".into());
            }
        }
        let bytes = std::fs::read(&runtime)
            .with_context(|| format!("ONNX Runtime missing at {}", runtime.display()))?;
        if runtime == dir.join("onnxruntime.dll") && std::env::var_os("ORT_DYLIB_PATH").is_none() {
            anyhow::ensure!(
                format!("{:x}", Sha256::digest(&bytes))
                    == "579b636403983254346a5c1d80bd28f1519cd1e284cd204f8d4ff41f8d711559",
                "ONNX Runtime DLL SHA-256 mismatch"
            );
        }
        ort::init_from(runtime.to_string_lossy()).commit()?;
        let _ = INITIALIZED.set(());
    }
    #[cfg(not(windows))]
    if requested() {
        force_cpu("DirectML requires Windows; CPU fallback".into());
    }
    Ok(())
}

fn builder(gpu: bool, name: &str) -> Result<SessionBuilder> {
    let mut builder = Session::builder()?
        .with_intra_threads(1)?
        .with_inter_threads(1)?;
    if gpu {
        let device = std::env::var("IRIS_WORKER_DIRECTML_DEVICE")
            .unwrap_or_else(|_| "0".into())
            .parse::<i32>()?;
        anyhow::ensure!(device >= 0, "DirectML device index must be nonnegative");
        builder = builder
            .with_memory_pattern(false)?
            .with_parallel_execution(false)?
            .with_execution_providers([DirectMLExecutionProvider::default()
                .with_device_id(device)
                .build()
                .error_on_failure()])?;
    }
    if let Some(directory) = std::env::var_os("IRIS_ORT_PROFILE_DIR") {
        std::fs::create_dir_all(&directory)?;
        let path = Path::new(&directory).join(format!(
            "{}-{}-{}",
            std::process::id(),
            name,
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        builder = builder.with_profiling(path)?;
    }
    Ok(builder)
}

pub(super) fn session(bytes: &[u8], name: &str) -> Result<Session> {
    if directml_active() {
        match builder(true, name).and_then(|b| b.commit_from_memory(bytes).map_err(Into::into)) {
            Ok(session) => {
                note(format!(
                    "DirectML registered for {name}; unsupported operators may execute on CPU"
                ));
                return Ok(session);
            }
            Err(error) => note(format!(
                "DirectML unavailable for {name}; CPU fallback: {error}"
            )),
        }
    }
    Ok(builder(false, name)?.commit_from_memory(bytes)?)
}
