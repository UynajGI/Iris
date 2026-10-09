use super::*;
use crate::worker::AnalysisWorker;
use iris_core::vision::AnalysisSettings;

fn start_job(s: &AppState, id: i64, kind: &str) -> Result<Job, ApiError> {
    let mut jobs = s
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("job lock poisoned"))?;
    if jobs
        .get(&id)
        .is_some_and(|j| matches!(j.progress.state.as_str(), "running" | "paused"))
    {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "project already has an active job".into(),
        ));
    }
    let j = Job {
        progress: JobProgress {
            id: uuid::Uuid::new_v4().to_string(),
            kind: kind.into(),
            state: "running".into(),
            ..Default::default()
        },
        cancel: Arc::new(AtomicBool::new(false)),
        pause: Arc::new(AtomicBool::new(false)),
    };
    jobs.insert(id, j.clone());
    Ok(j)
}
fn update(s: &AppState, id: i64, f: impl FnOnce(&mut JobProgress)) {
    if let Ok(mut jobs) = s.jobs.lock() {
        if let Some(j) = jobs.get_mut(&id) {
            f(&mut j.progress);
            let event = if j.progress.kind == "scan" {
                "scan:progress"
            } else {
                "analysis:stage"
            };
            s.emit(event, id, json!(j.progress));
        }
    }
}
fn emit_analysis(s: &AppState, project_id: i64, photo_id: i64, analysis: Value) {
    let mut payload = json!({"photo_id":photo_id});
    payload["analysis"] = analysis;
    s.emit("verdict:updated", project_id, payload);
}
fn bounded_worker_count(limit: usize, available: usize, pending: usize) -> usize {
    limit.min(available).min(pending)
}
#[cfg(test)]
mod retry_scope_tests {
    use super::*;
    #[test]
    fn source_failures_are_isolated_and_retry_excludes_new_unanalysed_photos() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("photos");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.jpg"), b"a").unwrap();
        std::fs::write(root.join("b.jpg"), b"b").unwrap();
        let state = AppState::new(
            &temp.path().join("db.sqlite"),
            temp.path().join("no-models"),
        )
        .unwrap();
        let (project, first) = {
            let mut services = state.services.lock().unwrap();
            let project = services.create_project(&root).unwrap().id;
            let stamp = std::fs::metadata(root.join("b.jpg"))
                .unwrap()
                .modified()
                .unwrap()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .min(i64::MAX as u128) as i64;
            for (name, mtime) in [("a.jpg", 0), ("b.jpg", stamp)] {
                services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,?2,?2,'jpeg',?3,1,1,1)",[project.to_string(),name.into(),mtime.to_string()]).unwrap();
            }
            let photos = services.photos(project, PhotoFilter::default()).unwrap();
            let settings = services.settings(project).unwrap();
            services.save_analysis(photos[0].id,json!({"version":iris_core::vision::ANALYSIS_VERSION,"settings":settings,"faces":[]})).unwrap();
            services.save_analysis(photos[1].id,json!({"version":iris_core::vision::ANALYSIS_VERSION,"settings":settings,"faces":[]})).unwrap();
            (project, photos[0].id)
        };
        let job = start_job(&state, project, "analysis").unwrap_or_else(|_| panic!("start job"));
        let result = analyze_project(&state, project, &job).unwrap();
        assert_eq!(result["failed"], 1);
        assert_eq!(result["reused"], 1);
        assert_eq!(
            state
                .services
                .lock()
                .unwrap()
                .photo(first)
                .unwrap()
                .analysis_status,
            iris_core::AnalysisStatus::Stale
        );
        finish(&state, project, false, Ok(result));
        let selected = retry_ids(&state, project).unwrap_or_else(|_| panic!("retry IDs"));
        assert_eq!(selected, std::collections::HashSet::from([first]));
        std::fs::write(root.join("new.jpg"), b"n").unwrap();
        state.services.lock().unwrap().store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'new.jpg','new.jpg','jpeg',0,1,1,1)",[project]).unwrap();
        let job = start_job(&state, project, "analysis").unwrap_or_else(|_| panic!("retry job"));
        let result = analyze_project_scoped(&state, project, &job, Some(&selected)).unwrap();
        assert_eq!(result["failed"], 1);
        assert_eq!(result["reused"], 0);
        assert_eq!(state.jobs.lock().unwrap()[&project].progress.total, 1);
        assert_eq!(
            state.jobs.lock().unwrap()[&project]
                .progress
                .failed_photo_ids,
            vec![first]
        );
    }
}
fn finish(s: &AppState, id: i64, cancelled: bool, result: anyhow::Result<Value>) {
    let mut finished = None;
    update(s, id, |p| {
        if p.kind == "scan" {
            p.total = p.completed;
        }
        match result {
            Ok(v) => {
                p.state = if cancelled {
                    "cancelled"
                } else if !p.errors.is_empty() {
                    "failed"
                } else {
                    "completed"
                }
                .into();
                p.result = Some(v)
            }
            Err(e) if cancelled && e.is::<crate::worker::WorkerCancelled>() => {
                p.state = "cancelled".into();
            }
            Err(e) => {
                p.state = "failed".into();
                p.errors.push(e.to_string())
            }
        }
        finished = Some(p.clone());
    });
    if let Some(report) =
        finished.filter(|report| report.kind == "analysis" && !report.execution.is_empty())
    {
        let saved = (|| -> anyhow::Result<()> {
            s.services
                .lock()
                .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
                .save_analysis_run(id, &serde_json::to_value(report)?)
        })();
        if let Err(error) = saved {
            update(s, id, |p| {
                p.errors.push(format!("分析运行记录未保存：{error}"))
            });
        }
    }
}

#[cfg(test)]
mod cancellation_finish_tests {
    use super::*;

    fn state() -> (tempfile::TempDir, AppState) {
        let temporary = tempfile::tempdir().unwrap();
        let state = AppState::new(
            &temporary.path().join("library.sqlite"),
            temporary.path().join("models"),
        )
        .unwrap();
        state.jobs.lock().unwrap().insert(
            1,
            Job {
                progress: JobProgress {
                    kind: "analysis".into(),
                    state: "running".into(),
                    ..Default::default()
                },
                cancel: Arc::new(AtomicBool::new(true)),
                pause: Arc::new(AtomicBool::new(false)),
            },
        );
        (temporary, state)
    }

    #[test]
    fn startup_cancellation_is_terminal_cancelled_through_error_context() {
        let (_temporary, state) = state();
        let error = anyhow::Error::new(crate::worker::WorkerCancelled).context("initialize worker");
        finish(&state, 1, true, Err(error));
        let jobs = state.jobs.lock().unwrap();
        assert_eq!(jobs[&1].progress.state, "cancelled");
        assert!(jobs[&1].progress.errors.is_empty());
    }

    #[test]
    fn cancellation_flag_does_not_hide_real_model_failure_or_message_lookalike() {
        for message in ["model hash mismatch", "analysis cancelled"] {
            let (_temporary, state) = state();
            finish(&state, 1, true, Err(anyhow::anyhow!(message)));
            let jobs = state.jobs.lock().unwrap();
            assert_eq!(jobs[&1].progress.state, "failed");
            assert_eq!(jobs[&1].progress.errors, vec![message]);
        }
    }

    #[test]
    fn cancellation_error_without_active_cancel_signal_is_not_silently_suppressed() {
        let (_temporary, state) = state();
        finish(&state, 1, false, Err(crate::worker::WorkerCancelled.into()));
        assert_eq!(state.jobs.lock().unwrap()[&1].progress.state, "failed");
    }

    #[test]
    fn analysis_event_preserves_nested_analysis_and_wire_envelope() {
        let (_temporary, state) = state();
        let mut receiver = state.events.subscribe();
        let analysis = json!({"version":"test-engine","faces":[{
            "landmarks":[[1.25,2.5],[3.,4.]],"quality":{"available":true,"value":0.75}
        }],"score_breakdown":{"terms":[{"value":null,"weight":0.4}]},
            "warnings":["uncertain"],"structure":vec![0.5;64]});
        let expected = json!({"event":"verdict:updated","project_id":7,
            "data":{"photo_id":23,"analysis":analysis}});
        emit_analysis(&state, 7, 23, analysis);
        assert_eq!(receiver.try_recv().unwrap(), expected);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn worker_policy_is_validated_and_bounded_by_cpu_and_pending_work() {
        let (_temporary, original) = state();
        assert_eq!(original.worker_limit, 12);
        for invalid in [0, 17, usize::MAX] {
            assert!(original.clone().with_worker_limit(invalid).is_err());
        }
        for (limit, available, pending, expected) in [
            (1, 16, 30, 1),
            (2, 16, 30, 2),
            (8, 16, 30, 8),
            (12, 16, 30, 12),
            (16, 32, 30, 16),
            (16, 4, 30, 4),
            (16, 16, 3, 3),
            (16, 16, 0, 0),
        ] {
            let configured = original.clone().with_worker_limit(limit).unwrap();
            assert_eq!(
                bounded_worker_count(configured.worker_limit, available, pending),
                expected
            );
        }
    }
}

#[derive(Default, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ScanRequest {
    pub retry_failed_only: bool,
}
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
        Some(prior.progress.failed_scan_paths.clone())
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

#[derive(Default, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AnalyzeRequest {
    pub retry_failed_only: bool,
}
fn retry_ids(s: &AppState, id: i64) -> Result<std::collections::HashSet<i64>, ApiError> {
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
fn analyze_project_scoped(
    s: &AppState,
    id: i64,
    job: &Job,
    selected: Option<&std::collections::HashSet<i64>>,
) -> anyhow::Result<Value> {
    if job.cancel.load(Ordering::Relaxed) {
        return Ok(json!({"analyzed":0,"reused":0,"failed":0,"workers":0}));
    }
    let started = std::time::Instant::now();
    let (mut photos, settings_value) = {
        let services = s
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
        std::fs::read_dir(&services.project(id)?.root)
            .map_err(|error| anyhow::anyhow!("project folder unavailable: {error}"))?;
        (
            services.photos(id, PhotoFilter::default())?,
            services.settings(id)?,
        )
    };
    if let Some(selected) = selected {
        photos.retain(|photo| selected.contains(&photo.id));
    }
    let read_ms = started.elapsed().as_secs_f64() * 1000.;
    let validation_started = std::time::Instant::now();
    let settings: AnalysisSettings = serde_json::from_value(settings_value.clone())?;
    settings.validate()?;
    if settings.face_detector == iris_core::vision::FaceDetectorProvider::Scrfd500m {
        iris_core::vision::validate_detector_model(&s.model_dir, &settings)?;
    }
    iris_core::vision::validate_occlusion_model(&s.model_dir, &settings)?;
    // A missing optional model may fall back; wrong or damaged bytes never do.
    if settings.embedding_provider == iris_core::vision::EmbeddingProvider::Dinov3Vits16 {
        anyhow::ensure!(
            settings.embedding_model_sha256.as_deref().is_some_and(
                |hash| hash.eq_ignore_ascii_case(iris_core::vision::dinov3::MODEL_SHA256)
            ),
            "DINOv3 model hash is not the approved artifact"
        );
        let status = iris_core::vision::embedding_model_status(&s.model_dir, &settings);
        if status.state == iris_core::vision::ModelAvailability::Invalid {
            anyhow::bail!(
                "DINOv3 artifact invalid: {}",
                status.reason.unwrap_or_default()
            );
        }
    }
    update(s, id, |p| p.total = photos.len());
    let mut pending = Vec::new();
    let mut reused = 0;
    let mut preflight_failures = 0;
    for photo in &photos {
        let resolved = s
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
            .photo_path(photo.id)
            .and_then(|path| {
                verify_fingerprint(photo, &path)?;
                Ok(path)
            });
        let path = match resolved {
            Ok(path) => path,
            Err(error) => {
                preflight_failures += 1;
                s.services
                    .lock()
                    .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
                    .invalidate_source_analysis(photo.id)?;
                update(s, id, |progress| {
                    progress.failed_photo_ids.push(photo.id);
                    progress.errors.push(format!("{}: {error}", photo.filename));
                });
                continue;
            }
        };
        if photo
            .analysis
            .as_ref()
            .is_some_and(|v| iris_core::services::analysis_matches_settings(v, &settings_value))
        {
            reused += 1;
        } else {
            pending.push((photo.clone(), path));
        }
    }
    update(s, id, |p| p.completed = reused + preflight_failures);
    let validation_ms = validation_started.elapsed().as_secs_f64() * 1000.;
    if pending.is_empty() {
        let grouping_started = std::time::Instant::now();
        let semantic = prepare_semantic_groups(s, id, job, &settings_value, selected)?;
        return Ok(
            json!({"analyzed":0,"reused":reused,"failed":preflight_failures,"workers":0,
            "semantic":semantic,
            "timing_ms":{"read":read_ms,"validate_sources":validation_ms,
                "worker_init":0.,"processing":0.,"persist_within_processing":0.,
                "grouping":grouping_started.elapsed().as_secs_f64()*1000.,
                "total":started.elapsed().as_secs_f64()*1000.}}),
        );
    }
    // Bounded persistent child pool: hard deadlines can terminate native calls.
    // Independent runtimes can initialize concurrently without global ORT state races.
    let workers = bounded_worker_count(
        if settings.execution_provider != iris_core::vision::ExecutionProvider::Cpu {
            1
        } else {
            s.worker_limit
        },
        std::thread::available_parallelism().map_or(1, usize::from),
        pending.len(),
    );
    let initialization_started = std::time::Instant::now();
    let engines = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    AnalysisWorker::new_with_settings(
                        &s.model_dir,
                        std::time::Duration::from_secs(30),
                        job.cancel.clone(),
                        &settings,
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("worker initialization panicked")))
            })
            .collect::<anyhow::Result<Vec<_>>>()
    })?;
    let initialization_ms = initialization_started.elapsed().as_secs_f64() * 1000.;
    if let Some(engine) = engines.first() {
        let feedback = engine.execution_feedback("quality");
        update(s, id, |p| p.execution.push(feedback));
    }
    let processing_started = std::time::Instant::now();
    let mut persist_ms = 0.;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (mut analyzed, mut failures) = (0, preflight_failures);
    std::thread::scope(|scope| -> anyhow::Result<()> {
        let (tx, rx) = std::sync::mpsc::sync_channel(workers);
        for mut engine in engines {
            let tx = tx.clone();
            let pending = &pending;
            let next = &next;
            let mut base_settings = settings.clone();
            base_settings.embedding_provider = iris_core::vision::EmbeddingProvider::None;
            base_settings.embedding_model_sha256 = None;
            base_settings.semantic_similarity_threshold = None;
            scope.spawn(move || loop {
                while job.pause.load(Ordering::Relaxed) && !job.cancel.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                if job.cancel.load(Ordering::Relaxed) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some((photo, path)) = pending.get(index) else {
                    break;
                };
                let started = std::time::Instant::now();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    verify_fingerprint(photo, path)?;
                    let value = engine.analyze(path, &base_settings)?;
                    verify_fingerprint(photo, path)?;
                    Ok(value)
                }))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("analysis worker panicked")))
                .and_then(|a| serde_json::to_value(a).map_err(Into::into));
                if job.cancel.load(Ordering::Relaxed) {
                    break;
                }
                if tx
                    .send((
                        photo.clone(),
                        result,
                        started.elapsed().as_secs_f64() * 1000.0,
                    ))
                    .is_err()
                {
                    break;
                }
            });
        }
        drop(tx);
        for (photo, result, elapsed_ms) in rx {
            if job.cancel.load(Ordering::Relaxed) {
                continue;
            }
            match result {
                Ok(mut value) => {
                    let persist_started = std::time::Instant::now();
                    value["settings"] = settings_value.clone();
                    value["elapsed_ms"] = json!(elapsed_ms);
                    let mut service = s
                        .services
                        .lock()
                        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
                    if service.settings(id)? != settings_value {
                        job.cancel.store(true, Ordering::Relaxed);
                        anyhow::bail!("settings changed during analysis; restart analysis to use the new settings")
                    }
                    service.save_analysis(photo.id, &value)?;
                    drop(service);
                    record_execution_success(s, id, "quality", &value);
                    persist_ms += persist_started.elapsed().as_secs_f64() * 1000.;
                    emit_analysis(s, id, photo.id, value);
                    analyzed += 1;
                }
                Err(e) => {
                    failures += 1;
                    update(s, id, |p| {
                        p.errors.push(format!("{}: {e}", photo.filename));
                        p.failed_photo_ids.push(photo.id);
                    });
                }
            }
            update(s, id, |p| p.completed = analyzed + failures + reused);
        }
        Ok(())
    })?;
    let processing_ms = processing_started.elapsed().as_secs_f64() * 1000.;
    let grouping_started = std::time::Instant::now();
    let semantic = prepare_semantic_groups(s, id, job, &settings_value, selected)?;
    Ok(
        json!({"analyzed":analyzed,"reused":reused,"failed":failures,"workers":workers,
        "semantic":semantic,
        "timing_ms":{"read":read_ms,"validate_sources":validation_ms,
            "worker_init":initialization_ms,"processing":processing_ms,
            "persist_within_processing":persist_ms,
            "grouping":grouping_started.elapsed().as_secs_f64()*1000.,
            "total":started.elapsed().as_secs_f64()*1000.}}),
    )
}

fn record_execution_success(s: &AppState, id: i64, phase: &str, value: &Value) {
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

fn prepare_semantic_groups(
    s: &AppState,
    id: i64,
    job: &Job,
    settings: &Value,
    selected: Option<&std::collections::HashSet<i64>>,
) -> anyhow::Result<Value> {
    let parsed: AnalysisSettings = serde_json::from_value(settings.clone())?;
    let mut grouping = parsed.clone();
    let mut report =
        json!({"mode":"phash", "candidates":0, "computed":0, "reused":0, "warnings":[]});
    if parsed.embedding_provider == iris_core::vision::EmbeddingProvider::Dinov3Vits16 {
        let status = iris_core::vision::embedding_model_status(&s.model_dir, &parsed);
        if status.state == iris_core::vision::ModelAvailability::Invalid {
            anyhow::bail!(
                "DINOv3 artifact invalid: {}",
                status.reason.unwrap_or_default()
            );
        }
        let photos = s
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
            .photos(id, PhotoFilter::default())?;
        let items: Vec<_> = photos
            .iter()
            .filter(|p| p.analysis_status == iris_core::AnalysisStatus::Current)
            .filter_map(|p| {
                Some((
                    p.id.to_string(),
                    serde_json::from_value(p.analysis.clone()?).ok()?,
                    p.taken_at.as_deref().and_then(parse_capture_time),
                ))
            })
            .collect();
        let candidates = iris_core::vision::semantic_candidates(&items);
        report["mode"] = json!("dinov3");
        report["candidates"] = json!(candidates.len());
        let mut warnings = Vec::new();
        let mut computed = 0;
        let mut reused = 0;
        let mut worker = None;
        if status.state == iris_core::vision::ModelAvailability::Missing {
            warnings.push(format!(
                "DINOv3 unavailable; pHash fallback: {}",
                status.reason.unwrap_or_default()
            ));
        } else {
            for photo in photos.iter().filter(|p| {
                candidates.contains(&p.id.to_string())
                    && selected.is_none_or(|ids| ids.contains(&p.id))
            }) {
                while job.pause.load(Ordering::Relaxed) && !job.cancel.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                if job.cancel.load(Ordering::Relaxed) {
                    return Err(crate::worker::WorkerCancelled.into());
                }
                let value = photo.analysis.as_ref().expect("current candidate");
                if iris_core::vision::embedding_matches_settings(value, &parsed) {
                    reused += 1;
                    continue;
                }
                let path = s
                    .services
                    .lock()
                    .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
                    .photo_path(photo.id)?;
                verify_fingerprint(photo, &path)?;
                let result = (|| -> anyhow::Result<_> {
                    if worker.is_none() {
                        worker = Some(AnalysisWorker::new_with_settings(
                            &s.model_dir,
                            std::time::Duration::from_secs(30),
                            job.cancel.clone(),
                            &parsed,
                        )?);
                        let feedback = worker.as_ref().unwrap().execution_feedback("embedding");
                        update(s, id, |p| p.execution.push(feedback));
                    }
                    worker.as_mut().unwrap().embed(
                        &path,
                        &parsed,
                        serde_json::from_value(value.clone())?,
                    )
                })();
                verify_fingerprint(photo, &path)?;
                match result {
                    Ok(analysis) => {
                        let mut updated = value.clone();
                        updated["embedding"] = serde_json::to_value(&analysis.embedding)?;
                        updated["warnings"] = serde_json::to_value(&analysis.warnings)?;
                        let mut services = s
                            .services
                            .lock()
                            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
                        anyhow::ensure!(
                            services.settings(id)? == *settings,
                            "settings changed during embedding; restart analysis"
                        );
                        services.save_semantic_embedding(photo.id, &analysis)?;
                        drop(services);
                        let previous = value["warnings"].as_array();
                        let notes: Vec<_> = analysis
                            .warnings
                            .iter()
                            .filter(|warning| {
                                !previous.is_some_and(|entries| {
                                    entries
                                        .iter()
                                        .any(|entry| entry.as_str() == Some(warning.as_str()))
                                })
                            })
                            .collect();
                        record_execution_success(s, id, "embedding", &json!({"warnings": notes}));
                        emit_analysis(s, id, photo.id, updated);
                        computed += 1;
                    }
                    Err(error) => {
                        if job.cancel.load(Ordering::Relaxed) {
                            return Err(error);
                        }
                        // Recheck bytes so corruption cannot masquerade as an inference fallback.
                        iris_core::vision::validate_embedding_model(&s.model_dir, &parsed)?;
                        warnings.push(format!(
                            "DINOv3 unavailable for {}: {error}; pHash fallback",
                            photo.filename
                        ));
                        break;
                    }
                }
            }
        }
        if !warnings.is_empty() {
            grouping.embedding_provider = iris_core::vision::EmbeddingProvider::None;
            report["mode"] = json!("phash_fallback");
        }
        report["computed"] = json!(computed);
        report["reused"] = json!(reused);
        report["warnings"] = json!(warnings);
    }
    let mut services = s
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
    if services.settings(id)? != *settings {
        job.cancel.store(true, Ordering::Relaxed);
        anyhow::bail!("settings changed during analysis; restart analysis to use the new settings")
    }
    let complete = services.photos(id, PhotoFilter::default())?;
    let groups = build_groups(id, &complete, &grouping);
    services.save_groups(id, groups)?;
    Ok(report)
}
fn verify_fingerprint(photo: &Photo, path: &std::path::Path) -> anyhow::Result<()> {
    let metadata = std::fs::metadata(path)?;
    let mtime = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos()
        .min(i64::MAX as u128) as i64;
    anyhow::ensure!(
        metadata.len() == photo.size_bytes && mtime == photo.mtime,
        "photo changed since scan; rescan the project"
    );
    Ok(())
}
fn build_groups(project_id: i64, photos: &[Photo], settings: &AnalysisSettings) -> Vec<BurstGroup> {
    let items: Vec<_> = photos
        .iter()
        .filter(|p| p.analysis_status == iris_core::AnalysisStatus::Current)
        .filter_map(|p| {
            let analysis: iris_core::vision::VisionAnalysis =
                serde::Deserialize::deserialize(p.analysis.as_ref()?).ok()?;
            let taken = p.taken_at.as_deref().and_then(parse_capture_time);
            Some((p.id.to_string(), analysis, taken))
        })
        .collect();
    let burst =
        if settings.embedding_provider == iris_core::vision::EmbeddingProvider::Dinov3Vits16 {
            iris_core::vision::group_semantic(
                &items,
                settings.semantic_similarity_threshold.unwrap_or(1.),
            )
        } else {
            iris_core::vision::group_similar(&items)
        }
        .into_iter()
        .map(|members| ("burst", members));
    let duplicates = iris_core::vision::group_duplicates(&items)
        .into_iter()
        .map(|members| ("duplicate", members));
    burst
        .chain(duplicates)
        .filter_map(|(kind, members)| {
            let mut variants = std::collections::HashSet::new();
            let ids: Vec<i64> = members
                .iter()
                .filter_map(|id| id.parse().ok())
                .filter(|id| {
                    photos
                        .iter()
                        .find(|p| p.id == *id)
                        .and_then(|p| p.capture_variant_id.as_ref())
                        .is_none_or(|v| variants.insert(v.clone()))
                })
                .collect();
            (ids.len() > 1).then(|| BurstGroup {
                id: uuid::Uuid::new_v4().to_string(),
                project_id,
                kind: kind.into(),
                member_photo_ids: ids,
            })
        })
        .collect()
}

#[cfg(test)]
mod group_freshness_tests {
    use super::*;

    #[test]
    fn partial_refresh_failure_keeps_history_but_rebuilds_only_current_members() {
        let temporary = tempfile::tempdir().unwrap();
        let state = AppState::new(
            &temporary.path().join("library.sqlite"),
            temporary.path().join("models"),
        )
        .unwrap();
        let root = temporary.path().join("photos");
        std::fs::create_dir(&root).unwrap();
        let mut services = state.services.lock().unwrap();
        let id = services.create_project(&root).unwrap().id;
        let settings = services.settings(id).unwrap();
        let mut history = json!({
            "width":24,"height":16,"original_width":24,"original_height":16,
            "orientation":1,"faces":[],"sharpness_lap":1.,"sharpness_fft":1.,
            "niqe":null,"exposure":{"mean":100.,"shadow_clip":0.,
            "highlight_clip":0.,"verdict":"normal"},"composite_score":99.,
            "verdict":"recommend","phash":"0","structure":vec![0.5;64],
            "warnings":[],"version":"iris-vision-previous-engine","settings":settings
        });
        let mut ids = Vec::new();
        for name in ["failed.jpg", "refreshed-a.jpg", "refreshed-b.jpg"] {
            services.store.conn.execute(
                "INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height,taken_at) VALUES(?1,?2,?2,'jpeg',0,0,24,16,'2026:01:01 12:00:00')",
                (id, name),
            ).unwrap();
            let photo_id = services.store.conn.last_insert_rowid();
            services.save_analysis(photo_id, &history).unwrap();
            ids.push(photo_id);
        }
        // Model the persistence boundary after one worker fails and two succeed:
        // failed results do not replace their old analysis, successful ones do.
        let old_analysis = history.clone();
        history["version"] = json!(iris_core::vision::ANALYSIS_VERSION);
        history["composite_score"] = json!(50.);
        for photo_id in &ids[1..] {
            services.save_analysis(*photo_id, &history).unwrap();
        }
        assert!(services.project(id).unwrap().groups_dirty);
        drop(services);
        let job = start_job(&state, id, "analysis").unwrap_or_else(|e| panic!("{}", e.1));
        update(&state, id, |p| {
            p.errors.push("failed.jpg: analysis failed".into())
        });
        prepare_semantic_groups(&state, id, &job, &settings, None).unwrap();
        let services = state.services.lock().unwrap();
        let groups = services.groups(id).unwrap();
        assert!(groups.iter().any(|g| g.kind == "burst"));
        assert!(groups.iter().any(|g| g.kind == "duplicate"));
        for group in groups {
            assert_eq!(group.member_photo_ids, ids[1..]);
        }
        assert_eq!(services.photo(ids[0]).unwrap().analysis, Some(old_analysis));
        assert_eq!(
            services.photo(ids[0]).unwrap().analysis_status,
            iris_core::AnalysisStatus::Stale
        );
        assert_eq!(services.project(id).unwrap().pending_analysis, 1);
        assert!(!services.project(id).unwrap().groups_dirty);
    }
}

fn parse_capture_time(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|v| v.timestamp())
        .or_else(|| {
            ["%Y-%m-%d %H:%M:%S", "%Y:%m:%d %H:%M:%S"]
                .iter()
                .find_map(|format| chrono::NaiveDateTime::parse_from_str(value, format).ok())
                .map(|v| v.and_utc().timestamp())
        })
}

#[cfg(test)]
mod grouping_tests {
    use super::*;
    #[test]
    fn scanner_displayed_exif_date_and_original_exif_date_match() {
        let scanner = parse_capture_time("2025-06-18 10:20:30");
        assert!(scanner.is_some());
        assert_eq!(scanner, parse_capture_time("2025:06:18 10:20:30"));
        assert_eq!(scanner, parse_capture_time("2025-06-18T10:20:30Z"));
        assert_eq!(parse_capture_time("unknown"), None);
    }
}

#[utoipa::path(post,path="/api/v1/projects/{id}/export/xmp",params(("id"=i64,Path)),request_body=ExportRequest,responses((status=200,body=ExportReport)))]
pub async fn export_xmp(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<ExportRequest>,
) -> ApiResult<ExportReport> {
    Ok(Json(
        run(s, move |s| s.export_xmp(id, &b.scope, b.overwrite)).await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/export/copy",params(("id"=i64,Path)),request_body=ExportRequest,responses((status=200,body=ExportReport)))]
pub async fn export_copy(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<ExportRequest>,
) -> ApiResult<ExportReport> {
    Ok(Json(
        run(s, move |s| {
            s.export_copy(id, std::path::Path::new(&b.destination), &b.scope)
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/export/csv",params(("id"=i64,Path)),request_body=DestinationRequest,responses((status=200,body=ExportReport)))]
pub async fn export_csv(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<DestinationRequest>,
) -> ApiResult<ExportReport> {
    Ok(Json(
        run(s, move |s| {
            s.export_csv(id, std::path::Path::new(&b.destination))
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/import/csv",params(("id"=i64,Path)),request_body=SourceRequest,responses((status=200,body=DecisionBatch)))]
pub async fn import_csv(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<SourceRequest>,
) -> ApiResult<DecisionBatch> {
    Ok(Json(
        run(s, move |s| {
            s.import_csv(id, std::path::Path::new(&b.source))
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/quarantine/preview",params(("id"=i64,Path)),responses((status=200,body=QuarantinePlan)))]
pub async fn quarantine_preview(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<QuarantinePlan> {
    Ok(Json(run(s, move |s| s.quarantine_preview(id)).await?))
}
#[utoipa::path(get,path="/api/v1/projects/{id}/quarantine",params(("id"=i64,Path)),responses((status=200,body=Vec<QuarantinePlan>)))]
pub async fn quarantine_history(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Vec<QuarantinePlan>> {
    Ok(Json(run(s, move |s| s.quarantine_history(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/quarantine/commit",params(("id"=i64,Path)),request_body=ManifestRequest,responses((status=200,body=QuarantinePlan)))]
pub async fn quarantine_commit(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<ManifestRequest>,
) -> ApiResult<QuarantinePlan> {
    Ok(Json(
        run(s, move |s| {
            anyhow::ensure!(
                s.quarantine_plan(&b.manifest_id)?.project_id == id,
                "manifest belongs to another project"
            );
            s.quarantine_commit(&b.manifest_id)
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/quarantine/restore",params(("id"=i64,Path)),request_body=ManifestRequest,responses((status=200,body=QuarantinePlan)))]
pub async fn quarantine_restore(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<ManifestRequest>,
) -> ApiResult<QuarantinePlan> {
    Ok(Json(
        run(s, move |s| {
            anyhow::ensure!(
                s.quarantine_plan(&b.manifest_id)?.project_id == id,
                "manifest belongs to another project"
            );
            s.quarantine_restore(&b.manifest_id)
        })
        .await?,
    ))
}

#[derive(Deserialize, ToSchema)]
pub struct ProjectRequest {
    pub project_id: i64,
}
#[derive(Serialize, ToSchema)]
pub struct ModelStatusResponse {
    pub selected: iris_core::vision::FaceDetectorProvider,
    pub detectors: Vec<iris_core::vision::DetectorModelStatus>,
    pub occlusion_selected: iris_core::vision::OcclusionProvider,
    pub occlusion: Vec<iris_core::vision::OcclusionModelStatus>,
    pub embedding_selected: iris_core::vision::EmbeddingProvider,
    pub embeddings: Vec<iris_core::vision::EmbeddingModelStatus>,
}

/// Read-only artifact checks; no inference runtime is initialized.
pub fn models_status(s: &AppState, project_id: i64) -> anyhow::Result<ModelStatusResponse> {
    let value = s
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
        .settings(project_id)?;
    let settings: AnalysisSettings = serde_json::from_value(value)?;
    let selected = settings.face_detector;
    let occlusion_selected = settings.occlusion_provider;
    let embedding_selected = settings.embedding_provider;
    let detectors = [
        iris_core::vision::FaceDetectorProvider::Yunet,
        iris_core::vision::FaceDetectorProvider::Scrfd500m,
    ]
    .into_iter()
    .map(|provider| {
        let mut probe = settings.clone();
        probe.face_detector = provider;
        probe.occlusion_provider = iris_core::vision::OcclusionProvider::None;
        iris_core::vision::detector_model_status(&s.model_dir, &probe)
    })
    .collect();
    let occlusion = [
        iris_core::vision::OcclusionProvider::None,
        iris_core::vision::OcclusionProvider::Faceocc,
    ]
    .into_iter()
    .map(|provider| {
        let mut probe = settings.clone();
        probe.face_detector = iris_core::vision::FaceDetectorProvider::Yunet;
        probe.occlusion_provider = provider;
        iris_core::vision::occlusion_model_status(&s.model_dir, &probe)
    })
    .collect();
    let embeddings = [
        iris_core::vision::EmbeddingProvider::None,
        iris_core::vision::EmbeddingProvider::Dinov3Vits16,
    ]
    .into_iter()
    .map(|provider| {
        let mut probe = settings.clone();
        probe.embedding_provider = provider;
        iris_core::vision::embedding_model_status(&s.model_dir, &probe)
    })
    .collect();
    Ok(ModelStatusResponse {
        selected,
        detectors,
        occlusion_selected,
        occlusion,
        embedding_selected,
        embeddings,
    })
}

#[utoipa::path(get,path="/api/v1/models",params(("project_id"=i64,Query)),responses((status=200,body=ModelStatusResponse)))]
pub async fn models(
    State(s): State<AppState>,
    Query(q): Query<ProjectRequest>,
) -> ApiResult<ModelStatusResponse> {
    Ok(Json(
        tokio::task::spawn_blocking(move || models_status(&s, q.project_id))
            .await
            .map_err(anyhow::Error::from)??,
    ))
}
#[utoipa::path(get,path="/api/v1/settings",params(("project_id"=i64,Query)),responses((status=200,body=AnalysisSettings)))]
pub async fn settings(
    State(s): State<AppState>,
    Query(b): Query<ProjectRequest>,
) -> ApiResult<Value> {
    Ok(Json(run(s, move |s| s.settings(b.project_id)).await?))
}
#[utoipa::path(put,path="/api/v1/settings",params(("project_id"=i64,Query)),request_body=AnalysisSettings,responses((status=200,body=AnalysisSettings)))]
pub async fn put_settings(
    State(s): State<AppState>,
    Query(q): Query<ProjectRequest>,
    Json(b): Json<Value>,
) -> ApiResult<Value> {
    Ok(Json(
        run(s, move |s| {
            let _: AnalysisSettings = serde_json::from_value(b.clone())?;
            s.set_settings(q.project_id, b)
        })
        .await?,
    ))
}
#[utoipa::path(get,path="/api/v1/profiles",responses((status=200,body=Vec<Profile>)))]
pub async fn profiles(State(s): State<AppState>) -> ApiResult<Vec<Profile>> {
    Ok(Json(run(s, |s| s.profiles()).await?))
}
#[utoipa::path(post,path="/api/v1/profiles",request_body=Profile,responses((status=200,body=Profile)))]
pub async fn save_profile(State(s): State<AppState>, Json(b): Json<Profile>) -> ApiResult<Profile> {
    Ok(Json(
        run(s, move |s| {
            let _: AnalysisSettings = serde_json::from_value(b.settings.clone())?;
            s.save_profile(&b.name, b.settings)
        })
        .await?,
    ))
}
#[utoipa::path(delete,path="/api/v1/profiles/{name}",params(("name"=String,Path)),responses((status=200,body=bool)))]
pub async fn delete_profile(
    State(s): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<bool> {
    Ok(Json(run(s, move |s| s.delete_profile(&name)).await?))
}
#[utoipa::path(post,path="/api/v1/profiles/{name}/apply",params(("name"=String,Path)),request_body=ProjectRequest,responses((status=200,body=AnalysisSettings)))]
pub async fn apply_profile(
    State(s): State<AppState>,
    Path(name): Path<String>,
    Json(b): Json<ProjectRequest>,
) -> ApiResult<Value> {
    Ok(Json(
        run(s, move |s| s.apply_profile(b.project_id, &name)).await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/profiles/{name}/estimate",params(("id"=i64,Path),("name"=String,Path)),responses((status=200,body=Value)))]
pub async fn estimate_profile(
    State(s): State<AppState>,
    Path((id, name)): Path<(i64, String)>,
) -> ApiResult<Value> {
    Ok(Json(run(s, move |s| s.estimate_profile(id, &name)).await?))
}
#[utoipa::path(get,path="/api/v1/projects/{id}/cache",params(("id"=i64,Path)),responses((status=200,body=CacheStatus)))]
pub async fn cache_status(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<CacheStatus> {
    Ok(Json(run(s, move |s| s.cache_status(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/cache/cleanup",params(("id"=i64,Path)),responses((status=200,body=CacheStatus)))]
pub async fn cache_cleanup(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<CacheStatus> {
    Ok(Json(run(s, move |s| s.cache_cleanup(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/cache/migrate",params(("id"=i64,Path)),request_body=DestinationRequest,responses((status=200,body=CacheStatus)))]
pub async fn cache_migrate(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<DestinationRequest>,
) -> ApiResult<CacheStatus> {
    Ok(Json(
        run(s, move |s| {
            s.cache_migrate(id, std::path::Path::new(&b.destination))
        })
        .await?,
    ))
}

#[utoipa::path(get,path="/api/v1/projects/{id}/cache/migrations",params(("id"=i64,Path)),responses((status=200,body=Vec<CacheMigration>)))]
pub async fn cache_migrations(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Vec<CacheMigration>> {
    Ok(Json(run(s, move |s| s.cache_migrations(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/cache/migrations/{migration_id}/cleanup",params(("id"=i64,Path),("migration_id"=String,Path)),responses((status=200,body=CacheMigration)))]
pub async fn cache_cleanup_old(
    State(s): State<AppState>,
    Path((id, migration_id)): Path<(i64, String)>,
) -> ApiResult<CacheMigration> {
    Ok(Json(
        run(s, move |s| s.cache_cleanup_old(id, &migration_id)).await?,
    ))
}

#[utoipa::path(get,path="/api/v1/photos/{id}/thumb",params(("id"=i64,Path)),responses((status=200,description="JPEG thumbnail",content_type="image/jpeg",body=Vec<u8>)))]
pub async fn thumb(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    media(s, id, 320).await
}
#[utoipa::path(get,path="/api/v1/photos/{id}/preview",params(("id"=i64,Path)),responses((status=200,description="JPEG preview",content_type="image/jpeg",body=Vec<u8>)))]
pub async fn preview(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    media(s, id, 2560).await
}
#[utoipa::path(get,path="/api/v1/photos/{id}/original",params(("id"=i64,Path)),responses((status=200,description="Unmodified source bytes; RAW and HEIC review uses JPEG previews",content((Vec<u8> = "image/jpeg"),(Vec<u8> = "image/png"),(Vec<u8> = "image/webp"),(Vec<u8> = "image/heic"),(Vec<u8> = "application/octet-stream")))))]
pub async fn original(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let path = run(s, move |s| s.photo_path(id)).await?;
    let content_type = match iris_core::vision::supported_format(&path) {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        Some("raw") => "application/octet-stream",
        _ => "image/jpeg",
    };
    let file = tokio::fs::File::open(path).await?;
    let length = file.metadata().await?.len();
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(file));
    Ok(Response::builder()
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(axum::http::header::CONTENT_LENGTH, length)
        .header(axum::http::header::CACHE_CONTROL, "private, no-store")
        .body(body)
        .map_err(anyhow::Error::from)?)
}
async fn media(s: AppState, id: i64, edge: u32) -> Result<Response, ApiError> {
    let bytes = run(s, move |s| s.cached_image(id, edge)).await?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/jpeg"),
            (
                axum::http::header::CACHE_CONTROL,
                "private, max-age=0, must-revalidate",
            ),
        ],
        bytes,
    )
        .into_response())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{id}/scan", post(scan))
        .route("/api/v1/projects/{id}/analyze", post(analyze))
        .route("/api/v1/projects/{id}/export/xmp", post(export_xmp))
        .route("/api/v1/projects/{id}/export/copy", post(export_copy))
        .route("/api/v1/projects/{id}/export/csv", post(export_csv))
        .route("/api/v1/projects/{id}/import/csv", post(import_csv))
        .route("/api/v1/projects/{id}/quarantine", get(quarantine_history))
        .route(
            "/api/v1/projects/{id}/quarantine/preview",
            post(quarantine_preview),
        )
        .route(
            "/api/v1/projects/{id}/quarantine/commit",
            post(quarantine_commit),
        )
        .route(
            "/api/v1/projects/{id}/quarantine/restore",
            post(quarantine_restore),
        )
        .route("/api/v1/settings", get(settings).put(put_settings))
        .route("/api/v1/models", get(models))
        .route("/api/v1/profiles", get(profiles).post(save_profile))
        .route(
            "/api/v1/profiles/{name}",
            axum::routing::delete(delete_profile),
        )
        .route("/api/v1/profiles/{name}/apply", post(apply_profile))
        .route(
            "/api/v1/projects/{id}/profiles/{name}/estimate",
            post(estimate_profile),
        )
        .route("/api/v1/projects/{id}/cache", get(cache_status))
        .route("/api/v1/projects/{id}/cache/cleanup", post(cache_cleanup))
        .route("/api/v1/projects/{id}/cache/migrate", post(cache_migrate))
        .route(
            "/api/v1/projects/{id}/cache/migrations",
            get(cache_migrations),
        )
        .route(
            "/api/v1/projects/{id}/cache/migrations/{migration_id}/cleanup",
            post(cache_cleanup_old),
        )
        .route("/api/v1/photos/{id}/thumb", get(thumb))
        .route("/api/v1/photos/{id}/preview", get(preview))
        .route("/api/v1/photos/{id}/original", get(original))
}
