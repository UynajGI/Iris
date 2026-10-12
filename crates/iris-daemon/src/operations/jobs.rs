use super::*;

pub(super) fn start_job(s: &AppState, id: i64, kind: &str) -> Result<Job, ApiError> {
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
pub(super) fn update(s: &AppState, id: i64, f: impl FnOnce(&mut JobProgress)) {
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
pub(super) fn emit_analysis(s: &AppState, project_id: i64, photo_id: i64, analysis: Value) {
    let mut payload = json!({"photo_id":photo_id,"analysis_status":"current"});
    payload["analysis"] = analysis;
    s.emit("verdict:updated", project_id, payload);
}
pub(super) fn bounded_worker_count(limit: usize, available: usize, pending: usize) -> usize {
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
pub(super) fn finish(s: &AppState, id: i64, cancelled: bool, result: anyhow::Result<Value>) {
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
            "data":{"photo_id":23,"analysis":analysis,"analysis_status":"current"}});
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
