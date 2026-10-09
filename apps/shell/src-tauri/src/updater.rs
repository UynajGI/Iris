//! Installer updates. Trust is fixed at build time; the webview supplies no URLs,
//! keys, paths, installer arguments, or artifact bytes.
use crate::DaemonHost;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{Emitter, Runtime, Url};
use tauri_plugin_updater::{Update, Updater, UpdaterExt};

type Result<T> = std::result::Result<T, UpdateError>;
type Task<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
type Progress = Arc<dyn Fn(usize, Option<u64>) + Send + Sync>;
type Notify = Arc<dyn Fn(UpdateStatus) + Send + Sync>;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct UpdateError {
    pub code: &'static str,
    pub message: String,
}
impl UpdateError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for UpdateError {}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    NotConfigured,
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Downloaded,
    Installing,
    Failed,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct UpdateMetadata {
    pub version: String,
    pub current_version: String,
    /// Unrendered Markdown. The native updater never interprets HTML.
    pub notes: Option<String>,
    pub published_at_unix: Option<i64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct UpdateStatus {
    pub state: UpdatePhase,
    pub current_version: &'static str,
    pub update: Option<UpdateMetadata>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub reason: Option<&'static str>,
    pub error: Option<UpdateError>,
}

struct Trust {
    endpoint: Url,
    public_key: String,
}
enum Configuration {
    Disabled(&'static str),
    Enabled(Trust),
}
impl Configuration {
    fn embedded() -> Result<Self> {
        Self::parse(
            option_env!("IRIS_DISTRIBUTION"),
            option_env!("IRIS_UPDATE_ENDPOINT"),
            option_env!("IRIS_UPDATE_PUBLIC_KEY"),
        )
    }
    fn parse(
        distribution: Option<&str>,
        endpoint: Option<&str>,
        key: Option<&str>,
    ) -> Result<Self> {
        let invalid = |message| UpdateError::new("invalid_configuration", message);
        if endpoint.is_some() != key.is_some() {
            return Err(invalid(
                "Update endpoint and public key must be configured together",
            ));
        }
        match distribution {
            None | Some("portable") => {
                if endpoint.is_some() {
                    return Err(invalid(
                        "Portable distributions cannot enable installer updates",
                    ));
                }
                return Ok(Self::Disabled("installer_required"));
            }
            Some("nsis") => {}
            _ => return Err(invalid("Unsupported distribution kind")),
        }
        let (Some(endpoint), Some(key)) = (endpoint, key) else {
            return Ok(Self::Disabled("release_configuration_missing"));
        };
        let endpoint = Url::parse(endpoint).map_err(|_| invalid("Invalid update endpoint URL"))?;
        if !secure_url(&endpoint) {
            return Err(invalid(
                "Update endpoint must be HTTPS without credentials or a fragment",
            ));
        }
        let decoded = STANDARD
            .decode(key.trim())
            .map_err(|_| invalid("Update public key is not Tauri base64 key content"))?;
        let decoded = std::str::from_utf8(&decoded)
            .map_err(|_| invalid("Invalid update public key encoding"))?;
        minisign_verify::PublicKey::decode(decoded)
            .map_err(|_| invalid("Invalid update public key"))?;
        if !cfg!(windows) {
            return Ok(Self::Disabled("unsupported_platform"));
        }
        Ok(Self::Enabled(Trust {
            endpoint,
            public_key: key.trim().into(),
        }))
    }
}
fn secure_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

/// Read-only release-tool verification. Matches the updater's mandatory minisign
/// validation and additionally requires one exact, signed release version.
/// This function is deliberately not an IPC command and accepts no private key.
pub fn verify_release_artifact(
    bytes: &[u8],
    signature: &str,
    public_key: &str,
    version: &str,
) -> Result<()> {
    let invalid = || {
        UpdateError::new(
            "invalid_signature",
            "Artifact signature or public key is invalid",
        )
    };
    let public_key = STANDARD.decode(public_key.trim()).map_err(|_| invalid())?;
    let public_key = std::str::from_utf8(&public_key).map_err(|_| invalid())?;
    let public_key = minisign_verify::PublicKey::decode(public_key).map_err(|_| invalid())?;
    let signature = STANDARD.decode(signature.trim()).map_err(|_| invalid())?;
    let signature = std::str::from_utf8(&signature).map_err(|_| invalid())?;
    let signature = minisign_verify::Signature::decode(signature).map_err(|_| invalid())?;
    public_key
        .verify(bytes, &signature, true)
        .map_err(|_| invalid())?;
    let versions: Vec<_> = signature
        .trusted_comment()
        .split('\t')
        .filter_map(|field| field.strip_prefix("version:"))
        .collect();
    if versions.as_slice() != [version] {
        return Err(UpdateError::new(
            "signed_version_mismatch",
            "Signature must contain exactly the expected release version",
        ));
    }
    Ok(())
}

trait Candidate: Send + Sync {
    fn metadata(&self) -> UpdateMetadata;
    /// Success means that the publisher signature has been verified.
    fn download_verified(&self, progress: Progress) -> Task<'_, Vec<u8>>;
    fn install(&self, verified: &[u8]) -> Result<()>;
}
trait Backend: Send + Sync {
    fn check(&self) -> Task<'_, Option<Arc<dyn Candidate>>>;
}
struct OfficialBackend {
    updater: Updater,
    secure_transport: bool,
}
struct OfficialCandidate(Update);
impl Backend for OfficialBackend {
    fn check(&self) -> Task<'_, Option<Arc<dyn Candidate>>> {
        Box::pin(async move {
            let update = self
                .updater
                .check()
                .await
                .map_err(|_| UpdateError::new("check_failed", "Could not check for updates"))?;
            update
                .map(|update| {
                    if self.secure_transport && !secure_url(&update.download_url) {
                        return Err(UpdateError::new(
                            "invalid_release",
                            "Update download URL must use HTTPS without credentials or a fragment",
                        ));
                    }
                    Ok(Arc::new(OfficialCandidate(update)) as Arc<dyn Candidate>)
                })
                .transpose()
        })
    }
}
impl Candidate for OfficialCandidate {
    fn metadata(&self) -> UpdateMetadata {
        UpdateMetadata {
            version: self.0.version.clone(),
            current_version: self.0.current_version.clone(),
            notes: self.0.body.clone(),
            published_at_unix: self.0.date.map(|date| date.unix_timestamp()),
        }
    }
    fn download_verified(&self, progress: Progress) -> Task<'_, Vec<u8>> {
        Box::pin(async move {
            // The plugin's on_download_finish callback precedes signature verification.
            // Only the successful return from download may set Downloaded.
            self.0
                .download(move |chunk, total| progress(chunk, total), || {})
                .await
                .map_err(|_| {
                    UpdateError::new(
                        "download_failed",
                        "Update download or signature verification failed",
                    )
                })
        })
    }
    fn install(&self, verified: &[u8]) -> Result<()> {
        self.0.install(verified).map_err(|_| {
            UpdateError::new("install_failed", "Could not launch the update installer")
        })
    }
}

trait HostControl: Send + Sync {
    fn pause(&self) -> Result<()>;
    fn recover(&self) -> Result<()>;
}
struct ManagedHost {
    host: Arc<Mutex<DaemonHost>>,
    recovered: Arc<dyn Fn(bool) + Send + Sync>,
}
impl HostControl for ManagedHost {
    fn pause(&self) -> Result<()> {
        self.host
            .lock()
            .map_err(|_| UpdateError::new("host_failed", "Daemon host lock unavailable"))?
            .pause_for_update()
            .map_err(|_| {
                UpdateError::new(
                    "host_failed",
                    "Daemon did not stop gracefully; installation cancelled",
                )
            })
    }
    fn recover(&self) -> Result<()> {
        let result = self
            .host
            .lock()
            .map_err(|_| UpdateError::new("host_recovery_failed", "Daemon host lock unavailable"))?
            .resume_after_update_failure()
            .map(|_| ())
            .map_err(|_| {
                UpdateError::new(
                    "host_recovery_failed",
                    "Installation stopped, but the daemon could not restart",
                )
            });
        (self.recovered)(result.is_ok());
        result
    }
}
struct Inner {
    status: UpdateStatus,
    candidate: Option<Arc<dyn Candidate>>,
    /// Never set from IPC or disk. Bound to `candidate` by one successful download.
    verified: Option<Vec<u8>>,
}
struct Shared {
    inner: Mutex<Inner>,
    busy: AtomicBool,
    notify: Notify,
}
#[derive(Clone)]
pub struct UpdateService {
    backend: Option<Arc<dyn Backend>>,
    host: Arc<dyn HostControl>,
    shared: Arc<Shared>,
}
struct Operation(Arc<Shared>);
impl Drop for Operation {
    fn drop(&mut self) {
        let status = {
            let mut inner = self
                .0
                .inner
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if matches!(
                inner.status.state,
                UpdatePhase::Checking | UpdatePhase::Downloading | UpdatePhase::Installing
            ) {
                inner.status.state = UpdatePhase::Failed;
                inner.status.error = Some(UpdateError::new(
                    "operation_aborted",
                    "Update operation ended before completion",
                ));
                Some(inner.status.clone())
            } else {
                None
            }
        };
        self.0.busy.store(false, Ordering::Release);
        if let Some(status) = status {
            (self.0.notify)(status);
        }
    }
}
impl UpdateService {
    fn new(
        backend: Option<Arc<dyn Backend>>,
        reason: Option<&'static str>,
        host: Arc<dyn HostControl>,
        notify: Notify,
    ) -> Self {
        Self {
            backend,
            host,
            shared: Arc::new(Shared {
                inner: Mutex::new(Inner {
                    status: UpdateStatus {
                        state: if reason.is_some() {
                            UpdatePhase::NotConfigured
                        } else {
                            UpdatePhase::Idle
                        },
                        current_version: env!("CARGO_PKG_VERSION"),
                        update: None,
                        downloaded_bytes: 0,
                        total_bytes: None,
                        reason,
                        error: None,
                    },
                    candidate: None,
                    verified: None,
                }),
                busy: AtomicBool::new(false),
                notify,
            }),
        }
    }
    pub fn status(&self) -> UpdateStatus {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .status
            .clone()
    }
    fn change(&self, f: impl FnOnce(&mut Inner)) -> UpdateStatus {
        let status = {
            let mut inner = self
                .shared
                .inner
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            f(&mut inner);
            inner.status.clone()
        };
        (self.shared.notify)(status.clone());
        status
    }
    fn reserve(&self) -> Result<Operation> {
        if self.backend.is_none() {
            return Err(UpdateError::new(
                "not_configured",
                "Installer updates are not configured for this build",
            ));
        }
        self.shared
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| UpdateError::new("busy", "Another update operation is running"))?;
        Ok(Operation(self.shared.clone()))
    }
    fn failed(&self, error: UpdateError) -> UpdateError {
        self.change(|inner| {
            inner.status.state = UpdatePhase::Failed;
            inner.status.error = Some(error.clone());
        });
        error
    }
    pub async fn check(&self) -> Result<UpdateStatus> {
        if self.backend.is_none() {
            return Ok(self.status());
        }
        let _operation = self.reserve()?;
        self.change(|inner| {
            inner.candidate = None;
            inner.verified = None;
            inner.status.state = UpdatePhase::Checking;
            inner.status.update = None;
            inner.status.downloaded_bytes = 0;
            inner.status.total_bytes = None;
            inner.status.error = None;
        });
        match self.backend.as_ref().unwrap().check().await {
            Ok(candidate) => Ok(self.change(|inner| {
                inner.status.state = if candidate.is_some() {
                    UpdatePhase::Available
                } else {
                    UpdatePhase::UpToDate
                };
                inner.status.update = candidate.as_ref().map(|update| update.metadata());
                inner.candidate = candidate;
            })),
            Err(error) => Err(self.failed(error)),
        }
    }
    pub async fn download(&self) -> Result<UpdateStatus> {
        let _operation = self.reserve()?;
        let candidate = {
            let inner = self
                .shared
                .inner
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            inner.candidate.clone().ok_or_else(|| {
                UpdateError::new("invalid_state", "Check for an update before downloading")
            })?
        };
        self.change(|inner| {
            inner.verified = None;
            inner.status.state = UpdatePhase::Downloading;
            inner.status.error = None;
            inner.status.downloaded_bytes = 0;
            inner.status.total_bytes = None;
        });
        let service = self.clone();
        let progress = Arc::new(move |chunk: usize, total| {
            service.change(|inner| {
                inner.status.downloaded_bytes =
                    inner.status.downloaded_bytes.saturating_add(chunk as u64);
                inner.status.total_bytes = total;
            });
        });
        match candidate.download_verified(progress).await {
            Ok(bytes) => Ok(self.change(|inner| {
                inner.verified = Some(bytes);
                inner.status.state = UpdatePhase::Downloaded;
            })),
            Err(error) => Err(self.failed(error)),
        }
    }
    pub fn install(&self) -> Result<UpdateStatus> {
        let _operation = self.reserve()?;
        let (candidate, bytes) = {
            let mut inner = self
                .shared
                .inner
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let candidate = inner
                .candidate
                .clone()
                .ok_or_else(|| UpdateError::new("invalid_state", "No downloaded update"))?;
            let bytes = inner.verified.take().ok_or_else(|| {
                UpdateError::new(
                    "invalid_state",
                    "A verified download is required before installation",
                )
            })?;
            (candidate, bytes)
        };
        self.change(|inner| {
            inner.status.state = UpdatePhase::Installing;
            inner.status.error = None;
        });
        let outcome = self.host.pause().and_then(|_| candidate.install(&bytes));
        // On Windows successful official installation exits the process. Any
        // return must resume the daemon; do not leave supervision disabled.
        let recovery = self.host.recover();
        if let Err(error) = recovery {
            return Err(self.failed(error));
        }
        match outcome {
            Err(error) => Err(self.failed(error)),
            Ok(()) => Err(self.failed(UpdateError::new(
                "install_incomplete",
                "Installer returned without exiting; check the installed version before retrying",
            ))),
        }
    }
}

/// Called after the updater plugin and daemon host are registered. Invalid build
/// configuration fails startup explicitly, rather than claiming updates are enabled.
pub fn initialize<R: Runtime>(
    app: &tauri::AppHandle<R>,
    host: Arc<Mutex<DaemonHost>>,
) -> Result<UpdateService> {
    let notify_app = app.clone();
    let notify = Arc::new(move |status| {
        let _ = notify_app.emit("updater:status", status);
    });
    let recovery_app = app.clone();
    let host = Arc::new(ManagedHost {
        host,
        recovered: Arc::new(move |ok| {
            let _ = recovery_app.emit(
                if ok {
                    "daemon:restarted"
                } else {
                    "daemon:unavailable"
                },
                (),
            );
        }),
    });
    match Configuration::embedded()? {
        Configuration::Disabled(reason) => Ok(UpdateService::new(None, Some(reason), host, notify)),
        Configuration::Enabled(trust) => {
            let cleanup_host = host.clone();
            let updater = app
                .updater_builder()
                .endpoints(vec![trust.endpoint])
                .map_err(|_| UpdateError::new("invalid_configuration", "Invalid updater endpoint"))?
                .pubkey(trust.public_key)
                .timeout(Duration::from_secs(60))
                // Applies to both manifest and artifact requests, including redirects.
                .configure_client(|client| client.https_only(true))
                .on_before_exit(move || {
                    // This hook runs BEFORE ShellExecuteW, which can still fail.
                    // Do not call cleanup_before_exit here: it would destroy/hide
                    // webview resources before we know whether launch succeeded.
                    // A successful Windows install exits the process itself.
                    let _ = cleanup_host.pause();
                })
                .build()
                .map_err(|_| {
                    UpdateError::new("invalid_configuration", "Could not initialize updater")
                })?;
            Ok(UpdateService::new(
                Some(Arc::new(OfficialBackend {
                    updater,
                    secure_transport: true,
                })),
                None,
                host,
                notify,
            ))
        }
    }
}
#[cfg(feature = "desktop")]
fn main_window(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err(UpdateError::new(
            "forbidden",
            "Updates are only available to the main window",
        ))
    }
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn update_status(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, UpdateService>,
) -> Result<UpdateStatus> {
    main_window(&window)?;
    Ok(state.status())
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn check_for_update(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, UpdateService>,
) -> Result<UpdateStatus> {
    main_window(&window)?;
    state.check().await
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn download_update(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, UpdateService>,
) -> Result<UpdateStatus> {
    main_window(&window)?;
    state.download().await
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn install_update(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, UpdateService>,
) -> Result<UpdateStatus> {
    main_window(&window)?;
    let service = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.install())
        .await
        .map_err(|_| UpdateError::new("install_failed", "Installation task failed"))?
}

#[cfg(test)]
#[path = "updater_tests.rs"]
mod tests;
