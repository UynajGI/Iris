use super::*;
use crate::analysis_queue::BrowseContext;

#[utoipa::path(put,path="/api/v1/projects/{id}/browse-context",params(("id"=i64,Path)),request_body=BrowseContext,responses((status=200,body=BrowseContext)))]
pub async fn browse_context(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(context): Json<BrowseContext>,
) -> ApiResult<BrowseContext> {
    context.validate()?;
    let ids = context.ordered_ids();
    run(state.clone(), move |services| {
        services.require_available_photos(id, &ids)
    })
    .await?;
    // Storing context does not create or resume a scan/analysis job.
    Ok(Json(state.browse_contexts.update(id, context)))
}
