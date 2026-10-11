mod catalog;
mod invocation;
mod journal;
mod policy;
mod request;
#[cfg(test)]
mod tests;

use anyhow::Result;
use axum::Router;
use iris_daemon::AppState;
use rmcp::{model::*, service::RequestContext, ErrorData as McpError, RoleServer, ServerHandler};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
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
