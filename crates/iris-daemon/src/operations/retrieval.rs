use super::*;
use crate::worker::RetrievalWorker;
use anyhow::Context;
use iris_core::{
    services::{RetrievalCoverage, SearchHit},
    vision::retrieval::{self, RetrievalStatus},
};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SearchStatus {
    pub model: RetrievalStatus,
    pub coverage: Option<RetrievalCoverage>,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SearchResponse {
    pub space_key: String,
    pub coverage: RetrievalCoverage,
    pub hits: Vec<SearchHit>,
}

async fn model_status(state: &AppState) -> Result<RetrievalStatus, ApiError> {
    let dir = state.model_dir.clone();
    Ok(tokio::task::spawn_blocking(move || retrieval::status(&dir))
        .await
        .map_err(anyhow::Error::from)?)
}
fn ready(status: &RetrievalStatus) -> anyhow::Result<(&str, usize)> {
    anyhow::ensure!(
        status.state == "ready",
        "{}",
        status
            .reason
            .as_deref()
            .unwrap_or("local search model unavailable")
    );
    Ok((
        status
            .space_key
            .as_deref()
            .context("retrieval space unavailable")?,
        status
            .dimensions
            .context("retrieval dimensions unavailable")?,
    ))
}
#[utoipa::path(get,path="/api/v1/projects/{id}/search/status",params(("id"=i64,Path)),responses((status=200,body=SearchStatus)))]
pub async fn search_status(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<SearchStatus> {
    run(state.clone(), move |services| {
        services.require_available_photos(id, &[])
    })
    .await?;
    let model = model_status(&state).await?;
    let coverage = if let Some(space) = model.space_key.clone() {
        Some(
            run(state, move |services| {
                services.retrieval_coverage(id, &space)
            })
            .await?,
        )
    } else {
        None
    };
    Ok(Json(SearchStatus { model, coverage }))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/search",params(("id"=i64,Path)),request_body=SearchRequest,responses((status=200,body=SearchResponse)))]
pub async fn search(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<SearchRequest>,
) -> ApiResult<SearchResponse> {
    if request.query.trim().is_empty() || request.query.len() > 4096 {
        return Err(anyhow::anyhow!("enter a nonempty query, maximum 4096 bytes").into());
    }
    let limit = request.limit.unwrap_or(48);
    if !(1..=200).contains(&limit) {
        return Err(anyhow::anyhow!("search limit must be 1..=200").into());
    }
    run(state.clone(), move |services| {
        services.require_available_photos(id, &[])
    })
    .await?;
    let permit = state
        .retrieval_search_gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "local search is busy; retry shortly".into(),
            )
        })?;
    let model = model_status(&state).await?;
    let (space, _) = ready(&model)?;
    let space = space.to_owned();
    let worker_dir = state.model_dir.clone();
    let worker_space = space.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let _guard = SearchCancel(cancel.clone());
    let vector = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        RetrievalWorker::new(&worker_dir, cancel)?.text(&request.query, &worker_space)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(
        run(state, move |services| {
            Ok(SearchResponse {
                coverage: services.retrieval_coverage(id, &space)?,
                hits: services.search_embeddings(id, &space, &vector, limit)?,
                space_key: space,
            })
        })
        .await?,
    ))
}
struct SearchCancel(Arc<AtomicBool>);
impl Drop for SearchCancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[utoipa::path(post,path="/api/v1/projects/{id}/search/index",params(("id"=i64,Path)),responses((status=200,body=JobProgress)))]
pub async fn index_search(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<JobProgress> {
    run(state.clone(), move |services| {
        services.require_available_photos(id, &[])
    })
    .await?;
    let permit = state
        .retrieval_index_gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            ApiError(
                StatusCode::CONFLICT,
                "another local search index is running".into(),
            )
        })?;
    let model = model_status(&state).await?;
    let (space, dimensions) = ready(&model)?;
    let space = space.to_owned();
    let job = start_job(&state, id, "search_index")?;
    let initial = job.progress.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let result = index_project(&state, id, &job, &space, dimensions);
        finish(&state, id, job.cancel.load(Ordering::Relaxed), result);
    });
    Ok(Json(initial))
}
fn index_project(
    state: &AppState,
    project: i64,
    job: &Job,
    space: &str,
    dimensions: usize,
) -> anyhow::Result<Value> {
    let pending = state
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
        .retrieval_pending(project, space)?;
    update(state, project, |p| {
        p.total = pending.len();
        p.phase = "search_index".into();
    });
    if pending.is_empty() {
        return Ok(json!({"indexed":0,"reused":true}));
    }
    let mut worker = RetrievalWorker::new(&state.model_dir, job.cancel.clone())?;
    let mut queue = crate::analysis_queue::PendingOrder::new(pending.iter().map(|p| p.photo_id));
    while let Some(index) = queue.claim(&state.browse_contexts.current(project)) {
        wait_for_semantic_job(job)?;
        let photo = &pending[index];
        let result = index_photo(state, project, photo, space, dimensions, &mut worker);
        if let Err(error) = result {
            if job.cancel.load(Ordering::Relaxed) {
                return Err(error);
            }
            update(state, project, |p| {
                p.errors.push(format!("photo {}: {error}", photo.photo_id));
                p.failed_photo_ids.push(photo.photo_id);
            });
        }
        update(state, project, |p| p.completed += 1);
    }
    Ok(
        json!({"coverage":state.services.lock().map_err(|_|anyhow::anyhow!("service lock poisoned"))?.retrieval_coverage(project,space)?}),
    )
}

fn index_photo(
    state: &AppState,
    project: i64,
    photo: &iris_core::services::RetrievalPhoto,
    space: &str,
    dimensions: usize,
    worker: &mut RetrievalWorker,
) -> anyhow::Result<()> {
    let path = state
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
        .photo_path(photo.photo_id)?;
    let metadata = std::fs::metadata(&path)?;
    let mtime = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos()
        .min(i64::MAX as u128) as i64;
    anyhow::ensure!(
        mtime == photo.mtime && metadata.len() == photo.size_bytes,
        "source changed; rescan before indexing"
    );
    let vector = worker.image(&path, space)?;
    state
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
        .save_retrieval_embedding(project, photo, space, &vector, dimensions)
}
