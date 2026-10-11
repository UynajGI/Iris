use super::{catalog, IrisMcp};
use anyhow::{Context, Result};
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use base64::Engine;
use rmcp::model::*;
use serde_json::{json, Map, Value};
use std::path::Path;
use tower::ServiceExt;

impl IrisMcp {
    /// The only call path: MCP validates inputs, then delegates to the existing
    /// authenticated router in-process. No TCP listener or duplicate job engine.
    pub(super) async fn invoke(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<CallToolResult> {
        let endpoint = self
            .endpoints
            .iter()
            .find(|e| e.name == name)
            .context("unknown or disabled tool")?;
        let arguments = Value::Object(arguments);
        validate_schema(endpoint, &arguments)?;
        validate_bounds(&arguments)?;
        let mut journal = if endpoint.method != "get" {
            Some(self.journal.lock().await)
        } else {
            None
        };
        let request_id = write_request_id(name, &arguments, journal.is_some())?;
        self.authorize(endpoint, &arguments).await?;
        if let (Some(journal), Some(id)) = (journal.as_ref(), request_id) {
            if let Some(result) = journal.existing(id, name, &arguments)? {
                return Ok(serde_json::from_value(result)?);
            }
        }
        let (uri, body) = self.request(endpoint, &arguments)?;
        append_audit(journal.as_deref_mut(), request_id, name, &arguments, None)?;
        let result = self.dispatch(endpoint, name, &arguments, uri, body).await?;
        append_audit(
            journal.as_deref_mut(),
            request_id,
            name,
            &arguments,
            Some(&result),
        )?;
        Ok(result)
    }

    async fn dispatch(
        &self,
        endpoint: &catalog::Endpoint,
        name: &str,
        arguments: &Value,
        uri: String,
        body: Vec<u8>,
    ) -> Result<CallToolResult> {
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
        self.response(name, arguments, status, &mime, &bytes)
    }

    fn response(
        &self,
        name: &str,
        arguments: &Value,
        status: StatusCode,
        mime: &str,
        bytes: &[u8],
    ) -> Result<CallToolResult> {
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
            value = paginate(name, arguments, value);
            CallToolResult::structured(json!({"result":value}))
        };
        Ok(result)
    }
}

fn append_audit(
    journal: Option<&mut super::journal::Journal>,
    id: Option<&str>,
    name: &str,
    arguments: &Value,
    result: Option<&CallToolResult>,
) -> Result<()> {
    if let (Some(journal), Some(id)) = (journal, id) {
        let result = result.map(serde_json::to_value).transpose()?;
        journal.append(id, name, arguments, result)?;
    }
    Ok(())
}

fn write_request_id<'a>(
    name: &str,
    arguments: &'a Value,
    writing: bool,
) -> Result<Option<&'a str>> {
    let request_id = if writing {
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
    Ok(request_id)
}

fn validate_schema(endpoint: &catalog::Endpoint, arguments: &Value) -> Result<()> {
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
    Ok(())
}

fn validate_bounds(arguments: &Value) -> Result<()> {
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
    validate_photo_ids(arguments)
}

fn validate_photo_ids(arguments: &Value) -> Result<()> {
    if let Some(ids) = arguments["data"].get("photo_ids") {
        anyhow::ensure!(
            ids.is_null() || ids.as_array().is_some_and(|ids| ids.len() <= 1000),
            "mark at most 1000 explicit photo IDs per request"
        );
    }
    Ok(())
}

fn paginate(name: &str, arguments: &Value, mut value: Value) -> Value {
    if matches!(
        name,
        "iris_list_projects" | "iris_get_groups" | "iris_quarantine_history" | "iris_cache_history"
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
    value
}
