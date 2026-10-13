use iris_core::{
    Action, AnalysisStatus, ColorLabel, MarkRequest, Photo, PhotoFilter, Services, Store,
};
use rusqlite::params;
use serde_json::{json, Value};

fn fixture(count: usize) -> (tempfile::TempDir, Services, i64, Vec<i64>) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let project = services.create_project(&root).unwrap().id;
    let settings = services.settings(project).unwrap();
    let mut ids = Vec::new();
    for i in 0..count {
        let name = format!("{}-{:03}.jpg", ["é", "A", "a", "人像"][i % 4], count - i);
        let format = if i % 3 == 0 { "raw" } else { "jpeg" };
        let taken = (i % 5 != 0).then(|| format!("2026-01-{:02} 12:00:00", i % 3 + 1));
        services.store.conn.execute(
            "INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height,taken_at,missing,quarantined)
             VALUES(?1,?2,?2,?3,?4,?5,100,100,?6,?7,?8)",
            params![project,name,format,(i%4) as i64,(i%3) as i64,taken,i%13==0,i%17==0],
        ).unwrap();
        let id = services.store.conn.last_insert_rowid();
        ids.push(id);
        if i % 7 != 0 {
            let mut data = json!({
                "version":iris_core::vision::ANALYSIS_VERSION,
                "settings":settings,"faces":[],"composite_score":(i%4) as f64*20.,
                "verdict":if i%2==0 {"recommend"} else {"review"}
            });
            match i % 11 {
                0 => data["version"] = json!("previous-engine"),
                1 => data["source_invalid"] = json!(true),
                2 => data["settings"] = json!({}), // Historical defaults normalize.
                3 => data["settings"]["face_confidence"] = json!(-1),
                4 => data["source_invalid"] = json!(1), // Not boolean true.
                5 => data["composite_score"] = json!("invalid"),
                _ => {}
            }
            services.save_analysis(id, data).unwrap();
        }
        services
            .mark_photos(
                project,
                MarkRequest {
                    photo_ids: vec![id],
                    decision: Some(if i % 3 == 0 {
                        Action::Keep
                    } else {
                        Action::Pending
                    }),
                    rating: Some((i % 6) as u8),
                    color_label: Some(if i % 2 == 0 {
                        ColorLabel::Purple
                    } else {
                        ColorLabel::None
                    }),
                },
            )
            .unwrap();
    }
    (temp, services, project, ids)
}

fn score(photo: &Photo) -> f64 {
    if photo.analysis_status != AnalysisStatus::Current {
        return -1.;
    }
    photo
        .analysis
        .as_ref()
        .and_then(|a| a["composite_score"].as_f64())
        .unwrap_or(-1.)
}

// Independent beta3 behavior oracle; the production query must preserve all
// predicates, stale-result handling, reverse direction and stable tie ordering.
fn expected(mut photos: Vec<Photo>, filter: &PhotoFilter) -> Vec<i64> {
    photos.retain(|p| {
        (filter.include_missing.unwrap_or(false) || (!p.missing && !p.quarantined))
            && filter.decision.is_none_or(|v| v == p.decision)
            && filter.rating.is_none_or(|v| v == p.rating)
            && filter.color_label.is_none_or(|v| v == p.color_label)
            && filter.format.as_ref().is_none_or(|v| v == &p.format)
            && filter.verdict.as_ref().is_none_or(|v| {
                p.analysis_status == AnalysisStatus::Current
                    && p.analysis.as_ref().and_then(|a| a["verdict"].as_str()) == Some(v)
            })
    });
    match filter.sort.as_deref().unwrap_or("name") {
        "score" | "suggestion" => {
            photos.sort_by(|a, b| score(b).total_cmp(&score(a)).then(a.id.cmp(&b.id)))
        }
        "size" => photos.sort_by_key(|p| (p.size_bytes, p.id)),
        "date" => photos.sort_by_key(|p| (p.taken_at.clone().unwrap_or_default(), p.mtime, p.id)),
        "name" => photos.sort_by(|a, b| a.path.cmp(&b.path)),
        _ => unreachable!(),
    }
    if filter.descending.unwrap_or(false) {
        photos.reverse();
    }
    photos
        .into_iter()
        .skip(filter.offset.unwrap_or(0))
        .take(filter.limit.unwrap_or(usize::MAX))
        .map(|p| p.id)
        .collect()
}

#[test]
fn paged_filters_and_sorts_match_beta3_including_stale_and_tied_results() {
    let (_temp, services, project, ids) = fixture(36);
    let photos: Vec<_> = ids
        .into_iter()
        .map(|id| services.photo(id).unwrap())
        .collect();
    for sort in ["name", "score", "suggestion", "size", "date"] {
        for descending in [false, true] {
            for variant in 0..9 {
                for (offset, limit) in [(0, 100), (0, 3), (2, 5), (30, 8), (200, 4), (0, 0)] {
                    let mut filter = PhotoFilter {
                        sort: Some(sort.into()),
                        descending: Some(descending),
                        offset: Some(offset),
                        limit: Some(limit),
                        ..Default::default()
                    };
                    match variant {
                        0 => {}
                        1 => filter.rating = Some(4),
                        2 => filter.decision = Some(Action::Keep),
                        3 => filter.color_label = Some(ColorLabel::Purple),
                        4 => filter.format = Some("raw".into()),
                        5 => filter.verdict = Some("recommend".into()),
                        6 => filter.include_missing = Some(true),
                        7 => {
                            filter.rating = Some(2);
                            filter.color_label = Some(ColorLabel::Purple);
                            filter.format = Some("jpeg".into());
                        }
                        _ => filter.format = Some("jpeg' OR 1=1 --".into()),
                    }
                    let actual: Vec<_> = services
                        .photos(project, filter.clone())
                        .unwrap()
                        .into_iter()
                        .map(|p| p.id)
                        .collect();
                    assert_eq!(actual, expected(photos.clone(), &filter), "{filter:?}");
                }
            }
        }
    }
}

#[test]
fn filtered_pages_do_not_decode_offscreen_analysis_payloads() {
    let (_temp, mut services, project, ids) = fixture(3);
    services
        .store
        .conn
        .execute(
            "UPDATE photos SET missing=0,quarantined=0 WHERE project_id=?1",
            [project],
        )
        .unwrap();
    services.store.conn.execute("INSERT INTO analyses(photo_id,data,version,analyzed_at) VALUES(?1,'corrupt offscreen JSON','old','now')",[ids[0]]).unwrap();
    services
        .mark_photos(
            project,
            MarkRequest {
                photo_ids: vec![ids[1]],
                decision: Some(Action::Flag),
                rating: Some(5),
                color_label: Some(ColorLabel::Blue),
            },
        )
        .unwrap();
    let rows = services
        .photos(
            project,
            PhotoFilter {
                rating: Some(5),
                sort: Some("score".into()),
                limit: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(rows.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[1]]);
    // The damaged photo is still reported as damaged if explicitly requested.
    assert!(services.photo(ids[0]).is_err());
}

#[test]
fn projection_tracks_direct_updates_invalidations_and_deletes() {
    let (_temp, mut services, project, ids) = fixture(2);
    services
        .store
        .conn
        .execute(
            "UPDATE photos SET missing=0,quarantined=0 WHERE project_id=?1",
            [project],
        )
        .unwrap();
    let id = ids[1];
    let data = json!({"version":iris_core::vision::ANALYSIS_VERSION,"settings":{},"composite_score":99.,"verdict":"recommend"});
    services.save_analysis(id, &data).unwrap();
    let recommended = PhotoFilter {
        verdict: Some("recommend".into()),
        ..Default::default()
    };
    assert_eq!(
        services.photos(project, recommended.clone()).unwrap().len(),
        1
    );
    services.invalidate_source_analysis(id).unwrap();
    assert!(services
        .photos(project, recommended.clone())
        .unwrap()
        .is_empty());
    services
        .store
        .conn
        .execute(
            "UPDATE analyses SET data=?1 WHERE photo_id=?2",
            params![data.to_string(), id],
        )
        .unwrap();
    assert_eq!(
        services.photos(project, recommended.clone()).unwrap().len(),
        1
    );
    services
        .store
        .conn
        .execute("DELETE FROM analyses WHERE photo_id=?1", [id])
        .unwrap();
    assert!(services.photos(project, recommended).unwrap().is_empty());
    let count: i64 = services
        .store
        .conn
        .query_row(
            "SELECT COUNT(*) FROM analysis_catalog WHERE photo_id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn version_six_upgrade_backfills_catalog_and_preserves_recovery_backup() {
    let (temp, services, project, _ids) = fixture(8);
    let before: Vec<_> = services
        .photos(
            project,
            PhotoFilter {
                sort: Some("score".into()),
                ..Default::default()
            },
        )
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    let database = services.store.path.clone();
    services.store.conn.execute_batch("DROP TRIGGER catalog_analysis_insert; DROP TRIGGER catalog_analysis_update; DROP TRIGGER catalog_analysis_delete;
        DROP VIEW analysis_catalog_source; DROP TABLE analysis_catalog;
        DROP TABLE group_selection_intents; DROP TABLE selection_locks;
        DROP INDEX photo_catalog_name; DROP INDEX photo_catalog_size; DROP INDEX photo_catalog_date; PRAGMA user_version=6;").unwrap();
    drop(services);
    let store = Store::open(&database).unwrap();
    assert!(store.recovery_notice.as_ref().unwrap().contains(".v6."));
    let backup = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.path().extension().is_some_and(|x| x == "bak"))
        .unwrap()
        .path();
    let old = rusqlite::Connection::open(backup).unwrap();
    assert_eq!(
        old.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        6
    );
    let services = Services::new(store);
    let after: Vec<_> = services
        .photos(
            project,
            PhotoFilter {
                sort: Some("score".into()),
                ..Default::default()
            },
        )
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(before, after);
}

#[test]
fn query_projection_defaulting_matches_rust_freshness_gate() {
    let (_temp, services, project, _) = fixture(0);
    let settings = services.settings(project).unwrap();
    let canonical = serde_json::to_string(
        &serde_json::from_value::<iris_core::vision::AnalysisSettings>(settings.clone()).unwrap(),
    )
    .unwrap();
    for source_invalid in [
        Value::Null,
        json!(false),
        json!(true),
        json!(1),
        json!("true"),
    ] {
        for saved_settings in [
            json!({}),
            settings.clone(),
            Value::Null,
            json!({"face_confidence":-1}),
        ] {
            let data = json!({"version":iris_core::vision::ANALYSIS_VERSION,"source_invalid":source_invalid,"settings":saved_settings});
            let sql:bool=services.store.conn.query_row(
                "SELECT json_extract(?1,'$.version')=?2 AND iris_catalog_settings_v1(json_extract(?1,'$.settings')) IS ?3 AND json_type(?1,'$.source_invalid') IS NOT 'true'",
                params![data.to_string(),iris_core::vision::ANALYSIS_VERSION,canonical],|r|r.get(0)).unwrap();
            assert_eq!(
                sql,
                iris_core::services::analysis_matches_settings(&data, &settings),
                "{data}"
            );
        }
    }
}

#[test]
#[ignore = "synthetic 10000-photo catalog benchmark; run explicitly in release mode"]
fn catalog_scale_measurement() {
    use std::time::Instant;
    let (_temp, mut services, project, _) = fixture(0);
    let data = json!({"version":iris_core::vision::ANALYSIS_VERSION,"settings":services.settings(project).unwrap(),
        "composite_score":80.,"verdict":"review","synthetic_features":vec![0.123456;2048]}).to_string();
    let tx = services.store.conn.transaction().unwrap();
    for i in 0..10_000 {
        let name = format!("synthetic-{i:05}.jpg");
        tx.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,?2,?2,'jpeg',1,1,100,100)",params![project,name]).unwrap();
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO analyses(photo_id,data,version,analyzed_at) VALUES(?1,?2,?3,'fixture')",
            params![id, data, iris_core::vision::ANALYSIS_VERSION],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    let filter = PhotoFilter {
        sort: Some("score".into()),
        limit: Some(100),
        offset: Some(200),
        ..Default::default()
    };
    let mut paged = vec![];
    let mut all_then_sort = vec![];
    for _ in 0..7 {
        let start = Instant::now();
        let actual: Vec<_> = services
            .photos(project, filter.clone())
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();
        paged.push(start.elapsed().as_secs_f64() * 1000.);
        let start = Instant::now();
        // Beta3-style materialization oracle, not a separately built beta3 binary.
        let all = services.photos(project, PhotoFilter::default()).unwrap();
        assert_eq!(actual, expected(all, &filter));
        all_then_sort.push(start.elapsed().as_secs_f64() * 1000.);
    }
    paged.sort_by(f64::total_cmp);
    all_then_sort.sort_by(f64::total_cmp);
    println!(
        "CATALOG_MEASUREMENT {}",
        json!({"synthetic_photos":10000,"analysis_bytes_each":data.len(),"page_size":100,"warm_application_database":true,
        "paged_median_ms":paged[3],"paged_max_ms":paged[6],"materialize_sort_median_ms":all_then_sort[3],"materialize_sort_max_ms":all_then_sort[6],"equal_ids":true})
    );
}
