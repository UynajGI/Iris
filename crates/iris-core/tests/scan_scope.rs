use iris_core::{PhotoFilter, Services, Store};
use std::sync::atomic::AtomicBool;

#[test]
fn retry_scans_only_failures_and_unavailable_root_preserves_existing_photos() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let picture = image::RgbImage::new(8, 8);
    picture.save(root.join("good.jpg")).unwrap();
    std::fs::write(root.join("broken.jpg"), b"broken jpeg").unwrap();
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let project = services.create_project(&root).unwrap();
    let report = services.scan(project.id).unwrap();
    assert_eq!(report.added, 1);
    assert_eq!(report.failed_paths, ["broken.jpg"]);
    picture.save(root.join("broken.jpg")).unwrap();
    picture.save(root.join("not-requested.jpg")).unwrap();
    let retry = services
        .scan_scoped_with_cancel(
            project.id,
            Some(&report.failed_paths),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    assert_eq!(retry.added, 1);
    assert!(retry.errors.is_empty());
    let photos = services.photos(project.id, PhotoFilter::default()).unwrap();
    assert_eq!(photos.len(), 2);
    assert!(photos.iter().all(|photo| !photo.missing));
    assert!(services
        .scan_scoped_with_cancel(
            project.id,
            Some(&["../outside".into()]),
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err());
    std::fs::rename(&root, temp.path().join("offline")).unwrap();
    assert!(services.scan(project.id).unwrap().root_unavailable);
    assert_eq!(
        services
            .photos(project.id, PhotoFilter::default())
            .unwrap()
            .len(),
        2
    );
}
