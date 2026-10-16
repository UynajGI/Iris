// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use axum::response::{IntoResponse, Response};
use iris_core::{
    render::{
        color::{self, OutputSpace},
        engine::{RenderEngine, RenderQuality, RenderRequest, RenderedFrame},
        scopes::{self, Histogram, ScopeDomain},
        Frame, RenderContext,
    },
    services::EditRenderJob,
};
use std::sync::Arc;

pub(super) struct CancelContext(pub RenderContext);
impl Drop for CancelContext {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
struct CancelPreview {
    context: RenderContext,
    engine: Arc<RenderEngine>,
    session: String,
    sequence: u64,
    armed: bool,
}
impl CancelPreview {
    fn complete(&mut self) -> anyhow::Result<()> {
        self.context.check_cancelled()?;
        self.engine.check_request(&self.session, self.sequence)?;
        self.engine.finish_request(&self.session, self.sequence)?;
        self.armed = false;
        Ok(())
    }
}
impl Drop for CancelPreview {
    fn drop(&mut self) {
        if self.armed {
            self.context.cancel();
            let _ = self.engine.cancel_request(&self.session, self.sequence);
        }
    }
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct HistogramRequest {
    pub render: RenderRequest,
    pub domain: ScopeDomain,
    pub lower: f32,
    pub upper: f32,
    pub bins: u32,
}
#[derive(Serialize, ToSchema)]
pub struct HistogramResponse {
    pub revision: u64,
    pub sequence: u64,
    pub fingerprint: String,
    pub approximate: bool,
    /// Full-quality requests measure every output pixel; interactive requests
    /// measure the displayed proxy. These are the actual measured dimensions.
    pub preview_width: u32,
    pub preview_height: u32,
    pub histogram: Histogram,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SampleRequest {
    pub render: RenderRequest,
    pub rect: [f32; 4],
}
#[derive(Serialize, ToSchema)]
pub struct SampleResponse {
    pub revision: u64,
    pub sequence: u64,
    pub fingerprint: String,
    pub approximate: bool,
    pub preview_width: u32,
    pub preview_height: u32,
    pub domain: String,
    pub rgb: [f32; 3],
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CancelRequest {
    pub sequence: u64,
}

struct ScopePixels {
    frame: Arc<Frame>,
    approximate: bool,
    _rendered: Option<RenderedFrame>,
}
fn scope_pixels(
    job: &EditRenderJob,
    engine: &RenderEngine,
    request: &RenderRequest,
    context: &RenderContext,
) -> anyhow::Result<ScopePixels> {
    if request.quality == RenderQuality::Full {
        let frame = job.full_resolution(context)?;
        Ok(ScopePixels {
            frame,
            approximate: false,
            _rendered: None,
        })
    } else {
        let rendered = job.preview(engine, request, context)?;
        Ok(ScopePixels {
            frame: rendered.frame.clone(),
            approximate: rendered.approximate,
            _rendered: Some(rendered),
        })
    }
}

async fn render<T: Send + 'static>(
    state: AppState,
    id: String,
    request: RenderRequest,
    finish: impl FnOnce(&EditRenderJob, &RenderEngine, &RenderRequest, &RenderContext) -> anyhow::Result<T>
        + Send
        + 'static,
) -> Result<T, ApiError> {
    render_with_document(state, id, request, None, finish).await
}

async fn render_with_document<T: Send + 'static>(
    state: AppState,
    id: String,
    request: RenderRequest,
    document: Option<iris_core::render::document::EditDocument>,
    finish: impl FnOnce(&EditRenderJob, &RenderEngine, &RenderRequest, &RenderContext) -> anyhow::Result<T>
        + Send
        + 'static,
) -> Result<T, ApiError> {
    let revision = request.revision;
    let parent = state.render_engine.request_context();
    let context = state.render_engine.register_request(&request, &parent)?;
    let mut guard = CancelPreview {
        context: context.clone(),
        engine: state.render_engine.clone(),
        session: request.session.clone(),
        sequence: request.sequence,
        armed: true,
    };
    let permit = state
        .render_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(anyhow::Error::from)?;
    context.check_cancelled()?;
    let job = run(state.clone(), move |s| match document {
        Some(document) => s.prepare_draft_edit_render(&id, revision, document),
        None => s.prepare_edit_render(&id, revision),
    })
    .await?;
    let engine = state.render_engine;
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<T> {
        let _permit = permit;
        context.check_cancelled()?;
        let result = finish(&job, &engine, &request, &context)?;
        context.check_cancelled()?;
        engine.check_request(&request.session, request.sequence)?;
        Ok(result)
    })
    .await
    .map_err(anyhow::Error::from)??;
    guard.complete()?;
    Ok(result)
}

#[utoipa::path(operation_id="editing_preview_preview",post,path="/api/v1/edits/{id}/preview",params(("id"=String,Path)),request_body=RenderRequest,responses((status=200,body=Vec<u8>,content_type="image/jpeg",headers(("x-iris-revision"=u64,description="Edit revision"),("x-iris-sequence"=u64,description="Render sequence"),("x-iris-fingerprint"=String,description="Canonical rendering fingerprint"),("x-iris-approximate"=bool,description="True when proxy evaluation was used"),("x-iris-render-backend"=String,description="cpu, gpu or mixed for this render"),("x-iris-cpu-nodes"=u32,description="Nodes executed on CPU"),("x-iris-gpu-nodes"=u32,description="Nodes executed on GPU"),("x-iris-gpu-fallback"=bool,description="GPU attempt fell back to CPU")))))]
pub async fn preview(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<RenderRequest>,
) -> Result<Response, ApiError> {
    encoded_response(s, id, request, None, None).await
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftPreviewRequest {
    /// Revision is the saved base revision, not an unsaved counter.
    pub render: RenderRequest,
    pub document: iris_core::render::document::EditDocument,
}
/// Transient slider feedback: validates source binding and resources but writes
/// no history. Persist the chosen document separately through SaveEdit.
#[utoipa::path(operation_id="editing_preview_draft_preview",post,path="/api/v1/edits/{id}/preview-draft",params(("id"=String,Path)),request_body=DraftPreviewRequest,responses((status=200,body=Vec<u8>,content_type="image/jpeg",headers(("x-iris-revision"=u64,description="Saved base revision"),("x-iris-sequence"=u64,description="Latest-wins sequence"),("x-iris-fingerprint"=String,description="Draft graph fingerprint"),("x-iris-draft"=bool,description="Always true; no revision saved")))))]
pub async fn draft_preview(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<DraftPreviewRequest>,
) -> Result<Response, ApiError> {
    encoded_response(s, id, request.render, None, Some(request.document)).await
}

/// Native 1:1 pixels in final edited-image coordinates. The entire graph is
/// currently evaluated before cropping; max_edge does not resize this region.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegionRequest {
    pub render: RenderRequest,
    /// Supply exactly one of rect (absolute output pixels) or view (centered viewport).
    pub rect: Option<iris_core::render::region::PixelRect>,
    pub view: Option<iris_core::render::region::RegionView>,
    /// Transient adjustments; source identity and saved base revision remain checked.
    pub document: Option<iris_core::render::document::EditDocument>,
}

#[utoipa::path(operation_id="editing_preview_region",post,path="/api/v1/edits/{id}/region",params(("id"=String,Path)),request_body=RegionRequest,responses((status=200,body=Vec<u8>,content_type="image/jpeg",headers(("x-iris-revision"=u64,description="Edit revision"),("x-iris-sequence"=u64,description="Render sequence"),("x-iris-fingerprint"=String,description="Canonical rendering fingerprint"),("x-iris-approximate"=bool,description="Always false: native output pixels"),("x-iris-region-x"=u32,description="Output pixel origin x"),("x-iris-region-y"=u32,description="Output pixel origin y"),("x-iris-region-width"=u32,description="Native region width"),("x-iris-region-height"=u32,description="Native region height"),("x-iris-full-width"=u32,description="Full edited output width"),("x-iris-full-height"=u32,description="Full edited output height"),("x-iris-full-frame-evaluation"=bool,description="True: evaluates entire image before crop")))))]
pub async fn region(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<RegionRequest>,
) -> Result<Response, ApiError> {
    match (request.rect, request.view) {
        (Some(rect), None) => rect.validate()?,
        (None, Some(view)) => view.validate()?,
        _ => return Err(anyhow::anyhow!("supply exactly one region rect or view").into()),
    }
    let sequence = request.render.sequence;
    let result = render_with_document(
        s,
        id,
        request.render,
        request.document,
        move |job, _, _, context| match (request.rect, request.view) {
            (Some(rect), None) => job.encode_edit_region(context, rect),
            (None, Some(view)) => job.encode_edit_region_view(context, view),
            _ => unreachable!("region selection validated before rendering"),
        },
    )
    .await?;
    let metadata = result.metadata;
    Ok((
        [
            ("content-type", "image/jpeg".to_string()),
            ("cache-control", "no-store".to_string()),
            ("x-iris-revision", metadata.revision.to_string()),
            ("x-iris-sequence", sequence.to_string()),
            ("x-iris-fingerprint", metadata.fingerprint),
            ("x-iris-approximate", metadata.approximate.to_string()),
            ("x-iris-region-x", metadata.rect.x.to_string()),
            ("x-iris-region-y", metadata.rect.y.to_string()),
            ("x-iris-region-width", metadata.rect.width.to_string()),
            ("x-iris-region-height", metadata.rect.height.to_string()),
            ("x-iris-full-width", metadata.full_width.to_string()),
            ("x-iris-full-height", metadata.full_height.to_string()),
            (
                "x-iris-full-frame-evaluation",
                metadata.full_frame_evaluation.to_string(),
            ),
        ],
        result.jpeg,
    )
        .into_response())
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofRequest {
    pub render: RenderRequest,
    pub resource: String,
    /// Optional unsaved adjustments, validated against the saved source and base revision.
    pub document: Option<iris_core::render::document::EditDocument>,
}

#[utoipa::path(operation_id="editing_preview_proof",post,path="/api/v1/edits/{id}/proof",params(("id"=String,Path)),request_body=ProofRequest,responses((status=200,body=Vec<u8>,content_type="image/jpeg")))]
pub async fn proof(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ProofRequest>,
) -> Result<Response, ApiError> {
    let bytes = run(s.clone(), move |services| {
        let resource = services.edit_resource(&request.resource)?;
        anyhow::ensure!(
            resource.kind == EditResourceKind::Icc,
            "soft proof requires an ICC resource"
        );
        services.read_edit_resource(&request.resource)
    })
    .await?;
    encoded_response(s, id, request.render, Some(bytes), request.document).await
}

async fn encoded_response(
    s: AppState,
    id: String,
    request: RenderRequest,
    proof: Option<Vec<u8>>,
    document: Option<iris_core::render::document::EditDocument>,
) -> Result<Response, ApiError> {
    let is_draft = document.is_some();
    let is_proof = proof.is_some();
    let (bytes, revision, sequence, fingerprint, approximate, cpu_nodes, gpu_nodes, fallback) =
        render_with_document(
            s,
            id,
            request,
            document,
            move |job, engine, request, context| {
                let (bytes, result) =
                    job.encode_edit_preview(engine, request, context, proof.as_deref())?;
                Ok((
                    bytes,
                    result.revision,
                    result.sequence,
                    result.fingerprint,
                    result.approximate,
                    result.execution.cpu_nodes,
                    result.execution.gpu_nodes,
                    !result.execution.fallback_reasons.is_empty(),
                ))
            },
        )
        .await?;
    Ok((
        [
            ("content-type", "image/jpeg".to_string()),
            ("cache-control", "no-store".to_string()),
            ("x-iris-revision", revision.to_string()),
            ("x-iris-sequence", sequence.to_string()),
            ("x-iris-fingerprint", fingerprint),
            ("x-iris-approximate", approximate.to_string()),
            (
                "x-iris-render-backend",
                if gpu_nodes == 0 {
                    "cpu"
                } else if cpu_nodes == 0 {
                    "gpu"
                } else {
                    "mixed"
                }
                .to_string(),
            ),
            ("x-iris-cpu-nodes", cpu_nodes.to_string()),
            ("x-iris-gpu-nodes", gpu_nodes.to_string()),
            ("x-iris-gpu-fallback", fallback.to_string()),
            ("x-iris-soft-proof", is_proof.to_string()),
            ("x-iris-draft", is_draft.to_string()),
        ],
        bytes,
    )
        .into_response())
}

#[utoipa::path(operation_id="editing_preview_histogram",post,path="/api/v1/edits/{id}/histogram",params(("id"=String,Path)),request_body=HistogramRequest,responses((status=200,body=HistogramResponse)))]
pub async fn histogram(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<HistogramRequest>,
) -> ApiResult<HistogramResponse> {
    if !(2..=4096).contains(&request.bins) {
        return Err(anyhow::anyhow!("invalid histogram bin count").into());
    }
    let result = render(
        s,
        id,
        request.render,
        move |job, engine, render_request, context| {
            let result = scope_pixels(job, engine, render_request, context)?;
            let mapped;
            let frame = match request.domain {
                ScopeDomain::LinearRec2020 => result.frame.as_ref(),
                ScopeDomain::EncodedSrgb => {
                    let mut config = job.record.document.output.clone();
                    config.space = OutputSpace::Srgb;
                    mapped = color::output(&result.frame, &config, context)?;
                    &mapped
                }
            };
            let histogram = scopes::histogram(
                frame,
                request.domain,
                request.lower,
                request.upper,
                request.bins as usize,
                context,
            )?;
            Ok(HistogramResponse {
                revision: render_request.revision,
                sequence: render_request.sequence,
                fingerprint: job.record.document.fingerprint()?,
                approximate: result.approximate,
                preview_width: result.frame.width,
                preview_height: result.frame.height,
                histogram,
            })
        },
    )
    .await?;
    Ok(Json(result))
}

#[utoipa::path(operation_id="editing_preview_sample",post,path="/api/v1/edits/{id}/sample",params(("id"=String,Path)),request_body=SampleRequest,responses((status=200,body=SampleResponse)))]
pub async fn sample(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<SampleRequest>,
) -> ApiResult<SampleResponse> {
    let result = render(
        s,
        id,
        request.render,
        move |job, engine, render_request, context| {
            let result = scope_pixels(job, engine, render_request, context)?;
            let rgb = scopes::sample(&result.frame, request.rect, context)?;
            Ok(SampleResponse {
                revision: render_request.revision,
                sequence: render_request.sequence,
                fingerprint: job.record.document.fingerprint()?,
                approximate: result.approximate,
                preview_width: result.frame.width,
                preview_height: result.frame.height,
                domain: "linear_rec2020_d65".into(),
                rgb,
            })
        },
    )
    .await?;
    Ok(Json(result))
}

#[utoipa::path(operation_id="editing_preview_cancel",post,path="/api/v1/edit-render/{session}/cancel",params(("session"=String,Path)),request_body=CancelRequest,responses((status=200,body=EditSuccess)))]
pub async fn cancel(
    State(s): State<AppState>,
    Path(session): Path<String>,
    Json(request): Json<CancelRequest>,
) -> ApiResult<EditSuccess> {
    s.render_engine.cancel_request(&session, request.sequence)?;
    Ok(Json(EditSuccess { ok: true }))
}

/// End a session and release its sequence watermark. Reopening clients should
/// use a new identifier so abandoned requests cannot collide with new work.
#[utoipa::path(operation_id="editing_preview_close",delete,path="/api/v1/edit-render/{session}",params(("session"=String,Path)),responses((status=200,body=EditSuccess)))]
pub async fn close(
    State(s): State<AppState>,
    Path(session): Path<String>,
) -> ApiResult<EditSuccess> {
    s.render_engine.cancel_session(&session)?;
    Ok(Json(EditSuccess { ok: true }))
}
