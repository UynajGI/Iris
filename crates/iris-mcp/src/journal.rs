use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
};

pub struct Journal {
    file: File,
    entries: HashMap<String, Value>,
}
impl Journal {
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)?;
        let mut entries = HashMap::new();
        for line in BufReader::new(file.try_clone()?).lines() {
            let entry: Value = serde_json::from_str(&line?).context(
                "invalid MCP audit journal; preserve and inspect it before resuming writes",
            )?;
            let id = entry["request_id"]
                .as_str()
                .context("audit request ID missing")?
                .to_owned();
            entries.insert(id, entry);
        }
        Ok(Self { file, entries })
    }
    pub fn existing(&self, id: &str, name: &str, arguments: &Value) -> Result<Option<Value>> {
        let Some(entry) = self.entries.get(id) else {
            return Ok(None);
        };
        anyhow::ensure!(
            entry["tool"] == name && entry["arguments"] == *arguments,
            "request_id was already used for a different operation"
        );
        Ok(Some(entry.get("result").cloned().context("previous operation has an unknown outcome; inspect project/files before issuing a new request_id")?))
    }
    pub fn append(
        &mut self,
        id: &str,
        name: &str,
        arguments: &Value,
        result: Option<Value>,
    ) -> Result<()> {
        let mut entry =
            json!({"request_id":id,"tool":name,"arguments":arguments,"source":"agent:mcp"});
        if let Some(result) = result {
            entry["result"] = result;
        }
        serde_json::to_writer(&mut self.file, &entry)?;
        self.file.write_all(b"\n")?;
        self.file.sync_all()?;
        self.entries.insert(id.into(), entry);
        Ok(())
    }
}
