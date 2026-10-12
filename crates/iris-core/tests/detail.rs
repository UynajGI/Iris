use iris_core::vision::detail::{DetailEvidence, DetailRequest};
use iris_core::{Services, Store};
use std::sync::atomic::AtomicBool;

#[test]
fn detail_cache_is_independent_of_weights_and_rejects_changed_or_unavailable_sources() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("synthetic");
    std::fs::create_dir(&root).unwrap();
    let path = root.join("portrait.png");
    image::RgbImage::from_pixel(128, 128, image::Rgb([100, 150, 200]))
        .save(&path)
        .unwrap();
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let project = services.create_project(&root).unwrap().id;
    services.scan(project).unwrap();
    let photo = services
        .photos(project, Default::default())
        .unwrap()
        .remove(0);
    let request = DetailRequest::default();
    let cancel = AtomicBool::new(false);
    let prepared = services.prepare_detail(photo.id, &request).unwrap();
    let first = prepared.render(&cancel).unwrap();
    let report: DetailEvidence = serde_json::from_slice(&first).unwrap();
    assert_eq!(report.state, "full_detail");
    let cache = services.cache_status(project).unwrap();
    assert_eq!(cache.files, 1);
    let mut settings = services.settings(project).unwrap();
    settings["sharpness_weight"] = serde_json::json!(50.);
    services.set_settings(project, settings).unwrap();
    assert_eq!(
        services
            .prepare_detail(photo.id, &request)
            .unwrap()
            .render(&cancel)
            .unwrap(),
        first
    );
    assert_eq!(services.cache_status(project).unwrap().files, 1);
    assert!(services
        .prepare_detail(
            photo.id,
            &DetailRequest {
                face_index: Some(0),
                region: None
            }
        )
        .is_err());
    std::fs::write(&path, b"changed source").unwrap();
    assert!(prepared.render(&cancel).is_err());
    assert!(services.prepare_detail(photo.id, &request).is_err());
    services
        .store
        .conn
        .execute("UPDATE photos SET quarantined=1 WHERE id=?1", [photo.id])
        .unwrap();
    assert!(services.prepare_detail(photo.id, &request).is_err());
}
