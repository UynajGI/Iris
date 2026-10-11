use super::*;
use preflight::{AnalysisPlan, PendingPhoto};
use std::sync::{atomic::AtomicUsize, mpsc::SyncSender};

type Outcome = (Photo, anyhow::Result<Value>, f64);

pub(super) struct ProcessingSummary {
    pub analyzed: usize,
    pub failures: usize,
    pub persist_ms: f64,
}

// Bounded persistent children: hard deadlines can terminate native calls.
// Independent runtimes initialize concurrently without global ORT state races.
pub(super) fn initialize(
    context: &AnalysisContext<'_>,
    workers: usize,
) -> anyhow::Result<Vec<AnalysisWorker>> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    AnalysisWorker::new_with_settings(
                        &context.state.model_dir,
                        std::time::Duration::from_secs(30),
                        context.job.cancel.clone(),
                        context.settings,
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
            .collect()
    })
}

fn quality_settings(settings: &AnalysisSettings) -> AnalysisSettings {
    let mut base = settings.clone();
    base.embedding_provider = iris_core::vision::EmbeddingProvider::None;
    base.embedding_model_sha256 = None;
    base.semantic_similarity_threshold = None;
    base
}

fn run_worker(
    job: &Job,
    pending: &[PendingPhoto],
    next: &AtomicUsize,
    mut engine: AnalysisWorker,
    tx: SyncSender<Outcome>,
    settings: &AnalysisSettings,
) {
    let base_settings = quality_settings(settings);
    loop {
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
    }
}

fn persist_result(
    context: &AnalysisContext<'_>,
    photo: &Photo,
    mut value: Value,
    elapsed_ms: f64,
) -> anyhow::Result<(Value, f64)> {
    let persist_started = std::time::Instant::now();
    value["settings"] = context.settings_value.clone();
    value["elapsed_ms"] = json!(elapsed_ms);
    let mut service = context
        .state
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
    if service.settings(context.project_id)? != *context.settings_value {
        context.job.cancel.store(true, Ordering::Relaxed);
        anyhow::bail!("settings changed during analysis; restart analysis to use the new settings")
    }
    service.save_analysis(photo.id, &value)?;
    drop(service);
    record_execution_success(context.state, context.project_id, "quality", &value);
    Ok((value, persist_started.elapsed().as_secs_f64() * 1000.))
}

pub(super) fn process(
    context: &AnalysisContext<'_>,
    plan: &AnalysisPlan,
    engines: Vec<AnalysisWorker>,
) -> anyhow::Result<ProcessingSummary> {
    let next = AtomicUsize::new(0);
    let mut summary = ProcessingSummary {
        analyzed: 0,
        failures: plan.failures,
        persist_ms: 0.,
    };
    std::thread::scope(|scope| -> anyhow::Result<()> {
        let (tx, rx) = std::sync::mpsc::sync_channel(engines.len());
        for engine in engines {
            let tx = tx.clone();
            let next = &next;
            scope.spawn(move || {
                run_worker(
                    context.job,
                    &plan.pending,
                    next,
                    engine,
                    tx,
                    context.settings,
                )
            });
        }
        drop(tx);
        for (photo, result, elapsed_ms) in rx {
            if context.job.cancel.load(Ordering::Relaxed) {
                continue;
            }
            match result {
                Ok(value) => {
                    let (value, persist_ms) = persist_result(context, &photo, value, elapsed_ms)?;
                    summary.persist_ms += persist_ms;
                    emit_analysis(context.state, context.project_id, photo.id, value);
                    summary.analyzed += 1;
                }
                Err(error) => {
                    summary.failures += 1;
                    update(context.state, context.project_id, |p| {
                        p.errors.push(format!("{}: {error}", photo.filename));
                        p.failed_photo_ids.push(photo.id);
                    });
                }
            }
            update(context.state, context.project_id, |p| {
                p.completed = summary.analyzed + summary.failures + plan.reused
            });
        }
        Ok(())
    })?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_settings_cancel_before_saving_a_worker_result() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("photos");
        std::fs::create_dir(&root).unwrap();
        let state = AppState::new(
            &temporary.path().join("db.sqlite"),
            temporary.path().join("models"),
        )
        .unwrap();
        let (project_id, photo, settings_value) = {
            let mut services = state.services.lock().unwrap();
            let project_id = services.create_project(&root).unwrap().id;
            services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'a.jpg','a.jpg','jpeg',0,1,1,1)", [project_id]).unwrap();
            let photo = services
                .photos(project_id, PhotoFilter::default())
                .unwrap()
                .remove(0);
            (project_id, photo, services.settings(project_id).unwrap())
        };
        let job = start_job(&state, project_id, "analysis").unwrap_or_else(|_| panic!("start job"));
        let settings: AnalysisSettings = serde_json::from_value(settings_value.clone()).unwrap();
        let context = AnalysisContext {
            state: &state,
            project_id,
            job: &job,
            settings: &settings,
            settings_value: &settings_value,
        };
        let mut next = settings_value.clone();
        next["eyes_weight"] = json!(30.);
        state
            .services
            .lock()
            .unwrap()
            .set_settings(project_id, next)
            .unwrap();
        let error = persist_result(&context, &photo, json!({"faces":[]}), 10.).unwrap_err();
        assert!(error
            .to_string()
            .contains("settings changed during analysis"));
        assert!(job.cancel.load(Ordering::Relaxed));
        let count: i64 = state
            .services
            .lock()
            .unwrap()
            .store
            .conn
            .query_row("SELECT count(*) FROM analyses", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn quality_phase_disables_only_embedding_settings() {
        let original = AnalysisSettings::default();
        let mut semantic = original.clone();
        semantic.embedding_provider = iris_core::vision::EmbeddingProvider::Dinov3Vits16;
        semantic.embedding_model_sha256 = Some("unchanged-input".into());
        semantic.semantic_similarity_threshold = Some(0.8);
        let base = quality_settings(&semantic);
        assert_eq!(
            serde_json::to_value(base).unwrap(),
            serde_json::to_value(original).unwrap()
        );
        assert_eq!(
            semantic.embedding_model_sha256.as_deref(),
            Some("unchanged-input")
        );
        assert_eq!(semantic.semantic_similarity_threshold, Some(0.8));
    }
}
