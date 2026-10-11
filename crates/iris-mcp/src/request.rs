use super::{catalog, policy, IrisMcp};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

impl IrisMcp {
    pub(super) async fn authorize(&self, endpoint: &catalog::Endpoint, args: &Value) -> Result<()> {
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
                self.authorize_cache(&services, endpoint, args, &project.cache_root, id)?;
            }
        }
        self.authorize_data_paths(args)
    }

    fn authorize_data_paths(&self, args: &Value) -> Result<()> {
        for key in ["root", "destination", "source"] {
            if let Some(path) = args["data"][key].as_str() {
                if !path.is_empty() {
                    self.policy.check(Path::new(path))?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn request(
        &self,
        endpoint: &catalog::Endpoint,
        args: &Value,
    ) -> Result<(String, Vec<u8>)> {
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
    fn authorize_cache(
        &self,
        services: &iris_core::Services,
        endpoint: &catalog::Endpoint,
        args: &Value,
        cache_root: &str,
        id: i64,
    ) -> Result<()> {
        let cache = Path::new(cache_root);
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
        Ok(())
    }
}
