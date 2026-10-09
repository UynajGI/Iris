use iris_core::{Action, ColorLabel, MarkRequest, PhotoFilter, Services, Store};
use rusqlite::params;

#[test]
fn version_three_upgrade_preserves_existing_decisions_and_creates_backup() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("db.sqlite");
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let mut services = Services::new(Store::open(&database).unwrap());
    let project = services.create_project(&root).unwrap();
    services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'a.jpg','a.jpg','jpeg',1,1,1,1)",[project.id]).unwrap();
    let id = services.store.conn.last_insert_rowid();
    services
        .decisions(project.id, &[id], Action::Keep, "human", false)
        .unwrap();
    // Remove later additions to reproduce the actual v3 schema.
    services
        .store
        .conn
        .execute_batch("DROP TABLE analysis_runs; DROP TABLE photo_marks; PRAGMA user_version=3;")
        .unwrap();
    drop(services);
    let store = Store::open(&database).unwrap();
    assert!(store
        .recovery_notice
        .as_ref()
        .unwrap()
        .contains("recovery backup"));
    let services = Services::new(store);
    let photo = services.photo(id).unwrap();
    assert_eq!(
        (photo.decision, photo.rating, photo.color_label),
        (Action::Keep, 0, ColorLabel::None)
    );
}

#[test]
fn marks_are_atomic_scoped_persistent_and_share_the_undo_stack() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let database = temp.path().join("library.sqlite");
    let mut services = Services::new(Store::open(&database).unwrap());
    let project = services.create_project(&root).unwrap();
    for filename in ["a.jpg", "b.jpg"] {
        services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,?2,?2,'jpeg',1,1,1,1)",params![project.id,filename]).unwrap();
    }
    let photos = services.photos(project.id, PhotoFilter::default()).unwrap();
    let a = photos[0].id;
    let b = photos[1].id;
    assert_eq!(photos[0].rating, 0);
    let batch = services
        .mark_photos(
            project.id,
            MarkRequest {
                photo_ids: vec![a, a],
                decision: Some(Action::Keep),
                rating: Some(4),
                color_label: Some(ColorLabel::Purple),
            },
        )
        .unwrap();
    assert_eq!(batch.changed, 1);
    assert_eq!(services.photo(b).unwrap().rating, 0);
    drop(services);
    let mut services = Services::new(Store::open(&database).unwrap());
    let photo = services.photo(a).unwrap();
    assert_eq!(
        (photo.decision, photo.rating, photo.color_label),
        (Action::Keep, 4, ColorLabel::Purple)
    );
    assert_eq!(
        services
            .photos(
                project.id,
                PhotoFilter {
                    rating: Some(4),
                    color_label: Some(ColorLabel::Purple),
                    ..Default::default()
                }
            )
            .unwrap()
            .len(),
        1
    );
    assert!(services
        .mark_photos(
            project.id,
            MarkRequest {
                photo_ids: vec![a],
                decision: Some(Action::Reject),
                rating: Some(6),
                color_label: None
            }
        )
        .is_err());
    assert_eq!(services.photo(a).unwrap().decision, Action::Keep);
    // Existing decision operations must undo before the earlier combined mark.
    services
        .decisions(project.id, &[b], Action::Flag, "human", false)
        .unwrap();
    services.undo(project.id).unwrap();
    assert_eq!(services.photo(b).unwrap().decision, Action::Pending);
    assert_eq!(services.photo(a).unwrap().rating, 4);
    assert_eq!(services.undo(project.id).unwrap().changed, 1);
    let photo = services.photo(a).unwrap();
    assert_eq!(
        (photo.decision, photo.rating, photo.color_label),
        (Action::Pending, 0, ColorLabel::None)
    );
}

#[test]
fn foreign_project_ids_prevent_the_entire_mark_batch() {
    let temp = tempfile::tempdir().unwrap();
    let mut services = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    let mut ids = Vec::new();
    let mut projects = Vec::new();
    for name in ["one", "two"] {
        let root = temp.path().join(name);
        std::fs::create_dir(&root).unwrap();
        let project = services.create_project(&root).unwrap();
        services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'a.jpg','a.jpg','jpeg',1,1,1,1)",[project.id]).unwrap();
        ids.push(services.store.conn.last_insert_rowid());
        projects.push(project.id);
    }
    assert!(services
        .mark_photos(
            projects[0],
            MarkRequest {
                photo_ids: ids.clone(),
                decision: None,
                rating: Some(5),
                color_label: Some(ColorLabel::Red)
            }
        )
        .is_err());
    assert_eq!(services.photo(ids[0]).unwrap().rating, 0);
}
