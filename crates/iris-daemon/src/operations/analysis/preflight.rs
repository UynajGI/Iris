use super::*;

pub(super) type PendingPhoto = (Photo, std::path::PathBuf);

pub(super) struct AnalysisPlan {
    pub pending: Vec<PendingPhoto>,
    pub reused: usize,
    pub failures: usize,
}

pub(super) fn read_inputs(
    s: &AppState,
    id: i64,
    selected: Option<&std::collections::HashSet<i64>>,
) -> anyhow::Result<(Vec<Photo>, Value)> {
    let (mut photos, settings) = {
        let services = s
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
        std::fs::read_dir(&services.project(id)?.root)
            .map_err(|error| anyhow::anyhow!("project folder unavailable: {error}"))?;
        (
            services.photos(id, PhotoFilter::default())?,
            services.settings(id)?,
        )
    };
    if let Some(selected) = selected {
        photos.retain(|photo| selected.contains(&photo.id));
    }
    Ok((photos, settings))
}

pub(super) fn validate_models(context: &AnalysisContext<'_>) -> anyhow::Result<()> {
    let s = context.state;
    let settings = context.settings;
    settings.validate()?;
    if settings.face_detector == iris_core::vision::FaceDetectorProvider::Scrfd500m {
        iris_core::vision::validate_detector_model(&s.model_dir, settings)?;
    }
    iris_core::vision::validate_occlusion_model(&s.model_dir, settings)?;
    // A missing optional model may fall back; wrong or damaged bytes never do.
    if settings.embedding_provider == iris_core::vision::EmbeddingProvider::Dinov3Vits16 {
        anyhow::ensure!(
            settings.embedding_model_sha256.as_deref().is_some_and(
                |hash| hash.eq_ignore_ascii_case(iris_core::vision::dinov3::MODEL_SHA256)
            ),
            "DINOv3 model hash is not the approved artifact"
        );
        let status = iris_core::vision::embedding_model_status(&s.model_dir, settings);
        if status.state == iris_core::vision::ModelAvailability::Invalid {
            anyhow::bail!(
                "DINOv3 artifact invalid: {}",
                status.reason.unwrap_or_default()
            );
        }
    }
    Ok(())
}

pub(super) fn prepare_photos(
    context: &AnalysisContext<'_>,
    photos: &[Photo],
) -> anyhow::Result<AnalysisPlan> {
    let (s, id) = (context.state, context.project_id);
    update(s, id, |p| p.total = photos.len());
    let mut plan = AnalysisPlan {
        pending: Vec::new(),
        reused: 0,
        failures: 0,
    };
    for photo in photos {
        let resolved = s
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
            .photo_path(photo.id)
            .and_then(|path| {
                verify_fingerprint(photo, &path)?;
                Ok(path)
            });
        let path = match resolved {
            Ok(path) => path,
            Err(error) => {
                plan.failures += 1;
                s.services
                    .lock()
                    .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
                    .invalidate_source_analysis(photo.id)?;
                update(s, id, |p| {
                    p.failed_photo_ids.push(photo.id);
                    p.errors.push(format!("{}: {error}", photo.filename));
                });
                continue;
            }
        };
        if photo.analysis.as_ref().is_some_and(|value| {
            iris_core::services::analysis_matches_settings(value, context.settings_value)
        }) {
            plan.reused += 1;
        } else {
            plan.pending.push((photo.clone(), path));
        }
    }
    update(s, id, |p| p.completed = plan.reused + plan.failures);
    Ok(plan)
}
