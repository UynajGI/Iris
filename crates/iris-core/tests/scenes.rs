use iris_core::services::{ComparisonRequest, SceneCorrection};
use iris_core::{Action, Services, Store};
use serde_json::json;

fn fixture() -> (tempfile::TempDir, Services, i64, Vec<i64>) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("synthetic");
    std::fs::create_dir(&root).unwrap();
    let database = temp.path().join("library.sqlite");
    let mut services = Services::new(Store::open(&database).unwrap());
    let project = services.create_project(&root).unwrap().id;
    let settings = services.settings(project).unwrap();
    let mut ids = vec![];
    for name in ["a.jpg", "b.jpg", "c.jpg"] {
        services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height,taken_at) VALUES(?1,?2,?2,'jpeg',1,1,100,100,'2026-01-01 12:00:00')",(project,name)).unwrap();
        let id = services.store.conn.last_insert_rowid();
        ids.push(id);
        services.save_analysis(id,json!({"version":iris_core::vision::ANALYSIS_VERSION,"settings":settings,"faces":[],"phash":"0","structure":vec![0.5;64]})).unwrap();
    }
    (temp, services, project, ids)
}

#[test]
fn scene_corrections_survive_rebuild_reopen_and_do_not_change_decisions() {
    let (temp, mut services, project, ids) = fixture();
    let database = temp.path().join("library.sqlite");
    assert_eq!(services.scenes(project).unwrap().len(), 1);
    services
        .decisions(project, &[ids[0]], Action::Keep, "human", false)
        .unwrap();
    services
        .correct_scene(
            project,
            SceneCorrection {
                photo_ids: vec![ids[0]],
                reset: false,
            },
        )
        .unwrap();
    services.save_groups(project, vec![]).unwrap();
    let scenes = services.scenes(project).unwrap();
    assert_eq!(scenes.len(), 2);
    assert!(scenes
        .iter()
        .any(|s| s.kind == "manual" && s.member_photo_ids == vec![ids[0]]));
    drop(services);
    let mut services = Services::new(Store::open(&database).unwrap());
    assert_eq!(
        services
            .scenes(project)
            .unwrap()
            .iter()
            .map(|s| &s.id)
            .collect::<Vec<_>>(),
        scenes.iter().map(|s| &s.id).collect::<Vec<_>>()
    );
    assert_eq!(services.photo(ids[0]).unwrap().decision, Action::Keep);
    assert!(services
        .correct_scene(
            project,
            SceneCorrection {
                photo_ids: vec![ids[0], 99999],
                reset: false
            }
        )
        .is_err());
    assert_eq!(services.scenes(project).unwrap().len(), 2);
    services
        .correct_scene(
            project,
            SceneCorrection {
                photo_ids: vec![ids[0]],
                reset: true,
            },
        )
        .unwrap();
    assert_eq!(services.scenes(project).unwrap().len(), 1);
    let report = services
        .compare_photos(
            project,
            ComparisonRequest {
                photo_ids: ids[..2].to_vec(),
                reference_photo_id: None,
            },
        )
        .unwrap();
    assert_eq!(report.members.len(), 2);
    assert!(report.members[0].score_delta.is_none()); // Incomplete evidence stays unknown.
    assert!(services
        .compare_photos(
            project,
            ComparisonRequest {
                photo_ids: ids[..2].to_vec(),
                reference_photo_id: Some(ids[2])
            }
        )
        .is_err());
    services
        .store
        .conn
        .execute("UPDATE photos SET missing=1 WHERE id=?1", [ids[1]])
        .unwrap();
    assert!(services
        .scenes(project)
        .unwrap()
        .iter()
        .all(|s| !s.member_photo_ids.contains(&ids[1])));
    assert!(services
        .compare_photos(
            project,
            ComparisonRequest {
                photo_ids: ids[..2].to_vec(),
                reference_photo_id: None
            }
        )
        .is_err());
}

#[test]
fn scene_and_exact_hit_scopes_page_and_filter_in_the_database() {
    use iris_core::PhotoFilter;
    let (_temp, mut services, project, ids) = fixture();
    services
        .correct_scene(
            project,
            SceneCorrection {
                photo_ids: vec![ids[0], ids[2]],
                reset: false,
            },
        )
        .unwrap();
    services
        .decisions(project, &[ids[2]], Action::Keep, "human", false)
        .unwrap();
    let scene = services
        .scenes(project)
        .unwrap()
        .into_iter()
        .find(|s| s.kind == "manual")
        .unwrap();
    let filter = PhotoFilter {
        scene_id: Some(scene.id.clone()),
        limit: Some(1),
        offset: Some(1),
        ..Default::default()
    };
    assert_eq!(services.photos(project, filter).unwrap()[0].id, ids[2]);
    let filter = PhotoFilter {
        scene_id: Some(scene.id),
        decision: Some(Action::Keep),
        ..Default::default()
    };
    assert_eq!(
        services
            .photos(project, filter)
            .unwrap()
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![ids[2]]
    );
    let filter = PhotoFilter {
        photo_id: Some(ids[1]),
        ..Default::default()
    };
    assert_eq!(services.photos(project, filter).unwrap()[0].id, ids[1]);
    let filter = PhotoFilter {
        scene_id: Some("unknown".into()),
        ..Default::default()
    };
    assert!(services.photos(project, filter).unwrap().is_empty());
    std::fs::create_dir(_temp.path().join("other")).unwrap();
    let other = services
        .create_project(&_temp.path().join("other"))
        .unwrap()
        .id;
    assert!(services
        .photos(
            other,
            PhotoFilter {
                photo_id: Some(ids[1]),
                ..Default::default()
            }
        )
        .unwrap()
        .is_empty());
}

#[test]
fn scene_hierarchy_contains_only_nonempty_bursts_wholly_within_the_scene() {
    use iris_core::BurstGroup;
    let (_temp, mut services, project, ids) = fixture();
    services
        .correct_scene(
            project,
            SceneCorrection {
                photo_ids: vec![ids[0], ids[1]],
                reset: false,
            },
        )
        .unwrap();
    let group = |id: &str, kind: &str, members: Vec<i64>| BurstGroup {
        id: id.into(),
        project_id: project,
        kind: kind.into(),
        member_photo_ids: members,
        assessment: None,
    };
    services
        .save_groups(
            project,
            vec![
                group("inside", "burst", ids[..2].to_vec()),
                group("cross", "burst", vec![ids[0], ids[2]]),
                group("empty", "burst", vec![]),
                group("duplicate", "exact_duplicate", ids[..2].to_vec()),
            ],
        )
        .unwrap();
    let scenes = services.scenes(project).unwrap();
    let manual = scenes.iter().find(|s| s.kind == "manual").unwrap();
    assert_eq!(manual.burst_group_ids, vec!["inside"]);
    assert!(scenes
        .iter()
        .filter(|s| s.kind != "manual")
        .all(|s| s.burst_group_ids.is_empty()));
}
