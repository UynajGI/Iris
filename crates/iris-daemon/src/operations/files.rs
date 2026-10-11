use super::*;

#[utoipa::path(post,path="/api/v1/projects/{id}/export/xmp",params(("id"=i64,Path)),request_body=ExportRequest,responses((status=200,body=ExportReport)))]
pub async fn export_xmp(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<ExportRequest>,
) -> ApiResult<ExportReport> {
    Ok(Json(
        run(s, move |s| s.export_xmp(id, &b.scope, b.overwrite)).await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/export/copy",params(("id"=i64,Path)),request_body=ExportRequest,responses((status=200,body=ExportReport)))]
pub async fn export_copy(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<ExportRequest>,
) -> ApiResult<ExportReport> {
    Ok(Json(
        run(s, move |s| {
            s.export_copy(id, std::path::Path::new(&b.destination), &b.scope)
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/export/csv",params(("id"=i64,Path)),request_body=DestinationRequest,responses((status=200,body=ExportReport)))]
pub async fn export_csv(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<DestinationRequest>,
) -> ApiResult<ExportReport> {
    Ok(Json(
        run(s, move |s| {
            s.export_csv(id, std::path::Path::new(&b.destination))
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/import/csv",params(("id"=i64,Path)),request_body=SourceRequest,responses((status=200,body=DecisionBatch)))]
pub async fn import_csv(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<SourceRequest>,
) -> ApiResult<DecisionBatch> {
    Ok(Json(
        run(s, move |s| {
            s.import_csv(id, std::path::Path::new(&b.source))
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/quarantine/preview",params(("id"=i64,Path)),responses((status=200,body=QuarantinePlan)))]
pub async fn quarantine_preview(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<QuarantinePlan> {
    Ok(Json(run(s, move |s| s.quarantine_preview(id)).await?))
}
#[utoipa::path(get,path="/api/v1/projects/{id}/quarantine",params(("id"=i64,Path)),responses((status=200,body=Vec<QuarantinePlan>)))]
pub async fn quarantine_history(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Vec<QuarantinePlan>> {
    Ok(Json(run(s, move |s| s.quarantine_history(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/quarantine/commit",params(("id"=i64,Path)),request_body=ManifestRequest,responses((status=200,body=QuarantinePlan)))]
pub async fn quarantine_commit(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<ManifestRequest>,
) -> ApiResult<QuarantinePlan> {
    Ok(Json(
        run(s, move |s| {
            anyhow::ensure!(
                s.quarantine_plan(&b.manifest_id)?.project_id == id,
                "manifest belongs to another project"
            );
            s.quarantine_commit(&b.manifest_id)
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/quarantine/restore",params(("id"=i64,Path)),request_body=ManifestRequest,responses((status=200,body=QuarantinePlan)))]
pub async fn quarantine_restore(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<ManifestRequest>,
) -> ApiResult<QuarantinePlan> {
    Ok(Json(
        run(s, move |s| {
            anyhow::ensure!(
                s.quarantine_plan(&b.manifest_id)?.project_id == id,
                "manifest belongs to another project"
            );
            s.quarantine_restore(&b.manifest_id)
        })
        .await?,
    ))
}
