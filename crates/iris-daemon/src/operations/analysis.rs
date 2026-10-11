use super::*;

mod pool;
mod preflight;

#[utoipa::path(post,path="/api/v1/projects/{id}/analyze",params(("id"=i64,Path)),request_body=Option<AnalyzeRequest>,responses((status=200,body=JobProgress)))]
pub async fn analyze(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    body: axum::body::Bytes,
) -> ApiResult<JobProgress> {
    let request: AnalyzeRequest = if body.is_empty() {
        AnalyzeRequest::default()
    } else {
        serde_json::from_slice(&body).map_err(anyhow::Error::from)?
    };
    let selected = if request.retry_failed_only {
        Some(retry_ids(&s, id)?)
    } else {
        None
    };
    let job = start_job(&s, id, "analysis")?;
    if let Err(e) = run(s.clone(), move |s| s.project(id)).await {
        finish(&s, id, false, Err(anyhow::anyhow!(e.1.clone())));
        return Err(e);
    }
    let initial = job.progress.clone();
    tokio::task::spawn_blocking(move || {
        let result = analyze_project_scoped(&s, id, &job, selected.as_ref());
        finish(&s, id, job.cancel.load(Ordering::Relaxed), result);
    });
    Ok(Json(initial))
}

/// Shared headless analysis runner used by both HTTP and CLI.
pub fn analyze_project(s: &AppState, id: i64, job: &Job) -> anyhow::Result<Value> {
    analyze_project_scoped(s, id, job, None)
}
struct AnalysisContext<'a> {
    state: &'a AppState,
    project_id: i64,
    job: &'a Job,
    settings: &'a AnalysisSettings,
    settings_value: &'a Value,
}

pub(super) fn analyze_project_scoped(
    s: &AppState,
    id: i64,
    job: &Job,
    selected: Option<&std::collections::HashSet<i64>>,
) -> anyhow::Result<Value> {
    if job.cancel.load(Ordering::Relaxed) {
        return Ok(json!({"analyzed":0,"reused":0,"failed":0,"workers":0}));
    }
    let started = std::time::Instant::now();
    let (photos, settings_value) = preflight::read_inputs(s, id, selected)?;
    let read_ms = started.elapsed().as_secs_f64() * 1000.;
    let validation_started = std::time::Instant::now();
    let settings: AnalysisSettings = serde_json::from_value(settings_value.clone())?;
    let context = AnalysisContext {
        state: s,
        project_id: id,
        job,
        settings: &settings,
        settings_value: &settings_value,
    };
    preflight::validate_models(&context)?;
    let plan = preflight::prepare_photos(&context, &photos)?;
    let validation_ms = validation_started.elapsed().as_secs_f64() * 1000.;
    if plan.pending.is_empty() {
        let grouping_started = std::time::Instant::now();
        let semantic = prepare_semantic_groups(s, id, job, &settings_value, selected)?;
        return Ok(
            json!({"analyzed":0,"reused":plan.reused,"failed":plan.failures,"workers":0,
            "semantic":semantic,
            "timing_ms":{"read":read_ms,"validate_sources":validation_ms,
                "worker_init":0.,"processing":0.,"persist_within_processing":0.,
                "grouping":grouping_started.elapsed().as_secs_f64()*1000.,
                "total":started.elapsed().as_secs_f64()*1000.}}),
        );
    }
    let workers = bounded_worker_count(
        if settings.execution_provider != iris_core::vision::ExecutionProvider::Cpu {
            1
        } else {
            s.worker_limit
        },
        std::thread::available_parallelism().map_or(1, usize::from),
        plan.pending.len(),
    );
    let initialization_started = std::time::Instant::now();
    let engines = pool::initialize(&context, workers)?;
    let initialization_ms = initialization_started.elapsed().as_secs_f64() * 1000.;
    if let Some(engine) = engines.first() {
        let feedback = engine.execution_feedback("quality");
        update(s, id, |p| p.execution.push(feedback));
    }
    let processing_started = std::time::Instant::now();
    let processed = pool::process(&context, &plan, engines)?;
    let processing_ms = processing_started.elapsed().as_secs_f64() * 1000.;
    let grouping_started = std::time::Instant::now();
    let semantic = prepare_semantic_groups(s, id, job, &settings_value, selected)?;
    Ok(
        json!({"analyzed":processed.analyzed,"reused":plan.reused,"failed":processed.failures,"workers":workers,
        "semantic":semantic,
        "timing_ms":{"read":read_ms,"validate_sources":validation_ms,
            "worker_init":initialization_ms,"processing":processing_ms,
            "persist_within_processing":processed.persist_ms,
            "grouping":grouping_started.elapsed().as_secs_f64()*1000.,
            "total":started.elapsed().as_secs_f64()*1000.}}),
    )
}

pub(super) fn record_execution_success(s: &AppState, id: i64, phase: &str, value: &Value) {
    update(s, id, |p| {
        let Some(execution) = p.execution.iter_mut().find(|entry| entry.phase == phase) else {
            return;
        };
        execution.completed_items += 1;
        if let Some(warnings) = value["warnings"].as_array() {
            for warning in warnings
                .iter()
                .filter_map(Value::as_str)
                .filter(|text| text.starts_with("DirectML"))
            {
                if execution.warnings.len() < 100
                    && !execution.warnings.iter().any(|entry| entry == warning)
                {
                    execution.warnings.push(warning.into());
                }
            }
        }
    });
}
