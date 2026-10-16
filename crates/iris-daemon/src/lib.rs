use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        Path, Query, Request, State,
    },
    http::{HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use iris_core::{domain::*, Services, Store};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::broadcast;
use utoipa::{OpenApi, ToSchema};
pub mod model_installer;

/// Default cap for each project's analysis pool, not a daemon-wide worker cap.
pub const DEFAULT_WORKER_LIMIT: u8 = 12;
pub mod analysis_queue;
mod media_queue;
pub mod ownership;

#[derive(Clone)]
pub struct AppState {
    pub decision_source: String,
    pub services: Arc<Mutex<Services>>,
    pub token: String,
    pub recovery_notice: Option<String>,
    pub model_dir: PathBuf,
    pub jobs: Arc<Mutex<HashMap<i64, Job>>>,
    pub events: broadcast::Sender<Value>,
    pub installer: model_installer::Installer,
    worker_limit: usize,
    media_queue: media_queue::MediaQueue,
    browse_contexts: Arc<analysis_queue::BrowseContexts>,
    retrieval_index_gate: Arc<tokio::sync::Semaphore>,
    retrieval_search_gate: Arc<tokio::sync::Semaphore>,
    render_engine: Arc<iris_core::render::engine::RenderEngine>,
    render_gate: Arc<tokio::sync::Semaphore>,
    render_memory_bytes: u64,
    source_cache_bytes: u64,
    export_queue: Arc<operations::ExportQueue>,
}
#[derive(Clone)]
pub struct Job {
    pub progress: JobProgress,
    pub cancel: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionFeedback {
    pub phase: String,
    pub selected_provider: iris_core::vision::ExecutionProvider,
    pub device_id: Option<u32>,
    pub device_name: Option<String>,
    pub completed_items: usize,
    pub warnings: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct JobProgress {
    #[serde(default)]
    #[schema(required = false)]
    pub id: String,
    pub kind: String,
    pub state: String,
    pub completed: usize,
    pub total: usize,
    pub errors: Vec<String>,
    #[serde(default)]
    #[schema(required = false)]
    pub failed_photo_ids: Vec<i64>,
    #[serde(default)]
    #[schema(required = false)]
    pub failed_scan_paths: Vec<String>,
    #[serde(default)]
    #[schema(required = false)]
    pub root_unavailable: bool,
    #[serde(default)]
    #[schema(required = false)]
    pub found_photos: usize,
    /// Quality results are committed; optional semantic grouping may still be running.
    #[serde(default)]
    #[schema(required = false)]
    pub quality_ready: bool,
    #[serde(default)]
    #[schema(required = false)]
    pub phase: String,
    #[serde(default)]
    #[schema(required = false)]
    pub execution: Vec<ExecutionFeedback>,
    #[serde(default)]
    #[schema(required = false)]
    pub previous_execution: Vec<ExecutionFeedback>,
    pub result: Option<Value>,
}
impl Default for JobProgress {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: "none".into(),
            state: "idle".into(),
            completed: 0,
            total: 0,
            errors: vec![],
            failed_photo_ids: vec![],
            failed_scan_paths: vec![],
            root_unavailable: false,
            found_photos: 0,
            quality_ready: false,
            phase: String::new(),
            execution: vec![],
            previous_execution: vec![],
            result: None,
        }
    }
}
impl AppState {
    /// Actual initialized adapter diagnostics; does not create a GPU device.
    pub fn render_gpu_status(&self) -> Option<serde_json::Value> {
        self.render_engine.gpu_status()
    }
    /// Select a CPU baseline before serving requests; preserve configured budgets.
    pub fn with_cpu_rendering(mut self) -> anyhow::Result<Self> {
        self.render_engine = Arc::new(iris_core::render::engine::RenderEngine::new_cpu(
            self.render_memory_bytes,
            self.source_cache_bytes,
        )?);
        Ok(self)
    }
    pub fn new(database: &std::path::Path, model_dir: PathBuf) -> anyhow::Result<Self> {
        let (events, _) = broadcast::channel(512);
        let store = Store::open(database)?;
        let recovery_notice = store.recovery_notice.clone();
        let memory_mib = std::env::var("IRIS_RENDER_MEMORY_MIB")
            .ok()
            .map(|v| v.parse::<u64>())
            .transpose()?
            .unwrap_or(2048);
        anyhow::ensure!(
            (256..=65536).contains(&memory_mib),
            "IRIS_RENDER_MEMORY_MIB must be between 256 and 65536"
        );
        let render_memory_bytes = memory_mib * 1024 * 1024;
        let source_cache_bytes = (512 * 1024 * 1024).min(render_memory_bytes / 4);
        // Production callers acquire DatabaseOwner before constructing AppState;
        // a previous process can no longer own in-flight publication work.
        let mut services = Services::new(store).with_media_runtime_dir(model_dir.join("media"));
        services.recover_rendered_exports()?;
        Ok(Self {
            decision_source: "human".into(),
            services: Arc::new(Mutex::new(services)),
            recovery_notice,
            token: format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            ),
            model_dir,
            jobs: Default::default(),
            events,
            installer: Default::default(),
            worker_limit: usize::from(DEFAULT_WORKER_LIMIT),
            media_queue: Default::default(),
            browse_contexts: Default::default(),
            retrieval_index_gate: Arc::new(tokio::sync::Semaphore::new(1)),
            retrieval_search_gate: Arc::new(tokio::sync::Semaphore::new(1)),
            render_engine: Arc::new(iris_core::render::engine::RenderEngine::new(
                render_memory_bytes,
                source_cache_bytes,
            )?),
            render_memory_bytes,
            source_cache_bytes,
            render_gate: Arc::new(tokio::sync::Semaphore::new(2)),
            export_queue: Default::default(),
        })
    }
    /// Request context sharing the daemon render memory budget.
    pub fn render_context(&self) -> iris_core::render::RenderContext {
        self.render_engine.request_context()
    }

    /// Read-only request diagnostic for hosts and bounded integration tests.
    pub fn render_request_is_current(&self, session: &str, sequence: u64) -> bool {
        self.render_engine.check_request(session, sequence).is_ok()
    }

    /// Stop queued rendering and cancel work that has not reached publication.
    pub fn shutdown_rendering(&self) {
        self.export_queue.shutdown();
    }
    pub async fn wait_for_rendered_exports(&self) {
        self.export_queue.wait_stopped().await;
    }

    /// Per-project analysis resource policy; concurrent projects have separate pools.
    /// Does not change scoring settings or cached results.
    pub fn with_worker_limit(mut self, worker_limit: usize) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=16).contains(&worker_limit),
            "worker limit must be in 1..=16"
        );
        self.worker_limit = worker_limit;
        Ok(self)
    }
    fn emit(&self, event: &str, id: i64, data: Value) {
        let mut envelope = json!({"event":event,"project_id":id});
        // Move existing analysis trees; json! would serialize them into another Value.
        envelope["data"] = data;
        let _ = self.events.send(envelope);
    }
}
pub struct ApiError(StatusCode, String);
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(StatusCode::BAD_REQUEST, e.into().to_string())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
type ApiResult<T> = Result<Json<T>, ApiError>;
async fn run<T: Send + 'static>(
    state: AppState,
    f: impl FnOnce(&mut Services) -> anyhow::Result<T> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(move || {
        let mut s = state
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
        f(&mut s)
    })
    .await
    .map_err(anyhow::Error::from)?
    .map_err(ApiError::from)
}

async fn auth(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let bearer = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let protocol = req
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(',')
                .map(str::trim)
                .find_map(|s| s.strip_prefix("iris-token."))
        });
    if bearer.or(protocol) != Some(s.token.as_str()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"session token required"})),
        )
            .into_response();
    }
    next.run(req).await
}

#[derive(Serialize, ToSchema)]
pub struct Bootstrap {
    pub version: String,
    pub capabilities: Vec<String>,
    pub recovery_notice: Option<String>,
}
#[utoipa::path(get,path="/api/v1/bootstrap",responses((status=200,body=Bootstrap)))]
async fn bootstrap(State(state): State<AppState>) -> Json<Bootstrap> {
    Json(Bootstrap {
        version: env!("CARGO_PKG_VERSION").into(),
        recovery_notice: state.recovery_notice,
        capabilities: vec![
            "jpeg".into(),
            "png".into(),
            "webp".into(),
            "heic".into(),
            "dinov3_optional".into(),
            "raw".into(),
            "directml_optional".into(),
            "gpu_devices".into(),
            "decisions".into(),
            "xmp".into(),
            "quarantine".into(),
        ],
    })
}
#[utoipa::path(get,path="/api/v1/devices/gpu",responses((status=200,body=iris_core::devices::GpuDevices),(status=401,description="Session token required")))]
async fn gpu_devices() -> ApiResult<iris_core::devices::GpuDevices> {
    let devices = tokio::task::spawn_blocking(iris_core::devices::gpu_devices)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(devices))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateProject {
    pub root: String,
    #[serde(default)]
    pub auto_device: bool,
}
#[utoipa::path(get,path="/api/v1/projects",responses((status=200,body=Vec<Project>)))]
async fn projects(State(s): State<AppState>) -> ApiResult<Vec<Project>> {
    Ok(Json(run(s, |s| s.projects()).await?))
}
#[utoipa::path(post,path="/api/v1/projects",request_body=CreateProject,responses((status=200,body=Project)))]
async fn create_project(
    State(s): State<AppState>,
    Json(b): Json<CreateProject>,
) -> ApiResult<Project> {
    Ok(Json(
        run(s, move |s| {
            s.create_project_with_execution(
                std::path::Path::new(&b.root),
                if b.auto_device {
                    iris_core::vision::ExecutionProvider::Auto
                } else {
                    iris_core::vision::ExecutionProvider::Cpu
                },
            )
        })
        .await?,
    ))
}
#[utoipa::path(get,path="/api/v1/projects/{id}",params(("id"=i64,Path)),responses((status=200,body=Project)))]
async fn project(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Project> {
    Ok(Json(run(s, move |s| s.project(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/open",params(("id"=i64,Path)),responses((status=200,body=Project)))]
async fn open_project(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Project> {
    Ok(Json(run(s, move |s| s.open_project(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/hide",params(("id"=i64,Path)),responses((status=200,body=Project)))]
async fn hide_project(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Project> {
    Ok(Json(run(s, move |s| s.hide_project(id)).await?))
}
#[utoipa::path(get,path="/api/v1/projects/{id}/photos",params(("id"=i64,Path),("decision"=Option<Action>,Query),("rating"=Option<u8>,Query),("color_label"=Option<ColorLabel>,Query),("verdict"=Option<String>,Query),("format"=Option<String>,Query),("sort"=Option<String>,Query),("descending"=Option<bool>,Query),("include_missing"=Option<bool>,Query),("offset"=Option<usize>,Query),("limit"=Option<usize>,Query)),responses((status=200,body=Vec<Photo>)))]
async fn photos(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<PhotoFilter>,
) -> ApiResult<Vec<Photo>> {
    Ok(Json(run(s, move |s| s.photos(id, q)).await?))
}
#[utoipa::path(get,path="/api/v1/photos/{id}",params(("id"=i64,Path)),responses((status=200,body=Photo)))]
async fn photo(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Photo> {
    Ok(Json(run(s, move |s| s.photo(id)).await?))
}
#[utoipa::path(get,path="/api/v1/projects/{id}/groups",params(("id"=i64,Path)),responses((status=200,body=Vec<BurstGroup>)))]
async fn groups(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Vec<BurstGroup>> {
    Ok(Json(run(s, move |s| s.groups(id)).await?))
}
#[derive(Deserialize, ToSchema)]
pub struct DecisionRequest {
    pub photo_ids: Vec<i64>,
    pub action: Action,
    #[serde(default = "yes")]
    pub link_variants: bool,
}
fn yes() -> bool {
    true
}
#[utoipa::path(post,path="/api/v1/projects/{id}/decisions",params(("id"=i64,Path)),request_body=DecisionRequest,responses((status=200,body=DecisionBatch)))]
async fn decisions(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<DecisionRequest>,
) -> ApiResult<DecisionBatch> {
    let source = s.decision_source.clone();
    let v = run(s.clone(), move |s| {
        s.decisions(id, &b.photo_ids, b.action, &source, b.link_variants)
    })
    .await?;
    s.emit("session:changed", id, json!(v));
    Ok(Json(v))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/marks",params(("id"=i64,Path)),request_body=MarkRequest,responses((status=200,body=DecisionBatch)))]
async fn marks(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<MarkRequest>,
) -> ApiResult<DecisionBatch> {
    let source = s.decision_source.clone();
    let result = run(s.clone(), move |services| {
        services.mark_photos_as(id, request, &source)
    })
    .await?;
    s.emit("session:changed", id, json!(result));
    Ok(Json(result))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/undo",params(("id"=i64,Path)),responses((status=200,body=DecisionBatch)))]
async fn undo(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<DecisionBatch> {
    let v = run(s.clone(), move |s| s.undo(id)).await?;
    s.emit("session:changed", id, json!(v));
    Ok(Json(v))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/accept",params(("id"=i64,Path)),request_body=Option<AcceptRequest>,responses((status=200,body=DecisionBatch)))]
async fn accept(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    body: axum::body::Bytes,
) -> ApiResult<DecisionBatch> {
    // Only a genuinely empty body means all photos; never silently discard an
    // explicit scope because its Content-Type header is absent or incorrect.
    let request = if body.is_empty() {
        AcceptRequest::default()
    } else {
        serde_json::from_slice::<AcceptRequest>(&body)?
    };
    let v = run(s.clone(), move |s| s.accept_scoped(id, request)).await?;
    s.emit("session:changed", id, json!(v));
    Ok(Json(v))
}

#[derive(Deserialize, ToSchema)]
pub struct ExportRequest {
    #[serde(default = "all")]
    pub scope: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub destination: String,
}
fn all() -> String {
    "all".into()
}
#[derive(Deserialize, ToSchema)]
pub struct SourceRequest {
    pub source: String,
}
#[derive(Deserialize, ToSchema)]
pub struct ManifestRequest {
    pub manifest_id: String,
}
#[derive(Deserialize, ToSchema)]
pub struct DestinationRequest {
    pub destination: String,
}

#[utoipa::path(get,path="/api/v1/projects/{id}/progress",params(("id"=i64,Path)),responses((status=200,body=JobProgress)))]
async fn progress(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<JobProgress> {
    let saved = run(s.clone(), move |s| s.analysis_run(id)).await?;
    let previous: Option<JobProgress> = saved
        .map(serde_json::from_value)
        .transpose()
        .map_err(anyhow::Error::from)?;
    let live = s
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("job lock poisoned"))?
        .get(&id)
        .map(|j| j.progress.clone());
    let mut result = live.unwrap_or_else(|| previous.clone().unwrap_or_default());
    if let Some(previous) = previous.filter(|previous| previous.id != result.id) {
        result.previous_execution = previous.execution;
    }
    Ok(Json(result))
}
async fn control(s: AppState, id: i64, action: &str) -> ApiResult<JobProgress> {
    let mut jobs = s
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("job lock poisoned"))?;
    let job = jobs
        .get_mut(&id)
        .ok_or_else(|| anyhow::anyhow!("no job for project"))?;
    if !matches!(job.progress.state.as_str(), "running" | "paused") {
        return Err(anyhow::anyhow!("job is terminal").into());
    }
    match action {
        "cancel" => {
            job.cancel.store(true, Ordering::Relaxed);
            job.pause.store(false, Ordering::Relaxed)
        }
        "pause" => {
            job.pause.store(true, Ordering::Relaxed);
            job.progress.state = "paused".into()
        }
        _ => {
            job.pause.store(false, Ordering::Relaxed);
            job.progress.state = "running".into()
        }
    }
    Ok(Json(job.progress.clone()))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/cancel",params(("id"=i64,Path)),responses((status=200,body=JobProgress)))]
async fn cancel(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<JobProgress> {
    control(s, id, "cancel").await
}
#[utoipa::path(post,path="/api/v1/projects/{id}/pause",params(("id"=i64,Path)),responses((status=200,body=JobProgress)))]
async fn pause(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<JobProgress> {
    control(s, id, "pause").await
}
#[utoipa::path(post,path="/api/v1/projects/{id}/resume",params(("id"=i64,Path)),responses((status=200,body=JobProgress)))]
async fn resume(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<JobProgress> {
    control(s, id, "resume").await
}

async fn events(State(s): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let mut rx = s.events.subscribe();
    ws.protocols(["iris"]).on_upgrade(move|mut socket|async move {loop {tokio::select! { event=rx.recv()=>match event {Ok(v)=>if socket.send(Message::Text(v.to_string().into())).await.is_err(){break},Err(broadcast::error::RecvError::Lagged(_))=>{let _=socket.send(Message::Text(json!({"event":"resync","data":{}}).to_string().into())).await;},Err(_)=>break}, inbound=socket.recv()=>match inbound {Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break,_=>{}}}}})
}

mod operations;
pub mod worker;
pub use operations::*;

#[derive(OpenApi)]
#[openapi(paths(
    bootstrap,gpu_devices,projects,create_project,project,open_project,hide_project,photos,photo,groups,
    decisions,marks,undo,accept,progress,cancel,pause,resume,
    model_installer::catalog,model_installer::status,model_installer::start,model_installer::cancel,
    operations::scan,operations::analyze,operations::browse_context,
    operations::scenes,operations::correct_scene,operations::compare_photos,operations::detail,
    operations::search_status,operations::search,operations::index_search,
    operations::group_selection,operations::set_group_intent,operations::accept_group_selection,
    operations::export_xmp,operations::export_copy,operations::export_csv,operations::import_csv,
    operations::quarantine_history,operations::quarantine_preview,operations::quarantine_commit,operations::quarantine_restore,
    operations::models,operations::settings,operations::put_settings,operations::profiles,operations::save_profile,
    operations::delete_profile,operations::apply_profile,operations::estimate_profile,
    operations::cache_status,operations::cache_cleanup,operations::cache_migrate,operations::cache_migrations,operations::cache_cleanup_old,
    operations::thumb,operations::preview,operations::original,operations::photo_metadata
),components(schemas(PhotoFilter)),security(("session"=[])),modifiers(&Security))]
struct GeneratedApiDoc;

pub struct ApiDoc;
impl OpenApi for ApiDoc {
    fn openapi() -> utoipa::openapi::OpenApi {
        let mut doc = GeneratedApiDoc::openapi();
        doc.merge(operations::EditingApiDoc::openapi());
        ensure_response_descriptions(&mut doc);
        doc
    }
}

fn ensure_response_descriptions(doc: &mut utoipa::openapi::OpenApi) {
    // OpenAPI requires this field; utoipa 6 omits an empty description.
    for path in doc.paths.paths.values_mut() {
        for operation in [
            &mut path.get,
            &mut path.put,
            &mut path.post,
            &mut path.delete,
            &mut path.options,
            &mut path.head,
            &mut path.patch,
            &mut path.trace,
            &mut path.query,
        ]
        .into_iter()
        .flatten()
        {
            for response in operation.responses.responses.values_mut() {
                if let utoipa::openapi::RefOr::T(response) = response {
                    if response.description.is_empty() {
                        response.description = "Response".into();
                    }
                }
            }
        }
    }
}
struct Security;
impl utoipa::Modify for Security {
    fn modify(&self, doc: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::*;
        doc.components
            .get_or_insert(Default::default())
            .add_security_scheme(
                "session",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
    }
}
pub fn router(state: AppState) -> Router {
    operations::start_export_worker(state.clone());
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://tauri.localhost"),
            HeaderValue::from_static("tauri://localhost"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .expose_headers([
            axum::http::HeaderName::from_static("x-iris-revision"),
            axum::http::HeaderName::from_static("x-iris-sequence"),
            axum::http::HeaderName::from_static("x-iris-fingerprint"),
            axum::http::HeaderName::from_static("x-iris-approximate"),
            axum::http::HeaderName::from_static("x-iris-mask-domain"),
            axum::http::HeaderName::from_static("x-iris-render-backend"),
            axum::http::HeaderName::from_static("x-iris-cpu-nodes"),
            axum::http::HeaderName::from_static("x-iris-gpu-nodes"),
            axum::http::HeaderName::from_static("x-iris-gpu-fallback"),
            axum::http::HeaderName::from_static("x-iris-soft-proof"),
            axum::http::HeaderName::from_static("x-iris-draft"),
            axum::http::HeaderName::from_static("x-iris-region-x"),
            axum::http::HeaderName::from_static("x-iris-region-y"),
            axum::http::HeaderName::from_static("x-iris-region-width"),
            axum::http::HeaderName::from_static("x-iris-region-height"),
            axum::http::HeaderName::from_static("x-iris-full-width"),
            axum::http::HeaderName::from_static("x-iris-full-height"),
            axum::http::HeaderName::from_static("x-iris-full-frame-evaluation"),
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ]);
    Router::new()
        .route("/api/v1/models/optional", get(model_installer::catalog))
        .route(
            "/api/v1/models/install",
            get(model_installer::status).post(model_installer::start),
        )
        .route(
            "/api/v1/models/install/cancel",
            post(model_installer::cancel),
        )
        .route("/api/v1/bootstrap", get(bootstrap))
        .route("/api/v1/devices/gpu", get(gpu_devices))
        .route("/api/v1/projects", get(projects).post(create_project))
        .route("/api/v1/projects/{id}", get(project))
        .route("/api/v1/projects/{id}/open", post(open_project))
        .route("/api/v1/projects/{id}/hide", post(hide_project))
        .route("/api/v1/projects/{id}/photos", get(photos))
        .route("/api/v1/photos/{id}", get(photo))
        .route("/api/v1/projects/{id}/groups", get(groups))
        .route("/api/v1/projects/{id}/decisions", post(decisions))
        .route("/api/v1/projects/{id}/marks", post(marks))
        .route("/api/v1/projects/{id}/undo", post(undo))
        .route("/api/v1/projects/{id}/accept", post(accept))
        .route("/api/v1/projects/{id}/progress", get(progress))
        .route("/api/v1/projects/{id}/cancel", post(cancel))
        .route("/api/v1/projects/{id}/pause", post(pause))
        .route("/api/v1/projects/{id}/resume", post(resume))
        .merge(operations::routes())
        .route("/api/v1/events", get(events))
        .route(
            "/api/v1/openapi.json",
            get(|| async { Json(ApiDoc::openapi()) }),
        )
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(cors)
        .with_state(state)
}
