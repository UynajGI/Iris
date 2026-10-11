use super::*;

#[utoipa::path(get,path="/api/v1/projects/{id}/cache",params(("id"=i64,Path)),responses((status=200,body=CacheStatus)))]
pub async fn cache_status(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<CacheStatus> {
    Ok(Json(run(s, move |s| s.cache_status(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/cache/cleanup",params(("id"=i64,Path)),responses((status=200,body=CacheStatus)))]
pub async fn cache_cleanup(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<CacheStatus> {
    Ok(Json(run(s, move |s| s.cache_cleanup(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/cache/migrate",params(("id"=i64,Path)),request_body=DestinationRequest,responses((status=200,body=CacheStatus)))]
pub async fn cache_migrate(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<DestinationRequest>,
) -> ApiResult<CacheStatus> {
    Ok(Json(
        run(s, move |s| {
            s.cache_migrate(id, std::path::Path::new(&b.destination))
        })
        .await?,
    ))
}

#[utoipa::path(get,path="/api/v1/projects/{id}/cache/migrations",params(("id"=i64,Path)),responses((status=200,body=Vec<CacheMigration>)))]
pub async fn cache_migrations(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Vec<CacheMigration>> {
    Ok(Json(run(s, move |s| s.cache_migrations(id)).await?))
}
#[utoipa::path(post,path="/api/v1/projects/{id}/cache/migrations/{migration_id}/cleanup",params(("id"=i64,Path),("migration_id"=String,Path)),responses((status=200,body=CacheMigration)))]
pub async fn cache_cleanup_old(
    State(s): State<AppState>,
    Path((id, migration_id)): Path<(i64, String)>,
) -> ApiResult<CacheMigration> {
    Ok(Json(
        run(s, move |s| s.cache_cleanup_old(id, &migration_id)).await?,
    ))
}
