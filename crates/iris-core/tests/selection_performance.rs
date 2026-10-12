use iris_core::{PhotoFilter, Services, Store};
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn scan_publishes_durable_first_batch_and_cancellation_preserves_unvisited_photos() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    for index in 0..70 {
        image::RgbImage::new(16, 16)
            .save(root.join(format!("{index:03}.jpg")))
            .unwrap();
    }
    let database = temp.path().join("library.sqlite");
    let mut services = Services::new(Store::open(&database).unwrap());
    let id = services.create_project(&root).unwrap().id;
    let reader = Services::new(Store::open(&database).unwrap());
    let mut batches = Vec::new();
    services
        .scan_with_cancel(id, &AtomicBool::new(false), |report| {
            let count = reader.photos(id, PhotoFilter::default()).unwrap().len();
            assert_eq!(count, report.added);
            batches.push(count);
        })
        .unwrap();
    assert_eq!(batches, [1, 33, 65, 70]);
    std::fs::remove_file(root.join("069.jpg")).unwrap();
    let cancelled = AtomicBool::new(false);
    let report = services
        .scan_with_cancel(id, &cancelled, |_| cancelled.store(true, Ordering::Relaxed))
        .unwrap();
    assert!(report.cancelled);
    assert_eq!(report.missing, 0);
    assert_eq!(
        services.photos(id, PhotoFilter::default()).unwrap().len(),
        70
    );
}

#[test]
fn prepared_preview_outlives_database_and_rejects_changed_or_cancelled_sources() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let source = root.join("a.jpg");
    image::RgbImage::new(16, 16).save(&source).unwrap();
    let mut services = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    let id = services.create_project(&root).unwrap().id;
    services.scan(id).unwrap();
    let photo = services
        .photos(id, PhotoFilter::default())
        .unwrap()
        .remove(0);
    let job = services.prepare_cached_image(photo.id, 320).unwrap();
    drop(services);
    assert!(job
        .render(&AtomicBool::new(true))
        .unwrap_err()
        .to_string()
        .contains("cancelled"));
    assert!(job
        .render(&AtomicBool::new(false))
        .unwrap()
        .starts_with(&[255, 216]));
    std::fs::write(source, b"changed source").unwrap();
    assert!(job
        .render(&AtomicBool::new(false))
        .unwrap_err()
        .to_string()
        .contains("changed"));
}

#[test]
fn analysis_batch_rolls_back_faces_and_scores_together_on_invalid_photo() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    image::RgbImage::new(16, 16)
        .save(root.join("a.jpg"))
        .unwrap();
    let mut services = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    let id = services.create_project(&root).unwrap().id;
    services.scan(id).unwrap();
    let photo = services
        .photos(id, PhotoFilter::default())
        .unwrap()
        .remove(0);
    let old = serde_json::json!({"composite_score":12,"faces":[{"id":"old"}]});
    let new = serde_json::json!({"composite_score":99,"faces":[]});
    services.save_analysis(photo.id, &old).unwrap();
    assert!(services
        .save_analysis_batch(&[(photo.id, &new), (i64::MAX, &new)])
        .is_err());
    assert_eq!(services.photo(photo.id).unwrap().analysis.unwrap(), old);
    assert_eq!(
        services
            .store
            .conn
            .query_row(
                "SELECT count(*) FROM faces WHERE photo_id=?1",
                [photo.id],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn paginated_gallery_preserves_unicode_order_missing_policy_and_descending_offsets() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    for name in ["人像.jpg", "a_lower.jpg", "A_upper.jpg", "é.jpg", "b.jpg"] {
        image::RgbImage::new(8, 8).save(root.join(name)).unwrap();
    }
    let mut services = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    let id = services.create_project(&root).unwrap().id;
    services.scan(id).unwrap();
    let all = services.photos(id, PhotoFilter::default()).unwrap();
    for descending in [false, true] {
        let mut expected = all.clone();
        if descending {
            expected.reverse();
        }
        for offset in [0, 1, 4, 99] {
            let page = services
                .photos(
                    id,
                    PhotoFilter {
                        limit: Some(2),
                        offset: Some(offset),
                        descending: Some(descending),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(
                page.iter().map(|p| p.id).collect::<Vec<_>>(),
                expected
                    .iter()
                    .skip(offset)
                    .take(2)
                    .map(|p| p.id)
                    .collect::<Vec<_>>()
            );
        }
    }
    services
        .store
        .conn
        .execute("UPDATE photos SET missing=1 WHERE id=?1", [all[0].id])
        .unwrap();
    assert_eq!(
        services.photos(id, PhotoFilter::default()).unwrap().len(),
        4
    );
    assert_eq!(
        services
            .photos(
                id,
                PhotoFilter {
                    include_missing: Some(true),
                    ..Default::default()
                }
            )
            .unwrap()
            .len(),
        5
    );
}
