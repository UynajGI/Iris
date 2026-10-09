use iris_core::{AcceptCategory, AcceptRequest, Action, PhotoFilter, Services, Store};
use serde_json::json;
use std::{fs, path::Path};
use tempfile::TempDir;

#[test]
fn group_invalidation_survives_reopen_and_distinguishes_empty_groups() {
    let (temp, mut svc, id) = fixture();
    assert!(!svc.project(id).unwrap().groups_dirty);
    assert_eq!(svc.project(id).unwrap().pending_analysis, 0);
    svc.scan(id).unwrap();
    assert_eq!(svc.project(id).unwrap().pending_analysis, 2);
    for photo in svc.photos(id, PhotoFilter::default()).unwrap() {
        svc.save_analysis(
            photo.id,
            json!({"version":iris_core::vision::ANALYSIS_VERSION,"faces":[]}),
        )
        .unwrap();
    }
    svc.save_groups(id, vec![]).unwrap();
    assert!(!svc.project(id).unwrap().groups_dirty);
    assert_eq!(svc.project(id).unwrap().pending_analysis, 0);
    svc.scan(id).unwrap();
    assert!(
        !svc.project(id).unwrap().groups_dirty,
        "unchanged scan preserves completed zero-group result"
    );
    let settings = svc.settings(id).unwrap();
    svc.set_settings(id, settings.clone()).unwrap();
    assert!(!svc.project(id).unwrap().groups_dirty);
    let mut changed = settings;
    changed["eyes_weight"] = json!(0.3);
    svc.set_settings(id, changed).unwrap();
    assert!(svc.project(id).unwrap().groups_dirty);
    let db = svc.store.path.clone();
    drop(svc);
    let mut svc = Services::new(Store::open(db).unwrap());
    assert!(svc.project(id).unwrap().groups_dirty);
    svc.save_groups(id, vec![]).unwrap();
    jpg(&temp.path().join("photos/new.jpg"), 100);
    svc.scan(id).unwrap();
    assert!(svc.project(id).unwrap().groups_dirty);
    assert!(svc.project(id).unwrap().pending_analysis > 0);
}

#[test]
fn save_groups_checks_only_active_project_members_before_replacing_groups() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let members: Vec<_> = svc
        .photos(id, PhotoFilter::default())
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    let other_root = temp.path().join("other");
    fs::create_dir(&other_root).unwrap();
    jpg(&other_root.join("other.jpg"), 60);
    let other_id = svc.create_project(other_root).unwrap().id;
    svc.scan(other_id).unwrap();
    let foreign = svc.photos(other_id, PhotoFilter::default()).unwrap()[0].id;
    svc.store.conn.execute("INSERT INTO analyses(photo_id,data,version,analyzed_at) VALUES(?1,'invalid-json','current','now')", [members[0]]).unwrap();
    let group = |project_id, member_photo_ids| iris_core::BurstGroup {
        id: "group-test".into(),
        project_id,
        kind: "duplicate".into(),
        member_photo_ids,
    };
    svc.save_groups(id, vec![group(id, members.clone())])
        .unwrap();
    for invalid in [
        group(other_id, members.clone()),
        group(id, vec![foreign]),
        group(id, vec![i64::MAX]),
    ] {
        assert!(svc.save_groups(id, vec![invalid]).is_err());
        assert_eq!(svc.groups(id).unwrap()[0].member_photo_ids, members);
    }
    for (missing, quarantined) in [(true, false), (false, true)] {
        svc.store
            .conn
            .execute(
                "UPDATE photos SET missing=?1,quarantined=?2 WHERE id=?3",
                rusqlite::params![missing, quarantined, members[0]],
            )
            .unwrap();
        assert!(svc
            .save_groups(id, vec![group(id, members.clone())])
            .is_err());
    }
}

#[test]
fn schema_one_upgrade_backs_up_and_marks_existing_projects_dirty() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("v1.sqlite");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TABLE projects(id INTEGER PRIMARY KEY,root TEXT,name TEXT,created_at TEXT,cache_root TEXT);
        CREATE TABLE photos(id INTEGER PRIMARY KEY,project_id INTEGER,mtime INTEGER,size_bytes INTEGER,missing INTEGER,quarantined INTEGER,taken_at TEXT,capture_variant_id TEXT);
        CREATE TABLE analyses(photo_id INTEGER PRIMARY KEY,version TEXT,data TEXT);
        CREATE TABLE burst_groups(id TEXT,project_id INTEGER);
        CREATE TABLE settings(project_id INTEGER,data TEXT);
        INSERT INTO projects VALUES(1,'root','existing','today','cache');
        PRAGMA user_version=1;").unwrap();
    drop(conn);
    let svc = Services::new(Store::open(&db).unwrap());
    assert!(svc
        .store
        .recovery_notice
        .as_ref()
        .unwrap()
        .contains("backup"));
    let project = svc.project(1).unwrap();
    assert_eq!(project.name, "existing");
    assert!(project.groups_dirty);
    assert_eq!(project.pending_analysis, 0);
    assert_eq!(
        svc.store
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        iris_core::store::SCHEMA_VERSION
    );
}

fn jpg(path: &Path, value: u8) {
    image::RgbImage::from_pixel(24, 16, image::Rgb([value, 80, 90]))
        .save(path)
        .unwrap();
}
fn fixture() -> (TempDir, Services, i64) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    fs::create_dir(&root).unwrap();
    jpg(&root.join("a.jpg"), 50);
    jpg(&root.join("b.JPG"), 80);
    fs::write(root.join("unsupported.txt"), b"unsupported attachment").unwrap();
    let mut svc = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    let project = svc.create_project(&root).unwrap();
    (temp, svc, project.id)
}

fn current_suggestion(svc: &Services, project: i64, verdict: &str) -> serde_json::Value {
    json!({"version":iris_core::vision::ANALYSIS_VERSION,
        "settings":svc.settings(project).unwrap(), "verdict":verdict,"faces":[]})
}

fn migrated_cache_fixture() -> (TempDir, Services, i64, iris_core::CacheMigration) {
    let (temp, mut svc, id) = fixture();
    let source = std::path::PathBuf::from(svc.project(id).unwrap().cache_root);
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("a.bin"), b"first cache").unwrap();
    fs::write(source.join("nested/b.bin"), b"second cache").unwrap();
    svc.cache_migrate(id, &temp.path().join("new-cache"))
        .unwrap();
    let migration = svc.cache_migrations(id).unwrap().remove(0);
    (temp, svc, id, migration)
}

#[test]
fn recent_projects_persist_open_order_hide_and_reopen_without_deleting_data() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let root = temp.path().join("other");
    fs::create_dir(&root).unwrap();
    let other = svc.create_project(root).unwrap().id;
    assert_eq!(svc.projects().unwrap()[0].id, other);
    let opened = svc.open_project(id).unwrap();
    assert_eq!(svc.projects().unwrap()[0].id, id);
    svc.settings(id).unwrap();
    svc.photos(id, PhotoFilter::default()).unwrap();
    assert_eq!(
        svc.project(id).unwrap().last_opened_at,
        opened.last_opened_at
    );
    assert!(svc.hide_project(id).unwrap().hidden);
    assert_eq!(svc.photos(id, PhotoFilter::default()).unwrap().len(), 2);
    let db = svc.store.path.clone();
    drop(svc);
    let mut svc = Services::new(Store::open(db).unwrap());
    assert_eq!(
        svc.projects()
            .unwrap()
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![other]
    );
    let reopened = svc.create_project(temp.path().join("photos")).unwrap();
    assert_eq!(reopened.id, id);
    assert!(!reopened.hidden);
    assert_eq!(svc.projects().unwrap()[0].id, id);
    assert_eq!(svc.photos(id, PhotoFilter::default()).unwrap().len(), 2);
    assert!(svc.hide_project(i64::MAX).is_err());
    assert!(svc.open_project(i64::MAX).is_err());
}

#[test]
fn schema_two_upgrade_preserves_projects_and_has_pre_upgrade_backup() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("v2.sqlite");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TABLE projects(id INTEGER PRIMARY KEY,root TEXT,name TEXT,created_at TEXT,cache_root TEXT,groups_dirty INTEGER);
        CREATE TABLE photos(id INTEGER PRIMARY KEY,project_id INTEGER,missing INTEGER,quarantined INTEGER);
        CREATE TABLE analyses(photo_id INTEGER PRIMARY KEY,version TEXT,data TEXT NOT NULL DEFAULT '{}');
        INSERT INTO projects VALUES(1,'photos','kept','2020-01-01T00:00:00Z','cache',0);
        PRAGMA user_version=2;").unwrap();
    drop(conn);
    let svc = Services::new(Store::open(&db).unwrap());
    let p = svc.project(1).unwrap();
    assert_eq!(p.name, "kept");
    assert!(!p.hidden);
    assert_eq!(p.last_opened_at, "2020-01-01T00:00:00Z");
    assert!(svc.cache_migrations(1).unwrap().is_empty());
    assert!(svc.store.recovery_notice.is_some());
    let backup = fs::read_dir(temp.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "bak"))
        .unwrap();
    let backup = rusqlite::Connection::open(backup).unwrap();
    assert_eq!(
        backup
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        backup
            .query_row("SELECT name FROM projects WHERE id=1", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "kept"
    );
}

#[test]
fn cache_cleanup_old_checks_hashes_preserves_unlisted_files_and_is_idempotent() {
    let (_temp, mut svc, id, migration) = migrated_cache_fixture();
    assert_eq!(migration.state, "ready");
    assert_eq!(migration.files.len(), 2);
    let source = Path::new(&migration.source_root);
    fs::write(source.join("unlisted.txt"), b"new user file").unwrap();
    fs::write(source.join("nested/b.bin"), b"modified cache").unwrap();
    assert!(svc.cache_cleanup_old(id, &migration.id).is_err());
    assert!(
        source.join("a.bin").exists(),
        "preflight failure must not delete earlier files"
    );
    assert_eq!(
        fs::read(source.join("nested/b.bin")).unwrap(),
        b"modified cache"
    );
    fs::write(source.join("nested/b.bin"), b"second cache").unwrap();
    let result = svc.cache_cleanup_old(id, &migration.id).unwrap();
    assert_eq!(result.state, "cleaned");
    assert!(result.files.iter().all(|f| f.cleaned));
    assert!(!source.join("a.bin").exists());
    assert!(!source.join("nested/b.bin").exists());
    assert_eq!(
        fs::read(source.join("unlisted.txt")).unwrap(),
        b"new user file"
    );
    assert!(Path::new(&migration.destination_root)
        .join("a.bin")
        .exists());
    assert_eq!(
        svc.cache_cleanup_old(id, &migration.id).unwrap().state,
        "cleaned"
    );
}

#[test]
fn cache_cleanup_old_failure_and_unrecorded_unlink_resume_after_restart() {
    let (_temp, mut svc, id, migration) = migrated_cache_fixture();
    svc.store
        .conn
        .execute_batch(
            "CREATE TRIGGER fail_cache_progress BEFORE UPDATE ON cache_migrations
        WHEN NEW.state='cleanup_in_progress' AND json_extract(NEW.data,'$.error') IS NULL
        AND json_extract(NEW.data,'$.files[0].cleaned')=1
        BEGIN SELECT RAISE(ABORT,'injected cleanup journal failure'); END;",
        )
        .unwrap();
    assert!(svc.cache_cleanup_old(id, &migration.id).is_err());
    let progress = svc.cache_migrations(id).unwrap().remove(0);
    assert_eq!(progress.state, "cleanup_in_progress");
    assert!(progress.error.is_some());
    assert!(!Path::new(&migration.source_root).join("a.bin").exists());
    assert!(Path::new(&migration.source_root)
        .join("nested/b.bin")
        .exists());
    // Simulate exit after a later unlink but before its completion record.
    fs::remove_file(Path::new(&migration.source_root).join("nested/b.bin")).unwrap();
    let db = svc.store.path.clone();
    drop(svc);
    let mut svc = Services::new(Store::open(db).unwrap());
    svc.store
        .conn
        .execute_batch("DROP TRIGGER fail_cache_progress")
        .unwrap();
    assert_eq!(
        svc.cache_cleanup_old(id, &migration.id).unwrap().state,
        "cleaned"
    );
    assert!(svc.cache_migrations(id).unwrap()[0].error.is_none());
}

#[test]
fn cache_cleanup_old_refuses_missing_copy_current_cache_and_other_project_paths() {
    let (temp, mut svc, id, migration) = migrated_cache_fixture();
    let source = Path::new(&migration.source_root);
    let destination = Path::new(&migration.destination_root);
    fs::remove_file(destination.join("nested/b.bin")).unwrap();
    assert!(svc.cache_cleanup_old(id, &migration.id).is_err());
    assert!(source.join("a.bin").exists());
    fs::write(destination.join("nested/b.bin"), b"second cache").unwrap();
    svc.store
        .conn
        .execute(
            "UPDATE projects SET cache_root=?1 WHERE id=?2",
            rusqlite::params![migration.source_root, id],
        )
        .unwrap();
    assert!(svc.cache_cleanup_old(id, &migration.id).is_err());
    assert!(source.join("a.bin").exists());
    svc.store
        .conn
        .execute(
            "UPDATE projects SET cache_root=?1 WHERE id=?2",
            rusqlite::params![migration.destination_root, id],
        )
        .unwrap();
    let other_root = temp.path().join("other");
    fs::create_dir(&other_root).unwrap();
    let other = svc.create_project(&other_root).unwrap().id;
    assert!(svc.cache_cleanup_old(other, &migration.id).is_err());
    svc.store
        .conn
        .execute(
            "UPDATE projects SET cache_root=?1 WHERE id=?2",
            rusqlite::params![migration.source_root, other],
        )
        .unwrap();
    assert!(svc.cache_cleanup_old(id, &migration.id).is_err());
    svc.store
        .conn
        .execute(
            "UPDATE projects SET cache_root=?1,root=?2 WHERE id=?3",
            rusqlite::params![
                temp.path().join("other-cache").to_string_lossy(),
                migration.source_root,
                other
            ],
        )
        .unwrap();
    svc.hide_project(other).unwrap();
    assert!(svc.cache_cleanup_old(id, &migration.id).is_err());
    assert!(source.join("a.bin").exists());
    // The existing current-cache cleanup also protects hidden projects.
    svc.store
        .conn
        .execute(
            "UPDATE projects SET cache_root=?1 WHERE id=?2",
            rusqlite::params![migration.source_root, id],
        )
        .unwrap();
    assert!(svc.cache_cleanup(id).is_err());
    assert!(source.join("a.bin").exists());
}

#[test]
fn default_in_project_managed_cache_can_migrate_and_cleanup_without_touching_photos() {
    let temp = tempfile::tempdir().unwrap();
    jpg(&temp.path().join("photo.jpg"), 80);
    let mut svc = Services::new(Store::open(temp.path().join(".iris/library.sqlite3")).unwrap());
    let id = svc.create_project(temp.path()).unwrap().id;
    svc.scan(id).unwrap();
    let source = std::path::PathBuf::from(svc.project(id).unwrap().cache_root);
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("thumb.jpg"), b"cache").unwrap();
    assert_eq!(svc.scan(id).unwrap().unchanged, 1);
    svc.cache_cleanup(id).unwrap();
    assert!(!source.join("thumb.jpg").exists());
    fs::write(source.join("thumb.jpg"), b"cache").unwrap();
    svc.cache_migrate(id, &temp.path().join(".iris-cache/migrated"))
        .unwrap();
    let migration = svc.cache_migrations(id).unwrap().remove(0);
    svc.cache_cleanup_old(id, &migration.id).unwrap();
    assert!(temp.path().join("photo.jpg").exists());
    assert_eq!(svc.photos(id, PhotoFilter::default()).unwrap().len(), 1);
    assert!(!source.join("thumb.jpg").exists());
    svc.store
        .conn
        .execute(
            "UPDATE photos SET path='.iris-cache/migrated/thumb.jpg' WHERE project_id=?1",
            [id],
        )
        .unwrap();
    assert!(
        svc.cache_cleanup(id).is_err(),
        "managed directories containing a registered photo remain protected"
    );
    assert!(temp.path().join(".iris-cache/migrated/thumb.jpg").exists());
}

#[test]
fn incremental_scan_missing_cancel_and_durable_undo() {
    let (temp, mut svc, id) = fixture();
    let first = svc.scan(id).unwrap();
    assert_eq!(first.added, 2);
    assert_eq!(first.skipped, 1);
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    assert_eq!((photos[0].width, photos[0].height), (24, 16));
    svc.save_analysis(
        photos[0].id,
        json!({"verdict":"recommend","composite_score":88,"faces":[]}),
    )
    .unwrap();
    assert_eq!(svc.scan(id).unwrap().unchanged, 2);
    svc.decisions(id, &[photos[0].id], Action::Keep, "human", true)
        .unwrap();
    svc.decisions(id, &[photos[1].id], Action::Reject, "human", true)
        .unwrap();
    let db = svc.store.path.clone();
    drop(svc);
    let mut svc = Services::new(Store::open(db).unwrap());
    svc.undo(id).unwrap();
    assert_eq!(svc.photo(photos[0].id).unwrap().decision, Action::Keep);
    assert_eq!(svc.photo(photos[1].id).unwrap().decision, Action::Pending);
    fs::remove_file(temp.path().join("photos/a.jpg")).unwrap();
    let cancelled = svc
        .scan_with_cancel(id, &std::sync::atomic::AtomicBool::new(true), |_| {})
        .unwrap();
    assert!(cancelled.cancelled);
    assert!(!svc.photo(photos[0].id).unwrap().missing);
    assert_eq!(svc.scan(id).unwrap().missing, 1);
    assert!(svc.photo_path(photos[0].id).is_err());
    assert_eq!(svc.photos(id, PhotoFilter::default()).unwrap().len(), 1);
}

#[test]
fn scan_change_invalidates_analysis_and_accept_preserves_human_decision() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    for p in &photos {
        svc.save_analysis(p.id, current_suggestion(&svc, id, "recommend"))
            .unwrap();
    }
    svc.decisions(id, &[photos[0].id], Action::Flag, "human", true)
        .unwrap();
    assert_eq!(svc.accept(id).unwrap().changed, 1);
    assert_eq!(svc.photo(photos[0].id).unwrap().decision, Action::Flag);
    jpg(&temp.path().join("photos/b.JPG"), 200);
    let changed = svc.scan(id).unwrap();
    assert_eq!(changed.changed, 1);
    assert!(svc.photo(photos[1].id).unwrap().analysis.is_none());
}

#[test]
fn quarantine_preview_is_inert_stale_guard_and_restore_survive_restart() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    let source = svc.photo_path(p.id).unwrap();
    let bytes = fs::read(&source).unwrap();
    let original_modified = fs::metadata(&source).unwrap().modified().unwrap();
    svc.decisions(id, &[p.id], Action::Reject, "human", true)
        .unwrap();
    let plan = svc.quarantine_preview(id).unwrap();
    assert!(source.exists());
    assert!(!Path::new(&plan.items[0].destination).exists());
    svc.decisions(id, &[p.id], Action::Keep, "human", true)
        .unwrap();
    assert!(svc.quarantine_commit(&plan.id).is_err());
    assert!(source.exists());
    svc.undo(id).unwrap();
    svc.quarantine_commit(&plan.id).unwrap();
    assert!(!source.exists());
    assert!(svc.photo(p.id).unwrap().quarantined);
    assert_eq!(svc.scan(id).unwrap().unchanged, 1);
    drop(svc);
    let mut svc = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    svc.quarantine_restore(&plan.id).unwrap();
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(
        fs::metadata(&source).unwrap().modified().unwrap(),
        original_modified
    );
    assert_eq!(
        svc.scan(id).unwrap().changed,
        0,
        "restoring identical bytes must not invalidate their analysis fingerprint"
    );
    assert!(!svc.photo(p.id).unwrap().quarantined);
    assert!(svc.quarantine_commit(&plan.id).is_err());
}

#[test]
fn quarantine_restore_preflights_all_conflicts_before_moving_any_file() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let ids: Vec<_> = svc
        .photos(id, PhotoFilter::default())
        .unwrap()
        .iter()
        .map(|p| p.id)
        .collect();
    svc.decisions(id, &ids, Action::Reject, "human", false)
        .unwrap();
    let plan = svc.quarantine_preview(id).unwrap();
    svc.quarantine_commit(&plan.id).unwrap();
    // This item is restored last, so the old per-item checks caused a partial restore.
    fs::write(&plan.items[0].source, b"new user file").unwrap();
    assert!(svc.quarantine_restore(&plan.id).is_err());
    assert_eq!(fs::read(&plan.items[0].source).unwrap(), b"new user file");
    assert!(!Path::new(&plan.items[1].source).exists());
    for item in &plan.items {
        assert!(Path::new(&item.destination).exists());
        assert!(svc.photo(item.photo_id).unwrap().quarantined);
    }
    assert_eq!(svc.quarantine_plan(&plan.id).unwrap().state, "committed");
}

#[test]
fn quarantine_restore_runtime_failure_compensates_files_and_flags_then_retries() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let ids: Vec<_> = svc
        .photos(id, PhotoFilter::default())
        .unwrap()
        .iter()
        .map(|p| p.id)
        .collect();
    svc.decisions(id, &ids, Action::Reject, "human", false)
        .unwrap();
    let plan = svc.quarantine_preview(id).unwrap();
    svc.quarantine_commit(&plan.id).unwrap();
    // The first file and the failing file have both moved by the time SQLite refuses this update.
    svc.store.conn.execute_batch(&format!("CREATE TRIGGER fail_restore BEFORE UPDATE OF quarantined ON photos WHEN NEW.quarantined=0 AND NEW.id={} BEGIN SELECT RAISE(ABORT,'injected restore failure'); END;",plan.items[0].photo_id)).unwrap();
    let error = svc.quarantine_restore(&plan.id).unwrap_err().to_string();
    assert!(error.contains("rolled back"), "{error}");
    for item in &plan.items {
        assert!(!Path::new(&item.source).exists());
        assert!(Path::new(&item.destination).exists());
        assert!(svc.photo(item.photo_id).unwrap().quarantined);
    }
    assert_eq!(svc.quarantine_plan(&plan.id).unwrap().state, "committed");
    svc.store
        .conn
        .execute_batch("DROP TRIGGER fail_restore")
        .unwrap();
    let db = svc.store.path.clone();
    drop(svc);
    let mut svc = Services::new(Store::open(db).unwrap());
    assert_eq!(svc.quarantine_restore(&plan.id).unwrap().state, "restored");
    for item in &plan.items {
        assert!(Path::new(&item.source).exists());
        assert!(!Path::new(&item.destination).exists());
        assert!(!svc.photo(item.photo_id).unwrap().quarantined);
    }
}

#[test]
fn quarantine_restore_compensation_failure_keeps_durable_retry_journal() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let ids: Vec<_> = svc
        .photos(id, PhotoFilter::default())
        .unwrap()
        .iter()
        .map(|p| p.id)
        .collect();
    svc.decisions(id, &ids, Action::Reject, "human", false)
        .unwrap();
    let plan = svc.quarantine_preview(id).unwrap();
    svc.quarantine_commit(&plan.id).unwrap();
    svc.store.conn.execute_batch(&format!("CREATE TRIGGER fail_restore BEFORE UPDATE OF quarantined ON photos WHEN NEW.quarantined=0 AND NEW.id={} BEGIN SELECT RAISE(ABORT,'injected restore failure'); END;
        CREATE TRIGGER fail_compensation BEFORE UPDATE OF quarantined ON photos WHEN OLD.quarantined=0 AND NEW.quarantined=1 AND NEW.id={} BEGIN SELECT RAISE(ABORT,'injected compensation failure'); END;",plan.items[0].photo_id,plan.items[1].photo_id)).unwrap();
    let error = svc.quarantine_restore(&plan.id).unwrap_err().to_string();
    assert!(error.contains("compensation incomplete"), "{error}");
    assert_eq!(
        svc.quarantine_plan(&plan.id).unwrap().state,
        "restore_rollback_failed"
    );
    let data: String = svc
        .store
        .conn
        .query_row("SELECT data FROM quarantine WHERE id=?1", [&plan.id], |r| {
            r.get(0)
        })
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(
        data["restore_rollback"]["steps"].as_array().unwrap().len(),
        1
    );
    for item in &plan.items {
        assert!(Path::new(&item.destination).exists());
    }
    let db = svc.store.path.clone();
    drop(svc);
    let mut svc = Services::new(Store::open(db).unwrap());
    svc.store
        .conn
        .execute_batch("DROP TRIGGER fail_restore; DROP TRIGGER fail_compensation;")
        .unwrap();
    assert_eq!(svc.quarantine_restore(&plan.id).unwrap().state, "restored");
    for item in &plan.items {
        assert!(Path::new(&item.source).exists());
        assert!(!Path::new(&item.destination).exists());
        assert!(!svc.photo(item.photo_id).unwrap().quarantined);
    }
}

#[test]
fn quarantine_restore_recovers_write_ahead_step_after_restart() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let ids: Vec<_> = svc
        .photos(id, PhotoFilter::default())
        .unwrap()
        .iter()
        .map(|p| p.id)
        .collect();
    svc.decisions(id, &ids, Action::Reject, "human", false)
        .unwrap();
    let plan = svc.quarantine_preview(id).unwrap();
    let mut plan = svc.quarantine_commit(&plan.id).unwrap();
    // Simulate process exit after moving a file, before completing the item journal.
    fs::rename(&plan.items[1].destination, &plan.items[1].source).unwrap();
    svc.store
        .conn
        .execute(
            "UPDATE photos SET quarantined=0 WHERE id=?1",
            [plan.items[1].photo_id],
        )
        .unwrap();
    plan.state = "restoring".into();
    let mut persisted = serde_json::to_value(&plan).unwrap();
    persisted["restore_rollback"] = json!({"prior_state":"committed","steps":[{
        "index":1,"source_existed":false,"quarantine_existed":true,
        "missing":false,"quarantined":true,"prior_item_state":"moved"
    }]});
    svc.store
        .conn
        .execute(
            "UPDATE quarantine SET state='restoring',data=?1 WHERE id=?2",
            rusqlite::params![persisted.to_string(), plan.id],
        )
        .unwrap();
    let db = svc.store.path.clone();
    drop(svc);
    let mut svc = Services::new(Store::open(db).unwrap());
    assert_eq!(svc.quarantine_restore(&plan.id).unwrap().state, "restored");
    for item in &plan.items {
        assert!(Path::new(&item.source).exists());
        assert!(!Path::new(&item.destination).exists());
        assert!(!svc.photo(item.photo_id).unwrap().quarantined);
    }
}

#[test]
fn quarantine_modified_source_refused_and_restore_never_overwrites() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    svc.decisions(id, &[p.id], Action::Reject, "human", true)
        .unwrap();
    let plan = svc.quarantine_preview(id).unwrap();
    jpg(Path::new(&plan.items[0].source), 99);
    assert!(svc.quarantine_commit(&plan.id).is_err());
    svc.scan(id).unwrap();
    let plan = svc.quarantine_preview(id).unwrap();
    svc.quarantine_commit(&plan.id).unwrap();
    fs::write(&plan.items[0].source, b"new important file").unwrap();
    assert!(svc.quarantine_restore(&plan.id).is_err());
    assert_eq!(
        fs::read(&plan.items[0].source).unwrap(),
        b"new important file"
    );
    assert!(Path::new(&plan.items[0].destination).exists());
}

#[test]
fn exports_csv_are_transactional_preserve_sources_and_existing_outputs() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    let src = svc.photo_path(p.id).unwrap();
    let original = fs::read(&src).unwrap();
    svc.decisions(id, &[p.id], Action::Keep, "human", true)
        .unwrap();
    assert_eq!(svc.export_xmp(id, "keep", false).unwrap().written, 1);
    assert!(fs::read_to_string(src.with_extension("xmp"))
        .unwrap()
        .contains("Rating='5'"));
    assert_eq!(svc.export_xmp(id, "keep", false).unwrap().skipped, 1);
    assert_eq!(fs::read(&src).unwrap(), original);
    let copied = temp.path().join("export");
    assert_eq!(svc.export_copy(id, &copied, "keep").unwrap().written, 1);
    assert_eq!(svc.export_copy(id, &copied, "keep").unwrap().skipped, 1);
    assert!(svc
        .export_copy(id, &temp.path().join("photos/nested"), "keep")
        .is_err());
    let csv = temp.path().join("decisions.csv");
    svc.export_csv(id, &csv).unwrap();
    svc.decisions(id, &[p.id], Action::Flag, "human", false)
        .unwrap();
    svc.import_csv(id, &csv).unwrap();
    assert_eq!(svc.photo(p.id).unwrap().decision, Action::Keep);
    svc.undo(id).unwrap();
    assert_eq!(svc.photo(p.id).unwrap().decision, Action::Flag);
    let invalid = temp.path().join("invalid.csv");
    fs::write(&invalid, "path,action\na.jpg,reject\n../bad,keep\n").unwrap();
    assert!(svc.import_csv(id, &invalid).is_err());
    assert_eq!(svc.photo(p.id).unwrap().decision, Action::Flag);
}

#[test]
fn occlusion_provider_hash_and_threshold_changes_invalidate_features_and_estimates() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photo = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    let mut settings = svc.settings(id).unwrap();
    let mut changes = Vec::new();
    settings["occlusion_provider"] = json!("faceocc");
    settings["occlusion_model_sha256"] = json!("a".repeat(64));
    settings["occlusion_min_visible_fraction"] = json!(0.75);
    changes.push(settings.clone());
    settings["occlusion_model_sha256"] = json!("b".repeat(64));
    changes.push(settings.clone());
    settings["occlusion_min_visible_fraction"] = json!(0.8);
    changes.push(settings.clone());
    settings["occlusion_provider"] = json!("none");
    changes.push(settings);
    for change in changes {
        let mut analysis = current_suggestion(&svc, id, "recommend");
        analysis["faces"] = json!([{}]);
        svc.save_analysis(photo.id, analysis).unwrap();
        svc.save_profile("occlusion-change", change.clone())
            .unwrap();
        assert_eq!(
            svc.estimate_profile(id, "occlusion-change").unwrap()["requires_analysis"],
            true
        );
        svc.set_settings(id, change).unwrap();
        assert!(svc.photo(photo.id).unwrap().analysis.is_none());
        assert_eq!(
            svc.store
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM faces WHERE photo_id=?1",
                    [photo.id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
    let mut invalid = svc.settings(id).unwrap();
    invalid["occlusion_provider"] = json!("faceocc");
    invalid
        .as_object_mut()
        .unwrap()
        .remove("occlusion_min_visible_fraction");
    assert!(
        svc.set_settings(id, invalid).is_err(),
        "FaceOcc needs an explicit threshold"
    );
}

#[test]
fn profile_estimate_detects_removed_optional_settings_before_invalidating_analyses() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let defaults = svc.settings(id).unwrap();
    let mut configured = defaults.clone();
    configured["occlusion_model_sha256"] = json!("a".repeat(64));
    configured["occlusion_min_visible_fraction"] = json!(0.75);
    // Keeping optional configuration while the provider is disabled is valid.
    svc.set_settings(id, configured).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    for photo in &photos {
        svc.save_analysis(photo.id, full_current_analysis(&svc, id))
            .unwrap();
    }
    svc.save_profile("remove-optional", defaults.clone())
        .unwrap();
    let estimate = svc.estimate_profile(id, "remove-optional").unwrap();
    assert_eq!(
        estimate["changed_keys"],
        json!(["occlusion_min_visible_fraction", "occlusion_model_sha256"])
    );
    assert_eq!(estimate["requires_analysis"], true);
    assert_eq!(estimate["photos_requiring_refresh"], photos.len());
    assert_eq!(estimate["estimated_photos"], 0);
    assert_eq!(estimate["unavailable_photos"], photos.len());
    assert!(
        svc.photos(id, PhotoFilter::default())
            .unwrap()
            .iter()
            .all(|photo| photo.analysis.is_some()),
        "estimation is read-only"
    );
    assert_eq!(svc.apply_profile(id, "remove-optional").unwrap(), defaults);
    assert!(svc
        .photos(id, PhotoFilter::default())
        .unwrap()
        .iter()
        .all(|photo| photo.analysis.is_none()));
}

#[test]
fn xmp_destination_collisions_fail_before_any_writes_in_both_overwrite_modes() {
    let aliases = if cfg!(windows) {
        vec!["a.jpeg", "A.jpeg"]
    } else {
        vec!["a.jpeg"]
    };
    for alias in aliases {
        let (temp, mut svc, id) = fixture();
        let root = temp.path().join("photos");
        jpg(&root.join("00-first.jpg"), 45);
        jpg(&root.join(alias), 80);
        svc.scan(id).unwrap();
        let photos = svc.photos(id, PhotoFilter::default()).unwrap();
        let originals: Vec<_> = photos
            .iter()
            .map(|p| {
                let path = svc.photo_path(p.id).unwrap();
                (path.clone(), fs::read(path).unwrap())
            })
            .collect();
        for photo in &photos {
            let action = if photo.path == "a.jpg" {
                Action::Reject
            } else {
                Action::Keep
            };
            svc.decisions(id, &[photo.id], action, "human", false)
                .unwrap();
        }
        let existing = root.join("a.xmp");
        fs::write(&existing, b"existing sidecar metadata").unwrap();
        for overwrite in [false, true] {
            let error = svc
                .export_xmp(id, "all", overwrite)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("multiple selected photos share XMP destination"),
                "{error}"
            );
            assert!(
                !root.join("00-first.xmp").exists(),
                "no earlier photo may be written"
            );
            assert_eq!(fs::read(&existing).unwrap(), b"existing sidecar metadata");
            for (path, bytes) in &originals {
                assert_eq!(fs::read(path).unwrap(), *bytes);
            }
        }
        // A scoped export with only one of the colliding photos remains available.
        assert_eq!(svc.export_xmp(id, "reject", true).unwrap().written, 1);
        assert!(fs::read_to_string(existing)
            .unwrap()
            .contains("Rating='-1'"));
    }
}

#[test]
fn detector_provider_and_hash_changes_invalidate_features_and_profile_estimates() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    let mut settings = svc.settings(id).unwrap();
    let mut legacy = settings.clone();
    legacy.as_object_mut().unwrap().remove("face_detector");
    legacy.as_object_mut().unwrap().remove("scrfd_model_sha256");
    for key in [
        "occlusion_provider",
        "occlusion_model_sha256",
        "occlusion_min_visible_fraction",
    ] {
        legacy.as_object_mut().unwrap().remove(key);
    }
    svc.store
        .conn
        .execute(
            "INSERT INTO settings(project_id,data) VALUES(?1,?2)",
            rusqlite::params![id, legacy.to_string()],
        )
        .unwrap();
    assert_eq!(svc.settings(id).unwrap(), settings);
    svc.save_analysis(
        p.id,
        json!({"version":iris_core::vision::ANALYSIS_VERSION,"faces":[{}]}),
    )
    .unwrap();
    svc.set_settings(id, settings.clone()).unwrap();
    assert!(
        svc.photo(p.id).unwrap().analysis.is_some(),
        "legacy defaults are unchanged settings"
    );
    settings["face_detector"] = json!("scrfd_500m");
    settings["scrfd_model_sha256"] = json!("a".repeat(64));
    svc.save_profile("scrfd", settings.clone()).unwrap();
    assert_eq!(
        svc.estimate_profile(id, "scrfd").unwrap()["requires_analysis"],
        true
    );
    for hash in ['a', 'b'] {
        settings["scrfd_model_sha256"] = json!(hash.to_string().repeat(64));
        svc.set_settings(id, settings.clone()).unwrap();
        assert!(svc.photo(p.id).unwrap().analysis.is_none());
        assert_eq!(
            svc.store
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM faces WHERE photo_id=?1",
                    [p.id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        svc.save_analysis(
            p.id,
            json!({"version":iris_core::vision::ANALYSIS_VERSION,"faces":[{}]}),
        )
        .unwrap();
    }
    assert_eq!(
        svc.estimate_profile(id, "scrfd").unwrap()["requires_analysis"],
        true,
        "hash-only profile differences also require fresh inference"
    );
    settings["face_detector"] = json!("yunet");
    svc.set_settings(id, settings).unwrap();
    assert!(svc.photo(p.id).unwrap().analysis.is_none());
}

#[test]
fn variants_conflicts_profiles_cache_and_backup() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    svc.store
        .conn
        .execute("UPDATE photos SET capture_variant_id='same'", [])
        .unwrap();
    svc.decisions(id, &[photos[0].id], Action::Keep, "human", false)
        .unwrap();
    let outcome = svc
        .decisions(id, &[photos[1].id], Action::Reject, "human", true)
        .unwrap();
    assert_eq!(outcome.conflicts, vec![photos[0].id]);
    assert_eq!(svc.photo(photos[0].id).unwrap().decision, Action::Keep);
    svc.save_profile("Wedding", json!({"recommend_threshold":80}))
        .unwrap();
    svc.apply_profile(id, "Wedding").unwrap();
    assert_eq!(
        svc.settings(id).unwrap()["recommend_threshold"].as_f64(),
        Some(80.)
    );
    let cache = svc.project(id).unwrap().cache_root;
    fs::create_dir_all(&cache).unwrap();
    fs::write(Path::new(&cache).join("thumbnail.jpg"), b"cache bytes").unwrap();
    assert_eq!(svc.cache_status(id).unwrap().files, 1);
    let migrated = temp.path().join("migrated-cache");
    assert_eq!(svc.cache_migrate(id, &migrated).unwrap().files, 1);
    assert!(Path::new(&cache).join("thumbnail.jpg").exists());
    assert_eq!(svc.cache_cleanup(id).unwrap().files, 0);
    let backup = temp.path().join("backup.sqlite");
    svc.store.backup(&backup).unwrap();
    let reopened = Services::new(Store::open(backup).unwrap());
    assert_eq!(reopened.photo(photos[0].id).unwrap().decision, Action::Keep);
}

#[test]
fn migration_backup_and_future_schema_guard() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("legacy.sqlite");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TABLE legacy(value TEXT); INSERT INTO legacy VALUES('preserved');")
        .unwrap();
    drop(conn);
    let store = Store::open(&db).unwrap();
    assert!(store.recovery_notice.as_ref().unwrap().contains("backup"));
    assert_eq!(
        store
            .conn
            .query_row("SELECT value FROM legacy", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "preserved"
    );
    store.conn.execute_batch("PRAGMA user_version=999").unwrap();
    drop(store);
    assert!(Store::open(&db).is_err());
}

#[test]
fn migrated_cache_can_publish_a_new_preview_size_and_preserves_sources() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photo = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    let source = svc.photo_path(photo.id).unwrap();
    let original = fs::read(&source).unwrap();
    let thumbnail = svc.cached_image(photo.id, 320).unwrap();
    let old_root = svc.project(id).unwrap().cache_root;
    let destination = temp.path().join("migrated-cache");
    assert_eq!(svc.cache_migrate(id, &destination).unwrap().files, 1);
    assert_eq!(svc.cached_image(photo.id, 320).unwrap(), thumbnail);
    let preview = svc.cached_image(photo.id, 2560).unwrap();
    assert_eq!(svc.cached_image(photo.id, 2560).unwrap(), preview);
    assert_eq!(svc.cache_status(id).unwrap().files, 2);
    assert_eq!(fs::read_dir(old_root).unwrap().count(), 1);
    let decoded = image::load_from_memory(&preview).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (24, 16));
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn preview_cache_preserves_aspect_and_settings_estimates_actual_verdicts() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    let source = svc.photo_path(p.id).unwrap();
    let original = fs::read(&source).unwrap();
    let bytes = svc.cached_image(p.id, 320).unwrap();
    let image = image::load_from_memory(&bytes).unwrap();
    assert_eq!((image.width(), image.height()), (24, 16));
    assert_eq!(svc.cache_status(id).unwrap().files, 1);
    assert_eq!(svc.cached_image(p.id, 320).unwrap(), bytes);
    assert_eq!(fs::read(source).unwrap(), original);
    assert!(svc.cached_image(p.id, 9000).is_err());
    assert!(svc.set_settings(id, json!({"eyes_weight":-1})).is_err());
    assert!(svc.save_profile("invalid", json!({"nonsense":1})).is_err());
    let analysis = iris_core::vision::VisionAnalysis {
        width: 24,
        height: 16,
        original_width: 24,
        original_height: 16,
        orientation: 1,
        preview_source: None,
        faces: vec![],
        sharpness_lap: 1.,
        sharpness_fft: 1.,
        niqe: None,
        composition: None,
        score_breakdown: None,
        embedding: None,
        exposure: iris_core::vision::Exposure {
            mean: 10.,
            shadow_clip: 0.8,
            highlight_clip: 0.,
            verdict: "underexposed".into(),
        },
        composite_score: 0.,
        verdict: iris_core::vision::Verdict::RejectSuggest,
        phash: "0".into(),
        structure: vec![],
        warnings: vec![],
        version: iris_core::vision::ANALYSIS_VERSION.into(),
    };
    let mut analysis = serde_json::to_value(analysis).unwrap();
    analysis["settings"] = svc.settings(id).unwrap();
    svc.save_analysis(p.id, analysis).unwrap();
    svc.save_profile("cautious", json!({"reject_threshold":1}))
        .unwrap();
    let estimate = svc.estimate_profile(id, "cautious").unwrap();
    assert_eq!(estimate["estimated_photos"], 1);
    assert_eq!(estimate["verdict_changes"], 1);
    assert_eq!(estimate["before"]["reject_suggest"], 1);
    assert_eq!(estimate["after"]["review"], 1);
    assert_eq!(estimate["unavailable_photos"], 1);
    svc.save_profile("detection", json!({"face_confidence":0.8}))
        .unwrap();
    let estimate = svc.estimate_profile(id, "detection").unwrap();
    assert_eq!(estimate["requires_analysis"], true);
    assert_eq!(estimate["estimated_photos"], 0);
    assert_eq!(estimate["unavailable_photos"], 2);
    svc.apply_profile(id, "cautious").unwrap();
    let saved = svc.photo(p.id).unwrap().analysis.unwrap();
    assert_eq!(saved["verdict"], "review");
    assert_eq!(saved["settings"]["reject_threshold"].as_f64(), Some(1.));
    svc.apply_profile(id, "detection").unwrap();
    assert!(svc.photo(p.id).unwrap().analysis.is_none());
    let _ = temp;
}

fn full_current_analysis(svc: &Services, project_id: i64) -> serde_json::Value {
    json!({
        "width":24,"height":16,"original_width":24,"original_height":16,
        "orientation":1,"faces":[],"sharpness_lap":1.,"sharpness_fft":1.,
        "niqe":null,"exposure":{"mean":10.,"shadow_clip":0.8,
        "highlight_clip":0.,"verdict":"underexposed"},"composite_score":0.,
        "verdict":"reject_suggest","phash":"0","structure":[],"warnings":[],
        "version":iris_core::vision::ANALYSIS_VERSION,"settings":svc.settings(project_id).unwrap()
    })
}

#[test]
fn historical_analysis_is_visible_but_cannot_filter_or_rank_current_suggestions() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    let mut current = current_suggestion(&svc, id, "recommend");
    current["composite_score"] = json!(10.);
    svc.save_analysis(photos[1].id, current.clone()).unwrap();
    for old_version in [true, false] {
        let mut historical = current.clone();
        historical["composite_score"] = json!(99.);
        if old_version {
            historical["version"] = json!("iris-vision-previous-engine");
        } else {
            historical["settings"]["face_confidence"] = json!(0.99);
        }
        svc.save_analysis(photos[0].id, historical.clone()).unwrap();
        let stored = svc.photo(photos[0].id).unwrap();
        assert_eq!(stored.analysis_status, iris_core::AnalysisStatus::Stale);
        assert_eq!(stored.analysis, Some(historical));
        let recommended = svc
            .photos(
                id,
                PhotoFilter {
                    verdict: Some("recommend".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            recommended.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![photos[1].id]
        );
        for sort in ["score", "suggestion"] {
            let ranked = svc
                .photos(
                    id,
                    PhotoFilter {
                        sort: Some(sort.into()),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(
                ranked.iter().map(|p| p.id).collect::<Vec<_>>(),
                vec![photos[1].id, photos[0].id]
            );
        }
    }
}

#[test]
fn stale_analysis_cannot_supply_profile_estimates_or_be_promoted_by_rescoring() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    let current = full_current_analysis(&svc, id);
    let mut stale = current.clone();
    stale["version"] = json!("iris-vision-previous-engine");
    svc.save_analysis(photos[0].id, stale).unwrap();
    svc.save_analysis(photos[1].id, current).unwrap();
    svc.decisions(id, &[photos[0].id], Action::Keep, "human", false)
        .unwrap();
    svc.save_profile("same", svc.settings(id).unwrap()).unwrap();
    let estimate = svc.estimate_profile(id, "same").unwrap();
    assert_eq!(estimate["estimated_photos"], 1);
    assert_eq!(estimate["unavailable_photos"], 1);
    assert_eq!(estimate["photos_requiring_refresh"], 1);
    assert_eq!(estimate["requires_analysis"], true);
    assert_eq!(estimate["verdict_changes"], 0);
    assert_eq!(
        svc.photo(photos[0].id).unwrap().analysis_status,
        iris_core::AnalysisStatus::Stale
    );
    assert_eq!(
        svc.photo(photos[1].id).unwrap().analysis_status,
        iris_core::AnalysisStatus::Current
    );
    let mut changed = svc.settings(id).unwrap();
    changed["reject_threshold"] = json!(1.);
    svc.save_profile("scoring", changed.clone()).unwrap();
    let estimate = svc.estimate_profile(id, "scoring").unwrap();
    assert_eq!(estimate["estimated_photos"], 1);
    assert_eq!(estimate["verdict_changes"], 1);
    svc.set_settings(id, changed).unwrap();
    let stale = svc.photo(photos[0].id).unwrap();
    assert!(stale.analysis.is_none());
    assert_eq!(stale.analysis_status, iris_core::AnalysisStatus::Missing);
    assert_eq!(stale.decision, Action::Keep);
    assert_eq!(
        svc.photo(photos[1].id).unwrap().analysis_status,
        iris_core::AnalysisStatus::Current
    );
    assert_eq!(svc.project(id).unwrap().pending_analysis, 1);
}

#[test]
fn semantic_settings_preserve_quality_and_vectors_for_independent_regrouping() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photo = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    svc.decisions(id, &[photo.id], Action::Keep, "human", false)
        .unwrap();
    let mut enabled = svc.settings(id).unwrap();
    enabled["embedding_provider"] = json!("dinov3_vits16");
    enabled["embedding_model_sha256"] = json!("a".repeat(64));
    enabled["semantic_similarity_threshold"] = json!(0.9);
    svc.set_settings(id, enabled.clone()).unwrap();
    let mut analysis = full_current_analysis(&svc, id);
    svc.save_analysis(photo.id, &analysis).unwrap();
    assert_eq!(
        svc.photo(photo.id).unwrap().analysis_status,
        iris_core::AnalysisStatus::Current,
        "quality evidence is independent of candidate embedding availability"
    );
    let mut vector = vec![0.; 384];
    vector[0] = 1.;
    let preprocessing =
        iris_core::vision::embedding_model_status(Path::new("unused"), &Default::default())
            .preprocessing;
    analysis["embedding"] =
        json!({"model_sha256":"a".repeat(64),"preprocessing":preprocessing,"vector":vector});
    for (key, value) in [
        ("embedding_provider", json!("none")),
        ("embedding_model_sha256", json!("b".repeat(64))),
        ("semantic_similarity_threshold", json!(0.8)),
    ] {
        svc.set_settings(id, enabled.clone()).unwrap();
        svc.save_analysis(photo.id, &analysis).unwrap();
        assert_eq!(
            svc.photo(photo.id).unwrap().analysis_status,
            iris_core::AnalysisStatus::Current
        );
        let mut changed = enabled.clone();
        changed[key] = value;
        svc.save_profile("semantic-change", changed.clone())
            .unwrap();
        let estimate = svc.estimate_profile(id, "semantic-change").unwrap();
        assert_eq!(estimate["requires_analysis"], true); // other fixture photo remains unanalyzed
        assert_eq!(estimate["estimated_photos"], 1);
        assert!(estimate["changed_keys"]
            .as_array()
            .unwrap()
            .contains(&json!(key)));
        svc.set_settings(id, changed).unwrap();
        let retained = svc.photo(photo.id).unwrap();
        assert_eq!(retained.analysis_status, iris_core::AnalysisStatus::Current);
        assert_eq!(
            retained.analysis.as_ref().unwrap()["embedding"],
            analysis["embedding"]
        );
        assert_eq!(
            retained.analysis.as_ref().unwrap()["phash"],
            analysis["phash"]
        );
        assert_eq!(svc.photo(photo.id).unwrap().decision, Action::Keep);
    }
}

#[test]
fn semantic_threshold_preserves_stored_numeric_bytes() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photo = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    let mut settings = svc.settings(id).unwrap();
    settings["embedding_provider"] = json!("dinov3_vits16");
    settings["embedding_model_sha256"] = json!("a".repeat(64));
    settings["semantic_similarity_threshold"] = json!(0.9);
    svc.set_settings(id, settings.clone()).unwrap();
    let mut analysis = full_current_analysis(&svc, id);
    let mut vector = vec![0.; 384];
    vector[0] = 1.;
    analysis["embedding"] = json!({"model_sha256":"a".repeat(64),"preprocessing":"dinov3_vits16_rgb224_triangle_imagenet_pooler_l2_v1","vector":vector});
    svc.save_analysis(photo.id, &analysis).unwrap();
    svc.store.conn.execute("UPDATE analyses SET data=json_set(data,'$.embedding.vector[1]',json('0.00012345678901234567890123')) WHERE photo_id=?1", [photo.id]).unwrap();
    let read = |svc: &Services| -> String {
        svc.store
            .conn
            .query_row(
                "SELECT json_remove(data,'$.settings') FROM analyses WHERE photo_id=?1",
                [photo.id],
                |row| row.get(0),
            )
            .unwrap()
    };
    let before = read(&svc);
    settings["semantic_similarity_threshold"] = json!(0.91);
    svc.set_settings(id, settings).unwrap();
    assert_eq!(read(&svc), before);
    // The semantic persistence path must also preserve all existing quality
    // numbers, even those with a representation more precise than Rust f64.
    svc.store.conn.execute("UPDATE analyses SET data=json_set(data,'$.sharpness_fft',json('0.12345678901234567890123')) WHERE photo_id=?1", [photo.id]).unwrap();
    let quality = |svc: &Services| -> String {
        svc.store.conn.query_row("SELECT json_remove(data,'$.embedding','$.warnings') FROM analyses WHERE photo_id=?1", [photo.id], |row| row.get(0)).unwrap()
    };
    let before = quality(&svc);
    let typed = serde_json::from_value(analysis).unwrap();
    svc.save_semantic_embedding(photo.id, &typed).unwrap();
    assert_eq!(quality(&svc), before);
}

#[test]
fn settings_mismatch_and_scanned_source_change_have_safe_analysis_status() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    let mut mismatch = full_current_analysis(&svc, id);
    mismatch["settings"]["reject_threshold"] = json!(1.);
    svc.save_analysis(photos[0].id, mismatch).unwrap();
    svc.save_profile("same", svc.settings(id).unwrap()).unwrap();
    let estimate = svc.estimate_profile(id, "same").unwrap();
    assert_eq!(estimate["estimated_photos"], 0);
    assert_eq!(estimate["unavailable_photos"], 2);
    assert_eq!(estimate["photos_requiring_refresh"], 2);
    assert_eq!(estimate["before"], json!({}));
    assert_eq!(
        svc.photos(id, PhotoFilter::default()).unwrap()[0].analysis_status,
        iris_core::AnalysisStatus::Stale
    );
    let current = full_current_analysis(&svc, id);
    svc.save_analysis(photos[0].id, current).unwrap();
    svc.decisions(id, &[photos[0].id], Action::Flag, "human", false)
        .unwrap();
    image::RgbImage::new(25, 17)
        .save(temp.path().join("photos").join(&photos[0].path))
        .unwrap();
    assert_eq!(svc.scan(id).unwrap().changed, 1);
    let changed = svc.photo(photos[0].id).unwrap();
    assert_eq!(changed.analysis_status, iris_core::AnalysisStatus::Missing);
    assert_eq!(changed.decision, Action::Flag);
}

#[test]
fn bulk_accept_never_rejects_closed_eyes_even_if_stored_verdict_is_inconsistent() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    svc.save_analysis(
        p.id,
        json!({"verdict":"reject_suggest","faces":[{"left_eye":{"state":"closed"}}]}),
    )
    .unwrap();
    assert_eq!(svc.accept(id).unwrap().changed, 0);
    assert_eq!(svc.photo(p.id).unwrap().decision, Action::Pending);
}

#[test]
fn corrupt_changed_jpeg_cannot_retain_or_adopt_stale_recommendation() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    svc.save_analysis(p.id, json!({"verdict":"recommend"}))
        .unwrap();
    fs::write(svc.photo_path(p.id).unwrap(), b"corrupt changed jpeg").unwrap();
    assert_eq!(svc.scan(id).unwrap().errors.len(), 1);
    assert!(svc.photo(p.id).unwrap().analysis.is_none());
    assert_eq!(svc.accept(id).unwrap().changed, 0);
}

#[test]
fn large_photo_thumbnail_uses_bounded_decode_then_resizes_without_cropping() {
    let (temp, mut svc, id) = fixture();
    image::RgbImage::from_pixel(6000, 4000, image::Rgb([70, 80, 90]))
        .save(temp.path().join("photos/large.jpg"))
        .unwrap();
    svc.scan(id).unwrap();
    let p = svc
        .photos(id, PhotoFilter::default())
        .unwrap()
        .into_iter()
        .find(|p| p.filename == "large.jpg")
        .unwrap();
    let preview = image::load_from_memory(&svc.cached_image(p.id, 320).unwrap()).unwrap();
    assert_eq!((preview.width(), preview.height()), (320, 213));
}

#[test]
fn invalid_persisted_analysis_is_reported_instead_of_silently_hidden() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    let p = &photos[0];
    let paths: Vec<_> = photos
        .iter()
        .map(|p| svc.photo_path(p.id).unwrap())
        .collect();
    svc.save_analysis(p.id, json!({"verdict":"recommend"}))
        .unwrap();
    svc.store
        .conn
        .execute(
            "UPDATE analyses SET data='invalid-json' WHERE photo_id=?1",
            [p.id],
        )
        .unwrap();
    assert!(svc.photo(p.id).is_err());
    assert!(svc.accept(id).is_err());
    for (photo, expected) in photos.iter().zip(paths) {
        assert_eq!(
            svc.photo_path(photo.id).unwrap(),
            expected,
            "own or unrelated analysis JSON must not prevent path resolution"
        );
    }
}

#[test]
fn save_analysis_replaces_existing_result_and_rejects_unknown_photo() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    svc.save_analysis(p.id, json!({"version":"old","faces":[{"score":0.5}]}))
        .unwrap();
    let replacement =
        json!({"version":iris_core::vision::ANALYSIS_VERSION,"faces":[],"verdict":"recommend"});
    svc.save_analysis(p.id, replacement.clone()).unwrap();
    assert_eq!(svc.photo(p.id).unwrap().analysis, Some(replacement.clone()));
    assert_eq!(
        svc.store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM faces WHERE photo_id=?1",
                [p.id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert!(svc.save_analysis(i64::MAX, replacement).is_err());
    assert_eq!(
        svc.store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM analyses WHERE photo_id=?1",
                [i64::MAX],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn path_resolution_rejects_unavailable_and_escaping_sources() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    let original = svc.photo_path(p.id).unwrap();
    for (missing, quarantined) in [(true, false), (false, true)] {
        svc.store
            .conn
            .execute(
                "UPDATE photos SET missing=?1,quarantined=?2 WHERE id=?3",
                rusqlite::params![missing, quarantined, p.id],
            )
            .unwrap();
        assert!(svc
            .photo_path(p.id)
            .unwrap_err()
            .to_string()
            .contains("unavailable"));
    }
    svc.store
        .conn
        .execute(
            "UPDATE photos SET missing=0,quarantined=0 WHERE id=?1",
            [p.id],
        )
        .unwrap();
    let outside = temp.path().join("outside.jpg");
    jpg(&outside, 80);
    for path in [
        "../outside.jpg".to_owned(),
        outside.to_string_lossy().into_owned(),
    ] {
        svc.store
            .conn
            .execute(
                "UPDATE photos SET path=?1 WHERE id=?2",
                rusqlite::params![path, p.id],
            )
            .unwrap();
        assert!(svc
            .photo_path(p.id)
            .unwrap_err()
            .to_string()
            .contains("invalid relative path"));
    }
    svc.store
        .conn
        .execute(
            "UPDATE photos SET path=?1 WHERE id=?2",
            rusqlite::params![p.path, p.id],
        )
        .unwrap();
    fs::remove_file(original).unwrap();
    assert!(
        svc.photo_path(p.id).is_err(),
        "unscanned missing source must fail canonicalization"
    );
    assert!(svc.photo_path(i64::MAX).is_err());
}

#[test]
fn accept_rejects_stale_or_unknown_analysis_atomically_and_normalizes_legacy_settings() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    let current = current_suggestion(&svc, id, "recommend");
    svc.save_analysis(photos[0].id, current.clone()).unwrap();
    let mut stale_cases = vec![];
    let mut old = current.clone();
    old["version"] = json!("old-engine");
    stale_cases.push(old);
    let mut unknown = current.clone();
    unknown.as_object_mut().unwrap().remove("version");
    stale_cases.push(unknown);
    let mut no_settings = current.clone();
    no_settings.as_object_mut().unwrap().remove("settings");
    stale_cases.push(no_settings);
    let mut different = current.clone();
    different["settings"]["face_confidence"] = json!(0.8);
    stale_cases.push(different);
    for stale in stale_cases {
        svc.save_analysis(photos[1].id, stale).unwrap();
        assert!(svc
            .accept(id)
            .unwrap_err()
            .to_string()
            .contains("stale or unknown analysis"));
        for photo in &photos {
            assert_eq!(svc.photo(photo.id).unwrap().decision, Action::Pending);
        }
    }
    let mut legacy = current;
    legacy["settings"]
        .as_object_mut()
        .unwrap()
        .remove("face_detector");
    legacy["settings"]
        .as_object_mut()
        .unwrap()
        .remove("scrfd_model_sha256");
    svc.save_analysis(photos[1].id, legacy).unwrap();
    assert_eq!(svc.accept(id).unwrap().changed, 2);
    assert_eq!(svc.undo(id).unwrap().changed, 2);
}

#[test]
fn scoped_accept_limits_ids_and_category_preserves_human_decisions_and_undo_batch() {
    let (temp, mut svc, id) = fixture();
    jpg(&temp.path().join("photos/c.jpg"), 70);
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    for (photo, verdict) in photos
        .iter()
        .zip(["recommend", "reject_suggest", "recommend"])
    {
        svc.save_analysis(photo.id, current_suggestion(&svc, id, verdict))
            .unwrap();
    }
    svc.decisions(id, &[photos[2].id], Action::Flag, "human", false)
        .unwrap();
    let request = |ids, category| AcceptRequest {
        photo_ids: Some(ids),
        category,
    };
    assert_eq!(
        svc.accept_scoped(id, request(vec![], AcceptCategory::All))
            .unwrap()
            .changed,
        0
    );
    assert_eq!(
        svc.accept_scoped(id, request(vec![photos[1].id], AcceptCategory::Recommend))
            .unwrap()
            .changed,
        0
    );
    assert_eq!(
        svc.accept_scoped(
            id,
            request(
                vec![photos[0].id, photos[0].id, photos[2].id],
                AcceptCategory::Recommend
            )
        )
        .unwrap()
        .changed,
        1
    );
    assert_eq!(svc.photo(photos[1].id).unwrap().decision, Action::Pending);
    assert_eq!(svc.photo(photos[2].id).unwrap().decision, Action::Flag);
    assert_eq!(svc.undo(id).unwrap().changed, 1);
    let accepted = svc
        .accept_scoped(
            id,
            request(photos.iter().map(|p| p.id).collect(), AcceptCategory::All),
        )
        .unwrap();
    assert_eq!(accepted.changed, 2);
    assert_eq!(svc.photo(photos[0].id).unwrap().decision, Action::Keep);
    assert_eq!(svc.photo(photos[1].id).unwrap().decision, Action::Reject);
    assert_eq!(svc.undo(id).unwrap().id, accepted.id);
    assert_eq!(svc.photo(photos[0].id).unwrap().decision, Action::Pending);
    assert_eq!(svc.photo(photos[1].id).unwrap().decision, Action::Pending);
    assert_eq!(svc.photo(photos[2].id).unwrap().decision, Action::Flag);
    assert_eq!(
        svc.accept_scoped(
            id,
            AcceptRequest {
                photo_ids: None,
                category: AcceptCategory::RejectSuggest
            }
        )
        .unwrap()
        .changed,
        1
    );
    assert_eq!(svc.photo(photos[0].id).unwrap().decision, Action::Pending);
}

#[test]
fn scoped_accept_rejects_foreign_unknown_and_unavailable_ids_before_any_decisions() {
    let (temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    for p in &photos {
        svc.save_analysis(p.id, current_suggestion(&svc, id, "recommend"))
            .unwrap();
    }
    let other_root = temp.path().join("other");
    fs::create_dir(&other_root).unwrap();
    jpg(&other_root.join("other.jpg"), 70);
    let other = svc.create_project(other_root).unwrap().id;
    svc.scan(other).unwrap();
    let foreign = svc.photos(other, PhotoFilter::default()).unwrap()[0].id;
    for invalid in [foreign, i64::MAX] {
        assert!(svc
            .accept_scoped(
                id,
                AcceptRequest {
                    photo_ids: Some(vec![photos[0].id, invalid]),
                    category: AcceptCategory::All
                }
            )
            .is_err());
        assert_eq!(svc.photo(photos[0].id).unwrap().decision, Action::Pending);
    }
    for (missing, quarantined) in [(true, false), (false, true)] {
        svc.store
            .conn
            .execute(
                "UPDATE photos SET missing=?1,quarantined=?2 WHERE id=?3",
                rusqlite::params![missing, quarantined, photos[1].id],
            )
            .unwrap();
        assert!(svc
            .accept_scoped(
                id,
                AcceptRequest {
                    photo_ids: Some(photos.iter().map(|p| p.id).collect()),
                    category: AcceptCategory::All
                }
            )
            .is_err());
        assert_eq!(svc.photo(photos[0].id).unwrap().decision, Action::Pending);
    }
}

#[test]
fn bulk_accept_aborts_atomically_when_an_analyzed_source_changed_without_rescan() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let photos = svc.photos(id, PhotoFilter::default()).unwrap();
    for p in &photos {
        svc.save_analysis(p.id, current_suggestion(&svc, id, "recommend"))
            .unwrap();
    }
    fs::write(svc.photo_path(photos[1].id).unwrap(), b"replacement").unwrap();
    assert!(svc
        .accept(id)
        .unwrap_err()
        .to_string()
        .contains("changed since analysis"));
    for p in photos {
        assert_eq!(svc.photo(p.id).unwrap().decision, Action::Pending);
    }
}

#[test]
fn crash_between_copy_and_unlink_recovers_from_durable_journal() {
    let (_temp, mut svc, id) = fixture();
    svc.scan(id).unwrap();
    let p = svc.photos(id, PhotoFilter::default()).unwrap().remove(0);
    svc.decisions(id, &[p.id], Action::Reject, "human", false)
        .unwrap();
    let mut plan = svc.quarantine_preview(id).unwrap();
    let item = &plan.items[0];
    fs::create_dir_all(Path::new(&item.destination).parent().unwrap()).unwrap();
    fs::copy(&item.source, &item.destination).unwrap();
    plan.state = "moving".into();
    plan.items[0].state = "moving".into();
    svc.store
        .conn
        .execute(
            "UPDATE quarantine SET state='moving',data=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&plan).unwrap(), plan.id],
        )
        .unwrap();
    svc.store.conn.execute_batch("CREATE TRIGGER fail_deduplicating_restore BEFORE UPDATE OF quarantined ON photos WHEN EXISTS(SELECT 1 FROM quarantine WHERE state='restoring') BEGIN SELECT RAISE(ABORT,'injected dedup restore failure'); END;").unwrap();
    assert!(svc
        .quarantine_restore(&plan.id)
        .unwrap_err()
        .to_string()
        .contains("rolled back"));
    // Compensation must restore the duplicate quarantine copy without deleting
    // the source that already existed before this recovery attempt.
    assert!(Path::new(&plan.items[0].source).exists());
    assert!(Path::new(&plan.items[0].destination).exists());
    assert_eq!(svc.quarantine_plan(&plan.id).unwrap().state, "moving");
    svc.store
        .conn
        .execute_batch("DROP TRIGGER fail_deduplicating_restore")
        .unwrap();
    svc.quarantine_restore(&plan.id).unwrap();
    assert!(Path::new(&plan.items[0].source).exists());
    assert!(!Path::new(&plan.items[0].destination).exists());
    assert!(!svc.photo(p.id).unwrap().quarantined);
}
