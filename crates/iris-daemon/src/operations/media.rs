use super::*;

#[utoipa::path(get,path="/api/v1/photos/{id}/thumb",params(("id"=i64,Path)),responses((status=200,description="JPEG thumbnail",content_type="image/jpeg",body=Vec<u8>)))]
pub async fn thumb(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    media(s, id, 320).await
}
#[utoipa::path(get,path="/api/v1/photos/{id}/preview",params(("id"=i64,Path)),responses((status=200,description="JPEG preview",content_type="image/jpeg",body=Vec<u8>)))]
pub async fn preview(State(s): State<AppState>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    media(s, id, 2560).await
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
async fn media(s: AppState, id: i64, edge: u32) -> Result<Response, ApiError> {
    let bytes = run(s, move |s| s.cached_image(id, edge)).await?;
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
