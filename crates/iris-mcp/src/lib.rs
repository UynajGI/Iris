mod catalog;
mod journal;
mod policy;
#[cfg(test)]
mod tests;

use anyhow::{Context, Result};
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request},
    Router,
};
use base64::Engine;
use iris_daemon::AppState;
use rmcp::{model::*, service::RequestContext, ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Map, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tower::ServiceExt;
use utoipa::OpenApi;

#[derive(Clone)]
pub struct IrisMcp {
    pub state: AppState,
    router: Router,
    endpoints: Arc<Vec<catalog::Endpoint>>,
    api: Arc<Value>,
    policy: policy::Policy,
    data_dir: PathBuf,
    journal: Arc<tokio::sync::Mutex<journal::Journal>>,
}

impl IrisMcp {
    pub fn new(
        mut state: AppState,
        data_dir: &Path,
        roots: &[PathBuf],
        read_only: bool,
    ) -> Result<Self> {
        state.decision_source = "agent:mcp".into();
        Ok(Self {
            router: iris_daemon::router(state.clone()),
            state,
            endpoints: Arc::new(catalog::catalog(read_only)),
            api: Arc::new(serde_json::to_value(iris_daemon::ApiDoc::openapi())?),
            policy: policy::Policy::new(roots)?,
            data_dir: data_dir.canonicalize()?,
            journal: Arc::new(tokio::sync::Mutex::new(journal::Journal::open(
                &data_dir.join("mcp-audit.jsonl"),
            )?)),
        })
    }

    /// The only call path: MCP validates inputs, then delegates to the existing
    /// authenticated router in-process. No TCP listener or duplicate job engine.
    async fn invoke(&self, name: &str, arguments: Map<String, Value>) -> Result<CallToolResult> {
        let endpoint = self
            .endpoints
            .iter()
            .find(|e| e.name == name)
            .context("unknown or disabled tool")?;
        let arguments = Value::Object(arguments);
        let schema = serde_json::to_value(&endpoint.tool)?;
        let properties = schema["inputSchema"]["properties"]
            .as_object()
            .context("tool schema missing")?;
        for key in arguments.as_object().unwrap().keys() {
            anyhow::ensure!(properties.contains_key(key), "unknown argument {key}");
        }
        for key in schema["inputSchema"]["required"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            anyhow::ensure!(arguments.get(key).is_some(), "missing argument {key}");
        }
        for key in ["project_id", "photo_id"] {
            if let Some(id) = arguments.get(key) {
                anyhow::ensure!(
                    id.as_i64().is_some_and(|id| id > 0),
                    "{key} must be a positive integer"
                );
            }
        }
        if let Some(limit) = arguments.get("limit") {
            anyhow::ensure!(
                limit.as_u64().is_some_and(|v| (1..=100).contains(&v)),
                "limit must be an integer between 1 and 100"
            );
        }
        if let Some(offset) = arguments.get("offset") {
            anyhow::ensure!(
                offset.as_u64().is_some_and(|v| v <= i64::MAX as u64),
                "offset must be a nonnegative integer"
            );
        }
        if let Some(ids) = arguments["data"].get("photo_ids") {
            anyhow::ensure!(
                ids.is_null() || ids.as_array().is_some_and(|ids| ids.len() <= 1000),
                "mark at most 1000 explicit photo IDs per request"
            );
        }
        let mut journal = if endpoint.method != "get" {
            Some(self.journal.lock().await)
        } else {
            None
        };
        let request_id = if journal.is_some() {
            let id = arguments["request_id"]
                .as_str()
                .context("request_id is required for writes")?;
            anyhow::ensure!(!id.is_empty() && id.len() <= 128, "invalid request_id");
            Some(id)
        } else {
            None
        };
        if catalog::requires_confirmation(name) {
            anyhow::ensure!(
                arguments["confirm"] == true,
                "review the plan and explicitly set confirm:true"
            );
        }
        self.authorize(endpoint, &arguments).await?;
        if let (Some(journal), Some(id)) = (journal.as_ref(), request_id) {
            if let Some(result) = journal.existing(id, name, &arguments)? {
                return Ok(serde_json::from_value(result)?);
            }
        }
        let (uri, body) = self.request(endpoint, &arguments)?;
        if let (Some(journal), Some(id)) = (journal.as_mut(), request_id) {
            journal.append(id, name, &arguments, None)?;
        }
        let request = Request::builder()
            .method(Method::from_bytes(
                endpoint.method.to_uppercase().as_bytes(),
            )?)
            .uri(uri)
            .header("authorization", format!("Bearer {}", self.state.token))
            .header("content-type", "application/json")
            .body(Body::from(body))?;
        let response = self.router.clone().oneshot(request).await?;
        let status = response.status();
        let mime = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_owned();
        let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .context("response exceeds MCP result limit; narrow the requested scope")?;
        let result = if !status.is_success() {
            CallToolResult::error(vec![ContentBlock::text(
                String::from_utf8_lossy(&bytes).to_string(),
            )])
        } else if mime.starts_with("image/") {
            serde_json::from_value(
                json!({"content":[{"type":"image","mimeType":mime,"data":base64::engine::general_purpose::STANDARD.encode(&bytes)}],"isError":false}),
            )?
        } else {
            let mut value: Value = serde_json::from_slice(&bytes)?;
            if name == "iris_list_projects" {
                if let Some(items) = value.as_array_mut() {
                    items.retain(|p| {
                        p["root"]
                            .as_str()
                            .is_some_and(|r| self.policy.check(Path::new(r)).is_ok())
                    });
                }
            }
            if matches!(
                name,
                "iris_list_projects"
                    | "iris_get_groups"
                    | "iris_quarantine_history"
                    | "iris_cache_history"
            ) {
                if let Some(items) = value.as_array() {
                    let offset = arguments["offset"].as_u64().unwrap_or(0) as usize;
                    let limit = arguments["limit"].as_u64().unwrap_or(40).clamp(1, 100) as usize;
                    let total = items.len();
                    let page = items
                        .iter()
                        .skip(offset)
                        .take(limit)
                        .cloned()
                        .collect::<Vec<_>>();
                    let next = (offset.saturating_add(page.len()) < total)
                        .then_some(offset.saturating_add(page.len()));
                    value = json!({"items":page,"total":total,"next_offset":next});
                }
            }
            CallToolResult::structured(json!({"result":value}))
        };
        if let (Some(journal), Some(id)) = (journal.as_mut(), request_id) {
            journal.append(id, name, &arguments, Some(serde_json::to_value(&result)?))?;
        }
        Ok(result)
    }

    async fn authorize(&self, endpoint: &catalog::Endpoint, args: &Value) -> Result<()> {
        let services = self
            .state
            .services
            .lock()
            .map_err(|_| anyhow::anyhow!("service lock poisoned"))?;
        let project_id = if let Some(photo_id) = args["photo_id"].as_i64() {
            let photo = services.photo(photo_id)?;
            let project = services.project(photo.project_id)?;
            self.policy
                .check(&Path::new(&project.root).join(&photo.path))?;
            Some(photo.project_id)
        } else {
            args["project_id"]
                .as_i64()
                .or_else(|| args["data"]["project_id"].as_i64())
        };
        if let Some(id) = project_id {
            let project = services.project(id)?;
            self.policy.check(Path::new(&project.root))?;
            if endpoint.path.contains("/cache") {
                let cache = Path::new(&project.cache_root);
                let private_data = policy::Policy::new(&[self.data_dir.clone()])?;
                if private_data.check(cache).is_err() {
                    self.policy.check(cache)?;
                }
                if endpoint.name == "iris_cache_cleanup_old" {
                    let migration_id = args["migration_id"]
                        .as_str()
                        .context("migration_id required")?;
                    let migrations = services.cache_migrations(id)?;
                    let migration = migrations
                        .iter()
                        .find(|m| m.id == migration_id)
                        .context("migration not found")?;
                    let source = Path::new(&migration.source_root);
                    if private_data.check(source).is_err() {
                        self.policy.check(source)?;
                    }
                }
            }
        }
        for key in ["root", "destination", "source"] {
            if let Some(path) = args["data"][key].as_str() {
                if !path.is_empty() {
                    self.policy.check(Path::new(path))?;
                }
            }
        }
        Ok(())
    }

    fn request(&self, endpoint: &catalog::Endpoint, args: &Value) -> Result<(String, Vec<u8>)> {
        let operation = &self.api["paths"][format!("/api/v1{}", endpoint.path)][endpoint.method];
        let mut path = format!("/api/v1{}", endpoint.path);
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        for param in operation["parameters"].as_array().into_iter().flatten() {
            let name = param["name"].as_str().context("parameter name missing")?;
            let key = catalog::parameter_name(endpoint.path, name);
            let value = &args[key];
            if value.is_null() {
                anyhow::ensure!(param["required"] != true, "missing parameter {key}");
                continue;
            }
            let text = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            if param["in"] == "path" {
                let encoded: String =
                    url::form_urlencoded::byte_serialize(text.as_bytes()).collect();
                path = path.replace(&format!("{{{name}}}"), &encoded.replace('+', "%20"));
            } else {
                if endpoint.name != "iris_list_photos" || name != "limit" {
                    query.append_pair(name, &text);
                }
            }
        }
        if endpoint.name == "iris_list_photos" {
            query.append_pair(
                "limit",
                &args["limit"]
                    .as_u64()
                    .unwrap_or(40)
                    .clamp(1, 100)
                    .to_string(),
            );
        }
        let query = query.finish();
        if !query.is_empty() {
            path.push('?');
            path.push_str(&query);
        }
        let body = if args.get("data").is_some() {
            serde_json::to_vec(&args["data"])?
        } else {
            Vec::new()
        };
        Ok((path, body))
    }
}

impl ServerHandler for IrisMcp {
    fn get_info(&self) -> ServerConfig {
        let mut config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        config.server_info.name = "iris-mcp".into();
        config.server_info.version = env!("CARGO_PKG_VERSION").into();
        config.instructions = Some("Iris local photo culling. Create/open project, start scan, poll job_status, start analysis, poll, review paged results/previews, explicitly mark then export. Mutations require unique request_id; exact retries replay their result. No GUI. Local inference does not upload images, but previews returned to your agent may go to its model provider. No implicit model downloads. Analysis suggestions and manual/agent decisions are distinct. Never treat stale analysis or registered GPU providers as verified current results.".into());
        config
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.endpoints.iter().map(|e| e.tool.clone()).collect(),
            ..Default::default()
        })
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.endpoints
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.tool.clone())
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let result = match self
            .invoke(&request.name, request.arguments.unwrap_or_default())
            .await
        {
            Ok(result) => result,
            Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        };
        Ok(result.into())
    }
}
