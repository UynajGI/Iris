use iris_core::services::{FaceChoice, GroupIntent};
use iris_core::{Action, BurstGroup, MarkRequest, PhotoFilter, Services, Store};
use serde_json::{json, Value};

fn analysis(settings: &Value, score: f64, face_score: f64) -> Value {
    let exposure = json!({"mean":128.,"shadow_clip":0.,"highlight_clip":0.,"verdict":"normal"});
    let face = |index, x, quality| {
        json!({"index":index,"bbox":[x,50.,64.,64.],"confidence":0.99,
        "quality":{"method":"synthetic","crop_width":64,"crop_height":64,"sharpness_lap":100.,
            "exposure":exposure,"sharpness_score":quality,"exposure_score":quality,"resolution_score":100.,"score":quality},
        "head_pose":null,"left_eye":{"state":"open","blink_score":0.,"ear":0.3},
        "right_eye":{"state":"open","blink_score":0.,"ear":0.3},"smile_score":null,
        "landmark_confidence":0.99,"landmarks":[],"unreliable_reason":null})
    };
    json!({"version":iris_core::vision::ANALYSIS_VERSION,"settings":settings,
        "width":256,"height":256,"original_width":256,"original_height":256,"orientation":1,
        "faces":[face(0,30.,face_score),face(1,160.,90.)],"sharpness_lap":100.,"sharpness_fft":50.,
        "niqe":null,"exposure":exposure,"composite_score":score,"verdict":"recommend",
        "phash":"0","structure":vec![0.5;64],"warnings":[]})
}

fn rebuild(services: &mut Services, project: i64, ids: &[i64]) {
    services
        .save_groups(
            project,
            vec![BurstGroup {
                id: "burst".into(),
                project_id: project,
                kind: "burst".into(),
                member_photo_ids: ids.to_vec(),
                assessment: None,
            }],
        )
        .unwrap();
}

fn fixture() -> (tempfile::TempDir, Services, i64, Vec<i64>) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("synthetic");
    std::fs::create_dir(&root).unwrap();
    for index in 0..5 {
        image::RgbImage::from_pixel(256, 256, image::Rgb([100, 120, 140]))
            .save(root.join(format!("{index}.jpg")))
            .unwrap();
    }
    let mut services = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    let project = services.create_project(&root).unwrap().id;
    services.scan(project).unwrap();
    let ids: Vec<_> = services
        .photos(project, PhotoFilter::default())
        .unwrap()
        .iter()
        .map(|photo| photo.id)
        .collect();
    let settings = services.settings(project).unwrap();
    for (index, id) in ids.iter().enumerate() {
        services
            .save_analysis(
                *id,
                analysis(
                    &settings,
                    95. - index as f64 * 5.,
                    if index == 1 { 99. } else { 40. + index as f64 },
                ),
            )
            .unwrap();
    }
    rebuild(&mut services, project, &ids);
    (temp, services, project, ids)
}

#[test]
fn quota_fills_only_pending_slots_and_locks_survive_reopen_and_regroup() {
    let (temp, mut services, project, ids) = fixture();
    for (id, action) in [
        (ids[0], Action::Keep),
        (ids[2], Action::Reject),
        (ids[3], Action::Flag),
    ] {
        services
            .decisions(project, &[id], action, "human", false)
            .unwrap();
    }
    let result = services
        .set_group_intent(
            project,
            "burst",
            GroupIntent {
                target_keep: Some(3),
                locked_photo_ids: vec![ids[1]],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.recommended_photo_ids, vec![ids[0], ids[1], ids[4]]);
    assert_eq!(result.pending_keep_photo_ids, vec![ids[1], ids[4]]);
    assert_eq!(services.photo(ids[1]).unwrap().decision, Action::Pending);
    assert_eq!(
        services
            .accept_group_selection(project, "burst")
            .unwrap()
            .changed,
        2
    );
    assert_eq!(services.photo(ids[2]).unwrap().decision, Action::Reject);
    assert_eq!(services.photo(ids[3]).unwrap().decision, Action::Flag);
    services.undo(project).unwrap();
    assert_eq!(services.photo(ids[1]).unwrap().decision, Action::Pending);
    drop(services);
    let mut services = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    assert_eq!(
        services
            .group_selection(project, "burst")
            .unwrap()
            .intent
            .locked_photo_ids,
        vec![ids[1]]
    );
    rebuild(&mut services, project, &ids[..4]);
    let reset = services.group_selection(project, "burst").unwrap();
    assert_eq!(reset.intent.target_keep, None);
    assert_eq!(reset.intent.locked_photo_ids, vec![ids[1]]);
    assert_eq!(reset.over_target, 1);
}

#[test]
fn must_keep_blocks_destructive_marks_and_invalid_updates_are_atomic() {
    let (_temp, mut services, project, ids) = fixture();
    let intent = GroupIntent {
        target_keep: Some(1),
        locked_photo_ids: vec![ids[0]],
        ..Default::default()
    };
    services
        .set_group_intent(project, "burst", intent.clone())
        .unwrap();
    assert!(services
        .decisions(project, &[ids[0]], Action::Reject, "human", false)
        .is_err());
    assert!(services
        .mark_photos(
            project,
            MarkRequest {
                photo_ids: vec![ids[0]],
                decision: Some(Action::Reject),
                rating: None,
                color_label: None
            }
        )
        .is_err());
    assert!(services
        .set_group_intent(
            project,
            "burst",
            GroupIntent {
                locked_photo_ids: vec![999999],
                ..intent
            }
        )
        .is_err());
    assert_eq!(
        services
            .group_selection(project, "burst")
            .unwrap()
            .intent
            .locked_photo_ids,
        vec![ids[0]]
    );
    assert_eq!(services.photo(ids[0]).unwrap().decision, Action::Pending);
}

#[test]
fn changed_source_prevents_partial_adoption() {
    let (_temp, mut services, project, ids) = fixture();
    services
        .set_group_intent(
            project,
            "burst",
            GroupIntent {
                target_keep: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    let path = services.photo_path(ids[1]).unwrap();
    std::fs::write(path, b"changed synthetic fixture").unwrap();
    assert!(services.accept_group_selection(project, "burst").is_err());
    assert!(services
        .photos(project, PhotoFilter::default())
        .unwrap()
        .iter()
        .all(|photo| photo.decision == Action::Pending));
}

#[test]
fn overlapping_group_saves_apply_only_explicit_lock_changes() {
    let (_temp, mut services, project, ids) = fixture();
    let groups = ["burst", "duplicate"].map(|id| BurstGroup {
        id: id.into(),
        project_id: project,
        kind: id.into(),
        member_photo_ids: ids.clone(),
        assessment: None,
    });
    services.save_groups(project, groups.to_vec()).unwrap();
    let mut stale = services
        .group_selection(project, "duplicate")
        .unwrap()
        .intent;
    let mut lock = services.group_selection(project, "burst").unwrap().intent;
    lock.locked_photo_ids = vec![ids[0]];
    services.set_group_intent(project, "burst", lock).unwrap();
    stale.target_keep = Some(2);
    let mut shared = services
        .set_group_intent(project, "duplicate", stale)
        .unwrap()
        .intent;
    assert_eq!(shared.locked_photo_ids, vec![ids[0]]);
    let mut unlock = services.group_selection(project, "burst").unwrap().intent;
    unlock.locked_photo_ids.clear();
    services.set_group_intent(project, "burst", unlock).unwrap();
    shared.target_keep = Some(3);
    let unlocked = services
        .set_group_intent(project, "duplicate", shared)
        .unwrap();
    assert!(
        unlocked.intent.locked_photo_ids.is_empty(),
        "stale unchanged locks must not be reinserted"
    );
    let mut lock = unlocked.intent;
    lock.locked_photo_ids = vec![ids[1]];
    services
        .set_group_intent(project, "duplicate", lock)
        .unwrap();
    let mut unlock = services.group_selection(project, "burst").unwrap().intent;
    unlock.locked_photo_ids.clear();
    assert!(services
        .set_group_intent(project, "burst", unlock)
        .unwrap()
        .intent
        .locked_photo_ids
        .is_empty());
}

#[test]
fn linked_locked_variant_is_a_conflict_not_a_batch_failure() {
    let (temp, mut services, project, ids) = fixture();
    let connection = rusqlite::Connection::open(temp.path().join("db.sqlite")).unwrap();
    connection
        .execute(
            "UPDATE photos SET capture_variant_id='synthetic-pair' WHERE id IN (?1,?2)",
            rusqlite::params![ids[0], ids[1]],
        )
        .unwrap();
    rebuild(&mut services, project, &ids);
    services
        .set_group_intent(
            project,
            "burst",
            GroupIntent {
                locked_photo_ids: vec![ids[0]],
                ..Default::default()
            },
        )
        .unwrap();
    let result = services
        .decisions(project, &[ids[1]], Action::Reject, "human", true)
        .unwrap();
    assert_eq!(result.changed, 1);
    assert_eq!(result.conflicts, vec![ids[0]]);
    assert_eq!(services.photo(ids[0]).unwrap().decision, Action::Pending);
    assert_eq!(services.photo(ids[1]).unwrap().decision, Action::Reject);
    assert!(services
        .decisions(project, &[ids[0], ids[2]], Action::Reject, "human", true)
        .is_err());
    assert_eq!(services.photo(ids[2]).unwrap().decision, Action::Pending);
}

#[test]
fn project_adoption_obeys_group_targets() {
    let (_temp, mut services, project, ids) = fixture();
    services
        .set_group_intent(
            project,
            "burst",
            GroupIntent {
                target_keep: Some(1),
                locked_photo_ids: vec![ids[0]],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(services.accept(project).unwrap().changed, 1);
    assert_eq!(services.photo(ids[0]).unwrap().decision, Action::Keep);
    assert!(services
        .photos(project, PhotoFilter::default())
        .unwrap()
        .iter()
        .filter(|photo| photo.id != ids[0])
        .all(|photo| photo.decision == Action::Pending));
}

fn choice(
    selection: &iris_core::services::GroupSelection,
    photo: i64,
    index: Option<usize>,
) -> FaceChoice {
    FaceChoice {
        photo_id: photo,
        face_index: index,
        analysis_key: selection
            .faces
            .iter()
            .find(|face| face.photo_id == photo)
            .unwrap()
            .analysis_key
            .clone()
            .unwrap(),
    }
}

#[test]
fn main_face_quality_changes_order_and_manual_absence_is_persistent() {
    let (temp, mut services, project, ids) = fixture();
    let initial = services.group_selection(project, "burst").unwrap();
    assert_eq!(initial.recommended_photo_ids, vec![ids[0]]);
    let mut intent = GroupIntent {
        main_face: Some(choice(&initial, ids[0], Some(0))),
        ..Default::default()
    };
    let selected = services
        .set_group_intent(project, "burst", intent.clone())
        .unwrap();
    assert_eq!(selected.recommended_photo_ids, vec![ids[1]]);
    assert_eq!(selected.faces[1].state, "geometry_candidate");
    intent.face_choices = vec![choice(&selected, ids[1], None)];
    let corrected = services.set_group_intent(project, "burst", intent).unwrap();
    assert_eq!(corrected.faces[1].state, "absent");
    assert_eq!(corrected.recommended_photo_ids, vec![ids[4]]);
    drop(services);
    let services = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    assert_eq!(
        services.group_selection(project, "burst").unwrap().faces[1].state,
        "absent"
    );
}

#[test]
fn face_corrections_ignore_search_vectors_but_expire_after_analysis_changes() {
    let (_temp, mut services, project, ids) = fixture();
    let initial = services.group_selection(project, "burst").unwrap();
    let intent = GroupIntent {
        main_face: Some(choice(&initial, ids[0], Some(0))),
        ..Default::default()
    };
    services
        .set_group_intent(project, "burst", intent.clone())
        .unwrap();
    let mut value = services.photo(ids[0]).unwrap().analysis.unwrap();
    value["search_embedding"] = json!({"synthetic":true});
    services.save_analysis(ids[0], value.clone()).unwrap();
    rebuild(&mut services, project, &ids);
    assert_eq!(
        services.group_selection(project, "burst").unwrap().faces[0].state,
        "reference"
    );
    value["faces"][0]["bbox"] = json!([100., 50., 64., 64.]);
    services.save_analysis(ids[0], value).unwrap();
    rebuild(&mut services, project, &ids);
    let stale = services.group_selection(project, "burst").unwrap();
    assert!(stale.faces.iter().all(|face| face.state == "stale"));
    assert!(stale.pending_keep_photo_ids.is_empty());
    assert!(services.set_group_intent(project, "burst", intent).is_err());
}
