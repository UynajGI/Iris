use super::*;

#[utoipa::path(get,path="/api/v1/settings",params(("project_id"=i64,Query)),responses((status=200,body=AnalysisSettings)))]
pub async fn settings(
    State(s): State<AppState>,
    Query(b): Query<ProjectRequest>,
) -> ApiResult<Value> {
    Ok(Json(run(s, move |s| s.settings(b.project_id)).await?))
}
#[utoipa::path(put,path="/api/v1/settings",params(("project_id"=i64,Query)),request_body=AnalysisSettings,responses((status=200,body=AnalysisSettings)))]
pub async fn put_settings(
    State(s): State<AppState>,
    Query(q): Query<ProjectRequest>,
    Json(b): Json<Value>,
) -> ApiResult<Value> {
    Ok(Json(
        run(s, move |s| {
            let _: AnalysisSettings = serde_json::from_value(b.clone())?;
            s.set_settings(q.project_id, b)
        })
        .await?,
    ))
}
#[utoipa::path(get,path="/api/v1/profiles",responses((status=200,body=Vec<Profile>)))]
pub async fn profiles(State(s): State<AppState>) -> ApiResult<Vec<Profile>> {
    Ok(Json(run(s, |s| s.profiles()).await?))
}
#[utoipa::path(post,path="/api/v1/profiles",request_body=Profile,responses((status=200,body=Profile)))]
pub async fn save_profile(State(s): State<AppState>, Json(b): Json<Profile>) -> ApiResult<Profile> {
    Ok(Json(
        run(s, move |s| {
            let _: AnalysisSettings = serde_json::from_value(b.settings.clone())?;
            s.save_profile(&b.name, b.settings)
        })
        .await?,
    ))
}
#[utoipa::path(delete,path="/api/v1/profiles/{name}",params(("name"=String,Path)),responses((status=200,body=bool)))]
pub async fn delete_profile(
    State(s): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<bool> {
    Ok(Json(run(s, move |s| s.delete_profile(&name)).await?))
}
#[utoipa::path(post,path="/api/v1/profiles/{name}/apply",params(("name"=String,Path)),request_body=ProjectRequest,responses((status=200,body=AnalysisSettings)))]
pub async fn apply_profile(
    State(s): State<AppState>,
    Path(name): Path<String>,
    Json(b): Json<ProjectRequest>,
) -> ApiResult<Value> {
    Ok(Json(
        run(s, move |s| s.apply_profile(b.project_id, &name)).await?,
    ))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/profiles/{name}/estimate",params(("id"=i64,Path),("name"=String,Path)),responses((status=200,body=Value)))]
pub async fn estimate_profile(
    State(s): State<AppState>,
    Path((id, name)): Path<(i64, String)>,
) -> ApiResult<Value> {
    Ok(Json(run(s, move |s| s.estimate_profile(id, &name)).await?))
}
