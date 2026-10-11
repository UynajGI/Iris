use super::*;

#[utoipa::path(post,path="/api/v1/projects/{id}/scan",params(("id"=i64,Path)),request_body=Option<ScanRequest>,responses((status=200,body=JobProgress)))]
pub async fn scan(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    body: axum::body::Bytes,
) -> ApiResult<JobProgress> {
    let request: ScanRequest = if body.is_empty() {
        ScanRequest::default()
    } else {
        serde_json::from_slice(&body).map_err(anyhow::Error::from)?
    };
    let selected = if request.retry_failed_only {
        Some(scan_retry_paths(&s, id)?)
    } else {
        None
    };
    let job = start_job(&s, id, "scan")?;
    if let Err(e) = run(s.clone(), move |s| s.project(id)).await {
        finish(&s, id, false, Err(anyhow::anyhow!(e.1.clone())));
        return Err(e);
    }
    let initial = job.progress.clone();
    tokio::task::spawn_blocking(move || {
        let result = (|| {
            let database = {
                s.services
                    .lock()
                    .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
                    .store
                    .path
                    .clone()
            };
            let mut services = Services::new(Store::open(database)?)
                .with_media_runtime_dir(s.model_dir.join("media"));
            let r =
                services.scan_scoped_with_cancel(id, selected.as_deref(), &job.cancel, |r| {
                    update(&s, id, |p| {
                        p.found_photos = r.added + r.changed + r.unchanged;
                        p.failed_scan_paths = r.failed_paths.clone();
                        p.root_unavailable = r.root_unavailable;
                        p.completed = r.added + r.changed + r.unchanged + r.skipped;
                        p.errors = r.errors.clone()
                    });
                    while job.pause.load(Ordering::Relaxed) && !job.cancel.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                })?;
            // Skips and errors can follow the last per-photo callback, or be the
            // only entries. Publish the authoritative final report before finish.
            update(&s, id, |p| {
                p.found_photos = r.added + r.changed + r.unchanged;
                p.failed_scan_paths = r.failed_paths.clone();
                p.root_unavailable = r.root_unavailable;
                p.completed = r.added + r.changed + r.unchanged + r.skipped;
                p.errors = r.errors.clone();
            });
            Ok(json!(r))
        })();
        finish(&s, id, job.cancel.load(Ordering::Relaxed), result);
    });
    Ok(Json(initial))
}

fn scan_retry_paths(s: &AppState, id: i64) -> Result<Vec<String>, ApiError> {
    let jobs = s
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("job lock poisoned"))?;
    let prior = jobs
        .get(&id)
        .filter(|job| {
            job.progress.kind == "scan"
                && !matches!(job.progress.state.as_str(), "running" | "paused")
                && !job.progress.failed_scan_paths.is_empty()
        })
        .ok_or_else(|| anyhow::anyhow!("no scan failures available to retry"))?;
    Ok(prior.progress.failed_scan_paths.clone())
}

#[derive(Default, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AnalyzeRequest {
    pub retry_failed_only: bool,
}
pub(super) fn retry_ids(s: &AppState, id: i64) -> Result<std::collections::HashSet<i64>, ApiError> {
    let live = s
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("job lock poisoned"))?
        .get(&id)
        .map(|job| job.progress.clone());
    let previous = if live.is_some() {
        live
    } else {
        s.services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
            .analysis_run(id)?
            .map(serde_json::from_value::<JobProgress>)
            .transpose()
            .map_err(anyhow::Error::from)?
    };
    let previous = previous.ok_or_else(|| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "no previous analysis failures".into(),
        )
    })?;
    if previous.kind != "analysis"
        || matches!(previous.state.as_str(), "running" | "paused")
        || previous.failed_photo_ids.is_empty()
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "no failed photos available to retry".into(),
        ));
    }
    Ok(previous.failed_photo_ids.iter().copied().collect())
}
