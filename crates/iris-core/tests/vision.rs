use iris_core::vision::{
    decode_jpeg_preview, rescore, AnalysisSettings, Exposure, Eye, EyeState, Face, Verdict,
    VisionAnalysis, VisionEngine,
};
use std::{path::PathBuf, time::Instant};

#[test]
fn jpeg_idct_is_bounded_before_image_allocation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.jpg");
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
        std::fs::File::create(&path).unwrap(),
        80,
    );
    encoder
        .encode(
            &vec![100; 6000 * 4000],
            6000,
            4000,
            image::ExtendedColorType::L8,
        )
        .unwrap();
    let preview = decode_jpeg_preview(&path, 1280).unwrap();
    assert_eq!(
        (preview.original_width, preview.original_height),
        (6000, 4000)
    );
    assert_eq!(preview.image.dimensions(), (750, 500));
}

#[test]
fn missing_models_are_explicit_errors() {
    let dir = tempfile::tempdir().unwrap();
    let error = VisionEngine::new(dir.path())
        .err()
        .expect("must fail without models");
    assert!(error.to_string().contains("missing"));
}

#[test]
fn detector_settings_are_backward_compatible_and_hash_bound() {
    use iris_core::vision::FaceDetectorProvider;
    let mut settings: AnalysisSettings = serde_json::from_str("{}").unwrap();
    assert_eq!(settings.face_detector, FaceDetectorProvider::Yunet);
    assert!(settings.scrfd_model_sha256.is_none());
    settings.validate().unwrap();
    settings.face_detector = FaceDetectorProvider::Scrfd500m;
    assert!(settings.validate().is_err());
    for invalid in [
        "a".repeat(63),
        "g".repeat(64),
        format!("{}\n", "a".repeat(63)),
    ] {
        settings.scrfd_model_sha256 = Some(invalid);
        assert!(settings.validate().is_err());
    }
    settings.scrfd_model_sha256 = Some("A".repeat(64));
    settings.validate().unwrap();
    let encoded = serde_json::to_value(&settings).unwrap();
    assert_eq!(encoded["face_detector"], "scrfd_500m");
    assert_eq!(encoded["scrfd_model_sha256"], "A".repeat(64));
    assert!(serde_json::from_str::<AnalysisSettings>(r#"{"face_detector":"unknown"}"#).is_err());
}

fn poor_analysis() -> VisionAnalysis {
    VisionAnalysis {
        width: 10,
        height: 10,
        original_width: 10,
        original_height: 10,
        orientation: 1,
        preview_source: None,
        faces: vec![],
        sharpness_lap: 0.,
        sharpness_fft: 0.,
        niqe: None,
        exposure: Exposure {
            mean: 0.,
            shadow_clip: 1.,
            highlight_clip: 0.,
            verdict: "underexposed".into(),
        },
        composition: None,
        composite_score: 0.,
        score_breakdown: None,
        verdict: Verdict::Review,
        phash: "0".into(),
        structure: vec![],
        embedding: None,
        warnings: vec![],
        version: "test".into(),
    }
}

fn face_at(x: f32, y: f32) -> Face {
    Face {
        index: 0,
        bbox: [x - 10., y - 10., 20., 20.],
        confidence: 0.9,
        quality: None,
        quality_unavailable_reason: None,
        head_pose: None,
        left_eye: None,
        right_eye: None,
        left_eye_unreliable_reason: None,
        right_eye_unreliable_reason: None,
        left_eye_visibility: None,
        right_eye_visibility: None,
        smile_score: None,
        landmark_confidence: Some(0.99),
        landmarks: vec![[x, y, 0.]; 478],
        unreliable_reason: None,
    }
}

#[test]
fn composition_accepts_center_and_thirds_and_omits_no_subject() {
    let mut a = poor_analysis();
    a.width = 300;
    a.height = 300;
    a.exposure = Exposure {
        mean: 128.,
        shadow_clip: 0.,
        highlight_clip: 0.,
        verdict: "normal".into(),
    };
    let mut settings = AnalysisSettings::default();
    settings.eyes_weight = 0.;
    settings.sharpness_weight = 0.;
    settings.face_weight = 0.;
    settings.exposure_weight = 100.;
    settings.smile_weight = 0.;
    let empty = rescore(&a, &settings);
    assert!(empty.composition.is_none());
    assert!((empty.composite_score - 100.).abs() < 1e-6);
    a.faces = vec![face_at(150., 150.)];
    let center = rescore(&a, &settings);
    let c = center.composition.unwrap();
    assert!(c.score > 99.999);
    assert!(c.center_distance < 1e-6);
    a.faces = vec![face_at(100., 100.)];
    let thirds = rescore(&a, &settings);
    let c = thirds.composition.unwrap();
    assert!(c.score > 99.999);
    assert!(c.thirds_distance < 1e-6);
    a.faces = vec![face_at(0., 0.)];
    let corner = rescore(&a, &settings);
    assert!(corner.composition.as_ref().unwrap().score < 1e-6);
    assert!(
        (corner.composite_score - 75.).abs() < 1e-6,
        "composition must have exactly 25% inside the framing category"
    );
    assert_eq!(
        corner.verdict,
        Verdict::Review,
        "composition alone must not suggest rejection"
    );
}

#[test]
fn old_records_deserialize_and_composition_recomputes() {
    let mut a = poor_analysis();
    a.width = 300;
    a.height = 300;
    a.faces = vec![face_at(150., 150.)];
    let mut json = serde_json::to_value(&a).unwrap();
    assert!(json["faces"][0].get("left_eye_visibility").is_none());
    assert!(json["faces"][0].get("right_eye_visibility").is_none());
    json.as_object_mut().unwrap().remove("composition");
    let face = json["faces"][0].as_object_mut().unwrap();
    face.remove("left_eye_unreliable_reason");
    face.remove("right_eye_unreliable_reason");
    let restored: VisionAnalysis = serde_json::from_value(json).unwrap();
    assert!(restored.composition.is_none());
    assert!(restored.faces[0].left_eye_unreliable_reason.is_none());
    assert!(restored.faces[0].left_eye_visibility.is_none());
    assert!(restored.faces[0].right_eye_visibility.is_none());
    assert!(restored.faces[0].quality.is_none());
    assert!(restored.score_breakdown.is_none());
    let recalculated = rescore(&restored, &AnalysisSettings::default());
    assert_eq!(recalculated.version, restored.version);
    assert!(recalculated.faces[0].quality.is_none());
    let face_score = recalculated
        .score_breakdown
        .as_ref()
        .unwrap()
        .components
        .iter()
        .find(|c| c.id == "face")
        .unwrap();
    assert!(face_score.score.is_none());
    assert_eq!((face_score.observed_count, face_score.total_count), (0, 1));
    assert!(recalculated.composition.is_some());
    let mut moved = restored;
    moved.faces[0].landmarks.fill([0., 0., 0.]);
    let moved = rescore(&moved, &AnalysisSettings::default());
    assert!(moved.composition.unwrap().score < recalculated.composition.unwrap().score);
}

#[test]
fn fft_and_niqe_contribute_with_auditable_known_weights() {
    let mut a = poor_analysis();
    a.sharpness_lap = 1000.;
    a.sharpness_fft = 0.25;
    a.niqe = Some(4.);
    let s = AnalysisSettings {
        sharpness_weight: 100.,
        eyes_weight: 0.,
        face_weight: 0.,
        exposure_weight: 0.,
        smile_weight: 0.,
        ..AnalysisSettings::default()
    };
    let result = rescore(&a, &s);
    assert!((result.composite_score - 85.).abs() < 1e-10);
    let b = result.score_breakdown.unwrap();
    assert_eq!(b.effective_weight_total, 100.);
    let sharp = b.components.iter().find(|c| c.id == "sharpness").unwrap();
    for (t, expected) in sharp.terms.iter().zip([0.60, 0.20, 0.20]) {
        assert!((t.weight - expected).abs() < 1e-12);
    }
    assert!((sharp.terms.iter().map(|t| t.contribution).sum::<f64>() - 85.).abs() < 1e-10);
    a.sharpness_fft = 1.;
    assert!((rescore(&a, &s).composite_score - 95.).abs() < 1e-10);
    a.niqe = None;
    a.sharpness_fft = 0.25;
    let missing = rescore(&a, &s);
    assert!((missing.composite_score - 87.5).abs() < 1e-10);
    let sharp = missing
        .score_breakdown
        .as_ref()
        .unwrap()
        .components
        .iter()
        .find(|c| c.id == "sharpness")
        .unwrap();
    assert_eq!(sharp.terms[2].weight, 0.);
    assert!(sharp.terms[2]
        .missing_reason
        .as_ref()
        .unwrap()
        .contains("unavailable"));
    a.niqe = Some(4.);
    let disabled = rescore(
        &a,
        &AnalysisSettings {
            enable_niqe: false,
            ..s
        },
    );
    assert!((disabled.composite_score - 87.5).abs() < 1e-10);
}

#[test]
fn face_quality_coverage_and_final_contributions_are_explicit() {
    use iris_core::vision::FaceQuality;
    let mut a = poor_analysis();
    a.width = 300;
    a.height = 300;
    a.faces = vec![face_at(150., 150.), face_at(150., 150.)];
    a.faces[0].quality = Some(FaceQuality {
        method: "test observed crop".into(),
        crop_width: 128,
        crop_height: 128,
        sharpness_lap: 0.,
        exposure: Exposure {
            mean: 128.,
            shadow_clip: 0.,
            highlight_clip: 0.,
            verdict: "normal".into(),
        },
        sharpness_score: 0.,
        exposure_score: 100.,
        resolution_score: 100.,
        score: 50.,
    });
    let s = AnalysisSettings::default();
    let result = rescore(&a, &s);
    let b = result.score_breakdown.as_ref().unwrap();
    let face = b.components.iter().find(|c| c.id == "face").unwrap();
    assert_eq!(face.score, Some(50.));
    assert_eq!((face.observed_count, face.total_count), (1, 2));
    assert_eq!(face.terms[1].weight, 0.);
    assert!(face.terms[1].missing_reason.is_some());
    assert_eq!(b.effective_weight_total, 65.);
    assert!(
        (b.components.iter().map(|c| c.contribution).sum::<f64>() - result.composite_score).abs()
            < 1e-12
    );
    for c in &b.components {
        if let Some(score) = c.score {
            assert!((c.terms.iter().map(|t| t.contribution).sum::<f64>() - score).abs() < 1e-12);
        }
    }
    // Detection confidence remains independent from local technical quality.
    a.faces[0].confidence = 0.1;
    let changed = rescore(&a, &s);
    assert_eq!(changed.faces[0].confidence, 0.1);
    assert_eq!(
        changed
            .score_breakdown
            .unwrap()
            .components
            .iter()
            .find(|c| c.id == "face")
            .unwrap()
            .score,
        Some(50.)
    );
}

#[test]
fn closed_eyes_never_create_reject_and_unreliable_eyes_never_recommend() {
    let mut a = poor_analysis();
    a.faces.push(Face {
        index: 0,
        bbox: [0., 0., 10., 10.],
        confidence: 0.9,
        quality: None,
        quality_unavailable_reason: None,
        head_pose: None,
        left_eye: Some(Eye {
            state: EyeState::Closed,
            blink_score: Some(0.99),
            ear: 0.1,
        }),
        right_eye: None,
        left_eye_unreliable_reason: None,
        right_eye_unreliable_reason: None,
        left_eye_visibility: None,
        right_eye_visibility: None,
        smile_score: None,
        landmark_confidence: Some(0.9),
        landmarks: vec![],
        unreliable_reason: None,
    });
    assert_eq!(
        rescore(&a, &AnalysisSettings::default()).verdict,
        Verdict::Review
    );
    a.faces[0].left_eye = None;
    a.sharpness_lap = 1000.;
    a.exposure = Exposure {
        mean: 128.,
        shadow_clip: 0.,
        highlight_clip: 0.,
        verdict: "normal".into(),
    };
    assert_eq!(
        rescore(&a, &AnalysisSettings::default()).verdict,
        Verdict::Review
    );
}

#[test]
fn group_similarity_does_not_chain_drift_or_join_without_capture_time() {
    use iris_core::vision::group_similar;
    let mut a = poor_analysis();
    a.structure = vec![0.5; 64];
    a.phash = "0000000000000000".into();
    let mut b = a.clone();
    b.phash = "00000000000000ff".into();
    let mut c = b.clone();
    c.phash = "000000000000ffff".into();
    let items = vec![
        ("a".into(), a.clone(), Some(1)),
        ("b".into(), b, Some(2)),
        ("c".into(), c, Some(3)),
    ];
    assert_eq!(
        group_similar(&items),
        vec![vec!["a".to_string(), "b".to_string()]]
    );
    assert!(group_similar(&[("a".into(), a.clone(), None), ("b".into(), a, None)]).is_empty());
}

#[test]
fn missing_blendshapes_never_decide_eye_state_or_recommend_a_group_portrait() {
    use iris_core::vision::classify_eye;
    for ear in [0., 0.10, 0.20, 0.40, 1., f32::NAN] {
        assert_eq!(classify_eye(None, ear), EyeState::Uncertain);
    }
    let mut analysis = poor_analysis();
    analysis.sharpness_lap = 1000.;
    analysis.exposure = Exposure {
        mean: 128.,
        shadow_clip: 0.,
        highlight_clip: 0.,
        verdict: "normal".into(),
    };
    analysis.faces = vec![face_at(5., 5.), face_at(5., 5.)];
    for face in &mut analysis.faces {
        face.left_eye = Some(Eye {
            state: classify_eye(None, 0.4),
            blink_score: None,
            ear: 0.4,
        });
        face.right_eye = face.left_eye.clone();
    }
    let settings = AnalysisSettings::default();
    let scored = rescore(&analysis, &settings);
    assert_eq!(scored.verdict, Verdict::Review);
    assert_eq!(scored.faces[0].left_eye.as_ref().unwrap().ear, 0.4);
    // Simulate a v3 saved analysis with old EAR-only Open classifications.
    for face in &mut analysis.faces {
        face.left_eye.as_mut().unwrap().state = EyeState::Open;
        face.right_eye.as_mut().unwrap().state = EyeState::Open;
    }
    let rescored = rescore(&analysis, &settings);
    assert_eq!(rescored.verdict, Verdict::Review);
    assert!(rescored
        .faces
        .iter()
        .all(|face| face.left_eye.as_ref().unwrap().state == EyeState::Uncertain));
    // Observability rejection stays withheld, not converted into an eye state.
    analysis.faces[0].left_eye = None;
    analysis.faces[0].left_eye_unreliable_reason = Some("eye ROI is dark".into());
    let withheld = rescore(&analysis, &settings);
    assert!(withheld.faces[0].left_eye.is_none());
    assert_eq!(
        withheld.faces[0].left_eye_unreliable_reason.as_deref(),
        Some("eye ROI is dark")
    );
}

#[test]
fn corroborated_open_eyes_still_allow_recommendation() {
    use iris_core::vision::classify_eye;
    let mut analysis = poor_analysis();
    analysis.sharpness_lap = 1000.;
    analysis.exposure = Exposure {
        mean: 128.,
        shadow_clip: 0.,
        highlight_clip: 0.,
        verdict: "normal".into(),
    };
    let mut face = face_at(5., 5.);
    face.left_eye = Some(Eye {
        state: classify_eye(Some(0.1), 0.4),
        blink_score: Some(0.1),
        ear: 0.4,
    });
    face.right_eye = face.left_eye.clone();
    analysis.faces.push(face);
    assert_eq!(
        rescore(&analysis, &AnalysisSettings::default()).verdict,
        Verdict::Recommend
    );
}

#[test]
fn phash_index_keeps_eight_bit_neighbors_across_eight_bands() {
    use iris_core::vision::group_similar;
    let mut a = poor_analysis();
    a.structure = vec![0.5; 64];
    a.phash = "0000000000000000".into();
    let mut b = a.clone();
    let hash = (0..8).fold(0u64, |hash, band| hash | (1 << (band * 7)));
    b.phash = format!("{hash:016x}");
    assert_eq!(
        group_similar(&[("a".into(), a, Some(1)), ("b".into(), b, Some(2))]),
        vec![vec!["a".to_string(), "b".to_string()]]
    );
}

#[test]
fn duplicate_candidates_ignore_capture_time_but_require_structure() {
    use iris_core::vision::group_duplicates;
    let mut a = poor_analysis();
    a.structure = vec![0.5; 64];
    a.phash = "0000000000000000".into();
    let mut b = a.clone();
    b.phash = format!("{:016x}", (1u64 << 21) | 1);
    let mut unrelated = a.clone();
    unrelated.structure[4] = 0.7;
    let items = vec![
        ("a".into(), a.clone(), None),
        ("b".into(), b, Some(999999)),
        ("unrelated".into(), unrelated, None),
    ];
    assert_eq!(
        group_duplicates(&items),
        vec![vec!["a".to_string(), "b".to_string()]]
    );
    let mut third = a.clone();
    third.phash = "000000000000000f".into();
    let mut second = a.clone();
    second.phash = "0000000000000003".into();
    assert_eq!(
        group_duplicates(&[
            ("a".into(), a, None),
            ("b".into(), second, None),
            ("c".into(), third, None)
        ]),
        vec![vec!["a".to_string(), "b".to_string()]]
    );
}

#[test]
#[ignore = "requires downloaded pinned models and local user JPEG fixtures"]
fn real_models_all_jpeg_smoke() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut engine = VisionEngine::new(&root.join("models")).unwrap();
    let paths: Vec<_> = std::fs::read_dir(root.join("test-photos"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
        })
        .collect();
    assert_eq!(
        paths.len(),
        100,
        "expected authorized 100-photo fixture set"
    );
    let mut times = Vec::new();
    let mut face_count = 0;
    let mut reliable = 0;
    let mut smile_count = 0;
    let started = Instant::now();
    for path in paths {
        let t = Instant::now();
        let result = engine
            .analyze(&path, &AnalysisSettings::default())
            .unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
        times.push(t.elapsed().as_secs_f64());
        assert!(result.width.max(result.height) <= 1280);
        assert!(result.composite_score.is_finite());
        assert_eq!(result.phash.len(), 16);
        assert!(result.niqe.is_some_and(|n| n.is_finite() && n >= 0.));
        for face in &result.faces {
            face_count += 1;
            assert!(face.confidence.is_finite());
            if face.left_eye.is_some() {
                reliable += 1;
                assert_eq!(face.landmarks.len(), 478);
            }
            if face.smile_score.is_some() {
                smile_count += 1;
            }
            if face.unreliable_reason.is_some() {
                assert!(face.left_eye.is_none() && face.right_eye.is_none());
            }
        }
    }
    times.sort_by(f64::total_cmp);
    println!("100 JPEG actual models: {:.3}s, {:.2} photos/s, p50 {:.3}s, p95 {:.3}s, faces {}, reliable {}, smiles {}",started.elapsed().as_secs_f64(),100./started.elapsed().as_secs_f64(),times[50],times[95],face_count,reliable,smile_count);
    assert!(face_count > 0, "YuNet must detect actual wedding faces");
    assert!(
        smile_count > 0,
        "blendshape model must execute on actual faces"
    );
}

#[test]
#[ignore = "bounded sequential release profiling; set IRIS_PROFILE_PHOTOS to a JPG directory"]
fn profile_five_jpegs_sequential() {
    use iris_core::vision::AnalysisTimings;
    fn phases(t: AnalysisTimings) -> [f64; 5] {
        [
            t.decode_ms,
            t.metrics_ms,
            t.niqe_ms,
            t.faces_ms,
            t.phash_rescore_ms,
        ]
    }
    fn median(values: &[f64]) -> f64 {
        let mut values = values.to_vec();
        values.sort_by(f64::total_cmp);
        values[values.len() / 2]
    }
    let directory = PathBuf::from(
        std::env::var_os("IRIS_PROFILE_PHOTOS").expect("IRIS_PROFILE_PHOTOS is required"),
    );
    let mut photos: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg")
                })
        })
        .collect();
    photos.sort();
    photos.truncate(5);
    assert_eq!(photos.len(), 5, "provide at least five JPEG files");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut engine = VisionEngine::new(&root.join("models")).unwrap();
    let settings = AnalysisSettings::default();
    // One complete warm-up also checks the ordinary, non-timed API against the
    // timed API on this exact image. Model initialization is excluded entirely.
    let warmup = serde_json::to_value(engine.analyze(&photos[0], &settings).unwrap()).unwrap();
    let mut samples: Vec<[f64; 5]> = Vec::new();
    let mut per_photo = Vec::new();
    for (photo_index, path) in photos.iter().enumerate() {
        let mut reference = None;
        let mut photo_samples = Vec::new();
        let mut face_count = 0usize;
        let mut dimensions = [0u32; 2];
        for _ in 0..3 {
            let (analysis, timings) = engine.analyze_with_timings(path, &settings).unwrap();
            face_count = analysis.faces.len();
            dimensions = [analysis.width, analysis.height];
            let output = serde_json::to_value(&analysis).unwrap();
            if photo_index == 0 {
                assert_eq!(output, warmup, "timed and ordinary APIs must agree");
            }
            if let Some(expected) = &reference {
                assert_eq!(&output, expected, "non-timing output changed between runs");
            } else {
                reference = Some(output);
            }
            let values = phases(timings);
            assert!(values.iter().all(|v| v.is_finite() && *v >= 0.));
            photo_samples.push(values);
            samples.push(values);
        }
        let medians: Vec<_> = (0..5)
            .map(|i| median(&photo_samples.iter().map(|s| s[i]).collect::<Vec<_>>()))
            .collect();
        per_photo.push(serde_json::json!({"filename":path.file_name().unwrap().to_string_lossy(),"preview":dimensions,"faces":face_count,"median_ms":medians}));
    }
    let medians: Vec<_> = (0..5)
        .map(|i| median(&samples.iter().map(|s| s[i]).collect::<Vec<_>>()))
        .collect();
    let stage_sums: Vec<f64> = (0..5).map(|i| samples.iter().map(|s| s[i]).sum()).collect();
    let total: f64 = stage_sums.iter().sum();
    let shares: Vec<_> = stage_sums.iter().map(|sum| sum / total * 100.).collect();
    println!(
        "IRIS_STAGE_PROFILE={}",
        serde_json::json!({"analysis_version":iris_core::vision::ANALYSIS_VERSION,"photos":5,"warmups":1,"repetitions_per_photo":3,"sequential":true,"stages":["decode","metrics","niqe","faces","phash_rescore"],"median_ms":medians,"measured_total_ms":total,"stage_percent":shares,"non_timing_outputs_identical":true,"per_photo":per_photo})
    );
}
