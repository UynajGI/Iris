// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use iris_core::{
    render::RenderContext,
    services::{LensProfileSuggestion, PreparedRawWhiteBalance, RawWhiteBalanceRequest, Services},
};

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LensSuggestionRequest {
    pub expected_revision: u64,
    pub resource: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PrepareWhiteBalanceRequest {
    pub expected_revision: u64,
    pub request: RawWhiteBalanceRequest,
}

/// Preparation never writes an edit. The caller saves chosen parameters using
/// the existing expected-revision edit command after reviewing this response.
#[utoipa::path(operation_id="editing_calibration_lens_profile",post,path="/api/v1/edits/{id}/lens-profile",params(("id"=String,Path)),request_body=LensSuggestionRequest,responses((status=200,body=LensProfileSuggestion)))]
pub async fn lens_profile(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<LensSuggestionRequest>,
) -> ApiResult<LensProfileSuggestion> {
    Ok(Json(
        run_calibration(
            s,
            move |services| {
                services.prepare_lens_profile_suggestion(
                    &id,
                    request.expected_revision,
                    &request.resource,
                )
            },
            |job, context| job.suggest(context),
        )
        .await?,
    ))
}

/// RAW sensor gains, not a rendered RGB filter. Neutral sampling uses the
/// oriented/cropped decoded source coordinates before any edit geometry.
#[utoipa::path(operation_id="editing_calibration_raw_white_balance",post,path="/api/v1/edits/{id}/raw-white-balance",params(("id"=String,Path)),request_body=PrepareWhiteBalanceRequest,responses((status=200,body=PreparedRawWhiteBalance)))]
pub async fn raw_white_balance(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<PrepareWhiteBalanceRequest>,
) -> ApiResult<PreparedRawWhiteBalance> {
    Ok(Json(
        run_calibration(
            s,
            move |services| services.prepare_raw_white_balance(&id, request.expected_revision),
            move |job, context| job.prepare(context, &request.request),
        )
        .await?,
    ))
}

async fn run_calibration<J: Send + 'static, T: Send + 'static>(
    state: AppState,
    prepare: impl FnOnce(&mut Services) -> anyhow::Result<J> + Send + 'static,
    finish: impl FnOnce(J, &RenderContext) -> anyhow::Result<T> + Send + 'static,
) -> Result<T, ApiError> {
    let context = state.render_context();
    let _cancel = super::preview::CancelContext(context.clone());
    let permit = state
        .render_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(anyhow::Error::from)?;
    context.check_cancelled()?;
    let job = run(state, prepare).await?;
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        context.check_cancelled()?;
        finish(job, &context)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(result)
}
