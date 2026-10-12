use super::*;
use iris_core::services::ComparisonRequest;
use iris_core::services::{SceneCorrection, SceneGroup};
use iris_core::vision::comparison::ComparisonReport;

#[utoipa::path(post,path="/api/v1/projects/{id}/compare",params(("id"=i64,Path)),request_body=ComparisonRequest,responses((status=200,body=ComparisonReport)))]
pub async fn compare_photos(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ComparisonRequest>,
) -> ApiResult<ComparisonReport> {
    Ok(Json(
        run(state, move |services| services.compare_photos(id, request)).await?,
    ))
}

#[utoipa::path(get,path="/api/v1/projects/{id}/scenes",params(("id"=i64,Path)),responses((status=200,body=Vec<SceneGroup>)))]
pub async fn scenes(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Vec<SceneGroup>> {
    Ok(Json(run(state, move |services| services.scenes(id)).await?))
}
#[utoipa::path(put,path="/api/v1/projects/{id}/scenes",params(("id"=i64,Path)),request_body=SceneCorrection,responses((status=200,body=Vec<SceneGroup>)))]
pub async fn correct_scene(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<SceneCorrection>,
) -> ApiResult<Vec<SceneGroup>> {
    Ok(Json(
        run(state, move |services| services.correct_scene(id, request)).await?,
    ))
}
