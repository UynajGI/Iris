use super::*;
use iris_core::vision::retrieval::RetrievalEncoder;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    Image { path: PathBuf },
    Text { query: String },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrievalRequest {
    id: u64,
    input: Input,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrievalReply {
    id: u64,
    vector: Option<Vec<f32>>,
    space_key: String,
    error: Option<String>,
    fatal: bool,
}

/// Same owned-child deadlines/cancellation as quality inference, independent model sessions.
pub struct RetrievalWorker(AnalysisWorker);
impl RetrievalWorker {
    pub fn new(model_dir: &Path, cancel: Arc<AtomicBool>) -> Result<Self> {
        Self::with_executable_and_cancel(
            &worker_executable()?,
            model_dir,
            Duration::from_secs(60),
            cancel,
        )
    }
    pub fn with_executable_and_cancel(
        executable: &Path,
        model_dir: &Path,
        timeout: Duration,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self> {
        anyhow::ensure!(
            !timeout.is_zero(),
            "retrieval worker timeout must be positive"
        );
        let mut worker = AnalysisWorker {
            process_mode: "retrieval",
            execution_provider: iris_core::vision::ExecutionProvider::Cpu,
            effective_provider: iris_core::vision::ExecutionProvider::Cpu,
            startup_warnings: vec![],
            directml_device_id: 0,
            executable: std::path::absolute(executable)?,
            model_dir: std::path::absolute(model_dir)?,
            timeout,
            cancel: Some(cancel),
            process: None,
            next_id: 1,
        };
        worker.start()?;
        Ok(Self(worker))
    }
    pub fn image(&mut self, path: &Path, space: &str) -> Result<Vec<f32>> {
        self.execute(
            Input::Image {
                path: std::path::absolute(path)?,
            },
            space,
        )
    }
    pub fn text(&mut self, query: &str, space: &str) -> Result<Vec<f32>> {
        self.execute(
            Input::Text {
                query: query.into(),
            },
            space,
        )
    }
    fn execute(&mut self, input: Input, space: &str) -> Result<Vec<f32>> {
        self.0.check_cancel()?;
        if self.0.process.is_none() {
            self.0.start()?;
        }
        let id = self.0.next_id;
        self.0.next_id = id
            .checked_add(1)
            .context("retrieval request counter exhausted")?;
        let mut bytes = serde_json::to_vec(&RetrievalRequest { id, input })?;
        anyhow::ensure!(
            bytes.len() <= MAX_LINE,
            "retrieval request exceeds protocol limit"
        );
        bytes.push(b'\n');
        let result = self.exchange(bytes, id, space);
        if result.is_err() {
            if let Some(mut process) = self.0.process.take() {
                process.stop();
            }
        }
        result
    }
    fn exchange(&self, bytes: Vec<u8>, id: u64, space: &str) -> Result<Vec<f32>> {
        self.0
            .process
            .as_ref()
            .context("retrieval worker unavailable")?
            .writes
            .as_ref()
            .context("retrieval input closed")?
            .send(bytes)
            .context("retrieval writer exited")?;
        let reply: RetrievalReply = serde_json::from_str(&self.0.receive("retrieval")?)
            .context("invalid retrieval reply")?;
        anyhow::ensure!(reply.id == id, "retrieval reply id mismatch");
        anyhow::ensure!(
            reply.space_key == space,
            "retrieval model changed; rebuild the index"
        );
        anyhow::ensure!(
            !reply.fatal && reply.error.is_none(),
            "retrieval failed: {}",
            reply.error.unwrap_or_else(|| "worker panicked".into())
        );
        reply.vector.context("retrieval vector missing")
    }
}

pub(super) fn run(model_dir: &Path) -> Result<()> {
    let mut output = BufWriter::new(std::io::stdout().lock());
    let loaded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        RetrievalEncoder::load(model_dir)
    }))
    .unwrap_or_else(|_| Err(anyhow::anyhow!("retrieval initialization panicked")));
    let mut encoder = match loaded {
        Ok(encoder) => {
            send(
                &mut output,
                &Ready {
                    protocol: PROTOCOL.into(),
                    ready: true,
                    error: None,
                    effective_provider: Some(iris_core::vision::ExecutionProvider::Cpu),
                    warnings: vec![],
                },
            )?;
            encoder
        }
        Err(error) => {
            send(
                &mut output,
                &Ready {
                    protocol: PROTOCOL.into(),
                    ready: false,
                    error: Some(error.to_string()),
                    effective_provider: None,
                    warnings: vec![],
                },
            )?;
            return Ok(());
        }
    };
    let mut input = std::io::stdin().lock();
    while let Some(line) = read_line(&mut input)? {
        let request: RetrievalRequest = serde_json::from_str(&line)?;
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match request.input {
                Input::Image { path } => {
                    let preview = iris_core::vision::decode_preview_with_media(
                        &path,
                        1280,
                        Some(&model_dir.join("media")),
                    )?;
                    encoder.embed_image(&preview.image)
                }
                Input::Text { query } => encoder.embed_text(&query),
            }));
        let (vector, error, fatal) = match result {
            Ok(Ok(vector)) => (Some(vector), None, false),
            Ok(Err(error)) => (None, Some(error.to_string()), false),
            Err(_) => (None, Some("retrieval worker panicked".into()), true),
        };
        send(
            &mut output,
            &RetrievalReply {
                id: request.id,
                vector,
                space_key: encoder.space_key.clone(),
                error,
                fatal,
            },
        )?;
        if fatal {
            break;
        }
    }
    Ok(())
}
