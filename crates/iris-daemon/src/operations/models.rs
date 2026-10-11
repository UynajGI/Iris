use super::*;

/// Read-only artifact checks; no inference runtime is initialized.
pub fn models_status(s: &AppState, project_id: i64) -> anyhow::Result<ModelStatusResponse> {
    let value = s
        .services
        .lock()
        .map_err(|_| anyhow::anyhow!("service lock poisoned"))?
        .settings(project_id)?;
    let settings: AnalysisSettings = serde_json::from_value(value)?;
    let selected = settings.face_detector;
    let occlusion_selected = settings.occlusion_provider;
    let embedding_selected = settings.embedding_provider;
    let detectors = [
        iris_core::vision::FaceDetectorProvider::Yunet,
        iris_core::vision::FaceDetectorProvider::Scrfd500m,
    ]
    .into_iter()
    .map(|provider| {
        let mut probe = settings.clone();
        probe.face_detector = provider;
        probe.occlusion_provider = iris_core::vision::OcclusionProvider::None;
        iris_core::vision::detector_model_status(&s.model_dir, &probe)
    })
    .collect();
    let occlusion = [
        iris_core::vision::OcclusionProvider::None,
        iris_core::vision::OcclusionProvider::Faceocc,
    ]
    .into_iter()
    .map(|provider| {
        let mut probe = settings.clone();
        probe.face_detector = iris_core::vision::FaceDetectorProvider::Yunet;
        probe.occlusion_provider = provider;
        iris_core::vision::occlusion_model_status(&s.model_dir, &probe)
    })
    .collect();
    let embeddings = [
        iris_core::vision::EmbeddingProvider::None,
        iris_core::vision::EmbeddingProvider::Dinov3Vits16,
    ]
    .into_iter()
    .map(|provider| {
        let mut probe = settings.clone();
        probe.embedding_provider = provider;
        iris_core::vision::embedding_model_status(&s.model_dir, &probe)
    })
    .collect();
    Ok(ModelStatusResponse {
        selected,
        detectors,
        occlusion_selected,
        occlusion,
        embedding_selected,
        embeddings,
    })
}

#[utoipa::path(get,path="/api/v1/models",params(("project_id"=i64,Query)),responses((status=200,body=ModelStatusResponse)))]
pub async fn models(
    State(s): State<AppState>,
    Query(q): Query<ProjectRequest>,
) -> ApiResult<ModelStatusResponse> {
    Ok(Json(
        tokio::task::spawn_blocking(move || models_status(&s, q.project_id))
            .await
            .map_err(anyhow::Error::from)??,
    ))
}
