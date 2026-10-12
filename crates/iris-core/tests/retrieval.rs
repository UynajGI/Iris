use iris_core::vision::retrieval::{normalize, status};
use iris_core::{Services, Store};

fn fixture() -> (tempfile::TempDir, Services, i64) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("synthetic");
    std::fs::create_dir(&root).unwrap();
    for name in ["red.png", "blue.png", "other.png"] {
        image::RgbImage::new(32, 32).save(root.join(name)).unwrap();
    }
    let database = temp.path().join("library.sqlite");
    let mut services = Services::new(Store::open(&database).unwrap());
    let project = services.create_project(&root).unwrap().id;
    services.scan(project).unwrap();
    (temp, services, project)
}
#[test]
fn persistent_search_ranks_only_compatible_live_sources_and_reuses_weights_independently() {
    let (temp, mut services, project) = fixture();
    let database = temp.path().join("library.sqlite");
    let space = "a".repeat(64);
    let pending = services.retrieval_pending(project, &space).unwrap();
    assert_eq!(pending.len(), 3);
    for (i, photo) in pending.iter().enumerate() {
        services
            .save_retrieval_embedding(
                project,
                photo,
                &space,
                &[
                    if i == 0 { 1. } else { 0. },
                    if i == 1 { 1. } else { 0. },
                    if i == 2 { 1. } else { 0. },
                ],
                3,
            )
            .unwrap();
    }
    assert_eq!(
        services
            .retrieval_coverage(project, &space)
            .unwrap()
            .indexed,
        3
    );
    assert!(services
        .retrieval_pending(project, &space)
        .unwrap()
        .is_empty());
    assert_eq!(
        services
            .search_embeddings(project, &space, &[1., 0., 0.], 1)
            .unwrap()[0]
            .photo_id,
        pending[0].photo_id
    );
    assert!(services
        .search_embeddings(project, &"b".repeat(64), &[1., 0., 0.], 48)
        .unwrap()
        .is_empty());
    let mut settings = services.settings(project).unwrap();
    settings["sharpness_weight"] = serde_json::json!(50.);
    services.set_settings(project, settings).unwrap();
    assert!(services
        .retrieval_pending(project, &space)
        .unwrap()
        .is_empty());
    drop(services);
    let services = Services::new(Store::open(&database).unwrap());
    assert_eq!(
        services
            .search_embeddings(project, &space, &[0., 1., 0.], 2)
            .unwrap()[0]
            .photo_id,
        pending[1].photo_id
    );
    services
        .store
        .conn
        .execute(
            "UPDATE photos SET mtime=mtime+1 WHERE id=?1",
            [pending[0].photo_id],
        )
        .unwrap();
    services
        .store
        .conn
        .execute(
            "UPDATE photos SET quarantined=1 WHERE id=?1",
            [pending[1].photo_id],
        )
        .unwrap();
    assert_eq!(
        services
            .search_embeddings(project, &space, &[1., 0., 0.], 48)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        services.retrieval_pending(project, &space).unwrap().len(),
        1
    );
    assert_eq!(
        services
            .retrieval_coverage(project, &space)
            .unwrap()
            .pending,
        1
    );
    assert!(services
        .search_embeddings(project, &space, &[f32::NAN, 1., 0.], 48)
        .is_err());
    assert!(normalize(&[0., 0.], 2).is_err());
    assert!(normalize(&[1., 2.], 3).is_err());
    assert_eq!(status(temp.path()).state, "disabled");
}
