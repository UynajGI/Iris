// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[tokio::test]
async fn edit_ids_obey_photo_root_scope_and_read_only_mode() {
    use iris_core::{render, PhotoFilter};
    let temp = tempfile::tempdir().unwrap();
    let allowed = temp.path().join("allowed");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&allowed).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let state = AppState::new(&temp.path().join("db.sqlite3"), temp.path().join("models")).unwrap();
    let ctx = render::RenderContext::default();
    let frame = render::Frame::new(2, 2, vec![[0.25, 0.25, 0.25, 1.0]; 4]).unwrap();
    let png = render::output::encode(
        &frame,
        &Default::default(),
        render::output::OutputFormat::Png16,
        &ctx,
    )
    .unwrap();
    std::fs::write(outside.join("synthetic.png"), png).unwrap();
    let job = {
        let mut s = state.services.lock().unwrap();
        let project = s.create_project(outside).unwrap();
        s.scan(project.id).unwrap();
        let photos = s.photos(project.id, PhotoFilter::default()).unwrap();
        s.prepare_edit_source(photos[0].id).unwrap()
    };
    let source = job.decode(&ctx).unwrap();
    let record = state
        .services
        .lock()
        .unwrap()
        .create_edit_document(job.photo_id, "fixture", source.binding)
        .unwrap();
    let server = IrisMcp::new(state.clone(), temp.path(), &[allowed], false).unwrap();
    let args = json!({"edit_id":record.id});
    assert!(server
        .invoke("iris_get_edit", args.as_object().unwrap().clone())
        .await
        .is_err());
    let permitted = IrisMcp::new(state, temp.path(), &[temp.path().to_owned()], true).unwrap();
    let read = permitted
        .invoke("iris_get_edit", args.as_object().unwrap().clone())
        .await
        .unwrap();
    assert_ne!(read.is_error, Some(true));
    assert!(!permitted
        .endpoints
        .iter()
        .any(|e| e.name == "iris_save_edit"));
    let endpoint = permitted
        .endpoints
        .iter()
        .find(|e| e.name == "iris_get_edit")
        .unwrap();
    let (path, _) = permitted.request(endpoint, &args).unwrap();
    assert_eq!(path, format!("/api/v1/edits/{}", record.id));
}
