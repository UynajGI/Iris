// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use iris_daemon::{router, ApiDoc, AppState};
use serde_json::Value;
use tower::ServiceExt;
use utoipa::OpenApi;

#[tokio::test]
async fn metadata_route_is_authenticated_read_only_and_in_the_contract() {
    let temp = tempfile::tempdir().unwrap();
    let photos = temp.path().join("photos");
    std::fs::create_dir(&photos).unwrap();
    let source = photos.join("synthetic.jpg");
    std::fs::write(&source, [0xff, 0xd8, 0xff, 0xd9]).unwrap();
    let mtime = std::fs::metadata(&source)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .min(i64::MAX as u128) as i64;
    let state = AppState::new(&temp.path().join("db.sqlite"), temp.path().into()).unwrap();
    let id = {
        let mut services = state.services.lock().unwrap();
        let project = services.create_project(&photos).unwrap();
        services.store.conn.execute(
            "INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'synthetic.jpg','synthetic.jpg','jpeg',?2,4,32,24)",
            (project.id, mtime),
        ).unwrap();
        services.store.conn.last_insert_rowid()
    };
    let token = state.token.clone();
    let app = router(state);
    let path = format!("/api/v1/photos/{id}/metadata");
    let unauthenticated = app
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(
            Request::builder()
                .uri(&path)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["status"], "not_found");
    assert!(body["camera_model"].is_null());
    assert_eq!(std::fs::read(source).unwrap(), [0xff, 0xd8, 0xff, 0xd9]);
    let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
    assert!(schema["paths"]["/api/v1/photos/{id}/metadata"]["get"].is_object());
}
