// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use iris_daemon::{router, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

fn fixture() -> (tempfile::TempDir, AppState, Router, i64) {
    let temp = tempfile::tempdir().unwrap();
    let photos = temp.path().join("photos");
    std::fs::create_dir(&photos).unwrap();
    image::RgbImage::from_pixel(32, 24, image::Rgb([70, 120, 180]))
        .save(photos.join("synthetic.png"))
        .unwrap();
    let state = AppState::new(
        &temp.path().join("library.sqlite"),
        temp.path().join("models"),
    )
    .unwrap();
    let id = {
        let mut services = state.services.lock().unwrap();
        let project = services.create_project(&photos).unwrap();
        services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'synthetic.png','synthetic.png','png',1,1,32,24)",[project.id]).unwrap();
        services.store.conn.last_insert_rowid()
    };
    let app = router(state.clone());
    (temp, state, app, id)
}
async fn request(
    app: &Router,
    state: &AppState,
    method: &str,
    path: &str,
    value: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {}", state.token))
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
async fn create(app: &Router, state: &AppState, id: i64) -> Value {
    let response = request(
        app,
        state,
        "POST",
        &format!("/api/v1/photos/{id}/edits"),
        json!({"name":"Synthetic edit"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    value(response).await
}

#[tokio::test]
async fn editing_auth_state_conflicts_and_portable_recipe() {
    let (_temp, state, app, id) = fixture();
    let unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/editing/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);
    let invalid = request(
        &app,
        &state,
        "POST",
        &format!("/api/v1/photos/{id}/edits"),
        json!({"name":"bad","source":{"photo_id":"999"}}),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let edit = create(&app, &state, id).await;
    let edit_id = edit["id"].as_str().unwrap();
    let route = format!("/api/v1/edits/{edit_id}");
    assert_eq!(edit["document"]["source"]["photo_id"], id.to_string());
    let mut document = edit["document"].clone();
    document["nodes"][1]["name"] = json!("Changed");
    let save = json!({"expected_revision":0,"request_id":"one","document":document});
    let response = request(&app, &state, "PUT", &route, save.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(value(response).await["revision"], 1);
    let response = request(&app, &state, "PUT", &route, save).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(value(response).await["revision"], 1);
    let conflict = request(
        &app,
        &state,
        "POST",
        &format!("{route}/undo"),
        json!({"expected_revision":0,"request_id":"undo-stale"}),
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::BAD_REQUEST);
    let undo = request(
        &app,
        &state,
        "POST",
        &format!("{route}/undo"),
        json!({"expected_revision":1,"request_id":"undo"}),
    )
    .await;
    assert_eq!(value(undo).await["revision"], 2);
    let recipe =
        value(request(&app, &state, "GET", &format!("{route}/recipe"), Value::Null).await).await;
    assert!(recipe.get("source").is_none());
    let history = value(
        request(
            &app,
            &state,
            "GET",
            &format!("{route}/history?limit=10"),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(history.as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn preview_is_binary_revisioned_and_scopes_share_working_pixels() {
    let (_temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let route = format!("/api/v1/edits/{}", edit["id"].as_str().unwrap());
    let render = json!({"revision":0,"session":"test-preview","sequence":1,"quality":"interactive","max_edge":16});
    let preview = request(
        &app,
        &state,
        "POST",
        &format!("{route}/preview"),
        render.clone(),
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
    assert_eq!(preview.headers()["content-type"], "image/jpeg");
    assert_eq!(preview.headers()["x-iris-revision"], "0");
    assert_eq!(preview.headers()["x-iris-approximate"], "true");
    let bytes = preview.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(&[0xff, 0xd8]));
    let stale = request(
        &app,
        &state,
        "POST",
        &format!("{route}/preview"),
        render.clone(),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::BAD_REQUEST);
    let mut next = render.clone();
    next["sequence"] = json!(2);
    let histogram = request(
        &app,
        &state,
        "POST",
        &format!("{route}/histogram"),
        json!({"render":next,"domain":"linear_rec2020","lower":0.0,"upper":1.0,"bins":16}),
    )
    .await;
    assert_eq!(histogram.status(), StatusCode::OK);
    let histogram = value(histogram).await;
    assert_eq!(histogram["approximate"], true);
    assert_eq!(
        histogram["histogram"]["bins"][0].as_array().unwrap().len(),
        16
    );
    next["sequence"] = json!(3);
    let sample = request(
        &app,
        &state,
        "POST",
        &format!("{route}/sample"),
        json!({"render":next,"rect":[0.0,0.0,1.0,1.0]}),
    )
    .await;
    assert_eq!(sample.status(), StatusCode::OK);
    assert_eq!(value(sample).await["domain"], "linear_rec2020_d65");
    next["sequence"] = json!(4);
    next["quality"] = json!("full");
    let full = value(
        request(
            &app,
            &state,
            "POST",
            &format!("{route}/histogram"),
            json!({"render":next,"domain":"linear_rec2020","lower":0.0,"upper":1.0,"bins":16}),
        )
        .await,
    )
    .await;
    assert_eq!(full["approximate"], false);
    assert_eq!(full["preview_width"], 32);
    assert_eq!(full["histogram"]["visible_pixels"], 32 * 24);
}

#[tokio::test]
async fn resource_upload_is_bounded_binary_and_validated() {
    let (_temp, state, app, _) = fixture();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/edit-resources/import/lut")
                .header("authorization", format!("Bearer {}", state.token))
                .header("content-type", "application/octet-stream")
                .body(Body::from("LUT_1D_SIZE 2\n0 0 0\n1 1 1\n"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let resource = value(response).await;
    let download = request(
        &app,
        &state,
        "GET",
        &format!(
            "/api/v1/edit-resources/{}",
            resource["hash"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        download.headers()["content-type"],
        "application/octet-stream"
    );
    let oversized = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/edit-resources/import/lut")
                .header("authorization", format!("Bearer {}", state.token))
                .body(Body::from(vec![0u8; 32 * 1024 * 1024 + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[test]
fn editing_openapi_exposes_live_paths_and_recursive_documents() {
    use utoipa::OpenApi;
    let doc = serde_json::to_value(iris_daemon::ApiDoc::openapi()).unwrap();
    assert!(doc["paths"]["/api/v1/edits/{id}/preview"]["post"].is_object());
    assert!(doc["paths"]["/api/v1/edits/{id}/region"]["post"].is_object());
    assert!(doc["components"]["schemas"]["PixelRect"].is_object());
    assert!(doc["paths"]["/api/v1/edit-presets/{id}"]["put"].is_object());
    assert!(doc["components"]["schemas"]["EditDocument"].is_object());
    assert!(doc["components"]["schemas"]["MaskDefinition"].is_object());
}

#[tokio::test]
async fn rendered_export_queue_publishes_without_overwriting_original() {
    let (temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let directory = temp.path().join("export");
    std::fs::create_dir(&directory).unwrap();
    let original = std::fs::read(temp.path().join("photos/synthetic.png")).unwrap();
    let response=request(&app,&state,"POST","/api/v1/rendered-exports",json!({"exports":[{"document_id":edit["id"],"expected_revision":0,"destination_directory":directory.to_string_lossy(),"filename":"result.png","format":{"format":"png16"}}]})).await;
    assert_eq!(response.status(), StatusCode::OK);
    let queued = value(response).await;
    let export_id = queued[0]["id"].as_str().unwrap();
    let status = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let current = value(
                request(
                    &app,
                    &state,
                    "GET",
                    &format!("/api/v1/rendered-exports/{export_id}"),
                    Value::Null,
                )
                .await,
            )
            .await;
            if ["completed", "failed", "interrupted"]
                .contains(&current["state"].as_str().unwrap_or(""))
            {
                break current;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(status["state"], "completed", "{status}");
    assert!(directory.join("result.png").is_file());
    assert_eq!(
        std::fs::read(temp.path().join("photos/synthetic.png")).unwrap(),
        original
    );
    let duplicate=request(&app,&state,"POST","/api/v1/rendered-exports",json!({"exports":[{"document_id":edit["id"],"expected_revision":0,"destination_directory":directory.to_string_lossy(),"filename":"result.png","format":{"format":"png16"}}]})).await;
    assert_eq!(duplicate.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn proof_uses_an_icc_resource_and_marks_the_binary_response() {
    let (_temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let icc = iris_core::render::color::profile(iris_core::render::color::OutputSpace::Srgb, false)
        .unwrap()
        .icc()
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/edit-resources/import/icc")
                .header("authorization", format!("Bearer {}", state.token))
                .header("content-type", "application/octet-stream")
                .body(Body::from(icc))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let resource = value(response).await;
    let response=request(&app,&state,"POST",&format!("/api/v1/edits/{}/proof",edit["id"].as_str().unwrap()),json!({"resource":resource["hash"],"render":{"revision":0,"session":"proof","sequence":1,"quality":"full","max_edge":32}})).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/jpeg");
    assert_eq!(response.headers()["x-iris-soft-proof"], "true");
    assert_eq!(response.headers()["x-iris-draft"], "false");
    let original = response.into_body().collect().await.unwrap().to_bytes();
    let route = format!("/api/v1/edits/{}", edit["id"].as_str().unwrap());
    let history_route = format!("{route}/history");
    let history = value(request(&app, &state, "GET", &history_route, Value::Null).await).await;
    let mut document = edit["document"].clone();
    document["nodes"][1]["operation"]["parameters"]["exposure"] = json!(1.0);
    let mut payload = json!({"resource":resource["hash"],"document":document,
        "render":{"revision":0,"session":"proof","sequence":2,"quality":"full","max_edge":32}});
    let draft = request(
        &app,
        &state,
        "POST",
        &format!("{route}/proof"),
        payload.clone(),
    )
    .await;
    assert_eq!(draft.status(), StatusCode::OK);
    assert_eq!(draft.headers()["x-iris-soft-proof"], "true");
    assert_eq!(draft.headers()["x-iris-draft"], "true");
    assert_eq!(draft.headers()["x-iris-revision"], "0");
    let pixels = draft.into_body().collect().await.unwrap().to_bytes();
    let original = image::load_from_memory(&original).unwrap().to_rgb8();
    let adjusted = image::load_from_memory(&pixels).unwrap().to_rgb8();
    assert!(adjusted.get_pixel(0, 0)[0] > original.get_pixel(0, 0)[0]);
    payload["render"]["sequence"] = json!(3);
    payload["document"]["source"]["photo_id"] = json!("other");
    assert_eq!(
        request(
            &app,
            &state,
            "POST",
            &format!("{route}/proof"),
            payload.clone()
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    payload["render"]["sequence"] = json!(4);
    payload["render"]["revision"] = json!(1);
    payload["document"]["source"] = edit["document"]["source"].clone();
    assert_eq!(
        request(&app, &state, "POST", &format!("{route}/proof"), payload)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let current = value(request(&app, &state, "GET", &route, Value::Null).await).await;
    assert_eq!(current["revision"], 0);
    assert_eq!(current["document"], edit["document"]);
    assert_eq!(
        value(request(&app, &state, "GET", &history_route, Value::Null).await).await,
        history
    );
    state.shutdown_rendering();
    state.wait_for_rendered_exports().await;
}

#[tokio::test]
async fn native_region_uses_edited_coordinates_without_proxy_scaling() {
    let (_temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let route = format!("/api/v1/edits/{}", edit["id"].as_str().unwrap());
    let mut document = edit["document"].clone();
    let output_node = document["output_node"].as_str().unwrap().to_string();
    document["nodes"].as_array_mut().unwrap().push(json!({
        "id":"crop", "name":"Crop", "version":1, "enabled":true,
        "inputs":[output_node],
        "operation":{"type":"geometry","parameters":{"crop":[0.0,0.0,0.75,0.75]}}
    }));
    document["output_node"] = json!("crop");
    let saved = request(
        &app,
        &state,
        "PUT",
        &route,
        json!({
            "expected_revision":0,"request_id":"region-crop","document":document
        }),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::OK);
    let region_route = format!("{route}/region");
    let payload = json!({"render":{"revision":1,"session":"native","sequence":1,
        "quality":"interactive","max_edge":16},"rect":{"x":2,"y":1,"width":20,"height":16}});
    let response = request(&app, &state, "POST", &region_route, payload.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-iris-approximate"], "false");
    assert_eq!(response.headers()["x-iris-full-frame-evaluation"], "true");
    assert_eq!(response.headers()["x-iris-full-width"], "24");
    assert_eq!(response.headers()["x-iris-full-height"], "18");
    assert_eq!(response.headers()["x-iris-region-x"], "2");
    assert_eq!(response.headers()["x-iris-region-y"], "1");
    let jpeg = response.into_body().collect().await.unwrap().to_bytes();
    let image = image::load_from_memory(&jpeg).unwrap();
    assert_eq!((image.width(), image.height()), (20, 16));
    let stale = request(&app, &state, "POST", &region_route, payload.clone()).await;
    assert_eq!(stale.status(), StatusCode::BAD_REQUEST);
    for (sequence, rect) in [
        (2, json!({"x":23,"y":0,"width":2,"height":1})),
        (3, json!({"x":0,"y":0,"width":2049,"height":1})),
        (4, json!({"x":0,"y":0,"width":0,"height":1})),
    ] {
        let mut bad = payload.clone();
        bad["render"]["sequence"] = json!(sequence);
        bad["rect"] = rect;
        assert_eq!(
            request(&app, &state, "POST", &region_route, bad)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    state.shutdown_rendering();
    state.wait_for_rendered_exports().await;
}

#[tokio::test]
async fn native_region_accepts_draft_view_without_saving() {
    let (_temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let route = format!("/api/v1/edits/{}", edit["id"].as_str().unwrap());
    let history_route = format!("{route}/history");
    let history = value(request(&app, &state, "GET", &history_route, Value::Null).await).await;
    let region_route = format!("{route}/region");
    let mut payload = json!({"render":{"revision":0,"session":"native-draft","sequence":1,
        "quality":"interactive","max_edge":16},"view":{"center":[1.0,1.0],"width":20,"height":16}});
    let original = request(&app, &state, "POST", &region_route, payload.clone()).await;
    assert_eq!(original.status(), StatusCode::OK);
    assert_eq!(original.headers()["x-iris-region-x"], "12");
    let original = original.into_body().collect().await.unwrap().to_bytes();
    let mut document = edit["document"].clone();
    document["nodes"][1]["operation"]["parameters"]["exposure"] = json!(1.0);
    let output_node = document["output_node"].as_str().unwrap().to_string();
    document["nodes"].as_array_mut().unwrap().push(json!({
        "id":"view-crop","name":"Crop","version":1,"enabled":true,"inputs":[output_node],
        "operation":{"type":"geometry","parameters":{"crop":[0.0,0.0,0.75,0.75]}}
    }));
    document["output_node"] = json!("view-crop");
    payload["document"] = document;
    payload["render"]["sequence"] = json!(2);
    let draft = request(&app, &state, "POST", &region_route, payload.clone()).await;
    assert_eq!(draft.status(), StatusCode::OK);
    assert_eq!(draft.headers()["x-iris-full-width"], "24");
    assert_eq!(draft.headers()["x-iris-full-height"], "18");
    assert_eq!(draft.headers()["x-iris-region-x"], "4");
    assert_eq!(draft.headers()["x-iris-region-y"], "2");
    assert_eq!(draft.headers()["x-iris-approximate"], "false");
    let draft = draft.into_body().collect().await.unwrap().to_bytes();
    let adjusted = image::load_from_memory(&draft).unwrap().to_rgb8();
    assert_eq!(adjusted.dimensions(), (20, 16));
    let original = image::load_from_memory(&original).unwrap().to_rgb8();
    assert!(adjusted.get_pixel(0, 0)[0] > original.get_pixel(0, 0)[0]);
    for (sequence, field, bad_value) in [
        (3, "rect", json!({"x":0,"y":0,"width":2,"height":2})),
        (4, "view", Value::Null),
        (
            5,
            "view",
            json!({"center":[-0.1,0.5],"width":20,"height":16}),
        ),
    ] {
        let mut bad = payload.clone();
        bad["render"]["sequence"] = json!(sequence);
        bad[field] = bad_value;
        assert_eq!(
            request(&app, &state, "POST", &region_route, bad)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    payload["render"]["sequence"] = json!(6);
    payload["document"]["source"]["photo_id"] = json!("wrong");
    assert_eq!(
        request(&app, &state, "POST", &region_route, payload.clone())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    payload["document"]["source"]["photo_id"] = edit["document"]["source"]["photo_id"].clone();
    payload["render"]["sequence"] = json!(7);
    payload["render"]["revision"] = json!(1);
    assert_eq!(
        request(&app, &state, "POST", &region_route, payload)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let current = value(request(&app, &state, "GET", &route, Value::Null).await).await;
    assert_eq!(current["document"], edit["document"]);
    assert_eq!(current["revision"], 0);
    assert_eq!(
        value(request(&app, &state, "GET", &history_route, Value::Null).await).await,
        history
    );
    state.shutdown_rendering();
    state.wait_for_rendered_exports().await;
}

fn range_mask_document(edit: &Value) -> Value {
    let mut document = edit["document"].clone();
    let base = document["output_node"].clone();
    let nodes = document["nodes"].as_array_mut().unwrap();
    nodes.push(json!({"id":"range-mask","name":"Range","version":1,"enabled":true,
        "inputs":[base],"operation":{"type":"mask","parameters":{"kind":"luminance_range","low":0,"high":0.3,"feather":0}}}));
    nodes.push(
        json!({"id":"local","name":"Local","version":1,"enabled":true,
        "inputs":[base],"operation":{"type":"adjustments","parameters":{"exposure":4}}}),
    );
    nodes.push(json!({"id":"range-blend","name":"Blend","version":1,"enabled":true,
        "inputs":[base,"local","range-mask"],"operation":{"type":"blend","parameters":{"opacity":1}}}));
    document["output_node"] = json!("range-blend");
    document
}

#[tokio::test]
async fn mask_overlay_uses_working_input_not_output_display_or_local_adjustments() {
    let (_temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let route = format!("/api/v1/edits/{}", edit["id"].as_str().unwrap());
    let history_route = format!("{route}/history");
    let history = value(request(&app, &state, "GET", &history_route, Value::Null).await).await;
    let overlay_route = format!("{route}/mask-preview");
    let mut payload = json!({"render":{"revision":0,"session":"mask-preview","sequence":1,
        "quality":"interactive","max_edge":32},"document":range_mask_document(&edit),"mask_id":"range-mask"});
    let response = request(&app, &state, "POST", &overlay_route, payload.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(
        response.headers()["x-iris-mask-domain"],
        "linear_rec2020_d65"
    );
    let png = response.into_body().collect().await.unwrap().to_bytes();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (32, 24));
    assert!(image.pixels().all(|p| p[0] == 255 && p[3] == 140));
    payload["render"]["sequence"] = json!(2);
    payload["document"]["nodes"][1]["operation"]["parameters"]["exposure"] = json!(3.0);
    let response = request(&app, &state, "POST", &overlay_route, payload.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let png = response.into_body().collect().await.unwrap().to_bytes();
    assert!(image::load_from_memory(&png)
        .unwrap()
        .to_rgba8()
        .pixels()
        .all(|p| p[3] == 0));
    payload["render"]["sequence"] = json!(3);
    payload["document"] = range_mask_document(&edit);
    let mask = payload["document"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "range-mask")
        .unwrap();
    let range = mask["operation"]["parameters"].clone();
    mask["operation"]["parameters"] = json!({"kind":"combine","operation":"intersect","left":range,
        "right":{"kind":"linear_gradient","start":[0,0],"end":[1,0]}});
    let response = request(&app, &state, "POST", &overlay_route, payload.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let png = response.into_body().collect().await.unwrap().to_bytes();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert!(image.get_pixel(0, 0)[3] < 5 && image.get_pixel(31, 0)[3] > 135);
    payload["render"]["sequence"] = json!(4);
    payload["document"]["source"]["photo_id"] = json!("other");
    assert_eq!(
        request(&app, &state, "POST", &overlay_route, payload.clone())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    payload["document"] = range_mask_document(&edit);
    payload["render"]["sequence"] = json!(5);
    payload["render"]["revision"] = json!(1);
    assert_eq!(
        request(&app, &state, "POST", &overlay_route, payload)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        value(request(&app, &state, "GET", &route, Value::Null).await).await["document"],
        edit["document"]
    );
    assert_eq!(
        value(request(&app, &state, "GET", &history_route, Value::Null).await).await,
        history
    );
    state.shutdown_rendering();
    state.wait_for_rendered_exports().await;
}

#[tokio::test]
async fn color_mask_overlay_preserves_inversion_opacity_and_rejects_unsafe_grids() {
    let (_temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let route = format!(
        "/api/v1/edits/{}/mask-preview",
        edit["id"].as_str().unwrap()
    );
    let mut document = range_mask_document(&edit);
    let index = document["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|node| node["id"] == "range-mask")
        .unwrap();
    // This radius contains the working-linear fixture but not encoded display RGB.
    let range = json!({"kind":"color_range","color":[0,0,0],"radius":0.6,"feather":0});
    for (sequence, definition, alpha) in [
        (1, range.clone(), 140),
        (2, json!({"kind":"invert","mask":range}), 0),
        (3, json!({"kind":"opacity","mask":range,"opacity":0.5}), 70),
    ] {
        document["nodes"][index]["operation"]["parameters"] = definition;
        let payload = json!({"render":{"revision":0,"session":"color-mask","sequence":sequence,
            "quality":"interactive","max_edge":32},"document":document,"mask_id":"range-mask"});
        let response = request(&app, &state, "POST", &route, payload).await;
        assert_eq!(response.status(), StatusCode::OK);
        let png = response.into_body().collect().await.unwrap().to_bytes();
        assert!(image::load_from_memory(&png)
            .unwrap()
            .to_rgba8()
            .pixels()
            .all(|p| p[3] == alpha));
    }
    let base = json!({"render":{"revision":0,"session":"reject-mask","sequence":1,
        "quality":"interactive","max_edge":32},"document":document,"mask_id":"range-mask"});
    let mut invalid = Vec::new();
    let mut payload = base.clone();
    payload["mask_id"] = json!("missing");
    invalid.push(payload);
    let mut payload = base.clone();
    payload["document"]["nodes"][index]["enabled"] = json!(false);
    invalid.push(payload);
    let mut payload = base.clone();
    payload["render"]["max_edge"] = json!(1025);
    invalid.push(payload);
    let mut payload = base.clone();
    payload["render"]["quality"] = json!("full");
    invalid.push(payload);
    let mut payload = base;
    payload["document"]["nodes"].as_array_mut().unwrap().push(json!({
        "id":"after-mask-crop","name":"Crop","version":1,"enabled":true,
        "inputs":["range-blend"],"operation":{"type":"geometry","parameters":{"crop":[0,0,0.5,1]}}
    }));
    payload["document"]["output_node"] = json!("after-mask-crop");
    invalid.push(payload);
    for (index, mut payload) in invalid.into_iter().enumerate() {
        payload["render"]["sequence"] = json!(index + 1);
        assert_eq!(
            request(&app, &state, "POST", &route, payload)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    state.shutdown_rendering();
    state.wait_for_rendered_exports().await;
}

#[tokio::test]
async fn lens_profile_import_requires_exif_evidence_and_wb_requires_raw() {
    let (_temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let route = format!("/api/v1/edits/{}", edit["id"].as_str().unwrap());
    let profile = json!({"schema_version":1,"name":"Synthetic","model":"iris_inverse_radial_v1","coordinates":"oriented_uncropped_image_edges_half_short_edge_v1","camera_make":"FixtureCo","camera_model":"FixtureBody","lens_model":"FixtureLens","calibrated_dimensions":[32,24],"focal_mm":[40,60],"f_number":[2.8,8],"correction":{"radial":[-0.02,0.001],"chromatic_aberration":[0.002,-0.001],"vignette_stops":0.3}});
    let resource = value(
        request(
            &app,
            &state,
            "POST",
            "/api/v1/edit-resources/import/lens_profile",
            profile,
        )
        .await,
    )
    .await;
    assert_eq!(resource["kind"], "lens_profile");
    let response = request(
        &app,
        &state,
        "POST",
        &format!("{route}/lens-profile"),
        json!({"expected_revision":0,"resource":resource["hash"]}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let result = value(response).await;
    assert_eq!(result["matching"]["applicable"], false);
    assert!(result["matching"]["correction"].is_null());
    assert!(result["matching"]["issues"]
        .as_array()
        .unwrap()
        .contains(&json!("lens_model_missing")));
    let wb = request(
        &app,
        &state,
        "POST",
        &format!("{route}/raw-white-balance"),
        json!({"expected_revision":0,"request":{"kind":"as_shot"}}),
    )
    .await;
    assert_eq!(wb.status(), StatusCode::BAD_REQUEST);
    let injected = request(
        &app,
        &state,
        "POST",
        &format!("{route}/lens-profile"),
        json!({"expected_revision":0,"resource":resource["hash"],"camera_make":"FixtureCo"}),
    )
    .await;
    assert_eq!(injected.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let current = value(request(&app, &state, "GET", &route, Value::Null).await).await;
    assert_eq!(current["revision"], 0);
    assert_eq!(current["document"], edit["document"]);
    state.shutdown_rendering();
    state.wait_for_rendered_exports().await;
}

#[tokio::test]
async fn draft_preview_changes_pixels_without_history_and_rejects_wrong_source() {
    let (_temp, state, app, id) = fixture();
    let edit = create(&app, &state, id).await;
    let route = format!("/api/v1/edits/{}", edit["id"].as_str().unwrap());
    let history_route = format!("{route}/history");
    let history = value(request(&app, &state, "GET", &history_route, Value::Null).await).await;
    let render =
        json!({"revision":0,"session":"draft-test","sequence":1,"quality":"full","max_edge":32});
    let before = request(
        &app,
        &state,
        "POST",
        &format!("{route}/preview"),
        render.clone(),
    )
    .await;
    assert_eq!(before.status(), StatusCode::OK);
    let original_fingerprint = before.headers()["x-iris-fingerprint"].clone();
    let original = before.into_body().collect().await.unwrap().to_bytes();
    let mut document = edit["document"].clone();
    document["nodes"][1]["operation"]["parameters"]["exposure"] = json!(1.0);
    let mut payload = json!({"render":render,"document":document});
    payload["render"]["sequence"] = json!(2);
    let draft = request(
        &app,
        &state,
        "POST",
        &format!("{route}/preview-draft"),
        payload.clone(),
    )
    .await;
    assert_eq!(draft.status(), StatusCode::OK);
    assert_eq!(draft.headers()["x-iris-draft"], "true");
    assert_eq!(draft.headers()["x-iris-revision"], "0");
    assert_ne!(draft.headers()["x-iris-fingerprint"], original_fingerprint);
    let draft = draft.into_body().collect().await.unwrap().to_bytes();
    let original_pixel = image::load_from_memory(&original)
        .unwrap()
        .to_rgb8()
        .get_pixel(16, 12)
        .0;
    let draft_pixel = image::load_from_memory(&draft)
        .unwrap()
        .to_rgb8()
        .get_pixel(16, 12)
        .0;
    assert!(draft_pixel[0] > original_pixel[0]);
    assert_eq!(
        value(request(&app, &state, "GET", &history_route, Value::Null).await).await,
        history
    );
    let current = value(request(&app, &state, "GET", &route, Value::Null).await).await;
    assert_eq!(current["document"], edit["document"]);
    for (sequence, field, value) in [
        (3, "photo_id", json!("other")),
        (4, "fingerprint", json!("0".repeat(64))),
        (5, "width", json!(31)),
    ] {
        let mut bad = payload.clone();
        bad["render"]["sequence"] = json!(sequence);
        bad["document"]["source"][field] = value;
        assert_eq!(
            request(&app, &state, "POST", &format!("{route}/preview-draft"), bad)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    payload["render"]["sequence"] = json!(6);
    payload["render"]["revision"] = json!(1);
    assert_eq!(
        request(
            &app,
            &state,
            "POST",
            &format!("{route}/preview-draft"),
            payload
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    state.shutdown_rendering();
    state.wait_for_rendered_exports().await;
}

#[test]
fn merged_openapi_operation_ids_are_globally_unique() {
    use utoipa::OpenApi;
    let doc = serde_json::to_value(iris_daemon::ApiDoc::openapi()).unwrap();
    let mut seen = std::collections::BTreeMap::new();
    for (path, item) in doc["paths"].as_object().unwrap() {
        for method in [
            "get", "put", "post", "delete", "options", "head", "patch", "trace",
        ] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let id = operation["operationId"]
                .as_str()
                .expect("each operation needs an operationId");
            assert!(!id.is_empty(), "empty operationId at {method} {path}");
            let location = format!("{method} {path}");
            let previous = seen.insert(id, location.clone());
            assert!(
                previous.is_none(),
                "duplicate operationId {id}: {previous:?} and {location}"
            );
        }
    }
    assert_eq!(
        doc["paths"]["/api/v1/rendered-exports/{id}/cancel"]["post"]["operationId"],
        "editing_exports_cancel"
    );
    assert_eq!(
        doc["paths"]["/api/v1/edit-render/{session}/cancel"]["post"]["operationId"],
        "editing_preview_cancel"
    );
    assert_eq!(
        doc["paths"]["/api/v1/edits/{id}/preview"]["post"]["operationId"],
        "editing_preview_preview"
    );
}
