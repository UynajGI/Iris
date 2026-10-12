use super::*;
use iris_core::vision::detail::{DetailEvidence, DetailRequest};

#[utoipa::path(post,path="/api/v1/photos/{id}/detail",params(("id"=i64,Path)),request_body=DetailRequest,responses((status=200,body=DetailEvidence)))]
pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<DetailRequest>,
) -> ApiResult<DetailEvidence> {
    let prepared = run(state.clone(), move |services| {
        services.prepare_detail(id, &request)
    })
    .await?;
    // At most one full-resolution region pass; visible previews retain the other decoder.
    let bytes = state
        .media_queue
        .render(2, move |cancel| prepared.render(cancel))
        .await?;
    run(state, move |services| services.photo_path(id)).await?;
    Ok(Json(serde_json::from_slice(&bytes)?))
}

#[derive(Default, Deserialize)]
pub struct MediaQuery {
    #[serde(default)]
    prefetch: bool,
}
#[utoipa::path(get,path="/api/v1/photos/{id}/thumb",params(("id"=i64,Path)),responses((status=200,description="JPEG thumbnail",content_type="image/jpeg",body=Vec<u8>)))]
pub async fn thumb(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    media(s, id, 320, false).await
}
#[utoipa::path(get,path="/api/v1/photos/{id}/preview",params(("id"=i64,Path),("prefetch"=Option<bool>,Query,description="Low-priority speculative preview")),responses((status=200,description="JPEG preview",content_type="image/jpeg",body=Vec<u8>)))]
pub async fn preview(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<MediaQuery>,
) -> Result<Response, ApiError> {
    media(s, id, 2560, query.prefetch).await
}
#[utoipa::path(get,path="/api/v1/photos/{id}/original",params(("id"=i64,Path)),responses((status=200,description="Unmodified source bytes; RAW and HEIC review uses JPEG previews",content((Vec<u8> = "image/jpeg"),(Vec<u8> = "image/png"),(Vec<u8> = "image/webp"),(Vec<u8> = "image/heic"),(Vec<u8> = "application/octet-stream")))))]
pub async fn original(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let path = run(s, move |s| s.photo_path(id)).await?;
    let content_type = match iris_core::vision::supported_format(&path) {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        Some("raw") => "application/octet-stream",
        _ => "image/jpeg",
    };
    let file = tokio::fs::File::open(path).await?;
    let length = file.metadata().await?.len();
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(file));
    Ok(Response::builder()
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(axum::http::header::CONTENT_LENGTH, length)
        .header(axum::http::header::CACHE_CONTROL, "private, no-store")
        .body(body)
        .map_err(anyhow::Error::from)?)
}
async fn media(s: AppState, id: i64, edge: u32, prefetch: bool) -> Result<Response, ApiError> {
    let job = run(s.clone(), move |s| s.prepare_cached_image(id, edge)).await?;
    let priority = if prefetch {
        2
    } else if edge == 320 {
        1
    } else {
        0
    };
    let bytes = s
        .media_queue
        .render(priority, move |cancel| job.render(cancel))
        .await?;
    // A quarantine or source change during decoding must not serve an obsolete request.
    run(s, move |s| s.photo_path(id)).await?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/jpeg"),
            (
                axum::http::header::CACHE_CONTROL,
                "private, max-age=0, must-revalidate",
            ),
        ],
        bytes,
    )
        .into_response())
}
