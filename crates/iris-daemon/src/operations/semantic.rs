use super::*;

pub(super) fn prepare_semantic_groups(
    s: &AppState,
    id: i64,
    job: &Job,
    settings: &Value,
    selected: Option<&std::collections::HashSet<i64>>,
) -> anyhow::Result<Value> {
    let parsed: AnalysisSettings = serde_json::from_value(settings.clone())?;
    let mut grouping = parsed.clone();
    let report = if parsed.embedding_provider == iris_core::vision::EmbeddingProvider::Dinov3Vits16
    {
        dinov3_report(s, id, job, settings, &parsed, selected)?
    } else {
        json!({"mode":"phash", "candidates":0, "computed":0, "reused":0, "warnings":[]})
    };
    if report["mode"] == "phash_fallback" {
        grouping.embedding_provider = iris_core::vision::EmbeddingProvider::None;
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

fn semantic_candidates(photos: &[Photo]) -> std::collections::HashSet<String> {
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
    iris_core::vision::semantic_candidates(&items)
}

fn dinov3_report(
    s: &AppState,
    id: i64,
    job: &Job,
    settings_value: &Value,
    settings: &AnalysisSettings,
    selected: Option<&std::collections::HashSet<i64>>,
) -> anyhow::Result<Value> {
    let status = iris_core::vision::embedding_model_status(&s.model_dir, settings);
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
    let candidates = semantic_candidates(&photos);
    let make_pass = || EmbeddingPass {
        state: s,
        project_id: id,
        job,
        settings_value,
        settings,
        force_cpu: false,
        worker: None,
        warnings: Vec::new(),
        computed: 0,
        reused: 0,
    };
    let mut pass = make_pass();
    if status.state == iris_core::vision::ModelAvailability::Missing {
        pass.warnings.push(format!(
            "DINOv3 unavailable; pHash fallback: {}",
            status.reason.unwrap_or_default()
        ));
    } else {
        let mut pending: Vec<_> = photos
            .iter()
            .filter(|photo| {
                candidates.contains(&photo.id.to_string())
                    && selected.is_none_or(|ids| ids.contains(&photo.id))
            })
            .collect();
        pending.retain(|photo| {
            let cached = iris_core::vision::embedding_matches_settings(
                photo.analysis.as_ref().expect("current candidate"),
                settings,
            );
            pass.reused += usize::from(cached);
            !cached
        });
        if !pending.is_empty() {
            pass.ensure_worker()?;
        }
        let cpu = pass.worker.as_ref().is_some_and(|worker| {
            worker.execution_feedback("embedding").selected_provider
                == iris_core::vision::ExecutionProvider::Cpu
        });
        let workers = if cpu {
            s.worker_limit.min(4).min(
                std::thread::available_parallelism()
                    .map_or(1, usize::from)
                    .saturating_sub(2)
                    .max(1),
            )
        } else {
            1
        }
        .min(pending.len());
        let passes = run_embedding_workers(make_pass, &pending, workers, cpu, pass.worker.take())?;
        for completed in passes {
            pass.computed += completed.computed;
            pass.reused += completed.reused;
            pass.warnings.extend(completed.warnings);
        }
    }
    Ok(
        json!({"mode":if pass.warnings.is_empty() { "dinov3" } else { "phash_fallback" },
        "candidates":candidates.len(),"computed":pass.computed,"reused":pass.reused,"warnings":pass.warnings}),
    )
}

struct EmbeddingPass<'a> {
    state: &'a AppState,
    project_id: i64,
    job: &'a Job,
    settings_value: &'a Value,
    settings: &'a AnalysisSettings,
    force_cpu: bool,
    worker: Option<AnalysisWorker>,
    warnings: Vec<String>,
    computed: usize,
    reused: usize,
}

impl EmbeddingPass<'_> {
    // false means an inference fallback stopped the embedding pass, not a source failure.
    fn compute_photo(&mut self, photo: &Photo) -> anyhow::Result<bool> {
        wait_for_semantic_job(self.job)?;
        let value = photo.analysis.as_ref().expect("current candidate");
        if iris_core::vision::embedding_matches_settings(value, self.settings) {
            self.reused += 1;
            return Ok(true);
        }
        let path = self
            .state
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
            .photo_path(photo.id)?;
        verify_fingerprint(photo, &path)?;
        let result = self.embed(&path, value);
        // Keep this check outside inference fallback handling, including failed inference.
        verify_fingerprint(photo, &path)?;
        match result {
            Ok(analysis) => {
                self.save_embedding(photo, value, &analysis)?;
                self.computed += 1;
                Ok(true)
            }
            Err(error) => {
                if self.job.cancel.load(Ordering::Relaxed) {
                    return Err(error);
                }
                // Recheck bytes so corruption cannot masquerade as an inference fallback.
                iris_core::vision::validate_embedding_model(&self.state.model_dir, self.settings)?;
                self.warnings.push(format!(
                    "DINOv3 unavailable for {}: {error}; pHash fallback",
                    photo.filename
                ));
                Ok(false)
            }
        }
    }

    fn ensure_worker(&mut self) -> anyhow::Result<()> {
        if self.worker.is_none() {
            let mut execution_settings = self.settings.clone();
            if self.force_cpu {
                execution_settings.execution_provider = iris_core::vision::ExecutionProvider::Cpu;
                execution_settings.directml_device_id = None;
            }
            self.worker = Some(AnalysisWorker::new_with_settings(
                &self.state.model_dir,
                std::time::Duration::from_secs(30),
                self.job.cancel.clone(),
                &execution_settings,
            )?);
            let feedback = self
                .worker
                .as_ref()
                .unwrap()
                .execution_feedback("embedding");
            update(self.state, self.project_id, |p| {
                if !p.execution.iter().any(|entry| entry.phase == "embedding") {
                    p.execution.push(feedback);
                }
            });
        }
        Ok(())
    }

    fn embed(
        &mut self,
        path: &std::path::Path,
        value: &Value,
    ) -> anyhow::Result<iris_core::vision::VisionAnalysis> {
        self.ensure_worker()?;
        self.worker.as_mut().unwrap().embed(
            path,
            self.settings,
            serde_json::from_value(value.clone())?,
        )
    }

    fn save_embedding(
        &self,
        photo: &Photo,
        value: &Value,
        analysis: &iris_core::vision::VisionAnalysis,
    ) -> anyhow::Result<()> {
        let mut updated = value.clone();
        updated["embedding"] = serde_json::to_value(&analysis.embedding)?;
        updated["warnings"] = serde_json::to_value(&analysis.warnings)?;
        let mut services = self
            .state
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
        anyhow::ensure!(
            services.settings(self.project_id)? == *self.settings_value,
            "settings changed during embedding; restart analysis"
        );
        services.save_semantic_embedding(photo.id, analysis)?;
        drop(services);
        let notes = new_execution_warnings(value, &analysis.warnings);
        record_execution_success(
            self.state,
            self.project_id,
            "embedding",
            &json!({"warnings": notes}),
        );
        emit_analysis(self.state, self.project_id, photo.id, updated);
        Ok(())
    }
}

pub(super) fn wait_for_semantic_job(job: &Job) -> anyhow::Result<()> {
    while job.pause.load(Ordering::Relaxed) && !job.cancel.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if job.cancel.load(Ordering::Relaxed) {
        return Err(crate::worker::WorkerCancelled.into());
    }
    Ok(())
}

fn new_execution_warnings<'a>(previous: &Value, warnings: &'a [String]) -> Vec<&'a String> {
    let previous = previous["warnings"].as_array();
    warnings
        .iter()
        .filter(|warning| {
            !previous.is_some_and(|entries| {
                entries
                    .iter()
                    .any(|entry| entry.as_str() == Some(warning.as_str()))
            })
        })
        .collect()
}

pub(super) fn verify_fingerprint(photo: &Photo, path: &std::path::Path) -> anyhow::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_feedback_reports_only_new_warnings_in_original_order() {
        let warnings = vec!["old".into(), "DirectML fallback".into(), "other".into()];
        assert_eq!(
            new_execution_warnings(&json!({"warnings":["old",17]}), &warnings),
            vec![&warnings[1], &warnings[2]]
        );
        assert_eq!(
            new_execution_warnings(&json!({"warnings":null}), &warnings),
            warnings.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn semantic_cancellation_wins_over_pause() {
        let job = Job {
            progress: JobProgress::default(),
            cancel: Arc::new(AtomicBool::new(true)),
            pause: Arc::new(AtomicBool::new(true)),
        };
        let error = wait_for_semantic_job(&job).unwrap_err();
        assert!(error.is::<crate::worker::WorkerCancelled>());
    }
}
fn run_embedding_workers<'a>(
    make_pass: impl Fn() -> EmbeddingPass<'a>,
    pending: &[&Photo],
    workers: usize,
    cpu: bool,
    mut seed: Option<AnalysisWorker>,
) -> anyhow::Result<Vec<EmbeddingPass<'a>>> {
    let queue = Mutex::new(crate::analysis_queue::PendingOrder::new(
        pending.iter().map(|photo| photo.id),
    ));
    let stop = AtomicBool::new(false);

    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let mut pass = make_pass();
                pass.force_cpu = cpu;
                pass.worker = seed.take();
                let pending = &pending;
                let queue = &queue;
                let stop = &stop;
                scope.spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        wait_for_semantic_job(pass.job)?;
                        let browsing = pass.state.browse_contexts.current(pass.project_id);
                        let index = queue
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .claim(&browsing);
                        let Some(index) = index else {
                            break;
                        };
                        let photo = pending[index];
                        match pass.compute_photo(photo) {
                            Ok(true) => {}
                            Ok(false) => {
                                stop.store(true, Ordering::Relaxed);
                                break;
                            }
                            Err(error) => {
                                stop.store(true, Ordering::Relaxed);
                                return Err(error);
                            }
                        }
                    }
                    Ok(pass)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("embedding worker panicked")))
            })
            .collect::<anyhow::Result<Vec<_>>>()
    })
}
