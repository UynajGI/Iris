// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

pub(super) fn require_available(conn: &Connection, photo_id: i64) -> Result<()> {
    let available: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM photos WHERE id=?1 AND missing=0 AND quarantined=0)",
        [photo_id],
        |r| r.get(0),
    )?;
    ensure!(available, "photo is unavailable");
    Ok(())
}

fn retry(
    conn: &Connection,
    id: &str,
    command: &EditCommand,
    payload: &str,
) -> Result<Option<EditRecord>> {
    let previous:Option<(String,i64)>=conn.query_row("SELECT payload_hash,result_revision FROM edit_commands WHERE document_id=?1 AND request_id=?2",params![id,command.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    if let Some((hash, revision)) = previous {
        ensure!(
            hash == payload,
            "edit request id was already used with a different payload"
        );
        return Ok(Some(record(conn, id, Some(revision))?));
    }
    Ok(None)
}

fn append(
    conn: &Connection,
    current: &EditRecord,
    command: &EditCommand,
    document: &EditDocument,
    action: &str,
    cursor: i64,
    payload: &str,
) -> Result<EditRecord> {
    ensure!(
        current.revision == command.expected_revision,
        "edit revision conflict"
    );
    ensure!(
        current.revision < (MAX_HISTORY as u64),
        "edit history limit reached; create a virtual copy"
    );
    let revision = (current.revision + 1) as i64;
    let at = now();
    let json = bounded_json(document)?;
    let stored_bytes:i64=conn.query_row("SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM edit_revisions WHERE document_id=?1",[&current.id],|r|r.get(0))?;
    ensure!(
        stored_bytes + json.len() as i64 <= 64 * 1024 * 1024,
        "edit history byte limit reached; create a virtual copy"
    );
    conn.execute("INSERT INTO edit_revisions(document_id,revision,document_json,action,created_at) VALUES(?1,?2,?3,?4,?5)",params![current.id,revision,json,action,at])?;
    pin_resources(conn, &current.id, revision, document)?;
    conn.execute(
        "UPDATE edit_documents SET revision=?2,cursor=?3,updated_at=?4 WHERE id=?1",
        params![current.id, revision, cursor, at],
    )?;
    conn.execute("INSERT INTO edit_commands(document_id,request_id,payload_hash,result_revision) VALUES(?1,?2,?3,?4)",params![current.id,command.request_id,payload,revision])?;
    record(conn, &current.id, None)
}

pub(super) fn save_in_transaction(
    conn: &Connection,
    id: &str,
    request: &SaveEdit,
) -> Result<EditRecord> {
    save_with_context(conn, id, request, None)
}

pub(super) fn save_with_context(
    conn: &Connection,
    id: &str,
    request: &SaveEdit,
    context: Option<&str>,
) -> Result<EditRecord> {
    let command = EditCommand {
        expected_revision: request.expected_revision,
        request_id: request.request_id.clone(),
    };
    validate_command(&command)?;
    request.document.validate()?;
    let payload = match context {
        Some(value) => hash_bytes(bounded_json(&("sync", request, value))?.as_bytes()),
        None => hash_bytes(bounded_json(&("save", request))?.as_bytes()),
    };
    let current = record(conn, id, None)?;
    require_available(conn, current.photo_id)?;
    if let Some(result) = retry(conn, id, &command, &payload)? {
        return Ok(result);
    }
    ensure!(
        same_source(&current.document.source, &request.document.source),
        "source binding is immutable; explicit rebase required"
    );
    ensure!(
        current.revision == request.expected_revision,
        "edit revision conflict"
    );
    let cursor: i64 =
        conn.query_row("SELECT cursor FROM edit_documents WHERE id=?1", [id], |r| {
            r.get(0)
        })?;
    conn.execute(
        "DELETE FROM edit_states WHERE document_id=?1 AND position>?2",
        params![id, cursor],
    )?;
    let result = append(
        conn,
        &current,
        &command,
        &request.document,
        if context.is_some() { "batch" } else { "save" },
        cursor + 1,
        &payload,
    )?;
    conn.execute(
        "INSERT INTO edit_states(document_id,position,snapshot_revision) VALUES(?1,?2,?3)",
        params![id, cursor + 1, result.revision as i64],
    )?;
    Ok(result)
}

impl Services {
    pub fn save_edit_document(&mut self, id: &str, request: SaveEdit) -> Result<EditRecord> {
        self.edit_document(id)?;
        request.document.validate()?;
        self.verify_edit_resources(&request.document)?;
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = save_in_transaction(&tx, id, &request)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn undo_edit_document(&mut self, id: &str, command: EditCommand) -> Result<EditRecord> {
        self.move_edit_cursor(id, command, -1)
    }
    pub fn redo_edit_document(&mut self, id: &str, command: EditCommand) -> Result<EditRecord> {
        self.move_edit_cursor(id, command, 1)
    }
    fn move_edit_cursor(
        &mut self,
        id: &str,
        command: EditCommand,
        direction: i64,
    ) -> Result<EditRecord> {
        validate_command(&command)?;
        self.edit_document(id)?;
        let action = if direction < 0 { "undo" } else { "redo" };
        let payload = hash_bytes(bounded_json(&(action, &command))?.as_bytes());
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = record(&tx, id, None)?;
        require_available(&tx, current.photo_id)?;
        if let Some(result) = retry(&tx, id, &command, &payload)? {
            tx.commit()?;
            return Ok(result);
        }
        ensure!(
            current.revision == command.expected_revision,
            "edit revision conflict"
        );
        let (cursor, destination) = cursor_destination(&tx, id, direction, action)?;
        let result = append(
            &tx,
            &current,
            &command,
            &destination.document,
            action,
            cursor + direction,
            &payload,
        )?;
        tx.commit()?;
        Ok(result)
    }
}

fn cursor_destination(
    tx: &rusqlite::Connection,
    id: &str,
    direction: i64,
    action: &str,
) -> Result<(i64, EditRecord)> {
    let cursor: i64 = tx.query_row("SELECT cursor FROM edit_documents WHERE id=?1", [id], |r| {
        r.get(0)
    })?;
    let revision: Option<i64> = tx
        .query_row(
            "SELECT snapshot_revision FROM edit_states WHERE document_id=?1 AND position=?2",
            params![id, cursor + direction],
            |r| r.get(0),
        )
        .optional()?;
    let revision = revision.ok_or_else(|| anyhow::anyhow!("nothing to {action}"))?;
    let destination = record(tx, id, Some(revision))?;
    Ok((cursor, destination))
}
