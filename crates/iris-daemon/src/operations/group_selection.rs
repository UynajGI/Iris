use super::*;
use iris_core::services::{GroupIntent, GroupSelection};

#[utoipa::path(get,path="/api/v1/projects/{id}/groups/{group_id}/selection",params(("id"=i64,Path),("group_id"=String,Path)),responses((status=200,body=GroupSelection)))]
pub async fn group_selection(
    State(state): State<AppState>,
    Path((project, group)): Path<(i64, String)>,
) -> ApiResult<GroupSelection> {
    Ok(Json(
        run(state, move |services| {
            services.group_selection(project, &group)
        })
        .await?,
    ))
}

#[utoipa::path(put,path="/api/v1/projects/{id}/groups/{group_id}/selection",params(("id"=i64,Path),("group_id"=String,Path)),request_body=GroupIntent,responses((status=200,body=GroupSelection)))]
pub async fn set_group_intent(
    State(state): State<AppState>,
    Path((project, group)): Path<(i64, String)>,
    Json(intent): Json<GroupIntent>,
) -> ApiResult<GroupSelection> {
    let result = run(state.clone(), move |services| {
        services.set_group_intent(project, &group, intent)
    })
    .await?;
    state.emit(
        "session:changed",
        project,
        json!({"group_id": result.group_id}),
    );
    Ok(Json(result))
}

#[utoipa::path(post,path="/api/v1/projects/{id}/groups/{group_id}/selection/accept",params(("id"=i64,Path),("group_id"=String,Path)),responses((status=200,body=DecisionBatch)))]
pub async fn accept_group_selection(
    State(state): State<AppState>,
    Path((project, group)): Path<(i64, String)>,
) -> ApiResult<DecisionBatch> {
    let result = run(state.clone(), move |services| {
        services.accept_group_selection(project, &group)
    })
    .await?;
    state.emit("session:changed", project, json!(result));
    Ok(Json(result))
}
