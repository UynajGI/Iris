use super::*;

#[derive(Deserialize, ToSchema)]
pub struct ProjectRequest {
    pub project_id: i64,
}
#[derive(Serialize, ToSchema)]
pub struct ModelStatusResponse {
    pub selected: iris_core::vision::FaceDetectorProvider,
    pub detectors: Vec<iris_core::vision::DetectorModelStatus>,
    pub occlusion_selected: iris_core::vision::OcclusionProvider,
    pub occlusion: Vec<iris_core::vision::OcclusionModelStatus>,
    pub embedding_selected: iris_core::vision::EmbeddingProvider,
    pub embeddings: Vec<iris_core::vision::EmbeddingModelStatus>,
}
