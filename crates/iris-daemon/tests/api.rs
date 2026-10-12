use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use iris_daemon::{router, ApiDoc, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;
use utoipa::OpenApi;

async fn call(
    app: &axum::Router,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(json!(null)),
    )
}

#[tokio::test]
async fn token_required_for_bootstrap_schema_and_library() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::new(&temp.path().join("db.sqlite3"), temp.path().into()).unwrap();
    let token = state.token.clone();
    let app = router(state);
    for path in [
        "/api/v1/bootstrap",
        "/api/v1/projects",
        "/api/v1/openapi.json",
        "/api/v1/devices/gpu",
    ] {
        assert_eq!(
            call(&app, None, "GET", path, Value::Null).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(&app, Some("incorrect"), "GET", path, Value::Null)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(&app, Some(&token), "GET", path, Value::Null).await.0,
            StatusCode::OK
        );
    }
    let (_, bootstrap) = call(&app, Some(&token), "GET", "/api/v1/bootstrap", Value::Null).await;
    assert!(bootstrap.get("token").is_none());
}

#[tokio::test]
async fn browsing_context_is_scoped_revisioned_and_never_starts_analysis() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("public-synthetic");
    std::fs::create_dir(&root).unwrap();
    let state = AppState::new(&temp.path().join("db.sqlite3"), temp.path().into()).unwrap();
    let project = state
        .services
        .lock()
        .unwrap()
        .create_project(&root)
        .unwrap();
    let app = router(state.clone());
    let path = format!("/api/v1/projects/{}/browse-context", project.id);
    let newer = json!({"client_id":"window", "revision":2});
    assert_eq!(
        call(&app, None, "PUT", &path, newer.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, accepted) = call(&app, Some(&state.token), "PUT", &path, newer).await;
    assert_eq!(status, StatusCode::OK);
    let (_, older) = call(
        &app,
        Some(&state.token),
        "PUT",
        &path,
        json!({"client_id":"window","revision":1}),
    )
    .await;
    assert_eq!(older, accepted);
    assert!(state.jobs.lock().unwrap().is_empty());
    for invalid in [
        json!({"focused_photo_id":-1}),
        json!({"focused_photo_id":123456}),
        json!({"comparison_photo_ids":vec![1;33]}),
        json!({"nearby_photo_ids":vec![1;129]}),
    ] {
        assert_eq!(
            call(&app, Some(&state.token), "PUT", &path, invalid)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let unknown = format!("/api/v1/projects/{}/browse-context", project.id + 1);
    assert_eq!(
        call(&app, Some(&state.token), "PUT", &unknown, json!({}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
    assert_eq!(
        schema["paths"]["/api/v1/projects/{id}/browse-context"]["put"]["requestBody"]["content"]
            ["application/json"]["schema"]["$ref"],
        "#/components/schemas/BrowseContext"
    );
}

fn selection_fixture() -> (tempfile::TempDir, AppState, i64, Vec<i64>) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("synthetic");
    std::fs::create_dir(&root).unwrap();
    for name in ["a.png", "b.png"] {
        image::RgbImage::from_pixel(128, 128, image::Rgb([128; 3]))
            .save(root.join(name))
            .unwrap();
    }
    let state = AppState::new(
        &temp.path().join("library.sqlite"),
        temp.path().join("missing-models"),
    )
    .unwrap();
    let (project, ids) = {
        let mut services = state.services.lock().unwrap();
        let id = services.create_project(&root).unwrap().id;
        services.scan(id).unwrap();
        let ids: Vec<_> = services
            .photos(id, Default::default())
            .unwrap()
            .iter()
            .map(|p| p.id)
            .collect();
        (id, ids)
    };
    (temp, state, project, ids)
}
#[tokio::test]
async fn detail_scene_comparison_and_optional_search_have_real_scoped_contracts() {
    let (_temp, state, project, ids) = selection_fixture();
    let app = router(state.clone());
    let token = Some(state.token.as_str());
    let base = format!("/api/v1/projects/{project}");
    let (status, evidence) = call(
        &app,
        token,
        "POST",
        &format!("/api/v1/photos/{}/detail", ids[0]),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{evidence}");
    assert_eq!(evidence["state"], "full_detail");
    let (status, scenes) = call(&app, token, "GET", &format!("{base}/scenes"), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(scenes.as_array().unwrap().len(), 2);
    let (_, scenes) = call(
        &app,
        token,
        "PUT",
        &format!("{base}/scenes"),
        json!({"photo_ids":ids}),
    )
    .await;
    assert_eq!(scenes.as_array().unwrap().len(), 1);
    assert_eq!(scenes[0]["kind"], "manual");
    let (status, report) = call(
        &app,
        token,
        "POST",
        &format!("{base}/compare"),
        json!({"photo_ids":ids}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(report["members"].as_array().unwrap().len(), 2);
    assert!(report["members"][0]["score_delta"].is_null());
    let (_, status) = call(
        &app,
        token,
        "GET",
        &format!("{base}/search/status"),
        Value::Null,
    )
    .await;
    assert_eq!(status["model"]["state"], "disabled");
    assert_eq!(
        call(
            &app,
            token,
            "POST",
            &format!("{base}/search"),
            json!({"query":"人像"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            token,
            "POST",
            &format!("{base}/search/index"),
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert!(state.jobs.lock().unwrap().is_empty());
    assert!(state
        .services
        .lock()
        .unwrap()
        .photos(project, Default::default())
        .unwrap()
        .iter()
        .all(|p| p.decision == iris_core::Action::Pending));
    let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
    for path in [
        "/api/v1/photos/{id}/detail",
        "/api/v1/projects/{id}/scenes",
        "/api/v1/projects/{id}/compare",
        "/api/v1/projects/{id}/search",
        "/api/v1/projects/{id}/search/index",
    ] {
        assert!(schema["paths"].get(path).is_some(), "{path}");
    }
}

#[tokio::test]
async fn gpu_inventory_needs_no_project_or_models_and_has_a_typed_contract() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::new(
        &temp.path().join("db.sqlite3"),
        temp.path().join("missing-models"),
    )
    .unwrap();
    let token = state.token.clone();
    let app = router(state);
    let (status, body) = call(
        &app,
        Some(&token),
        "GET",
        "/api/v1/devices/gpu",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["inference_verified"], false);
    let adapters = body["adapters"].as_array().unwrap();
    if body["status"] == "available" {
        for (index, adapter) in adapters.iter().enumerate() {
            assert_eq!(adapter["device_id"], index);
            assert!(adapter["name"].is_string());
            assert!(adapter["is_software"].is_boolean());
        }
    } else {
        assert!(matches!(
            body["status"].as_str(),
            Some("unavailable" | "unsupported")
        ));
        assert!(adapters.is_empty());
        assert!(body["reason"].is_string());
        assert!(body["default_device_id"].is_null());
    }
    let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
    assert_eq!(
        schema["paths"]["/api/v1/devices/gpu"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["$ref"],
        "#/components/schemas/GpuDevices"
    );
}

#[tokio::test]
async fn progress_keeps_project_validation_and_returns_recorded_job_state() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let state = AppState::new(&temp.path().join("db.sqlite3"), temp.path().into()).unwrap();
    let id = state
        .services
        .lock()
        .unwrap()
        .create_project(root)
        .unwrap()
        .id;
    let app = router(state.clone());
    let path = format!("/api/v1/projects/{id}/progress");
    let (status, value) = call(&app, Some(&state.token), "GET", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["state"], "idle");
    let progress = iris_daemon::JobProgress {
        kind: "analysis".into(),
        state: "running".into(),
        completed: 12,
        total: 20,
        errors: vec![],
        result: None,
        ..Default::default()
    };
    state.jobs.lock().unwrap().insert(
        id,
        iris_daemon::Job {
            progress: progress.clone(),
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            pause: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
    );
    let (status, value) = call(&app, Some(&state.token), "GET", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, serde_json::to_value(progress).unwrap());
    let (status, value) = call(
        &app,
        Some(&state.token),
        "GET",
        "/api/v1/projects/999999/progress",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].as_str().unwrap().contains("project"));
}

#[tokio::test]
async fn project_settings_profiles_and_missing_models_are_real_failures() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    // A valid generated JPEG keeps this a real uncached inference request.
    let jpeg = "ffd8ffe000104a46494600010100000100010000ffdb004300100b0c0e0c0a100e0d0e1211101318281a181616183123251d283a333d3c3933383740485c4e404457453738506d51575f626768673e4d71797064785c656763ffdb0043011112121815182f1a1a2f6342384263636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363ffc00011080002000203012200021101031101ffc4001500010100000000000000000000000000000005ffc40014100100000000000000000000000000000000ffc40014010100000000000000000000000000000002ffc40014110100000000000000000000000000000000ffda000c03010002110311003f008c0183ffd9";
    let bytes: Vec<_> = (0..jpeg.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&jpeg[i..i + 2], 16).unwrap())
        .collect();
    std::fs::write(root.join("uncached.jpg"), bytes).unwrap();
    let state = AppState::new(
        &temp.path().join("db.sqlite3"),
        temp.path().join("missing-models"),
    )
    .unwrap();
    let token = state.token.clone();
    let app = router(state.clone());
    let (status, project) = call(
        &app,
        Some(&token),
        "POST",
        "/api/v1/projects",
        json!({"root":root}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{project}");
    let id = project["id"].as_i64().unwrap();
    assert_eq!(state.services.lock().unwrap().scan(id).unwrap().added, 1);
    let (status, settings) = call(
        &app,
        Some(&token),
        "GET",
        &format!("/api/v1/settings?project_id={id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(
            &app,
            Some(&token),
            "POST",
            "/api/v1/profiles",
            json!({"name":"baseline","settings":settings})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            Some(&token),
            "POST",
            "/api/v1/profiles/baseline/apply",
            json!({"project_id":id})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            Some(&token),
            "POST",
            &format!("/api/v1/projects/{id}/analyze"),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut terminal = Value::Null;
    // A real Windows worker process may need more than one second for cold startup.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let (_, p) = call(
            &app,
            Some(&token),
            "GET",
            &format!("/api/v1/projects/{id}/progress"),
            Value::Null,
        )
        .await;
        terminal = p;
        if terminal["state"] == "failed" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(terminal["state"], "failed");
    assert!(!terminal["errors"].as_array().unwrap().is_empty());
}

#[test]
fn openapi_exposes_service_contract_and_auth() {
    let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let required = schema["components"]["schemas"]["AcceptRequest"]["required"].as_array();
    assert!(
        required.is_none_or(|fields| fields.is_empty()),
        "accept fields must be optional"
    );
    assert_ne!(
        schema["paths"]["/api/v1/projects/{id}/accept"]["post"]["requestBody"]["required"],
        true
    );
    assert!(schema["paths"].as_object().unwrap().len() >= 30);
    assert_eq!(
        schema["components"]["securitySchemes"]["session"]["scheme"],
        "bearer"
    );
    for path in [
        "/api/v1/projects/{id}/quarantine/commit",
        "/api/v1/projects/{id}/analyze",
        "/api/v1/photos/{id}/thumb",
    ] {
        assert!(schema["paths"].get(path).is_some(), "{path}");
    }
}

#[tokio::test]
async fn openapi_response_descriptions_survive_serialization_and_http_export() {
    let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let mut responses = 0;
    for (path, item) in schema["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            if let Some(values) = operation.get("responses").and_then(Value::as_object) {
                for (status, response) in values {
                    if response.get("$ref").is_none() {
                        assert!(
                            response["description"].is_string(),
                            "{method} {path} {status}"
                        );
                        responses += 1;
                    }
                }
            }
        }
    }
    assert!(responses >= 40, "all documented responses must be checked");
    assert_eq!(
        schema["paths"]["/api/v1/devices/gpu"]["get"]["responses"]["401"]["description"],
        "Session token required"
    );
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::new(&temp.path().join("db.sqlite3"), temp.path().into()).unwrap();
    let token = state.token.clone();
    let (status, exported) = call(
        &router(state),
        Some(&token),
        "GET",
        "/api/v1/openapi.json",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(exported, schema);
}
