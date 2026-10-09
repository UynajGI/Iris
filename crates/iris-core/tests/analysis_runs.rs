use iris_core::{Services, Store};
use serde_json::json;

#[test]
fn version_four_upgrade_and_run_history_preserve_project_settings() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("library.sqlite");
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let mut services = Services::new(Store::open(&database).unwrap());
    let project = services.create_project(&root).unwrap();
    let settings = services.settings(project.id).unwrap();
    services
        .store
        .conn
        .execute_batch("DROP TABLE analysis_runs; PRAGMA user_version=4;")
        .unwrap();
    drop(services);

    let store = Store::open(&database).unwrap();
    assert!(store.recovery_notice.as_ref().unwrap().contains(".v4."));
    let services = Services::new(store);
    assert_eq!(services.settings(project.id).unwrap(), settings);
    assert_eq!(services.analysis_run(project.id).unwrap(), None);
    let report = json!({"id":"run-one","state":"failed","failed_photo_ids":[17],
        "execution":[{"phase":"quality","selected_provider":"directml","device_id":1,
        "device_name":"Test GPU","completed_items":3,"warnings":["CPU fallback"]}]});
    services.save_analysis_run(project.id, &report).unwrap();
    assert!(services.save_analysis_run(project.id + 1, &report).is_err());
    drop(services);

    let services = Services::new(Store::open(&database).unwrap());
    assert_eq!(services.analysis_run(project.id).unwrap(), Some(report));
    assert_eq!(services.settings(project.id).unwrap(), settings);
    let replacement = json!({"id":"run-two","execution":[]});
    services
        .save_analysis_run(project.id, &replacement)
        .unwrap();
    assert_eq!(
        services.analysis_run(project.id).unwrap(),
        Some(replacement)
    );
}
