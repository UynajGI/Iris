use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use iris_daemon::{router, AppState};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tower::ServiceExt;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../iris-core/tests/fixtures/media")
}

async fn request(
    app: &Router,
    state: &AppState,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, String, Vec<u8>) {
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
    let mime = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    (
        status,
        mime,
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
}

async fn scan(app: &Router, state: &AppState, id: i64) -> Value {
    let (status, _, _) = request(
        app,
        state,
        "POST",
        &format!("/api/v1/projects/{id}/scan"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for _ in 0..500 {
        let (_, _, bytes) = request(
            app,
            state,
            "GET",
            &format!("/api/v1/projects/{id}/progress"),
            Value::Null,
        )
        .await;
        let progress: Value = serde_json::from_slice(&bytes).unwrap();
        if matches!(
            progress["state"].as_str(),
            Some("completed" | "failed" | "cancelled")
        ) {
            return progress;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("scan did not finish within the bounded test wait");
}

async fn verify_mixed(app: &Router, state: &AppState, id: i64, root: &Path, expected: usize) {
    let progress = scan(app, state, id).await;
    assert_eq!(progress["state"], "completed", "{progress}");
    assert_eq!(progress["result"]["added"], expected);
    let (status, _, bytes) = request(
        app,
        state,
        "GET",
        &format!("/api/v1/projects/{id}/photos"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let photos: Vec<Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(photos.len(), expected);
    for photo in photos {
        let photo_id = photo["id"].as_i64().unwrap();
        let original = fs::read(root.join(photo["path"].as_str().unwrap())).unwrap();
        let (status, mime, bytes) = request(
            app,
            state,
            "GET",
            &format!("/api/v1/photos/{photo_id}/original"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(mime, format!("image/{}", photo["format"].as_str().unwrap()));
        assert_eq!(
            bytes, original,
            "original endpoint must preserve exact file bytes"
        );
        for endpoint in ["thumb", "preview"] {
            let (status, mime, bytes) = request(
                app,
                state,
                "GET",
                &format!("/api/v1/photos/{photo_id}/{endpoint}"),
                Value::Null,
            )
            .await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{photo:?} {endpoint}: {}",
                String::from_utf8_lossy(&bytes)
            );
            assert_eq!(mime, "image/jpeg");
            assert!(bytes.starts_with(&[0xff, 0xd8]) && bytes.ends_with(&[0xff, 0xd9]));
        }
        assert_eq!(
            fs::read(root.join(photo["path"].as_str().unwrap())).unwrap(),
            original
        );
    }
}

#[tokio::test]
async fn png_webp_jpeg_http_original_mime_bytes_and_jpeg_previews() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("mixed");
    fs::create_dir(&root).unwrap();
    for ext in ["jpg", "png", "webp"] {
        fs::copy(
            fixtures().join(format!("quadrants.{ext}")),
            root.join(format!("quadrants.{ext}")),
        )
        .unwrap();
    }
    let state = AppState::new(
        &temp.path().join("library.sqlite"),
        temp.path().join("models-unused"),
    )
    .unwrap();
    let id = state
        .services
        .lock()
        .unwrap()
        .create_project(&root)
        .unwrap()
        .id;
    verify_mixed(&router(state.clone()), &state, id, &root, 3).await;
}

#[tokio::test]
#[ignore = "requires built models/media HEIC DLLs; run tools/setup-heif-runtime.py"]
async fn background_scan_and_http_previews_use_explicit_media_runtime() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("mixed");
    fs::create_dir(&root).unwrap();
    fs::copy(fixtures().join("quadrants.heic"), root.join("照片.heic")).unwrap();
    // A missing explicitly selected runtime must fail. Falling back to the
    // developer's source-tree DLLs would hide a lost background-scan setting.
    let missing = AppState::new(
        &temp.path().join("missing.sqlite"),
        temp.path().join("missing-custom-models"),
    )
    .unwrap();
    let id = missing
        .services
        .lock()
        .unwrap()
        .create_project(&root)
        .unwrap()
        .id;
    let failed = scan(&router(missing.clone()), &missing, id).await;
    assert_eq!(failed["state"], "failed", "{failed}");
    assert!(failed["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.as_str().unwrap().contains("HEIC runtime missing")));

    let model_dir = temp.path().join("自定义模型目录");
    fs::create_dir_all(model_dir.join("media")).unwrap();
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/media");
    for name in ["libheif.dll", "libde265.dll", "libwinpthread-1.dll"] {
        fs::copy(runtime.join(name), model_dir.join("media").join(name)).unwrap();
    }
    for ext in ["jpg", "png", "webp"] {
        fs::copy(
            fixtures().join(format!("quadrants.{ext}")),
            root.join(format!("quadrants.{ext}")),
        )
        .unwrap();
    }
    let state = AppState::new(&temp.path().join("library.sqlite"), model_dir).unwrap();
    let id = state
        .services
        .lock()
        .unwrap()
        .create_project(&root)
        .unwrap()
        .id;
    verify_mixed(&router(state.clone()), &state, id, &root, 4).await;
}
