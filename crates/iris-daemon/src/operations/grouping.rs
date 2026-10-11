use super::*;

pub(super) fn build_groups(
    project_id: i64,
    photos: &[Photo],
    settings: &AnalysisSettings,
) -> Vec<BurstGroup> {
    let items: Vec<_> = photos
        .iter()
        .filter(|p| p.analysis_status == iris_core::AnalysisStatus::Current)
        .filter_map(|p| {
            let analysis: iris_core::vision::VisionAnalysis =
                serde::Deserialize::deserialize(p.analysis.as_ref()?).ok()?;
            let taken = p.taken_at.as_deref().and_then(parse_capture_time);
            Some((p.id.to_string(), analysis, taken))
        })
        .collect();
    let burst =
        if settings.embedding_provider == iris_core::vision::EmbeddingProvider::Dinov3Vits16 {
            iris_core::vision::group_semantic(
                &items,
                settings.semantic_similarity_threshold.unwrap_or(1.),
            )
        } else {
            iris_core::vision::group_similar(&items)
        }
        .into_iter()
        .map(|members| ("burst", members));
    let duplicates = iris_core::vision::group_duplicates(&items)
        .into_iter()
        .map(|members| ("duplicate", members));
    burst
        .chain(duplicates)
        .filter_map(|(kind, members)| {
            let mut variants = std::collections::HashSet::new();
            let ids: Vec<i64> = members
                .iter()
                .filter_map(|id| id.parse().ok())
                .filter(|id| {
                    photos
                        .iter()
                        .find(|p| p.id == *id)
                        .and_then(|p| p.capture_variant_id.as_ref())
                        .is_none_or(|v| variants.insert(v.clone()))
                })
                .collect();
            (ids.len() > 1).then(|| BurstGroup {
                id: uuid::Uuid::new_v4().to_string(),
                project_id,
                kind: kind.into(),
                member_photo_ids: ids,
                assessment: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod group_freshness_tests {
    use super::*;

    #[test]
    fn partial_refresh_failure_keeps_history_but_rebuilds_only_current_members() {
        let temporary = tempfile::tempdir().unwrap();
        let state = AppState::new(
            &temporary.path().join("library.sqlite"),
            temporary.path().join("models"),
        )
        .unwrap();
        let root = temporary.path().join("photos");
        std::fs::create_dir(&root).unwrap();
        let mut services = state.services.lock().unwrap();
        let id = services.create_project(&root).unwrap().id;
        let settings = services.settings(id).unwrap();
        let mut history = json!({
            "width":24,"height":16,"original_width":24,"original_height":16,
            "orientation":1,"faces":[],"sharpness_lap":1.,"sharpness_fft":1.,
            "niqe":null,"exposure":{"mean":100.,"shadow_clip":0.,
            "highlight_clip":0.,"verdict":"normal"},"composite_score":99.,
            "verdict":"recommend","phash":"0","structure":vec![0.5;64],
            "warnings":[],"version":"iris-vision-previous-engine","settings":settings
        });
        let mut ids = Vec::new();
        for name in ["failed.jpg", "refreshed-a.jpg", "refreshed-b.jpg"] {
            services.store.conn.execute(
                "INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height,taken_at) VALUES(?1,?2,?2,'jpeg',0,0,24,16,'2026:01:01 12:00:00')",
                (id, name),
            ).unwrap();
            let photo_id = services.store.conn.last_insert_rowid();
            services.save_analysis(photo_id, &history).unwrap();
            ids.push(photo_id);
        }
        // Model the persistence boundary after one worker fails and two succeed:
        // failed results do not replace their old analysis, successful ones do.
        let old_analysis = history.clone();
        history["version"] = json!(iris_core::vision::ANALYSIS_VERSION);
        history["composite_score"] = json!(50.);
        for photo_id in &ids[1..] {
            services.save_analysis(*photo_id, &history).unwrap();
        }
        assert!(services.project(id).unwrap().groups_dirty);
        drop(services);
        let job = start_job(&state, id, "analysis").unwrap_or_else(|e| panic!("{}", e.1));
        update(&state, id, |p| {
            p.errors.push("failed.jpg: analysis failed".into())
        });
        prepare_semantic_groups(&state, id, &job, &settings, None).unwrap();
        let services = state.services.lock().unwrap();
        let groups = services.groups(id).unwrap();
        assert!(groups.iter().any(|g| g.kind == "burst"));
        assert!(groups.iter().any(|g| g.kind == "duplicate"));
        for group in groups {
            assert_eq!(group.member_photo_ids, ids[1..]);
        }
        assert_eq!(services.photo(ids[0]).unwrap().analysis, Some(old_analysis));
        assert_eq!(
            services.photo(ids[0]).unwrap().analysis_status,
            iris_core::AnalysisStatus::Stale
        );
        assert_eq!(services.project(id).unwrap().pending_analysis, 1);
        assert!(!services.project(id).unwrap().groups_dirty);
    }
}

pub(super) fn parse_capture_time(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|v| v.timestamp())
        .or_else(|| {
            ["%Y-%m-%d %H:%M:%S", "%Y:%m:%d %H:%M:%S"]
                .iter()
                .find_map(|format| chrono::NaiveDateTime::parse_from_str(value, format).ok())
                .map(|v| v.and_utc().timestamp())
        })
}

#[cfg(test)]
mod grouping_tests {
    use super::*;
    #[test]
    fn scanner_displayed_exif_date_and_original_exif_date_match() {
        let scanner = parse_capture_time("2025-06-18 10:20:30");
        assert!(scanner.is_some());
        assert_eq!(scanner, parse_capture_time("2025:06:18 10:20:30"));
        assert_eq!(scanner, parse_capture_time("2025-06-18T10:20:30Z"));
        assert_eq!(parse_capture_time("unknown"), None);
    }
}
