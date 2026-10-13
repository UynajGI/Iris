// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{run, ApiError, ApiResult, AppState};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use iris_core::services::{
    BatchEditTarget, EditCommand, EditHistoryEntry, EditPreset, EditRecipe, EditRecord,
    EditResource, EditResourceKind, SaveEdit,
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
mod calibration;
mod exports;
mod preview;
pub(crate) use exports::{start as start_export_worker, ExportQueue};

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EditName {
    pub name: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct EditPage {
    pub limit: u32,
    pub offset: u32,
}
impl Default for EditPage {
    fn default() -> Self {
        Self {
            limit: 20,
            offset: 0,
        }
    }
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EditRevision {
    pub expected_revision: u64,
}
#[derive(Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RecipeQuery {
    pub include_masks: bool,
}
impl Default for RecipeQuery {
    fn default() -> Self {
        Self {
            include_masks: false,
        }
    }
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportRecipe {
    pub command: EditCommand,
    pub recipe: EditRecipe,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchSettings {
    pub targets: Vec<BatchEditTarget>,
    pub request_id: String,
    #[serde(default)]
    pub include_masks: bool,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PresetSettings {
    pub name: String,
    pub recipe: EditRecipe,
}
#[derive(Serialize, ToSchema)]
pub struct EditSuccess {
    pub ok: bool,
}
#[derive(Serialize, ToSchema)]
pub struct EditingCapabilities {
    pub engine_version: String,
    pub document_version: u32,
    pub working_space: String,
    pub working_precision: String,
    pub compute_backend: String,
    pub traditional_masks: bool,
    pub manual_lens_corrections: bool,
    pub preview_formats: Vec<String>,
    pub resource_kinds: Vec<String>,
    pub max_resource_bytes: u64,
    pub render_memory_bytes: u64,
    pub source_cache_bytes: u64,
    pub max_preview_edge: u32,
    pub limitations: Vec<String>,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/rendered-exports",
            get(exports::list).post(exports::queue),
        )
        .route(
            "/api/v1/rendered-exports/{id}",
            get(exports::status).delete(exports::delete),
        )
        .route(
            "/api/v1/rendered-exports/{id}/cancel",
            post(exports::cancel),
        )
        .route("/api/v1/rendered-exports/{id}/retry", post(exports::retry))
        .route(
            "/api/v1/rendered-exports/{id}/recover",
            post(exports::recover),
        )
        .route(
            "/api/v1/photos/{id}/edits",
            get(list_edits).post(create_edit),
        )
        .route(
            "/api/v1/edits/{id}",
            get(get_edit).put(save_edit).delete(delete_edit),
        )
        .route("/api/v1/edits/{id}/duplicate", post(duplicate_edit))
        .route("/api/v1/edits/{id}/undo", post(undo_edit))
        .route("/api/v1/edits/{id}/redo", post(redo_edit))
        .route("/api/v1/edits/{id}/history", get(edit_history))
        .route(
            "/api/v1/edits/{id}/recipe",
            get(export_recipe).put(import_recipe),
        )
        .route("/api/v1/edits/{id}/batch", post(batch_settings))
        .route(
            "/api/v1/edit-presets",
            get(list_presets).post(create_preset),
        )
        .route(
            "/api/v1/edit-presets/{id}",
            get(get_preset).put(update_preset).delete(delete_preset),
        )
        .route(
            "/api/v1/edit-resources/import/{kind}",
            post(import_resource).layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .route("/api/v1/edit-resources/{hash}", get(resource_bytes))
        .route("/api/v1/editing/capabilities", get(capabilities))
        .route("/api/v1/edits/{id}/preview", post(preview::preview))
        .route(
            "/api/v1/edits/{id}/preview-draft",
            post(preview::draft_preview),
        )
        .route("/api/v1/edits/{id}/proof", post(preview::proof))
        .route("/api/v1/edits/{id}/region", post(preview::region))
        .route(
            "/api/v1/edits/{id}/lens-profile",
            post(calibration::lens_profile),
        )
        .route(
            "/api/v1/edits/{id}/raw-white-balance",
            post(calibration::raw_white_balance),
        )
        .route("/api/v1/edits/{id}/histogram", post(preview::histogram))
        .route("/api/v1/edits/{id}/sample", post(preview::sample))
        .route(
            "/api/v1/edit-render/{session}/cancel",
            post(preview::cancel),
        )
        .route(
            "/api/v1/edit-render/{session}",
            axum::routing::delete(preview::close),
        )
}

#[utoipa::path(operation_id="editing_capabilities",get,path="/api/v1/editing/capabilities",responses((status=200,body=EditingCapabilities)))]
pub async fn capabilities(State(state): State<AppState>) -> ApiResult<EditingCapabilities> {
    Ok(Json(EditingCapabilities {
        engine_version: iris_core::render::ENGINE_VERSION.into(),
        document_version: iris_core::render::document::DOCUMENT_VERSION,
        working_space: "linear_rec2020_d65".into(),
        working_precision: "fp32".into(),
        compute_backend: "per_render_execution_headers".into(),
        traditional_masks: true,
        manual_lens_corrections: true,
        preview_formats: vec!["image/jpeg".into()],
        resource_kinds: vec!["lut".into(), "icc".into(), "lens_profile".into()],
        max_resource_bytes: 32 * 1024 * 1024,
        render_memory_bytes: state.render_memory_bytes,
        source_cache_bytes: state.source_cache_bytes,
        max_preview_edge: 8192,
        limitations: vec![
            "Native region previews return 1:1 edited pixels (maximum 2048 per edge and 4 million pixels), currently evaluating the full frame before cropping.".into(),
            "Warm interactive source checks use file metadata; metadata-preserving external edits require Full quality or explicit source-validation refresh for a full SHA-256 check.".into(),
            "Operations exceeding the configured render memory budget fail explicitly; increase IRIS_RENDER_MEMORY_MIB (256..65536) when needed.".into(),
            "GPU acceleration is limited to the supported exposure fast path; other operations use CPU.".into(),
            "RAW float development supports standard RGB 2x2 Bayer sensors up to 64 megapixels and 512 MiB compressed input, with PPG demosaicing and explicit D65 calibration; X-Trans, non-Bayer and complex DNG calibration/opcodes are unsupported.".into(),
            "Lens corrections use manual parameters or explicitly imported Iris calibration JSON; EXIF matching only proposes optical coefficients. No Lensfun database or automatic profile download is included.".into(),
            "Embedded input ICC must be RGB. Proof resource profiles are separately validated for supported proof color spaces.".into(),
            "RAW worker memory includes decoder scratch outside the render buffer budget; real-camera fidelity and performance remain unverified.".into(),
            "Exported pixels are orientation-normalized; source EXIF metadata is not retained.".into(),
        ],
    }))
}
#[utoipa::path(operation_id="editing_create_edit",post,path="/api/v1/photos/{id}/edits",params(("id"=i64,Path)),request_body=EditName,responses((status=200,body=EditRecord)))]
pub async fn create_edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<EditName>,
) -> ApiResult<EditRecord> {
    let job = run(state.clone(), move |s| s.prepare_edit_source(id)).await?;
    let context = state.render_engine.request_context();
    let guard = preview::CancelContext(context.clone());
    let permit = state
        .render_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(anyhow::Error::from)?;
    let source = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        job.decode(&context)
    })
    .await
    .map_err(anyhow::Error::from)??;
    let binding = source.binding;
    drop(source.frame);
    let result = run(state, move |s| {
        s.create_edit_document(id, &body.name, binding)
    })
    .await?;
    drop(guard);
    Ok(Json(result))
}
#[utoipa::path(operation_id="editing_list_edits",get,path="/api/v1/photos/{id}/edits",params(("id"=i64,Path),("limit"=Option<u32>,Query),("offset"=Option<u32>,Query)),responses((status=200,body=Vec<EditRecord>)))]
pub async fn list_edits(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(page): Query<EditPage>,
) -> ApiResult<Vec<EditRecord>> {
    Ok(Json(
        run(s, move |s| s.edit_documents(id, page.limit, page.offset)).await?,
    ))
}
#[utoipa::path(operation_id="editing_get_edit",get,path="/api/v1/edits/{id}",params(("id"=String,Path)),responses((status=200,body=EditRecord)))]
pub async fn get_edit(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<EditRecord> {
    Ok(Json(run(s, move |s| s.edit_document(&id)).await?))
}
#[utoipa::path(operation_id="editing_save_edit",put,path="/api/v1/edits/{id}",params(("id"=String,Path)),request_body=SaveEdit,responses((status=200,body=EditRecord)))]
pub async fn save_edit(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SaveEdit>,
) -> ApiResult<EditRecord> {
    Ok(Json(
        run(s, move |s| s.save_edit_document(&id, body)).await?,
    ))
}
#[utoipa::path(operation_id="editing_delete_edit",delete,path="/api/v1/edits/{id}",params(("id"=String,Path)),request_body=EditRevision,responses((status=200,body=EditSuccess)))]
pub async fn delete_edit(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<EditRevision>,
) -> ApiResult<EditSuccess> {
    run(s, move |s| {
        s.delete_edit_document(&id, body.expected_revision)
    })
    .await?;
    Ok(Json(EditSuccess { ok: true }))
}
#[utoipa::path(operation_id="editing_duplicate_edit",post,path="/api/v1/edits/{id}/duplicate",params(("id"=String,Path)),request_body=EditName,responses((status=200,body=EditRecord)))]
pub async fn duplicate_edit(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<EditName>,
) -> ApiResult<EditRecord> {
    Ok(Json(
        run(s, move |s| s.duplicate_edit_document(&id, &body.name)).await?,
    ))
}
#[utoipa::path(operation_id="editing_undo_edit",post,path="/api/v1/edits/{id}/undo",params(("id"=String,Path)),request_body=EditCommand,responses((status=200,body=EditRecord)))]
pub async fn undo_edit(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<EditCommand>,
) -> ApiResult<EditRecord> {
    Ok(Json(
        run(s, move |s| s.undo_edit_document(&id, body)).await?,
    ))
}
#[utoipa::path(operation_id="editing_redo_edit",post,path="/api/v1/edits/{id}/redo",params(("id"=String,Path)),request_body=EditCommand,responses((status=200,body=EditRecord)))]
pub async fn redo_edit(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<EditCommand>,
) -> ApiResult<EditRecord> {
    Ok(Json(
        run(s, move |s| s.redo_edit_document(&id, body)).await?,
    ))
}
#[utoipa::path(operation_id="editing_edit_history",get,path="/api/v1/edits/{id}/history",params(("id"=String,Path),("limit"=Option<u32>,Query),("offset"=Option<u32>,Query)),responses((status=200,body=Vec<EditHistoryEntry>)))]
pub async fn edit_history(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(page): Query<EditPage>,
) -> ApiResult<Vec<EditHistoryEntry>> {
    Ok(Json(
        run(s, move |s| s.edit_history(&id, page.limit, page.offset)).await?,
    ))
}
#[utoipa::path(operation_id="editing_export_recipe",get,path="/api/v1/edits/{id}/recipe",params(("id"=String,Path),("include_masks"=Option<bool>,Query)),responses((status=200,body=EditRecipe)))]
pub async fn export_recipe(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
) -> ApiResult<EditRecipe> {
    Ok(Json(
        run(s, move |s| s.export_edit_recipe(&id, query.include_masks)).await?,
    ))
}
#[utoipa::path(operation_id="editing_import_recipe",put,path="/api/v1/edits/{id}/recipe",params(("id"=String,Path)),request_body=ImportRecipe,responses((status=200,body=EditRecord)))]
pub async fn import_recipe(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ImportRecipe>,
) -> ApiResult<EditRecord> {
    Ok(Json(
        run(s, move |s| {
            s.import_edit_recipe(&id, body.command, &serde_json::to_string(&body.recipe)?)
        })
        .await?,
    ))
}
#[utoipa::path(operation_id="editing_batch_settings",post,path="/api/v1/edits/{id}/batch",params(("id"=String,Path)),request_body=BatchSettings,responses((status=200,body=Vec<EditRecord>)))]
pub async fn batch_settings(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<BatchSettings>,
) -> ApiResult<Vec<EditRecord>> {
    Ok(Json(
        run(s, move |s| {
            s.batch_copy_edit_settings(&id, &body.targets, &body.request_id, body.include_masks)
        })
        .await?,
    ))
}
#[utoipa::path(operation_id="editing_list_presets",get,path="/api/v1/edit-presets",params(("limit"=Option<u32>,Query),("offset"=Option<u32>,Query)),responses((status=200,body=Vec<EditPreset>)))]
pub async fn list_presets(
    State(s): State<AppState>,
    Query(page): Query<EditPage>,
) -> ApiResult<Vec<EditPreset>> {
    Ok(Json(
        run(s, move |s| s.edit_presets(page.limit, page.offset)).await?,
    ))
}
#[utoipa::path(operation_id="editing_get_preset",get,path="/api/v1/edit-presets/{id}",params(("id"=String,Path)),responses((status=200,body=EditPreset)))]
pub async fn get_preset(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<EditPreset> {
    Ok(Json(run(s, move |s| s.edit_preset(&id)).await?))
}
#[utoipa::path(operation_id="editing_create_preset",post,path="/api/v1/edit-presets",request_body=PresetSettings,responses((status=200,body=EditPreset)))]
pub async fn create_preset(
    State(s): State<AppState>,
    Json(body): Json<PresetSettings>,
) -> ApiResult<EditPreset> {
    Ok(Json(
        run(s, move |s| {
            s.save_edit_preset(None, &body.name, body.recipe)
        })
        .await?,
    ))
}
#[utoipa::path(operation_id="editing_update_preset",put,path="/api/v1/edit-presets/{id}",params(("id"=String,Path)),request_body=PresetSettings,responses((status=200,body=EditPreset)))]
pub async fn update_preset(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PresetSettings>,
) -> ApiResult<EditPreset> {
    Ok(Json(
        run(s, move |s| {
            s.save_edit_preset(Some(&id), &body.name, body.recipe)
        })
        .await?,
    ))
}
#[utoipa::path(operation_id="editing_delete_preset",delete,path="/api/v1/edit-presets/{id}",params(("id"=String,Path)),responses((status=200,body=EditSuccess)))]
pub async fn delete_preset(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<EditSuccess> {
    run(s, move |s| s.delete_edit_preset(&id)).await?;
    Ok(Json(EditSuccess { ok: true }))
}
#[utoipa::path(operation_id="editing_import_resource",post,path="/api/v1/edit-resources/import/{kind}",params(("kind"=EditResourceKind,Path)),request_body(content=Vec<u8>,content_type="application/octet-stream"),responses((status=200,body=EditResource),(status=413,description="Resource exceeds byte limit")))]
pub async fn import_resource(
    State(s): State<AppState>,
    Path(kind): Path<EditResourceKind>,
    bytes: Bytes,
) -> ApiResult<EditResource> {
    Ok(Json(
        run(s, move |s| s.import_edit_resource(kind, &bytes)).await?,
    ))
}
#[utoipa::path(operation_id="editing_resource_bytes",get,path="/api/v1/edit-resources/{hash}",params(("hash"=String,Path)),responses((status=200,body=Vec<u8>,content_type="application/octet-stream")))]
pub async fn resource_bytes(
    State(s): State<AppState>,
    Path(hash): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    use axum::response::IntoResponse;
    let bytes = run(s, move |s| s.read_edit_resource(&hash)).await?;
    Ok((
        [
            ("content-type", "application/octet-stream"),
            ("cache-control", "no-store"),
        ],
        bytes,
    )
        .into_response())
}

#[derive(OpenApi)]
#[openapi(
    components(schemas(iris_core::render::lens_profile::LensProfile)),
    paths(
        capabilities,
        create_edit,
        list_edits,
        get_edit,
        save_edit,
        delete_edit,
        duplicate_edit,
        undo_edit,
        redo_edit,
        edit_history,
        export_recipe,
        import_recipe,
        batch_settings,
        list_presets,
        get_preset,
        create_preset,
        update_preset,
        delete_preset,
        import_resource,
        resource_bytes,
        preview::preview,
        preview::draft_preview,
        preview::proof,
        preview::region,
        calibration::lens_profile,
        calibration::raw_white_balance,
        preview::histogram,
        preview::sample,
        preview::cancel,
        preview::close,
        exports::queue,
        exports::list,
        exports::status,
        exports::cancel,
        exports::retry,
        exports::recover,
        exports::delete
    )
)]
pub(crate) struct EditingApiDoc;
