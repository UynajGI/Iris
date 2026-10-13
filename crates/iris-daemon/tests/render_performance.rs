// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicitly ignored debug-build backend prototypes, not UI or consumer-GPU benchmarks.
//! Run serially with --ignored --nocapture --test-threads=1 on a provisioned GPU runner.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use iris_daemon::{router, AppState};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tower::ServiceExt;

async fn request(
    app: &Router,
    state: &AppState,
    path: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("authorization", format!("Bearer {}", state.token))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
async fn fixture(cpu: bool) -> (tempfile::TempDir, AppState, Router, String) {
    let temp = tempfile::tempdir().unwrap();
    let photos = temp.path().join("photos");
    std::fs::create_dir(&photos).unwrap();
    image::RgbImage::from_fn(1024, 768, |x, y| {
        image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8])
    })
    .save(photos.join("synthetic.png"))
    .unwrap();
    let mut state = AppState::new(
        &temp.path().join("library.sqlite"),
        temp.path().join("models"),
    )
    .unwrap();
    if cpu {
        state = state.with_cpu_rendering().unwrap();
    }
    let id = {
        let mut services = state.services.lock().unwrap();
        let project = services.create_project(&photos).unwrap();
        services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'synthetic.png','synthetic.png','png',1,1,1024,768)", [project.id]).unwrap();
        services.store.conn.last_insert_rowid()
    };
    let app = router(state.clone());
    let edit = value(
        request(
            &app,
            &state,
            &format!("/api/v1/photos/{id}/edits"),
            json!({"name":"Performance synthetic"}),
        )
        .await,
    )
    .await;
    let mut document = edit["document"].clone();
    let parent = document["output_node"].clone();
    document["nodes"].as_array_mut().unwrap().push(json!({"id":"exposure-smoke","name":"Exposure","version":1,"enabled":true,"inputs":[parent],"operation":{"type":"adjustments","parameters":{"exposure":1.0}}}));
    document["output_node"] = json!("exposure-smoke");
    let route = format!("/api/v1/edits/{}", edit["id"].as_str().unwrap());
    let response = app.clone().oneshot(Request::builder().method("PUT").uri(&route)
        .header("authorization",format!("Bearer {}",state.token)).header("content-type","application/json")
        .body(Body::from(json!({"expected_revision":0,"request_id":"performance-save","document":document}).to_string())).unwrap()).await.unwrap();
    assert_eq!(value(response).await["revision"], 1);
    (temp, state, app, route)
}
fn render(sequence: u64, session: &str) -> Value {
    json!({"revision":1,"session":session,"sequence":sequence,"quality":"interactive","max_edge":1024})
}
async fn sample(
    app: &Router,
    state: &AppState,
    route: &str,
    sequence: u64,
    gpu: bool,
) -> (f64, Vec<u8>) {
    let start = Instant::now();
    let response = request(
        app,
        state,
        &format!("{route}/preview"),
        render(sequence, "timing"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let nodes: u32 = response.headers()["x-iris-gpu-nodes"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        nodes > 0,
        gpu,
        "required backend not executed; GPU unavailable/fallback is a failure, not a skip"
    );
    assert_eq!(response.headers()["x-iris-gpu-fallback"], "false");
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (start.elapsed().as_secs_f64() * 1000.0, bytes)
}
fn report(cpu: bool, cold: f64, warm: &mut [f64]) {
    warm.sort_by(f64::total_cmp);
    println!(
        "{}",
        json!({"kind":"debug_backend_prototype","cpu_only":cpu,"gpu_required":!cpu,"width":1024,"height":768,"cold_ms":cold,"warm_samples":warm.len(),"warm_p50_ms":warm[2],"warm_p95_ms":warm[4],"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"debug_assertions":cfg!(debug_assertions),"runner":std::env::var("RUNNER_NAME").unwrap_or_default(),"adapter_environment":std::env::var("VK_ICD_FILENAMES").unwrap_or_default(),"scope":"in-process HTTP router through collected JPEG body; no socket/UI timing; no stage timing"})
    );
}
#[tokio::test]
#[ignore = "requires provisioned GPU; debug backend timing prototype, run serially"]
async fn daemon_preview_gpu_cpu_backend_prototype() {
    let mut outputs = Vec::new();
    for cpu in [true, false] {
        let (_temp, state, app, route) = fixture(cpu).await;
        let (cold, bytes) = sample(&app, &state, &route, 1, !cpu).await;
        outputs.push(bytes);
        let mut warm = Vec::new();
        for seq in 2..=6 {
            warm.push(sample(&app, &state, &route, seq, !cpu).await.0);
        }
        report(cpu, cold, &mut warm);
        state.shutdown_rendering();
        state.wait_for_rendered_exports().await;
    }
    // Power-of-two exposure is exactly representable; same encoding configuration.
    assert_eq!(outputs[0], outputs[1], "CPU/GPU encoded previews differ");
}
async fn export_status(app: &Router, state: &AppState, id: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/rendered-exports/{id}"))
                .header("authorization", format!("Bearer {}", state.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    value(response).await
}
async fn cancel_pending(app: &Router, state: &AppState, route: &str) {
    // The native holder releases after ten seconds even if a runtime worker
    // synchronously blocks on Services. Async timeouts are not our unlock path.
    let services = state.services.clone();
    let (ready_send, ready_receive) = tokio::sync::oneshot::channel();
    let (release_send, release_receive) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match services.try_lock() {
                Ok(lock) => {
                    let _ = ready_send.send(());
                    let _ = release_receive.recv_timeout(Duration::from_secs(10));
                    drop(lock);
                    return;
                }
                Err(std::sync::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                _ => return,
            }
        }
    });
    ready_receive
        .await
        .expect("bounded holder could not acquire Services");
    let worker_app = app.clone();
    let worker_state = state.clone();
    let path = format!("{route}/preview");
    let pending = tokio::spawn(async move {
        request(&worker_app, &worker_state, &path, render(1, "cancel-check")).await
    });
    let registered = tokio::time::timeout(Duration::from_secs(5), async {
        while !state.render_request_is_current("cancel-check", 1) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await;
    if registered.is_ok() {
        value(
            request(
                app,
                state,
                "/api/v1/edit-render/cancel-check/cancel",
                json!({"sequence":1}),
            )
            .await,
        )
        .await;
    }
    let _ = release_send.send(());
    holder.join().unwrap();
    let response = tokio::time::timeout(Duration::from_secs(15), pending)
        .await
        .expect("cancelled preview did not finish")
        .unwrap();
    assert!(
        registered.is_ok(),
        "preview did not register within five seconds"
    );
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_ne!(
        response.headers().get("content-type").unwrap(),
        "image/jpeg"
    );
}
#[tokio::test]
#[ignore = "bounded concurrent backend smoke; run serially"]
async fn previews_allow_export_progress_and_reject_cancelled_old_sequence() {
    let (temp, state, app, route) = fixture(true).await;
    let directory = temp.path().join("exports");
    std::fs::create_dir(&directory).unwrap();
    let id = route.rsplit('/').next().unwrap();
    let jobs = value(request(&app,&state,"/api/v1/rendered-exports",json!({"exports":[{"document_id":id,"expected_revision":1,"destination_directory":directory,"filename":"result.png","format":{"format":"png16"}}]})).await).await;
    let export_id = jobs[0]["id"].as_str().unwrap();
    let preview_path = format!("{route}/preview");
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        for seq in 1..=100 {
            let (preview, status) = tokio::join!(
                request(&app, &state, &preview_path, render(seq, "pressure")),
                export_status(&app, &state, export_id)
            );
            assert_eq!(preview.status(), StatusCode::OK);
            preview.into_body().collect().await.unwrap();
            match status["state"].as_str().unwrap() {
                "completed" => return,
                "queued" | "running" | "publishing" => {}
                _ => panic!("export failed: {status}"),
            }
        }
        panic!("export did not complete within 100 previews");
    })
    .await;
    assert!(result.is_ok(), "export starved beyond 30 seconds");
    assert!(directory.join("result.png").is_file());
    cancel_pending(&app, &state, &route).await;
    let old = request(
        &app,
        &state,
        &format!("{route}/preview"),
        render(1, "cancel-check"),
    )
    .await;
    assert_eq!(old.status(), StatusCode::BAD_REQUEST);
    assert_ne!(old.headers().get("content-type").unwrap(), "image/jpeg");
    let next = request(
        &app,
        &state,
        &format!("{route}/preview"),
        render(2, "cancel-check"),
    )
    .await;
    assert_eq!(next.status(), StatusCode::OK);
    state.shutdown_rendering();
    state.wait_for_rendered_exports().await;
}
