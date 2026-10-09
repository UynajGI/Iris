use rmcp::model::Tool;
use serde_json::{json, Map, Value};
use utoipa::OpenApi;

#[derive(Clone)]
pub struct Endpoint {
    pub name: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub tool: Tool,
}

pub fn catalog(read_only: bool) -> Vec<Endpoint> {
    let api = serde_json::to_value(iris_daemon::ApiDoc::openapi()).expect("OpenAPI serializes");
    let routes = [
        ("iris_status", "get", "/bootstrap", "Local engine version and capabilities; no GUI or network listener."),
        ("iris_devices", "get", "/devices/gpu", "Enumerate actual local GPU devices. Registration is not proof of GPU inference."),
        ("iris_list_projects", "get", "/projects", "List permitted projects. offset/limit paginate results."),
        ("iris_create_project", "post", "/projects", "Create/open a photo directory. Does not scan or analyze; call start_scan next."),
        ("iris_project", "get", "/projects/{id}", "Read project freshness and pending analysis count."),
        ("iris_open_project", "post", "/projects/{id}/open", "Reopen an existing project by ID."),
        ("iris_hide_project", "post", "/projects/{id}/hide", "Hide a recent project without deleting photographs or project data."),
        ("iris_start_scan", "post", "/projects/{id}/scan", "Start a background scan; returns task ID. Poll job_status. Optional failed-path-only retry."),
        ("iris_start_analysis", "post", "/projects/{id}/analyze", "Start local background inference; returns task ID. Poll job_status. Optional failed-photo-only retry. Never download models implicitly."),
        ("iris_job_status", "get", "/projects/{id}/progress", "Read current/last task ID, progress, failures and device evidence. Check ID when another task was started."),
        ("iris_cancel_job", "post", "/projects/{id}/cancel", "Cancel the active project task, retaining completed work."),
        ("iris_pause_job", "post", "/projects/{id}/pause", "Pause the active project task."),
        ("iris_resume_job", "post", "/projects/{id}/resume", "Resume a paused task."),
        ("iris_list_photos", "get", "/projects/{id}/photos", "List/filter photographs with analysis, marks and freshness. Pagination defaults to 40, maximum 100."),
        ("iris_get_photo", "get", "/photos/{id}", "Read one photograph's technical analysis, marks and freshness. Old analysis is not a current recommendation."),
        ("iris_get_preview", "get", "/photos/{id}/preview", "Return an oriented bounded JPEG preview to the calling agent. Request only when visual review is authorized; cloud agents may transmit it to their model provider."),
        ("iris_get_thumbnail", "get", "/photos/{id}/thumb", "Return a small JPEG thumbnail to the calling agent; same image-sharing boundary as preview."),
        ("iris_get_groups", "get", "/projects/{id}/groups", "Read duplicate/similar groups. offset/limit paginate groups; use member IDs to fetch photos."),
        ("iris_set_decisions", "post", "/projects/{id}/decisions", "Set explicit photo decisions with optional RAW/JPEG variant linking. Recorded as agent:mcp; undo supported."),
        ("iris_set_marks", "post", "/projects/{id}/marks", "Set decision, 0–5 stars and/or fixed color on explicit IDs; omitted fields remain unchanged. Undo supported."),
        ("iris_accept_suggestions", "post", "/projects/{id}/accept", "Adopt current engine suggestions for pending photos; specify photo_ids to limit scope. Does not replace human decisions."),
        ("iris_undo", "post", "/projects/{id}/undo", "Undo the latest project marking session, not arbitrary filesystem operations."),
        ("iris_get_settings", "get", "/settings", "Read all project analysis settings before replacing them."),
        ("iris_set_settings", "put", "/settings", "Replace complete validated settings. Analysis/group freshness may change; analysis never starts automatically."),
        ("iris_model_status", "get", "/models", "Check selected model artifacts, hashes and availability without inference."),
        ("iris_optional_models", "get", "/models/optional", "List optional model installation and license status. Installation is not implicit."),
        ("iris_export_copy", "post", "/projects/{id}/export/copy", "Copy selected scope to an allowed destination. Existing files are skipped; source photos stay in place."),
        ("iris_export_csv", "post", "/projects/{id}/export/csv", "Export all project paths/decisions to an allowed new CSV file. Does not export star/color fields."),
        ("iris_export_xmp", "post", "/projects/{id}/export/xmp", "Write decision XMP sidecars beside originals. Existing sidecars are preserved unless overwrite is explicitly true."),
        ("iris_import_csv", "post", "/projects/{id}/import/csv", "Import decisions from an allowed CSV; validates known paths and duplicates, supports undo."),
        ("iris_quarantine_history", "get", "/projects/{id}/quarantine", "Read recoverable quarantine batches and failures."),
        ("iris_quarantine_preview", "post", "/projects/{id}/quarantine/preview", "Create a persisted removal plan without moving photos. Review plan before commit."),
        ("iris_quarantine_commit", "post", "/projects/{id}/quarantine/commit", "Move only files in a reviewed manifest into recoverable quarantine. Requires explicit confirm:true."),
        ("iris_quarantine_restore", "post", "/projects/{id}/quarantine/restore", "Restore a reviewed quarantine manifest; refuses conflicts and preserves failure records. Requires confirm:true."),
        ("iris_cache_status", "get", "/projects/{id}/cache", "Read cache location and usage."),
        ("iris_cache_cleanup", "post", "/projects/{id}/cache/cleanup", "Remove regenerable project cache only. Requires explicit confirm:true."),
        ("iris_cache_migrate", "post", "/projects/{id}/cache/migrate", "Copy/verify/switch cache to an allowed destination. Old cache cleanup is a separate manifest operation."),
        ("iris_cache_history", "get", "/projects/{id}/cache/migrations", "Read migration manifests before cleaning the old cache."),
        ("iris_cache_cleanup_old", "post", "/projects/{id}/cache/migrations/{migration_id}/cleanup", "Clean only files in the specified old-cache manifest after review. Requires confirm:true."),
        ("iris_list_profiles", "get", "/profiles", "List saved analysis profiles."),
        ("iris_save_profile", "post", "/profiles", "Save a named validated analysis settings profile."),
        ("iris_delete_profile", "delete", "/profiles/{name}", "Delete the specified saved profile, not photographs. Requires confirm:true."),
        ("iris_apply_profile", "post", "/profiles/{name}/apply", "Apply a saved profile to project_id without starting analysis."),
        ("iris_estimate_profile", "post", "/projects/{id}/profiles/{name}/estimate", "Estimate effect of a saved profile from available results without changing project decisions."),
    ];
    routes.into_iter().filter(|(_,m,_,_)| !read_only || *m == "get").map(|(name, method, path, description)| {
        let operation = &api["paths"][format!("/api/v1{path}")][method];
        assert!(operation.is_object(), "missing route {method} {path}");
        let mut props = Map::new();
        let mut required = Vec::new();
        for param in operation["parameters"].as_array().into_iter().flatten() {
            let original = param["name"].as_str().unwrap();
            let key = parameter_name(path, original);
            props.insert(key.to_string(), param["schema"].clone());
            if param["required"] == true { required.push(json!(key)); }
        }
        let body = &operation["requestBody"]["content"]["application/json"]["schema"];
        if body.is_object() {
            props.insert("data".into(), body.clone());
            if operation["requestBody"]["required"] == true { required.push(json!("data")); }
        }
        if matches!(name, "iris_list_photos"|"iris_list_projects"|"iris_get_groups"|"iris_quarantine_history"|"iris_cache_history") {
            props.insert("limit".into(), json!({"type":"integer","minimum":1,"maximum":100,"default":40}));
            props.insert("offset".into(), json!({"type":"integer","minimum":0,"default":0}));
        }
        if method != "get" {
            props.insert("request_id".into(), json!({"type":"string","minLength":1,"maxLength":128,"description":"Unique operation ID. Reuse only to retry exactly the same operation; successful results are replayed without repeating writes."}));
            required.push(json!("request_id"));
        }
        let destructive = requires_confirmation(name);
        if destructive { props.insert("confirm".into(), json!({"type":"boolean","const":true})); required.push(json!("confirm")); }
        let mut schema = json!({"type":"object","properties":props,"required":required,"additionalProperties":false});
        let mut defs = Map::new();
        rewrite_refs(&mut schema, &api["components"]["schemas"], &mut defs);
        if !defs.is_empty() { schema["$defs"] = Value::Object(defs); }
        let tool = serde_json::from_value(json!({"name":name,"description":description,"inputSchema":schema,"annotations":{"readOnlyHint":method=="get","destructiveHint":destructive,"idempotentHint":method=="get","openWorldHint":false}})).expect("valid MCP tool");
        Endpoint { name, method, path, tool }
    }).collect()
}

pub fn parameter_name<'a>(path: &str, name: &'a str) -> &'a str {
    if name == "id" {
        if path.starts_with("/photos/") {
            "photo_id"
        } else {
            "project_id"
        }
    } else {
        name
    }
}
pub fn requires_confirmation(name: &str) -> bool {
    matches!(
        name,
        "iris_quarantine_commit"
            | "iris_quarantine_restore"
            | "iris_cache_cleanup"
            | "iris_cache_cleanup_old"
            | "iris_delete_profile"
    )
}
fn rewrite_refs(value: &mut Value, schemas: &Value, defs: &mut Map<String, Value>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(name) = reference
                    .strip_prefix("#/components/schemas/")
                    .map(str::to_owned)
                {
                    map.insert("$ref".into(), json!(format!("#/$defs/{name}")));
                    if !defs.contains_key(&name) {
                        defs.insert(name.clone(), Value::Null);
                        let mut definition = schemas[&name].clone();
                        rewrite_refs(&mut definition, schemas, defs);
                        defs.insert(name, definition);
                    }
                }
            }
            for child in map.values_mut() {
                rewrite_refs(child, schemas, defs);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_refs(child, schemas, defs);
            }
        }
        _ => {}
    }
}
