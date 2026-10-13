// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared durable editing state. All snapshots are non-destructive; source bytes
//! are never written. Revisions increase for save, undo and redo alike.
use super::{now, Services};
use anyhow::{ensure, Result};
use iris_render::document::{hash_bytes, EditDocument, SourceBinding};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

mod recipes;
mod resources;
mod state;
pub use recipes::{BatchEditTarget, EditPreset, EditRecipe};
pub use resources::{EditResource, EditResourceKind};

pub const MAX_EDIT_JSON: usize = 2 * 1024 * 1024;
const MAX_HISTORY: i64 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EditRecord {
    pub id: String,
    pub photo_id: i64,
    pub name: String,
    pub revision: u64,
    pub document: EditDocument,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EditCommand {
    pub expected_revision: u64,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveEdit {
    pub expected_revision: u64,
    pub request_id: String,
    pub document: EditDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EditHistoryEntry {
    pub revision: u64,
    pub action: String,
    pub created_at: String,
}

fn validate_label(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control),
        "invalid edit name"
    );
    Ok(())
}
fn validate_command(command: &EditCommand) -> Result<()> {
    ensure!(
        command.expected_revision < i64::MAX as u64,
        "edit revision exceeds supported range"
    );
    ensure!(
        !command.request_id.is_empty()
            && command.request_id.len() <= 128
            && command
                .request_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b)),
        "invalid edit request id"
    );
    Ok(())
}
pub(super) fn bounded_json<T: Serialize>(value: &T) -> Result<String> {
    let json = serde_json::to_string(value)?;
    ensure!(
        json.len() <= MAX_EDIT_JSON,
        "edit snapshot exceeds size limit"
    );
    Ok(json)
}
pub(super) fn same_source(a: &SourceBinding, b: &SourceBinding) -> bool {
    a.photo_id == b.photo_id
        && a.fingerprint == b.fingerprint
        && a.width == b.width
        && a.height == b.height
        && a.reference_kind == b.reference_kind
}
fn record(conn: &Connection, id: &str, revision: Option<i64>) -> Result<EditRecord> {
    let (id,photo_id,name,revision,json,created_at,updated_at):(String,i64,String,i64,String,String,String)=conn.query_row(
        "SELECT d.id,d.photo_id,d.name,r.revision,r.document_json,d.created_at,r.created_at FROM edit_documents d JOIN edit_revisions r ON r.document_id=d.id AND r.revision=COALESCE(?2,d.revision) WHERE d.id=?1",
        params![id,revision],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?;
    ensure!(
        json.len() <= MAX_EDIT_JSON,
        "stored edit snapshot exceeds size limit"
    );
    let document: EditDocument = serde_json::from_str(&json)?;
    document.validate()?;
    Ok(EditRecord {
        id,
        photo_id,
        name,
        revision: revision.try_into()?,
        document,
        created_at,
        updated_at,
    })
}
fn pin_resources(
    conn: &Connection,
    id: &str,
    revision: i64,
    document: &EditDocument,
) -> Result<()> {
    for hash in document.resources() {
        conn.execute(
            "INSERT INTO edit_resource_pins(document_id,revision,hash) VALUES(?1,?2,?3)",
            params![id, revision, hash],
        )?;
    }
    Ok(())
}
fn paginated(limit: u32, offset: u32) -> Result<()> {
    ensure!(
        (1..=100).contains(&limit) && offset <= 100_000,
        "invalid edit pagination"
    );
    Ok(())
}

impl Services {
    /// SourceBinding must come from the authoritative decode boundary. The
    /// service verifies the photo identity and bytes, never accepts a file path.
    pub fn create_edit_document(
        &mut self,
        photo_id: i64,
        name: &str,
        source: SourceBinding,
    ) -> Result<EditRecord> {
        validate_label(name)?;
        ensure!(
            source.photo_id == photo_id.to_string(),
            "edit source photo mismatch"
        );
        let path = self.photo_path(photo_id)?;
        ensure!(
            super::digest(&path)? == source.fingerprint,
            "source changed before edit creation"
        );
        let document = EditDocument::new(source);
        document.validate()?;
        self.insert_edit_document(photo_id, name, &document)
    }
    /// Make an independent virtual copy with the same verified source and current
    /// settings, a fresh revision history, and its own resource pins.
    pub fn duplicate_edit_document(&mut self, id: &str, name: &str) -> Result<EditRecord> {
        validate_label(name)?;
        let source = self.edit_document(id)?;
        ensure!(
            super::digest(&self.photo_path(source.photo_id)?)?
                == source.document.source.fingerprint,
            "source changed before virtual copy"
        );
        self.verify_edit_resources(&source.document)?;
        self.insert_edit_document(source.photo_id, name, &source.document)
    }
    fn insert_edit_document(
        &mut self,
        photo_id: i64,
        name: &str,
        document: &EditDocument,
    ) -> Result<EditRecord> {
        let json = bounded_json(&document)?;
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM edit_documents WHERE photo_id=?1",
            [photo_id],
            |r| r.get(0),
        )?;
        ensure!(count < 100, "virtual copy limit reached");
        state::require_available(&tx, photo_id)?;
        let id = uuid::Uuid::new_v4().to_string();
        let at = now();
        tx.execute("INSERT INTO edit_documents(id,photo_id,name,revision,cursor,created_at,updated_at) VALUES(?1,?2,?3,0,0,?4,?4)",params![id,photo_id,name,at])?;
        tx.execute("INSERT INTO edit_revisions(document_id,revision,document_json,action,created_at) VALUES(?1,0,?2,'create',?3)",params![id,json,at])?;
        pin_resources(&tx, &id, 0, document)?;
        tx.execute(
            "INSERT INTO edit_states(document_id,position,snapshot_revision) VALUES(?1,0,0)",
            [&id],
        )?;
        let result = record(&tx, &id, None)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn edit_document(&self, id: &str) -> Result<EditRecord> {
        let result = record(&self.store.conn, id, None)?;
        self.photo_path(result.photo_id)?;
        Ok(result)
    }
    pub fn edit_documents(
        &self,
        photo_id: i64,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<EditRecord>> {
        paginated(limit, offset)?;
        self.photo_path(photo_id)?;
        let mut query=self.store.conn.prepare("SELECT id FROM edit_documents WHERE photo_id=?1 ORDER BY created_at,id LIMIT ?2 OFFSET ?3")?;
        let ids = query
            .query_map(params![photo_id, limit, offset], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.iter()
            .map(|id| record(&self.store.conn, id, None))
            .collect()
    }
    pub fn edit_history(&self, id: &str, limit: u32, offset: u32) -> Result<Vec<EditHistoryEntry>> {
        paginated(limit, offset)?;
        self.edit_document(id)?;
        let mut query=self.store.conn.prepare("SELECT revision,action,created_at FROM edit_revisions WHERE document_id=?1 ORDER BY revision DESC LIMIT ?2 OFFSET ?3")?;
        let rows = query
            .query_map(params![id, limit, offset], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(revision, action, created_at)| {
                Ok(EditHistoryEntry {
                    revision: u64::try_from(revision)?,
                    action,
                    created_at,
                })
            })
            .collect()
    }
    pub fn delete_edit_document(&mut self, id: &str, expected_revision: u64) -> Result<()> {
        let expected_revision = i64::try_from(expected_revision)?;
        self.edit_document(id)?;
        let changed = self.store.conn.execute(
            "DELETE FROM edit_documents WHERE id=?1 AND revision=?2",
            params![id, expected_revision],
        )?;
        ensure!(changed == 1, "edit revision conflict");
        Ok(())
    }
}
