mod contracts;
mod jobs;
pub use contracts::{ModelStatusResponse, ProjectRequest};
use jobs::*;
mod scan;
use scan::*;
pub use scan::{__path_scan, scan};
mod analysis;
use analysis::*;
pub use analysis::{__path_analyze, analyze, analyze_project};
mod semantic;
use semantic::*;
mod grouping;
use grouping::*;
mod files;
pub use files::{
    __path_export_copy, __path_export_csv, __path_export_xmp, __path_import_csv,
    __path_quarantine_commit, __path_quarantine_history, __path_quarantine_preview,
    __path_quarantine_restore, export_copy, export_csv, export_xmp, import_csv, quarantine_commit,
    quarantine_history, quarantine_preview, quarantine_restore,
};
mod models;
pub use models::{__path_models, models, models_status};
mod settings;
pub use settings::{
    __path_apply_profile, __path_delete_profile, __path_estimate_profile, __path_profiles,
    __path_put_settings, __path_save_profile, __path_settings, apply_profile, delete_profile,
    estimate_profile, profiles, put_settings, save_profile, settings,
};
mod cache;
pub use cache::{
    __path_cache_cleanup, __path_cache_cleanup_old, __path_cache_migrate, __path_cache_migrations,
    __path_cache_status, cache_cleanup, cache_cleanup_old, cache_migrate, cache_migrations,
    cache_status,
};
mod media;
pub use media::{__path_original, __path_preview, __path_thumb, original, preview, thumb};

use super::*;
use crate::worker::AnalysisWorker;
use iris_core::vision::AnalysisSettings;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{id}/scan", post(scan))
        .route("/api/v1/projects/{id}/analyze", post(analyze))
        .route("/api/v1/projects/{id}/export/xmp", post(export_xmp))
        .route("/api/v1/projects/{id}/export/copy", post(export_copy))
        .route("/api/v1/projects/{id}/export/csv", post(export_csv))
        .route("/api/v1/projects/{id}/import/csv", post(import_csv))
        .route("/api/v1/projects/{id}/quarantine", get(quarantine_history))
        .route(
            "/api/v1/projects/{id}/quarantine/preview",
            post(quarantine_preview),
        )
        .route(
            "/api/v1/projects/{id}/quarantine/commit",
            post(quarantine_commit),
        )
        .route(
            "/api/v1/projects/{id}/quarantine/restore",
            post(quarantine_restore),
        )
        .route("/api/v1/settings", get(settings).put(put_settings))
        .route("/api/v1/models", get(models))
        .route("/api/v1/profiles", get(profiles).post(save_profile))
        .route(
            "/api/v1/profiles/{name}",
            axum::routing::delete(delete_profile),
        )
        .route("/api/v1/profiles/{name}/apply", post(apply_profile))
        .route(
            "/api/v1/projects/{id}/profiles/{name}/estimate",
            post(estimate_profile),
        )
        .route("/api/v1/projects/{id}/cache", get(cache_status))
        .route("/api/v1/projects/{id}/cache/cleanup", post(cache_cleanup))
        .route("/api/v1/projects/{id}/cache/migrate", post(cache_migrate))
        .route(
            "/api/v1/projects/{id}/cache/migrations",
            get(cache_migrations),
        )
        .route(
            "/api/v1/projects/{id}/cache/migrations/{migration_id}/cleanup",
            post(cache_cleanup_old),
        )
        .route("/api/v1/photos/{id}/thumb", get(thumb))
        .route("/api/v1/photos/{id}/preview", get(preview))
        .route("/api/v1/photos/{id}/original", get(original))
}
