use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use iris_core::Action;
use iris_daemon::{analyze_project, router, AppState, Job, JobProgress};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tower::ServiceExt;

// A valid generated 2x2 JPEG; no private photos or model outputs in lifecycle tests.
const JPEG:&str="ffd8ffe000104a46494600010100000100010000ffdb004300100b0c0e0c0a100e0d0e1211101318281a181616183123251d283a333d3c3933383740485c4e404457453738506d51575f626768673e4d71797064785c656763ffdb0043011112121815182f1a1a2f6342384263636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363ffc00011080002000203012200021101031101ffc4001500010100000000000000000000000000000005ffc40014100100000000000000000000000000000000ffc40014010100000000000000000000000000000002ffc40014110100000000000000000000000000000000ffda000c03010002110311003f008c0183ffd9";
fn jpeg(path: &Path) {
    let bytes: Vec<_> = (0..JPEG.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&JPEG[i..i + 2], 16).unwrap())
        .collect();
    std::fs::write(path, bytes).unwrap();
}
fn fixture(count: usize) -> (tempfile::TempDir, AppState, Router, i64) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    for i in 0..count {
        jpeg(&root.join(format!("{i:05}.jpg")));
    }
    let state = AppState::new(
        &tmp.path().join("db.sqlite"),
        tmp.path().join("missing-models"),
    )
    .unwrap();
    let id = state
        .services
        .lock()
        .unwrap()
        .create_project(root)
        .unwrap()
        .id;
    let app = router(state.clone());
    (tmp, state, app, id)
}
fn job(kind: &str) -> Job {
    Job {
        progress: JobProgress {
            kind: kind.into(),
            state: "running".into(),
            ..Default::default()
        },
        cancel: Arc::new(AtomicBool::new(false)),
        pause: Arc::new(AtomicBool::new(false)),
    }
}
async fn call(
    app: &Router,
    state: &AppState,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {}", state.token))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn progress(app: &Router, state: &AppState, id: i64) -> Value {
    call(
        app,
        state,
        "GET",
        &format!("/api/v1/projects/{id}/progress"),
        Value::Null,
    )
    .await
    .1
}
async fn terminal(app: &Router, state: &AppState, id: i64) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let p = progress(app, state, id).await;
            if matches!(
                p["state"].as_str(),
                Some("completed" | "cancelled" | "failed")
            ) {
                return p;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("job failed to reach terminal state")
}

#[tokio::test]
async fn recent_projects_and_manifest_cache_cleanup_have_authenticated_scoped_routes() {
    let (tmp, state, app, id) = fixture(0);
    let source = std::path::PathBuf::from(
        state
            .services
            .lock()
            .unwrap()
            .project(id)
            .unwrap()
            .cache_root,
    );
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("old.bin"), b"cache").unwrap();
    let destination = tmp.path().join("migrated");
    let migration_route = format!("/api/v1/projects/{id}/cache/migrations");
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{id}/cache/migrate"),
            json!({"destination":destination})
        )
        .await
        .0,
        StatusCode::OK
    );
    let history = call(&app, &state, "GET", &migration_route, Value::Null)
        .await
        .1;
    let migration = history[0]["id"].as_str().unwrap();
    assert_eq!(history[0]["state"], "ready");
    std::fs::write(source.join("unlisted.txt"), b"preserve").unwrap();
    let other_root = tmp.path().join("other");
    std::fs::create_dir(&other_root).unwrap();
    let other = state
        .services
        .lock()
        .unwrap()
        .create_project(other_root)
        .unwrap()
        .id;
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{other}/cache/migrations/{migration}/cleanup"),
            json!({})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert!(source.join("old.bin").exists());
    let cleanup = format!("{migration_route}/{migration}/cleanup");
    for (method, path) in [
        ("GET", migration_route.clone()),
        ("POST", cleanup.clone()),
        ("POST", format!("/api/v1/projects/{id}/open")),
        ("POST", format!("/api/v1/projects/{id}/hide")),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        call(&app, &state, "POST", &cleanup, json!({})).await.1["state"],
        "cleaned"
    );
    assert!(!source.join("old.bin").exists());
    assert!(source.join("unlisted.txt").exists());
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{id}/hide"),
            json!({})
        )
        .await
        .1["hidden"],
        true
    );
    let projects = call(&app, &state, "GET", "/api/v1/projects", Value::Null)
        .await
        .1;
    assert_eq!(projects.as_array().unwrap().len(), 1);
    assert_eq!(projects[0]["id"], other);
    assert_eq!(
        call(
            &app,
            &state,
            "GET",
            &format!("/api/v1/projects/{id}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    let opened = call(
        &app,
        &state,
        "POST",
        &format!("/api/v1/projects/{id}/open"),
        json!({}),
    )
    .await
    .1;
    assert_eq!(opened["hidden"], false);
    assert!(opened["last_opened_at"].as_str().is_some());
    assert_eq!(
        call(&app, &state, "GET", "/api/v1/projects", Value::Null)
            .await
            .1[0]["id"],
        id
    );
}

#[tokio::test]
async fn controls_change_flags_and_reject_terminal_or_missing_jobs() {
    let (_tmp, state, app, id) = fixture(0);
    let active = job("analysis");
    state.jobs.lock().unwrap().insert(id, active.clone());
    let endpoint = |action: &str| format!("/api/v1/projects/{id}/{action}");
    assert_eq!(
        call(&app, &state, "POST", &endpoint("pause"), json!({}))
            .await
            .1["state"],
        "paused"
    );
    assert!(active.pause.load(Ordering::Relaxed));
    assert_eq!(
        call(&app, &state, "POST", &endpoint("resume"), json!({}))
            .await
            .1["state"],
        "running"
    );
    assert!(!active.pause.load(Ordering::Relaxed));
    call(&app, &state, "POST", &endpoint("pause"), json!({})).await;
    assert_eq!(
        call(&app, &state, "POST", &endpoint("cancel"), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    assert!(active.cancel.load(Ordering::Relaxed));
    assert!(!active.pause.load(Ordering::Relaxed));
    state
        .jobs
        .lock()
        .unwrap()
        .get_mut(&id)
        .unwrap()
        .progress
        .state = "cancelled".into();
    for action in ["pause", "resume", "cancel"] {
        assert_eq!(
            call(&app, &state, "POST", &endpoint(action), json!({}))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            "/api/v1/projects/999/cancel",
            json!({})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_job_rejects_duplicate_without_waiting_for_busy_database() {
    let (_tmp, state, app, id) = fixture(0);
    state.jobs.lock().unwrap().insert(id, job("scan"));
    for action in ["scan", "analyze"] {
        let guard = state.services.lock().unwrap();
        let response = tokio::time::timeout(
            Duration::from_millis(250),
            call(
                &app,
                &state,
                "POST",
                &format!("/api/v1/projects/{id}/{action}"),
                json!({}),
            ),
        )
        .await;
        drop(guard);
        assert!(
            response.is_ok(),
            "duplicate {action} waited for active scan's database lock"
        );
        assert_eq!(response.unwrap().0, StatusCode::CONFLICT);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_scan_pause_resume_cancel_and_terminal_progress() {
    let (_tmp, state, app, id) = fixture(1000);
    let endpoint = |action: &str| format!("/api/v1/projects/{id}/{action}");
    assert_eq!(
        call(&app, &state, "POST", &endpoint("scan"), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    let paused = call(&app, &state, "POST", &endpoint("pause"), json!({})).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let settled = progress(&app, &state, id).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let later = progress(&app, &state, id).await;
    let library = tokio::time::timeout(
        Duration::from_millis(500),
        call(
            &app,
            &state,
            "GET",
            &format!("/api/v1/projects/{id}/photos"),
            Value::Null,
        ),
    )
    .await;
    let settings = tokio::time::timeout(
        Duration::from_millis(500),
        call(
            &app,
            &state,
            "GET",
            &format!("/api/v1/settings?project_id={id}"),
            Value::Null,
        ),
    )
    .await;
    let resumed = call(&app, &state, "POST", &endpoint("resume"), json!({})).await;
    let cancel = call(&app, &state, "POST", &endpoint("cancel"), json!({})).await;
    let final_progress = terminal(&app, &state, id).await;
    assert_eq!(paused.0, StatusCode::OK);
    assert_eq!(settled["state"], "paused");
    assert_eq!(later["state"], "paused");
    assert_eq!(
        later["completed"], settled["completed"],
        "scan advanced while paused"
    );
    assert_eq!(resumed.0, StatusCode::OK);
    assert_eq!(cancel.0, StatusCode::OK);
    assert_eq!(final_progress["state"], "cancelled");
    assert!(final_progress["completed"].as_u64().unwrap() < 1000);
    let result = &final_progress["result"];
    let completed: u64 = ["added", "changed", "unchanged", "skipped"]
        .iter()
        .map(|key| result[key].as_u64().unwrap())
        .sum();
    assert_eq!(final_progress["completed"], completed);
    assert_eq!(final_progress["total"], completed);
    assert_eq!(final_progress["errors"], result["errors"]);
    assert_eq!(result["cancelled"], true);
    assert!(
        library.is_ok(),
        "paused scan held the shared service lock and blocked library reads"
    );
    assert_eq!(library.unwrap().0, StatusCode::OK);
    assert!(settings.is_ok(), "paused scan blocked settings reads");
    assert_eq!(settings.unwrap().0, StatusCode::OK);
}

#[tokio::test]
async fn scan_completion_progress_counts_work_and_can_restart() {
    let (_tmp, state, app, id) = fixture(3);
    let scan = format!("/api/v1/projects/{id}/scan");
    for run in 0..2 {
        assert_eq!(
            call(&app, &state, "POST", &scan, json!({})).await.0,
            StatusCode::OK
        );
        let p = terminal(&app, &state, id).await;
        assert_eq!(p["state"], "completed");
        assert_eq!(p["completed"], 3);
        assert_eq!(p["total"], 3);
        assert_eq!(p["result"][if run == 0 { "added" } else { "unchanged" }], 3);
    }
}

#[tokio::test]
async fn broken_only_scan_fails_with_errors_separate_from_successful_or_skipped_count() {
    let (tmp, state, app, id) = fixture(0);
    std::fs::write(tmp.path().join("photos/broken.jpg"), b"invalid JPEG").unwrap();
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{id}/scan"),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    let progress = terminal(&app, &state, id).await;
    assert_eq!(progress["state"], "failed");
    assert_eq!(progress["errors"], progress["result"]["errors"]);
    assert_eq!(progress["errors"].as_array().unwrap().len(), 1);
    // Errors can describe directories, so they do not increment the photo count.
    assert_eq!(progress["completed"], 0);
    assert_eq!(progress["total"], 0);
}

#[tokio::test]
async fn skipped_only_scan_counts_final_report_without_photo_callbacks() {
    let (tmp, state, app, id) = fixture(0);
    for filename in ["notes.txt", "unsupported.bin"] {
        std::fs::write(
            tmp.path().join("photos").join(filename),
            b"not a supported photo",
        )
        .unwrap();
    }
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{id}/scan"),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    let progress = terminal(&app, &state, id).await;
    assert_eq!(progress["state"], "completed");
    assert_eq!(progress["completed"], 2);
    assert_eq!(progress["total"], 2);
    assert_eq!(progress["result"]["skipped"], 2);
    assert_eq!(progress["errors"], json!([]));
}

#[tokio::test]
async fn mixed_scan_retains_final_errors_and_skips_after_valid_photos() {
    let (tmp, state, app, id) = fixture(1);
    std::fs::write(tmp.path().join("photos/zz-broken.jpg"), b"invalid JPEG").unwrap();
    std::fs::write(tmp.path().join("photos/zz-skipped.txt"), b"notes").unwrap();
    let endpoint = format!("/api/v1/projects/{id}/scan");
    for run in 0..2 {
        assert_eq!(
            call(&app, &state, "POST", &endpoint, json!({})).await.0,
            StatusCode::OK
        );
        let progress = terminal(&app, &state, id).await;
        assert_eq!(progress["state"], "failed");
        assert_eq!(progress["completed"], 2);
        assert_eq!(progress["total"], 2);
        assert_eq!(
            progress["result"][if run == 0 { "added" } else { "unchanged" }],
            1
        );
        assert_eq!(progress["result"]["skipped"], 1);
        assert_eq!(progress["errors"], progress["result"]["errors"]);
        assert_eq!(
            progress["errors"].as_array().unwrap().len(),
            1,
            "no duplicate or retained prior-run errors"
        );
    }
}

#[tokio::test]
async fn quarantine_manifest_cannot_be_committed_or_restored_via_another_project() {
    let (tmp, state, app, id) = fixture(1);
    let other_root = tmp.path().join("other");
    std::fs::create_dir(&other_root).unwrap();
    let (other, plan, source) = {
        let mut service = state.services.lock().unwrap();
        service.scan(id).unwrap();
        let p = service.photos(id, Default::default()).unwrap().remove(0);
        let source = service.photo_path(p.id).unwrap();
        service
            .decisions(id, &[p.id], Action::Reject, "human", false)
            .unwrap();
        let plan = service.quarantine_preview(id).unwrap();
        (service.create_project(other_root).unwrap().id, plan, source)
    };
    let body = json!({"manifest_id":plan.id});
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{other}/quarantine/commit"),
            body.clone()
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert!(source.exists());
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{id}/quarantine/commit"),
            body.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(!source.exists());
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{other}/quarantine/restore"),
            body.clone()
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert!(!source.exists());
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &format!("/api/v1/projects/{id}/quarantine/restore"),
            body
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(source.exists());
}

#[test]
fn cancelled_analysis_does_not_initialize_missing_models() {
    let (_tmp, state, _app, id) = fixture(0);
    let active = job("analysis");
    active.cancel.store(true, Ordering::Relaxed);
    let result = analyze_project(&state, id, &active);
    assert!(
        result.is_ok(),
        "pre-cancelled job should stop before opening missing models: {result:?}"
    );
}

#[tokio::test]
async fn accept_api_supports_explicit_scope_and_legacy_empty_body() {
    let (_tmp, state, app, id) = fixture(2);
    let ids = {
        let mut service = state.services.lock().unwrap();
        service.scan(id).unwrap();
        let settings = service.settings(id).unwrap();
        let ids: Vec<_> = service
            .photos(id, Default::default())
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();
        for (photo, verdict) in ids.iter().zip(["recommend", "reject_suggest"]) {
            service
                .save_analysis(
                    *photo,
                    json!({"version":iris_core::vision::ANALYSIS_VERSION,
                "settings":settings,"verdict":verdict,"faces":[]}),
                )
                .unwrap();
        }
        ids
    };
    let path = format!("/api/v1/projects/{id}/accept");
    assert_eq!(
        call(
            &app,
            &state,
            "POST",
            &path,
            json!({"photo_ids":[],"category":"all"})
        )
        .await
        .1["changed"],
        0
    );
    let result = call(
        &app,
        &state,
        "POST",
        &path,
        json!({"photo_ids":ids,"category":"recommend"}),
    )
    .await;
    assert_eq!(result.0, StatusCode::OK);
    assert_eq!(result.1["changed"], 1);
    assert_eq!(
        state
            .services
            .lock()
            .unwrap()
            .photo(ids[1])
            .unwrap()
            .decision,
        Action::Pending
    );
    call(
        &app,
        &state,
        "POST",
        &format!("/api/v1/projects/{id}/undo"),
        json!({}),
    )
    .await;
    assert_eq!(
        call(&app, &state, "POST", &path, json!({"category":"review"}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    for (body, expected) in [
        (r#"{"photo_ids":[]}"#, StatusCode::OK),
        ("invalid-json", StatusCode::BAD_REQUEST),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&path)
                    .header("authorization", format!("Bearer {}", state.token))
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        for photo in &ids {
            assert_eq!(
                state
                    .services
                    .lock()
                    .unwrap()
                    .photo(*photo)
                    .unwrap()
                    .decision,
                Action::Pending
            );
        }
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&path)
                .header("authorization", format!("Bearer {}", state.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let result: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result["changed"], 2);
    let service = state.services.lock().unwrap();
    assert_eq!(service.photo(ids[0]).unwrap().decision, Action::Keep);
    assert_eq!(service.photo(ids[1]).unwrap().decision, Action::Reject);
}

#[test]
fn cached_analysis_rebuilds_groups_without_models_or_workers() {
    let (_tmp, state, _app, id) = fixture(2);
    let state = state.with_worker_limit(16).unwrap();
    let ids = {
        let mut service = state.services.lock().unwrap();
        service.scan(id).unwrap();
        let settings = service.settings(id).unwrap();
        let analysis = json!({
            "width":2,"height":2,"original_width":2,"original_height":2,
            "orientation":1,"faces":[],"sharpness_lap":100.0,"sharpness_fft":0.5,
            "niqe":null,"exposure":{"mean":0.5,"shadow_clip":0.0,
            "highlight_clip":0.0,"verdict":"normal"},"composite_score":0.8,
            "verdict":"recommend","phash":"0123456789abcdef",
            "structure":vec![0.5;64],"warnings":[],
            "version":iris_core::vision::ANALYSIS_VERSION,"settings":settings
        });
        serde_json::from_value::<iris_core::vision::VisionAnalysis>(analysis.clone()).unwrap();
        let ids: Vec<_> = service
            .photos(id, Default::default())
            .unwrap()
            .into_iter()
            .map(|photo| photo.id)
            .collect();
        for photo_id in &ids {
            service.save_analysis(*photo_id, analysis.clone()).unwrap();
        }
        assert!(service.project(id).unwrap().groups_dirty);
        assert!(service.groups(id).unwrap().is_empty());
        ids
    };
    assert!(!state.model_dir.exists());
    let mut result = analyze_project(&state, id, &job("analysis")).unwrap();
    let timings = result.as_object_mut().unwrap().remove("timing_ms").unwrap();
    assert_eq!(timings["worker_init"], 0.);
    assert!(timings["total"].as_f64().unwrap() >= timings["validate_sources"].as_f64().unwrap());
    assert_eq!(
        result,
        json!({"analyzed":0,"reused":2,"failed":0,"workers":0,"semantic":{"mode":"phash","candidates":0,"computed":0,"reused":0,"warnings":[]}})
    );
    let service = state.services.lock().unwrap();
    let groups = service.groups(id).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].kind, "duplicate");
    assert_eq!(groups[0].member_photo_ids.len(), 2);
    assert!(ids.iter().all(|id| groups[0].member_photo_ids.contains(id)));
    assert!(!service.project(id).unwrap().groups_dirty);
}

#[test]
#[ignore = "requires provisioned real models and ONNX Runtime DLL"]
fn configured_two_worker_pool_analyzes_real_jpegs_and_then_reuses_cache() {
    let (_temporary, mut state, _app, id) = fixture(3);
    state.model_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
    let state = state.with_worker_limit(2).unwrap();
    state.services.lock().unwrap().scan(id).unwrap();
    let analyzed = analyze_project(&state, id, &job("analysis")).unwrap();
    assert_eq!(analyzed["analyzed"], 3);
    assert_eq!(analyzed["failed"], 0);
    assert_eq!(
        analyzed["workers"],
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(2)
    );
    let cached = analyze_project(&state, id, &job("analysis")).unwrap();
    assert_eq!(cached["reused"], 3);
    assert_eq!(cached["workers"], 0);
}

#[test]
fn legacy_yunet_cached_settings_reuse_without_model_files() {
    let (_tmp, state, _app, id) = fixture(1);
    {
        let mut service = state.services.lock().unwrap();
        service.scan(id).unwrap();
        let photo = service.photos(id, Default::default()).unwrap().remove(0);
        let mut settings = service.settings(id).unwrap();
        settings.as_object_mut().unwrap().remove("face_detector");
        settings
            .as_object_mut()
            .unwrap()
            .remove("scrfd_model_sha256");
        for key in [
            "occlusion_provider",
            "occlusion_model_sha256",
            "occlusion_min_visible_fraction",
        ] {
            settings.as_object_mut().unwrap().remove(key);
        }
        service
            .store
            .conn
            .execute(
                "INSERT INTO settings(project_id,data) VALUES(?1,?2)",
                (id, settings.to_string()),
            )
            .unwrap();
        service
            .save_analysis(
                photo.id,
                json!({"settings":settings,
            "version":iris_core::vision::ANALYSIS_VERSION,"verdict":"recommend"}),
            )
            .unwrap();
    }
    let result = analyze_project(&state, id, &job("analysis")).unwrap();
    assert_eq!(result["reused"], 1);
    assert_eq!(result["workers"], 0);
}

#[test]
fn all_v4_cached_analysis_requires_v5_inference_and_preserves_human_decisions() {
    let (_tmp, state, _app, id) = fixture(3);
    let ids = {
        let mut service = state.services.lock().unwrap();
        service.scan(id).unwrap();
        let settings = service.settings(id).unwrap();
        let ids: Vec<_> = service
            .photos(id, Default::default())
            .unwrap()
            .into_iter()
            .map(|photo| photo.id)
            .collect();
        let prior_version = "iris-vision-v4-conservative-ear-fallback-2026-10-06";
        assert_ne!(prior_version, iris_core::vision::ANALYSIS_VERSION);
        for (photo_id, action) in ids.iter().zip([Action::Keep, Action::Reject, Action::Flag]) {
            service
                .save_analysis(
                    *photo_id,
                    json!({"version":prior_version,
                "settings":settings,"verdict":"recommend","faces":[]}),
                )
                .unwrap();
            service
                .decisions(id, &[*photo_id], action, "human", false)
                .unwrap();
        }
        assert_eq!(service.project(id).unwrap().pending_analysis, 3);
        assert!(service
            .photos(id, Default::default())
            .unwrap()
            .iter()
            .all(|photo| photo.analysis_status == iris_core::AnalysisStatus::Stale));
        ids
    };
    assert!(!state.model_dir.exists());
    // Old-engine cache cannot report success with zero workers just because every row exists.
    assert!(analyze_project(&state, id, &job("analysis")).is_err());
    let service = state.services.lock().unwrap();
    for (photo_id, action) in ids
        .into_iter()
        .zip([Action::Keep, Action::Reject, Action::Flag])
    {
        assert_eq!(service.photo(photo_id).unwrap().decision, action);
    }
    assert_eq!(service.project(id).unwrap().pending_analysis, 3);
}

#[test]
fn faceocc_cached_analysis_requires_selected_artifacts_before_cache_reuse() {
    let (_tmp, state, _app, id) = fixture(1);
    {
        let mut service = state.services.lock().unwrap();
        service.scan(id).unwrap();
        let photo = service.photos(id, Default::default()).unwrap().remove(0);
        let mut settings = service.settings(id).unwrap();
        settings["occlusion_provider"] = json!("faceocc");
        settings["occlusion_model_sha256"] = json!("a".repeat(64));
        settings["occlusion_min_visible_fraction"] = json!(0.75);
        let settings = service.set_settings(id, settings).unwrap();
        service
            .save_analysis(
                photo.id,
                json!({"version":iris_core::vision::ANALYSIS_VERSION,
            "settings":settings,"verdict":"recommend"}),
            )
            .unwrap();
    }
    let error = analyze_project(&state, id, &job("analysis"))
        .unwrap_err()
        .to_string();
    assert!(error.to_lowercase().contains("faceocc"), "{error}");
}

#[test]
fn scrfd_cached_analysis_still_requires_matching_model_artifacts() {
    let (_tmp, state, _app, id) = fixture(1);
    {
        let mut service = state.services.lock().unwrap();
        service.scan(id).unwrap();
        let photo = service.photos(id, Default::default()).unwrap().remove(0);
        let mut settings = service.settings(id).unwrap();
        settings["face_detector"] = json!("scrfd_500m");
        settings["scrfd_model_sha256"] = json!("a".repeat(64));
        service.set_settings(id, settings.clone()).unwrap();
        service
            .save_analysis(
                photo.id,
                json!({"settings":settings,
            "version":iris_core::vision::ANALYSIS_VERSION,"verdict":"recommend"}),
            )
            .unwrap();
    }
    assert!(analyze_project(&state, id, &job("analysis")).is_err());
}

#[tokio::test]
async fn models_endpoint_reports_artifact_status_without_initializing_workers() {
    let (_tmp, state, app, id) = fixture(0);
    let path = format!("/api/v1/models?project_id={id}");
    let response = app
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let (status, models) = call(&app, &state, "GET", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(models["selected"], "yunet");
    assert_eq!(models["occlusion_selected"], "none");
    assert_eq!(models["occlusion"][0]["provider"], "none");
    assert_eq!(models["occlusion"][0]["state"], "disabled");
    assert_eq!(models["occlusion"][1]["provider"], "faceocc");
    let detectors = models["detectors"].as_array().unwrap();
    assert_eq!(detectors.len(), 2);
    assert_eq!(detectors[0]["provider"], "yunet");
    assert_eq!(detectors[0]["state"], "missing");
    assert_eq!(detectors[1]["provider"], "scrfd_500m");
    assert_ne!(detectors[1]["state"], "available");
    assert!(detectors
        .iter()
        .all(|d| d["reason"].as_str().is_some_and(|r| !r.is_empty())));
    let expected_hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    {
        let mut service = state.services.lock().unwrap();
        let mut settings = service.settings(id).unwrap();
        settings["face_detector"] = json!("scrfd_500m");
        settings["scrfd_model_sha256"] = json!(expected_hash);
        service.set_settings(id, settings).unwrap();
    }
    let missing = call(&app, &state, "GET", &path, Value::Null).await.1;
    assert_eq!(missing["selected"], "scrfd_500m");
    assert_eq!(missing["detectors"][1]["state"], "missing");
    // Synthetic bytes exercise artifact checks only, never claim a valid ONNX graph.
    let optional = state.model_dir.join("optional");
    std::fs::create_dir_all(&optional).unwrap();
    std::fs::write(optional.join("scrfd_500m.onnx"), b"abc").unwrap();
    std::fs::write(
        optional.join("scrfd_500m.metadata.json"),
        json!({
            "model_id":"scrfd_500m_kps", "source_url":"https://example.invalid/model",
            "license_url":"https://example.invalid/license", "license_note":"synthetic test fixture"
        })
        .to_string(),
    )
    .unwrap();
    let available = call(&app, &state, "GET", &path, Value::Null).await.1;
    assert_eq!(available["detectors"][1]["state"], "available");
    assert_eq!(available["detectors"][1]["sha256"], expected_hash);
    std::fs::write(optional.join("scrfd_500m.onnx"), b"changed weights").unwrap();
    let invalid = call(&app, &state, "GET", &path, Value::Null).await.1;
    assert_eq!(invalid["detectors"][1]["state"], "invalid");
    assert_ne!(invalid["detectors"][1]["sha256"], expected_hash);
    assert!(invalid["detectors"][1]["reason"]
        .as_str()
        .unwrap()
        .contains("mismatch"));
    assert!(state.jobs.lock().unwrap().is_empty());
    assert_eq!(
        call(
            &app,
            &state,
            "GET",
            "/api/v1/models?project_id=99999",
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

fn faceocc_fixture_metadata() -> Value {
    json!({"model_id":"faceocc_visible_face_v1","source_url":"https://example.invalid/model",
        "license_url":"https://example.invalid/license","license_note":"synthetic test fixture"})
}

#[tokio::test]
async fn occlusion_model_status_reports_disabled_missing_invalid_and_artifact_available() {
    let (_tmp, state, app, id) = fixture(0);
    let path = format!("/api/v1/models?project_id={id}");
    let expected_hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    {
        let mut service = state.services.lock().unwrap();
        let mut settings = service.settings(id).unwrap();
        settings["occlusion_provider"] = json!("faceocc");
        settings["occlusion_model_sha256"] = json!(expected_hash);
        settings["occlusion_min_visible_fraction"] = json!(0.75);
        service.set_settings(id, settings).unwrap();
    }
    let missing = call(&app, &state, "GET", &path, Value::Null).await;
    assert_eq!(missing.0, StatusCode::OK);
    assert_eq!(missing.1["occlusion_selected"], "faceocc");
    assert_eq!(missing.1["occlusion"][0]["state"], "disabled");
    assert_eq!(missing.1["occlusion"][1]["state"], "missing");
    let optional = state.model_dir.join("optional");
    std::fs::create_dir_all(&optional).unwrap();
    // Artifact status is deliberately separate from ONNX graph/inference acceptance.
    std::fs::write(optional.join("faceocc.onnx"), b"abc").unwrap();
    std::fs::write(
        optional.join("faceocc.metadata.json"),
        faceocc_fixture_metadata().to_string(),
    )
    .unwrap();
    let available = call(&app, &state, "GET", &path, Value::Null).await.1;
    assert_eq!(available["occlusion"][1]["state"], "available");
    assert_eq!(available["occlusion"][1]["sha256"], expected_hash);
    // An unrelated absent SCRFD declaration must not poison FaceOcc inspection.
    assert_eq!(available["detectors"][1]["state"], "invalid");
    std::fs::write(optional.join("faceocc.onnx"), b"changed weights").unwrap();
    let invalid = call(&app, &state, "GET", &path, Value::Null).await.1;
    assert_eq!(invalid["occlusion"][1]["state"], "invalid");
    assert_ne!(invalid["occlusion"][1]["sha256"], expected_hash);
    assert!(invalid["occlusion"][1]["reason"]
        .as_str()
        .unwrap()
        .contains("mismatch"));
    assert!(state.jobs.lock().unwrap().is_empty());
}

#[test]
fn empty_project_analysis_does_not_initialize_missing_models() {
    let (_tmp, state, _app, id) = fixture(0);
    let mut result = analyze_project(&state, id, &job("analysis")).unwrap();
    result.as_object_mut().unwrap().remove("timing_ms");
    assert_eq!(
        result,
        json!({"analyzed":0,"reused":0,"failed":0,"workers":0,"semantic":{"mode":"phash","candidates":0,"computed":0,"reused":0,"warnings":[]}})
    );
}

#[test]
fn changed_cached_source_is_rejected_before_model_loading() {
    let (_tmp, state, _app, id) = fixture(1);
    {
        let mut service = state.services.lock().unwrap();
        service.scan(id).unwrap();
        let p = service.photos(id, Default::default()).unwrap().remove(0);
        let settings = service.settings(id).unwrap();
        service.save_analysis(p.id,json!({"settings":settings,"version":iris_core::vision::ANALYSIS_VERSION,"verdict":"recommend"})).unwrap();
        std::fs::write(service.photo_path(p.id).unwrap(), b"changed source").unwrap();
    }
    // Source failures are isolated per photo, before loading any model.
    let result = analyze_project(&state, id, &job("analysis")).unwrap();
    assert_eq!(result["failed"], 1);
    assert_eq!(result["analyzed"], 0);
    assert_eq!(result["reused"], 0);
    assert_eq!(result["workers"], 0);
    let service = state.services.lock().unwrap();
    let photo = service.photos(id, Default::default()).unwrap().remove(0);
    assert_eq!(photo.analysis_status, iris_core::AnalysisStatus::Stale);
    assert_eq!(photo.analysis.as_ref().unwrap()["source_invalid"], true);
    assert_eq!(service.project(id).unwrap().pending_analysis, 1);
    assert_eq!(
        std::fs::read(service.photo_path(photo.id).unwrap()).unwrap(),
        b"changed source"
    );
}
