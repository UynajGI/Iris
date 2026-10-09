//! Persistent process isolation: native decoder/model calls have a hard deadline.
//! The parent kills only its owned child; completed database work stays in the parent.
use anyhow::{bail, Context, Result};
use iris_core::vision::{AnalysisSettings, VisionAnalysis, VisionEngine};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const PROTOCOL: &str = "iris-analysis-worker-v1";
const MAX_LINE: usize = 16 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(20);

/// A user cancellation signal, distinct from model initialization or I/O failures.
#[derive(Debug)]
pub struct WorkerCancelled;
impl std::fmt::Display for WorkerCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("analysis cancelled")
    }
}
impl std::error::Error for WorkerCancelled {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    protocol: String,
    ready: bool,
    error: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    path: PathBuf,
    settings: AnalysisSettings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base_analysis: Option<VisionAnalysis>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    id: u64,
    analysis: Option<VisionAnalysis>,
    error: Option<String>,
    fatal: bool,
}

enum Incoming {
    Line(String),
    Error(String),
}
struct Process {
    child: Child,
    #[cfg(windows)]
    process_tree: iris_core::owned_job::OwnedJob,
    writes: Option<mpsc::Sender<Vec<u8>>>,
    replies: mpsc::Receiver<Incoming>,
    reader: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
}
impl Process {
    fn stop(&mut self) {
        self.writes.take();
        // Killing closes both pipes, including a writer blocked on a wedged worker.
        #[cfg(windows)]
        self.process_tree.terminate();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.writer.take() {
            let _ = handle.join();
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        // An idle persistent worker exits cleanly when its stdin closes.
        self.writes.take();
        // Profiling writes are diagnostic evidence; allow ORT to finish JSON
        // before applying the owned-child hard stop.
        let grace = if std::env::var_os("IRIS_ORT_PROFILE_DIR").is_some() {
            Duration::from_secs(5)
        } else {
            Duration::from_millis(100)
        };
        let deadline = Instant::now() + grace;
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.stop();
    }
}

pub struct AnalysisWorker {
    execution_provider: iris_core::vision::ExecutionProvider,
    directml_device_id: u32,
    executable: PathBuf,
    model_dir: PathBuf,
    timeout: Duration,
    cancel: Option<Arc<AtomicBool>>,
    process: Option<Process>,
    next_id: u64,
}
impl AnalysisWorker {
    /// Worker selection is configuration evidence, not proof of GPU kernel execution.
    pub fn execution_feedback(&self, phase: &str) -> crate::ExecutionFeedback {
        let gpu = self.execution_provider == iris_core::vision::ExecutionProvider::Directml;
        let device_name = if gpu {
            iris_core::devices::gpu_devices()
                .adapters
                .into_iter()
                .find(|adapter| adapter.device_id == self.directml_device_id)
                .map(|adapter| adapter.name)
        } else {
            None
        };
        crate::ExecutionFeedback {
            phase: phase.into(),
            selected_provider: self.execution_provider,
            device_id: gpu.then_some(self.directml_device_id),
            device_name,
            completed_items: 0,
            warnings: Vec::new(),
        }
    }
    pub fn new(model_dir: &Path, timeout: Duration) -> Result<Self> {
        Self::with_executable(&worker_executable()?, model_dir, timeout)
    }
    pub fn new_with_cancel(
        model_dir: &Path,
        timeout: Duration,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self> {
        Self::configured(&worker_executable()?, model_dir, timeout, Some(cancel))
    }
    pub fn new_with_settings(
        model_dir: &Path,
        timeout: Duration,
        cancel: Arc<AtomicBool>,
        settings: &AnalysisSettings,
    ) -> Result<Self> {
        use iris_core::vision::ExecutionProvider;
        let (provider, device_id) = if settings.execution_provider == ExecutionProvider::Auto {
            match iris_core::devices::preferred_auto_device(&iris_core::devices::gpu_devices()) {
                Some(id) => (ExecutionProvider::Directml, id),
                None => (ExecutionProvider::Cpu, 0),
            }
        } else {
            (
                settings.execution_provider,
                settings.directml_device_id.unwrap_or(0),
            )
        };
        if provider == ExecutionProvider::Cpu {
            return Self::new_with_cancel(model_dir, timeout, cancel);
        }
        let mut worker = Self {
            executable: std::path::absolute(worker_executable()?)?,
            model_dir: std::path::absolute(model_dir)?,
            timeout,
            cancel: Some(cancel),
            process: None,
            next_id: 1,
            execution_provider: provider,
            directml_device_id: device_id,
        };
        worker.start()?;
        Ok(worker)
    }
    /// Explicit executable supports packaged installations and isolated protocol tests.
    pub fn with_executable(executable: &Path, model_dir: &Path, timeout: Duration) -> Result<Self> {
        Self::configured(executable, model_dir, timeout, None)
    }
    pub fn with_executable_and_cancel(
        executable: &Path,
        model_dir: &Path,
        timeout: Duration,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self> {
        Self::configured(executable, model_dir, timeout, Some(cancel))
    }
    fn configured(
        executable: &Path,
        model_dir: &Path,
        timeout: Duration,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<Self> {
        if timeout.is_zero() {
            bail!("analysis worker timeout must be positive");
        }
        let mut worker = Self {
            execution_provider: iris_core::vision::ExecutionProvider::Cpu,
            directml_device_id: 0,
            executable: std::path::absolute(executable)?,
            model_dir: std::path::absolute(model_dir)?,
            timeout,
            cancel,
            process: None,
            next_id: 1,
        };
        worker.start()?;
        Ok(worker)
    }
    pub fn set_cancel(&mut self, cancel: Arc<AtomicBool>) {
        self.cancel = Some(cancel);
    }
    pub fn analyze(&mut self, path: &Path, settings: &AnalysisSettings) -> Result<VisionAnalysis> {
        self.execute(path, settings, None)
    }
    pub fn embed(
        &mut self,
        path: &Path,
        settings: &AnalysisSettings,
        analysis: VisionAnalysis,
    ) -> Result<VisionAnalysis> {
        self.execute(path, settings, Some(analysis))
    }
    fn execute(
        &mut self,
        path: &Path,
        settings: &AnalysisSettings,
        base_analysis: Option<VisionAnalysis>,
    ) -> Result<VisionAnalysis> {
        settings.validate()?;
        self.check_cancel()?;
        if self.process.is_none() {
            self.start()?;
        }
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .context("worker request counter exhausted")?;
        let request = Request {
            id,
            path: std::path::absolute(path)?,
            settings: settings.clone(),
            base_analysis,
        };
        let mut bytes = serde_json::to_vec(&request)?;
        if bytes.len() > MAX_LINE {
            bail!("worker request exceeds protocol limit");
        }
        bytes.push(b'\n');
        let result = (|| -> Result<VisionAnalysis> {
            self.process
                .as_ref()
                .context("worker unavailable")?
                .writes
                .as_ref()
                .context("worker input closed")?
                .send(bytes)
                .context("worker writer exited")?;
            let line = self.receive("analysis")?;
            let reply: Reply =
                serde_json::from_str(&line).context("malformed analysis worker reply")?;
            if reply.id != id {
                bail!("analysis worker reply id mismatch");
            }
            match (reply.analysis, reply.error) {
                (Some(analysis), None) if !reply.fatal => Ok(analysis),
                (None, Some(message)) if !reply.fatal => {
                    Err(anyhow::anyhow!("analysis failed: {message}"))
                }
                (None, Some(message)) => {
                    Err(anyhow::anyhow!("analysis worker fatal error: {message}"))
                }
                _ => Err(anyhow::anyhow!("invalid analysis worker reply envelope")),
            }
        })();
        // Restart after any unsuccessful request; never reuse uncertain native state.
        if result.is_err() {
            if let Some(mut process) = self.process.take() {
                process.stop();
            }
        }
        result
    }
    fn check_cancel(&self) -> Result<()> {
        if self
            .cancel
            .as_ref()
            .is_some_and(|v| v.load(Ordering::Relaxed))
        {
            return Err(WorkerCancelled.into());
        }
        Ok(())
    }
    fn receive(&self, phase: &str) -> Result<String> {
        let deadline = Instant::now() + self.timeout;
        loop {
            self.check_cancel()?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!(
                    "{phase} worker timed out after {} ms",
                    self.timeout.as_millis()
                );
            }
            match self
                .process
                .as_ref()
                .context("worker unavailable")?
                .replies
                .recv_timeout(remaining.min(POLL))
            {
                Ok(Incoming::Line(line)) => return Ok(line),
                Ok(Incoming::Error(error)) => bail!("{phase} worker: {error}"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => bail!("{phase} worker disconnected"),
            }
        }
    }
    fn start(&mut self) -> Result<()> {
        self.check_cancel()?;
        let mut command = Command::new(&self.executable);
        command
            .env(
                "IRIS_WORKER_EXECUTION_PROVIDER",
                if self.execution_provider == iris_core::vision::ExecutionProvider::Directml {
                    "directml"
                } else {
                    "cpu"
                },
            )
            .env(
                "IRIS_WORKER_DIRECTML_DEVICE",
                self.directml_device_id.to_string(),
            );
        command
            .arg("--internal-analysis-worker")
            .arg("--model-dir")
            .arg(&self.model_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("start analysis worker {}", self.executable.display()))?;
        #[cfg(windows)]
        let process_tree = iris_core::owned_job::OwnedJob::attach(&mut child)?;
        let stdin = child.stdin.take().context("worker stdin unavailable")?;
        let stdout = child.stdout.take().context("worker stdout unavailable")?;
        let (tx, rx) = mpsc::channel();
        let reader_tx = tx.clone();
        let reader = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_line(&mut reader) {
                    Ok(Some(line)) => {
                        if reader_tx.send(Incoming::Line(line)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = reader_tx.send(Incoming::Error(
                            "unexpected end of output (worker exited)".into(),
                        ));
                        break;
                    }
                    Err(error) => {
                        let _ = reader_tx.send(Incoming::Error(error.to_string()));
                        break;
                    }
                }
            }
        });
        let (writes, requests) = mpsc::channel::<Vec<u8>>();
        let writer = std::thread::spawn(move || {
            let mut writer = BufWriter::new(stdin);
            for request in requests {
                if let Err(error) = writer.write_all(&request).and_then(|_| writer.flush()) {
                    let _ = tx.send(Incoming::Error(format!("request write failed: {error}")));
                    break;
                }
            }
        });
        self.process = Some(Process {
            child,
            #[cfg(windows)]
            process_tree,
            writes: Some(writes),
            replies: rx,
            reader: Some(reader),
            writer: Some(writer),
        });
        let handshake = (|| -> Result<()> {
            let line = self.receive("startup")?;
            let ready: Ready =
                serde_json::from_str(&line).context("malformed analysis worker handshake")?;
            if ready.protocol != PROTOCOL {
                bail!("analysis worker protocol mismatch");
            }
            if !ready.ready {
                bail!(
                    "analysis worker startup failed: {}",
                    ready
                        .error
                        .unwrap_or_else(|| "unknown model initialization error".into())
                );
            }
            if ready.error.is_some() {
                bail!("invalid analysis worker handshake envelope");
            }
            Ok(())
        })();
        if handshake.is_err() {
            if let Some(mut process) = self.process.take() {
                process.stop();
            }
        }
        handshake
    }
}

fn worker_executable() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("IRIS_DAEMON_WORKER_PATH") {
        let path = PathBuf::from(path);
        if !path.is_file() {
            bail!("IRIS_DAEMON_WORKER_PATH is not an executable file");
        }
        return Ok(path);
    }
    let current = std::env::current_exe()?;
    if current
        .file_stem()
        .is_some_and(|s| s == "iris-daemon" || s == "iris-mcp")
    {
        return Ok(current);
    }
    let filename = if cfg!(windows) {
        "iris-daemon.exe"
    } else {
        "iris-daemon"
    };
    let parent = current.parent().context("executable parent unavailable")?;
    for dir in [Some(parent), parent.parent()].into_iter().flatten() {
        let path = dir.join(filename);
        if path.is_file() {
            return Ok(path);
        }
    }
    bail!("iris-daemon worker executable not found beside application; set IRIS_DAEMON_WORKER_PATH")
}

fn read_line(reader: &mut impl BufRead) -> Result<Option<String>> {
    let mut line = String::new();
    let count = reader.take((MAX_LINE + 1) as u64).read_line(&mut line)?;
    if count == 0 {
        return Ok(None);
    }
    if count > MAX_LINE || !line.ends_with('\n') {
        bail!("worker protocol line exceeds limit or is truncated");
    }
    Ok(Some(line))
}
fn send(writer: &mut impl Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Internal subprocess mode. No application database, HTTP server, or photo upload.
pub fn run(model_dir: &Path) -> Result<()> {
    let mut output = BufWriter::new(std::io::stdout().lock());
    let engine = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        VisionEngine::new(model_dir)
    }))
    .unwrap_or_else(|_| Err(anyhow::anyhow!("model initialization panicked")));
    let mut engine = match engine {
        Ok(engine) => {
            send(
                &mut output,
                &Ready {
                    protocol: PROTOCOL.into(),
                    ready: true,
                    error: None,
                },
            )?;
            engine
        }
        Err(error) => {
            send(
                &mut output,
                &Ready {
                    protocol: PROTOCOL.into(),
                    ready: false,
                    error: Some(error.to_string()),
                },
            )?;
            return Ok(());
        }
    };
    let mut input = std::io::stdin().lock();
    while let Some(line) = read_line(&mut input)? {
        let request: Request = serde_json::from_str(&line).context("invalid worker request")?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match request.base_analysis {
                Some(analysis) => engine.embed_analysis(&request.path, &request.settings, analysis),
                None => engine.analyze(&request.path, &request.settings),
            }
        }));
        let reply = match result {
            Ok(Ok(analysis)) => Reply {
                id: request.id,
                analysis: Some(analysis),
                error: None,
                fatal: false,
            },
            Ok(Err(error)) => Reply {
                id: request.id,
                analysis: None,
                error: Some(error.to_string()),
                fatal: false,
            },
            Err(_) => Reply {
                id: request.id,
                analysis: None,
                error: Some("analysis worker panicked".into()),
                fatal: true,
            },
        };
        let fatal = reply.fatal;
        send(&mut output, &reply)?;
        if fatal {
            break;
        }
    }
    Ok(())
}
