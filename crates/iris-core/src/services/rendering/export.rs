// SPDX-License-Identifier: GPL-3.0-or-later
//! Durable immutable export snapshots. Preparation/status touch SQLite; rendering
//! and publication are explicit jobs to execute without the service mutex.
use super::{source_digest, EditRenderJob};
use crate::services::{EditRecord, Services};
use anyhow::{ensure, Context, Result};
use iris_render::{
    document::{hash_bytes, validate_hash},
    output::{self, OutputFormat},
    RenderContext,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};
use tempfile::NamedTempFile;
use utoipa::ToSchema;

const MAX_SNAPSHOT: usize = 2 * 1024 * 1024 + 16384;
const RESOURCE_QUOTA: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderedExportRequest {
    pub document_id: String,
    pub expected_revision: u64,
    pub destination_directory: String,
    pub filename: String,
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenderedExportState {
    Queued,
    Running,
    Publishing,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RenderedExportStatus {
    pub id: String,
    pub batch_id: String,
    pub document_id: String,
    pub revision: u64,
    pub state: RenderedExportState,
    pub destination: String,
    pub format: OutputFormat,
    pub created_at: String,
    pub updated_at: String,
    pub result_hash: Option<String>,
    pub result_bytes: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    record: EditRecord,
    source: PathBuf,
    directory: PathBuf,
    filename: String,
    format: OutputFormat,
}

pub struct RenderedExportJob {
    pub id: String,
    snapshot: Snapshot,
    render: EditRenderJob,
}
pub struct PreparedRenderedExport {
    pub id: String,
    snapshot: Snapshot,
    temporary: NamedTempFile,
    hash: String,
    bytes: u64,
    authorized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RenderedExportResult {
    pub id: String,
    pub destination: String,
    pub hash: String,
    pub bytes: u64,
}

pub struct RenderedExportRecovery {
    id: String,
    snapshot: Snapshot,
    hash: Option<String>,
    bytes: Option<u64>,
}

fn destination(snapshot: &Snapshot) -> PathBuf {
    snapshot.directory.join(&snapshot.filename)
}
fn validate_filename(filename: &str, format: OutputFormat) -> Result<()> {
    ensure!(
        !filename.is_empty()
            && filename.len() <= 240
            && !filename
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
            && !filename.ends_with([' ', '.']),
        "invalid export filename"
    );
    let path = Path::new(filename);
    let mut parts = path.components();
    ensure!(
        matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none(),
        "export requires a single filename"
    );
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    ensure!(
        ![
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ]
        .contains(&stem.as_str()),
        "reserved export filename"
    );
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    ensure!(
        match format {
            OutputFormat::Jpeg8 { quality } =>
                (1..=100).contains(&quality) && matches!(extension.as_str(), "jpg" | "jpeg"),
            OutputFormat::Png16 => extension == "png",
            OutputFormat::Tiff16 => matches!(extension.as_str(), "tif" | "tiff"),
        },
        "export extension/format mismatch"
    );
    Ok(())
}
fn require_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => anyhow::bail!(
            "export destination already exists; originals and existing files are never overwritten"
        ),
    }
}
fn recheck_directory(snapshot: &Snapshot) -> Result<()> {
    let metadata = fs::symlink_metadata(&snapshot.directory)?;
    ensure!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && snapshot.directory.canonicalize()? == snapshot.directory,
        "export directory changed or became a link"
    );
    validate_filename(&snapshot.filename, snapshot.format)?;
    Ok(())
}
fn snapshot(conn: &rusqlite::Connection, id: &str) -> Result<Snapshot> {
    let (json, revision): (String, i64) = conn.query_row(
        "SELECT snapshot_json,revision FROM rendered_export_jobs WHERE id=?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    ensure!(
        json.len() <= MAX_SNAPSHOT,
        "stored export snapshot exceeds limit"
    );
    let value: Snapshot = serde_json::from_str(&json)?;
    ensure!(
        sqlite_integer(value.record.revision)? == revision,
        "export snapshot revision mismatch"
    );
    value.record.document.validate()?;
    validate_filename(&value.filename, value.format)?;
    Ok(value)
}
fn status(conn: &rusqlite::Connection, id: &str) -> Result<RenderedExportStatus> {
    let snapshot = snapshot(conn, id)?;
    let (batch_id,state,created_at,updated_at,result_hash,result_bytes,error):(String,String,String,String,Option<String>,Option<i64>,Option<String>) = conn.query_row("SELECT batch_id,state,created_at,updated_at,result_hash,result_bytes,error FROM rendered_export_jobs WHERE id=?1",[id],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?;
    Ok(RenderedExportStatus {
        id: id.into(),
        batch_id,
        document_id: snapshot.record.id.clone(),
        revision: snapshot.record.revision,
        state: serde_json::from_value(serde_json::Value::String(state))?,
        destination: destination(&snapshot).to_string_lossy().into_owned(),
        format: snapshot.format,
        created_at,
        updated_at,
        result_hash,
        result_bytes: result_bytes.map(nonnegative_integer).transpose()?,
        error,
    })
}

impl Services {
    /// Return one durable queue candidate without exposing persistence to workers.
    pub fn next_queued_rendered_export(&self) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self.store.conn.query_row("SELECT id FROM rendered_export_jobs WHERE state='queued' ORDER BY created_at,id LIMIT 1", [], |row| row.get(0)).optional()?)
    }
    /// Atomic batch admission. The durable job owns copies of documents and LUT
    /// bytes; subsequent saves or deletion cannot mutate an admitted export.
    pub fn queue_rendered_exports(
        &mut self,
        requests: &[RenderedExportRequest],
    ) -> Result<Vec<RenderedExportStatus>> {
        ensure!(
            !requests.is_empty() && requests.len() <= 100,
            "export batch requires 1..100 inputs"
        );
        let mut prepared = Vec::with_capacity(requests.len());
        let mut destinations = BTreeSet::new();
        let mut total = 0_usize;
        for request in requests {
            let (snapshot, resources) = self.prepare_export_snapshot(request)?;
            let target = destination(&snapshot);
            let key = target.to_string_lossy().to_lowercase();
            ensure!(
                destinations.insert(key),
                "duplicate export destination in batch"
            );
            for bytes in resources.values() {
                total = total
                    .checked_add(bytes.len())
                    .context("export resources overflow")?;
            }
            ensure!(
                total <= RESOURCE_QUOTA,
                "export snapshot resource quota exceeded"
            );
            let json = serde_json::to_string(&snapshot)?;
            ensure!(
                json.len() <= MAX_SNAPSHOT,
                "export snapshot exceeds size limit"
            );
            prepared.push((uuid::Uuid::new_v4().to_string(), snapshot, json, resources));
        }
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        validate_queue_quota(&tx, requests.len(), total)?;
        let batch = uuid::Uuid::new_v4().to_string();
        let at = crate::services::now();
        for (id, snapshot, json, resources) in &prepared {
            insert_job(&tx, id, &batch, snapshot, json, &at)?;
            for (hash, bytes) in resources {
                ensure!(
                    hash_bytes(bytes) == *hash,
                    "export resource fingerprint mismatch"
                );
                tx.execute(
                    "INSERT INTO rendered_export_resources(job_id,hash,bytes) VALUES(?1,?2,?3)",
                    params![id, hash, bytes],
                )?;
            }
        }
        let result = prepared
            .iter()
            .map(|(id, _, _, _)| status(&tx, id))
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(result)
    }

    fn prepare_export_snapshot(
        &self,
        request: &RenderedExportRequest,
    ) -> Result<(Snapshot, BTreeMap<String, Vec<u8>>)> {
        validate_filename(&request.filename, request.format)?;
        ensure!(
            request.destination_directory.len() <= 32768,
            "export directory path too long"
        );
        let directory = Path::new(&request.destination_directory).canonicalize()?;
        ensure!(directory.is_dir(), "export directory must exist");
        let render = self.prepare_edit_render(&request.document_id, request.expected_revision)?;
        let snapshot = Snapshot {
            record: render.record,
            source: render.source.canonicalize()?,
            directory,
            filename: request.filename.clone(),
            format: request.format,
        };
        recheck_directory(&snapshot)?;
        let target = destination(&snapshot);
        require_absent(&target)?;
        ensure!(target != snapshot.source, "export cannot replace source");
        Ok((snapshot, render.resources))
    }

    pub fn rendered_export(&self, id: &str) -> Result<RenderedExportStatus> {
        status(&self.store.conn, id)
    }
    pub fn rendered_exports(&self, limit: u32, offset: u32) -> Result<Vec<RenderedExportStatus>> {
        ensure!(
            (1..=100).contains(&limit) && offset <= 100_000,
            "invalid export pagination"
        );
        let mut query = self.store.conn.prepare(
            "SELECT id FROM rendered_export_jobs ORDER BY created_at DESC,id LIMIT ?1 OFFSET ?2",
        )?;
        let ids = query
            .query_map(params![limit, offset], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.iter().map(|id| status(&self.store.conn, id)).collect()
    }
    pub fn cancel_rendered_export(&mut self, id: &str) -> Result<RenderedExportStatus> {
        let changed = self.store.conn.execute("UPDATE rendered_export_jobs SET state='cancelled',updated_at=?2,error=NULL WHERE id=?1 AND state IN ('queued','running','interrupted')",params![id,crate::services::now()])?;
        ensure!(
            changed == 1
                || matches!(
                    status(&self.store.conn, id)?.state,
                    RenderedExportState::Cancelled
                ),
            "export has entered publication or is already terminal"
        );
        status(&self.store.conn, id)
    }
    /// Call only after previous process workers are known to have stopped.
    /// Queued immutable jobs survive; in-flight jobs become explicitly resumable.
    pub fn recover_rendered_exports(&mut self) -> Result<u64> {
        let changed = self.store.conn.execute("UPDATE rendered_export_jobs SET state='interrupted',error='Export interrupted; inspect publication before retry',updated_at=?1 WHERE state IN ('running','publishing')",[crate::services::now()])?;
        Ok(u64::try_from(changed)?)
    }
    pub fn retry_rendered_export(&mut self, id: &str) -> Result<RenderedExportStatus> {
        let value = snapshot(&self.store.conn, id)?;
        recheck_directory(&value)?;
        require_absent(&destination(&value))?;
        let changed = self.store.conn.execute("UPDATE rendered_export_jobs SET state='queued',error=NULL,result_hash=NULL,result_bytes=NULL,temporary_path=NULL,updated_at=?2 WHERE id=?1 AND state IN ('failed','interrupted')",params![id,crate::services::now()])?;
        ensure!(changed == 1, "export cannot be retried in this state");
        status(&self.store.conn, id)
    }
    pub fn prepare_rendered_export(&mut self, id: &str) -> Result<RenderedExportJob> {
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let snapshot = snapshot(&tx, id)?;
        let mut resources = BTreeMap::new();
        let mut total = 0usize;
        {
            let mut query =
                tx.prepare("SELECT hash,bytes FROM rendered_export_resources WHERE job_id=?1")?;
            let mut rows = query.query([id])?;
            while let Some(row) = rows.next()? {
                let hash: String = row.get(0)?;
                let bytes: Vec<u8> = row.get(1)?;
                total = total
                    .checked_add(bytes.len())
                    .context("snapshot resource overflow")?;
                ensure!(
                    total <= 128 * 1024 * 1024 && hash_bytes(&bytes) == hash,
                    "invalid pinned export resource"
                );
                resources.insert(hash, bytes);
            }
        }
        ensure!(
            snapshot
                .record
                .document
                .resources()
                .iter()
                .all(|hash| resources.contains_key(hash)),
            "export snapshot missing resource"
        );
        let changed = tx.execute("UPDATE rendered_export_jobs SET state='running',updated_at=?2,error=NULL WHERE id=?1 AND state='queued'",params![id,crate::services::now()])?;
        ensure!(changed == 1, "export is not queued");
        tx.commit()?;
        let render = EditRenderJob {
            record: snapshot.record.clone(),
            source: snapshot.source.clone(),
            resources,
        };
        Ok(RenderedExportJob {
            id: id.into(),
            snapshot,
            render,
        })
    }
    /// Short commit gate: cancellation remains possible until this succeeds.
    /// Afterward publication is an uncancellable no-overwrite commit operation.
    pub fn authorize_rendered_export_publication(
        &mut self,
        prepared: &mut PreparedRenderedExport,
    ) -> Result<()> {
        let bytes = sqlite_integer(prepared.bytes)?;
        let changed = self.store.conn.execute("UPDATE rendered_export_jobs SET state='publishing',result_hash=?2,result_bytes=?3,temporary_path=?4,updated_at=?5 WHERE id=?1 AND state='running'",params![prepared.id,prepared.hash,bytes,prepared.temporary.path().to_string_lossy(),crate::services::now()])?;
        ensure!(changed == 1, "export cancelled or no longer running");
        prepared.authorized = true;
        Ok(())
    }
    pub fn finish_rendered_export(
        &mut self,
        result: &RenderedExportResult,
    ) -> Result<RenderedExportStatus> {
        validate_hash(&result.hash)?;
        let bytes = sqlite_integer(result.bytes)?;
        let changed = self.store.conn.execute("UPDATE rendered_export_jobs SET state='completed',error=NULL,temporary_path=NULL,updated_at=?4 WHERE id=?1 AND state IN ('publishing','interrupted') AND result_hash=?2 AND result_bytes=?3",params![result.id,result.hash,bytes,crate::services::now()])?;
        ensure!(
            changed == 1,
            "export result does not match publication journal"
        );
        status(&self.store.conn, &result.id)
    }
    pub fn fail_rendered_export(&mut self, id: &str, error: &str) -> Result<RenderedExportStatus> {
        let error: String = error.chars().take(2048).collect();
        // A publication I/O error can happen after the file became visible.
        // Keep its journal and require recovery inspection instead of rerunning.
        self.store.conn.execute("UPDATE rendered_export_jobs SET state=CASE WHEN state='publishing' THEN 'interrupted' ELSE 'failed' END,error=?2,updated_at=?3 WHERE id=?1 AND state IN ('running','publishing')",params![id,error,crate::services::now()])?;
        status(&self.store.conn, id)
    }
    pub fn prepare_rendered_export_recovery(&self, id: &str) -> Result<RenderedExportRecovery> {
        let state = status(&self.store.conn, id)?;
        ensure!(
            matches!(state.state, RenderedExportState::Interrupted),
            "export is not interrupted"
        );
        Ok(RenderedExportRecovery {
            id: id.into(),
            snapshot: snapshot(&self.store.conn, id)?,
            hash: state.result_hash,
            bytes: state.result_bytes,
        })
    }
    /// Explicit history cleanup also releases immutable resource snapshots.
    pub fn delete_rendered_export(&mut self, id: &str) -> Result<()> {
        ensure!(self.store.conn.execute("DELETE FROM rendered_export_jobs WHERE id=?1 AND state IN ('completed','cancelled','failed')",[id])?==1,"only terminal export history can be deleted");
        Ok(())
    }
}

impl RenderedExportJob {
    pub fn render(&self, context: &RenderContext) -> Result<PreparedRenderedExport> {
        context.check_cancelled()?;
        recheck_directory(&self.snapshot)?;
        require_absent(&destination(&self.snapshot))?;
        let frame = self.render.full_resolution(context)?;
        let _source_memory = context.reserve(frame.byte_len())?;
        let encoded =
            iris_render::color::output(&frame, &self.snapshot.record.document.output, context)?;
        let _encoded_memory = context.reserve(encoded.byte_len())?;
        let mut temporary = tempfile::Builder::new()
            .prefix(&format!(".iris-export-{}-", self.id))
            .suffix(".tmp")
            .tempfile_in(&self.snapshot.directory)?;
        output::write_output(
            temporary.as_file_mut(),
            &encoded,
            &self.snapshot.record.document.output,
            self.snapshot.format,
            context,
        )?;
        temporary.as_file().sync_all()?;
        let bytes = temporary.as_file().metadata()?.len();
        let hash = source_digest(temporary.path(), context)?;
        self.render.verify_source(context)?;
        context.check_cancelled()?;
        Ok(PreparedRenderedExport {
            id: self.id.clone(),
            snapshot: self.snapshot.clone(),
            temporary,
            hash,
            bytes,
            authorized: false,
        })
    }
}
impl PreparedRenderedExport {
    pub fn publish(self, context: &RenderContext) -> Result<RenderedExportResult> {
        ensure!(
            self.authorized,
            "export publication requires service authorization"
        );
        context.check_cancelled()?;
        recheck_directory(&self.snapshot)?;
        let path = destination(&self.snapshot);
        require_absent(&path)?;
        ensure!(
            source_digest(&self.snapshot.source, context)?
                == self.snapshot.record.document.source.fingerprint,
            "source changed before export publication"
        );
        ensure!(
            source_digest(self.temporary.path(), context)? == self.hash,
            "temporary export changed before publication"
        );
        context.check_cancelled()?;
        recheck_directory(&self.snapshot)?;
        self.temporary
            .persist_noclobber(&path)
            .map_err(|error| error.error)?
            .sync_all()?;
        Ok(RenderedExportResult {
            id: self.id,
            destination: path.to_string_lossy().into_owned(),
            hash: self.hash,
            bytes: self.bytes,
        })
    }
}
impl RenderedExportRecovery {
    /// Inspect an interrupted commit outside the service lock. A matching
    /// existing result can be finalized without overwriting it; no file is deleted.
    pub fn inspect(&self, context: &RenderContext) -> Result<Option<RenderedExportResult>> {
        recheck_directory(&self.snapshot)?;
        let path = destination(&self.snapshot);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
            Ok(meta) => ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "interrupted export destination is not a regular file"
            ),
        }
        let hash = self
            .hash
            .as_ref()
            .context("existing export has no publication journal")?;
        let bytes = fs::metadata(&path)?.len();
        ensure!(
            Some(bytes) == self.bytes && source_digest(&path, context)? == *hash,
            "existing destination does not match interrupted publication"
        );
        Ok(Some(RenderedExportResult {
            id: self.id.clone(),
            destination: path.to_string_lossy().into_owned(),
            hash: hash.clone(),
            bytes,
        }))
    }
}

fn insert_job(
    conn: &rusqlite::Connection,
    id: &str,
    batch: &str,
    snapshot: &Snapshot,
    json: &str,
    at: &str,
) -> Result<()> {
    let revision = sqlite_integer(snapshot.record.revision)?;
    conn.execute("INSERT INTO rendered_export_jobs(id,batch_id,document_id,revision,snapshot_json,state,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'queued',?6,?6)",params![id,batch,snapshot.record.id,revision,json,at])?;
    Ok(())
}

fn sqlite_integer(value: u64) -> Result<i64> {
    i64::try_from(value).context("export integer exceeds SQLite range")
}
fn nonnegative_integer(value: i64) -> Result<u64> {
    u64::try_from(value).context("negative integer in export storage")
}
fn check_quota(stored: i64, additional: usize, limit: i64) -> Result<()> {
    ensure!(
        stored >= 0 && stored <= limit,
        "invalid or exhausted export quota"
    );
    let additional =
        i64::try_from(additional).context("export quota increment exceeds SQLite range")?;
    ensure!(additional <= limit - stored, "export quota exceeded");
    Ok(())
}
fn validate_queue_quota(
    tx: &rusqlite::Transaction<'_>,
    request_count: usize,
    total: usize,
) -> Result<()> {
    let (active,jobs):(i64,i64) = tx.query_row("SELECT COALESCE(SUM(CASE WHEN state IN ('queued','running','publishing') THEN 1 ELSE 0 END),0),COUNT(*) FROM rendered_export_jobs",[],|row| Ok((row.get(0)?,row.get(1)?)))?;
    check_quota(active, request_count, 100)?;
    check_quota(jobs, request_count, 1000)?;
    let stored: i64 = tx.query_row(
        "SELECT COALESCE(SUM(length(bytes)),0) FROM rendered_export_resources",
        [],
        |row| row.get(0),
    )?;
    check_quota(stored, total, i64::try_from(RESOURCE_QUOTA)?)
}
