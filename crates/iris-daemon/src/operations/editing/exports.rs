// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use iris_core::{
    render::RenderContext,
    services::{RenderedExportRequest, RenderedExportStatus},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tokio::sync::Notify;

#[derive(Default)]
pub(crate) struct ExportQueue {
    started: AtomicBool,
    shutdown: AtomicBool,
    wake: Notify,
    stopped: Notify,
    active: Mutex<Option<(String, RenderContext)>>,
}
impl ExportQueue {
    pub(crate) async fn wait_stopped(&self) {
        while self.started.load(Ordering::Acquire) {
            self.stopped.notified().await;
        }
    }
    pub(crate) fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        if let Ok(active) = self.active.lock() {
            if let Some((_, context)) = active.as_ref() {
                context.cancel();
            }
        }
        self.wake.notify_one();
    }
    fn cancel(&self, id: &str) {
        if let Ok(active) = self.active.lock() {
            if let Some((_, context)) = active.as_ref().filter(|(active, _)| active == id) {
                context.cancel();
            }
        }
    }
}

pub(crate) fn start(state: AppState) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    if state.export_queue.started.swap(true, Ordering::AcqRel) {
        state.export_queue.wake.notify_one();
        return;
    }
    runtime.spawn(async move {
        loop {
            if state.export_queue.shutdown.load(Ordering::Acquire) {
                break;
            }
            let next = run(state.clone(), |s| s.next_queued_rendered_export()).await;
            match next {
                Ok(Some(id)) => {
                    if let Err(error) = execute(state.clone(), id.clone()).await {
                        let message = error.1;
                        let _ = run(state.clone(), move |s| {
                            s.fail_rendered_export(&id, &message)
                        })
                        .await;
                    }
                    if let Ok(mut active) = state.export_queue.active.lock() {
                        *active = None;
                    }
                }
                Ok(None) | Err(_) => state.export_queue.wake.notified().await,
            }
        }
        state.export_queue.started.store(false, Ordering::Release);
        state.export_queue.stopped.notify_one();
    });
}

async fn execute(state: AppState, id: String) -> Result<(), ApiError> {
    // One consumer is the low-priority export lane. It takes at most one of two
    // render permits, leaving another lane available to interactive previews.
    let permit = state
        .render_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(anyhow::Error::from)?;
    if state.export_queue.shutdown.load(Ordering::Acquire) {
        return Ok(());
    }
    let context = state.render_engine.request_context();
    *state
        .export_queue
        .active
        .lock()
        .map_err(|_| anyhow::anyhow!("export queue lock poisoned"))? =
        Some((id.clone(), context.clone()));
    let job = run(state.clone(), move |s| s.prepare_rendered_export(&id)).await?;
    let mut prepared = tokio::task::spawn_blocking(move || job.render(&context))
        .await
        .map_err(anyhow::Error::from)??;
    prepared = run(state.clone(), move |s| {
        s.authorize_rendered_export_publication(&mut prepared)?;
        Ok(prepared)
    })
    .await?;
    // Publication is an authorized, no-overwrite commit. It is deliberately not
    // linked to the pre-publication cancel flag or a disconnected HTTP request.
    let publication_context = state.render_engine.request_context();
    let result = tokio::task::spawn_blocking(move || prepared.publish(&publication_context))
        .await
        .map_err(anyhow::Error::from)??;
    run(state, move |s| s.finish_rendered_export(&result)).await?;
    drop(permit);
    Ok(())
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueExports {
    pub exports: Vec<RenderedExportRequest>,
}

#[utoipa::path(post,path="/api/v1/rendered-exports",request_body=QueueExports,responses((status=200,body=Vec<RenderedExportStatus>)))]
pub async fn queue(
    State(s): State<AppState>,
    Json(request): Json<QueueExports>,
) -> ApiResult<Vec<RenderedExportStatus>> {
    let statuses = run(s.clone(), move |services| {
        services.queue_rendered_exports(&request.exports)
    })
    .await?;
    start(s);
    Ok(Json(statuses))
}
#[utoipa::path(get,path="/api/v1/rendered-exports",params(("limit"=Option<u32>,Query),("offset"=Option<u32>,Query)),responses((status=200,body=Vec<RenderedExportStatus>)))]
pub async fn list(
    State(s): State<AppState>,
    Query(page): Query<EditPage>,
) -> ApiResult<Vec<RenderedExportStatus>> {
    Ok(Json(
        run(s, move |s| s.rendered_exports(page.limit, page.offset)).await?,
    ))
}
#[utoipa::path(get,path="/api/v1/rendered-exports/{id}",params(("id"=String,Path)),responses((status=200,body=RenderedExportStatus)))]
pub async fn status(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<RenderedExportStatus> {
    Ok(Json(run(s, move |s| s.rendered_export(&id)).await?))
}
#[utoipa::path(post,path="/api/v1/rendered-exports/{id}/cancel",params(("id"=String,Path)),responses((status=200,body=RenderedExportStatus)))]
pub async fn cancel(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<RenderedExportStatus> {
    let target = id.clone();
    let status = run(s.clone(), move |s| s.cancel_rendered_export(&target)).await?;
    s.export_queue.cancel(&id);
    Ok(Json(status))
}
#[utoipa::path(post,path="/api/v1/rendered-exports/{id}/retry",params(("id"=String,Path)),responses((status=200,body=RenderedExportStatus)))]
pub async fn retry(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<RenderedExportStatus> {
    let status = run(s.clone(), move |s| s.retry_rendered_export(&id)).await?;
    start(s);
    Ok(Json(status))
}
#[utoipa::path(post,path="/api/v1/rendered-exports/{id}/recover",params(("id"=String,Path)),responses((status=200,body=RenderedExportStatus)))]
pub async fn recover(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<RenderedExportStatus> {
    let target = id.clone();
    let job = run(s.clone(), move |s| {
        s.prepare_rendered_export_recovery(&target)
    })
    .await?;
    let context = s.render_engine.request_context();
    let guard = preview::CancelContext(context.clone());
    let permit = s
        .render_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(anyhow::Error::from)?;
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        job.inspect(&context)
    })
    .await
    .map_err(anyhow::Error::from)??;
    let status = match result {
        Some(result) => run(s, move |s| s.finish_rendered_export(&result)).await?,
        None => run(s, move |s| s.rendered_export(&id)).await?,
    };
    drop(guard);
    Ok(Json(status))
}
#[utoipa::path(delete,path="/api/v1/rendered-exports/{id}",params(("id"=String,Path)),responses((status=200,body=EditSuccess)))]
pub async fn delete(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<EditSuccess> {
    run(s, move |s| s.delete_rendered_export(&id)).await?;
    Ok(Json(EditSuccess { ok: true }))
}
