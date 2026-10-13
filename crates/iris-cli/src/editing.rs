// SPDX-License-Identifier: GPL-3.0-or-later
//! CLI orchestration only; document, history and export rules live in core.
use anyhow::{anyhow, ensure, Result};
use clap::Subcommand;
use iris_core::{EditCommand, RenderedExportRequest, SaveEdit, Services};
use iris_daemon::AppState;
use serde_json::Value;
use std::{io::Read, path::PathBuf, sync::MutexGuard};

#[derive(Subcommand)]
pub(crate) enum EditArgs {
    Create {
        photo: i64,
        name: String,
    },
    List {
        photo: i64,
    },
    Get {
        id: String,
    },
    /// JSON SaveEdit contains expected_revision, request_id and full document.
    Save {
        id: String,
        file: PathBuf,
    },
    Duplicate {
        id: String,
        name: String,
    },
    History {
        id: String,
    },
    Undo {
        id: String,
        revision: u64,
        request_id: String,
    },
    Redo {
        id: String,
        revision: u64,
        request_id: String,
    },
    /// Fixed-revision export request JSON. Runs synchronously and never overwrites.
    Export {
        file: PathBuf,
    },
    ExportStatus {
        id: String,
    },
}

fn services(state: &AppState) -> Result<MutexGuard<'_, Services>> {
    state
        .services
        .lock()
        .map_err(|_| anyhow!("service lock poisoned"))
}

fn json_file(path: PathBuf) -> Result<Value> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 2 * 1024 * 1024, "edit request exceeds 2MiB");
    Ok(serde_json::from_slice(&bytes)?)
}

pub(crate) fn execute(state: &AppState, command: EditArgs) -> Result<Value> {
    match command {
        EditArgs::Create { photo, name } => create(state, photo, &name),
        EditArgs::List { photo } => list(state, photo),
        EditArgs::Get { id } => get(state, &id),
        EditArgs::Save { id, file } => save(state, &id, file),
        EditArgs::Duplicate { id, name } => duplicate(state, &id, &name),
        EditArgs::History { id } => history(state, &id),
        EditArgs::Undo {
            id,
            revision,
            request_id,
        } => cursor(state, &id, revision, request_id, false),
        EditArgs::Redo {
            id,
            revision,
            request_id,
        } => cursor(state, &id, revision, request_id, true),
        EditArgs::ExportStatus { id } => export_status(state, &id),
        EditArgs::Export { file } => export(state, file),
    }
}

fn create(state: &AppState, photo: i64, name: &str) -> Result<Value> {
    let job = services(state)?.prepare_edit_source(photo)?;
    let source = job.decode(&state.render_context())?;
    Ok(serde_json::to_value(
        services(state)?.create_edit_document(photo, name, source.binding)?,
    )?)
}
fn list(state: &AppState, photo: i64) -> Result<Value> {
    Ok(serde_json::to_value(
        services(state)?.edit_documents(photo, 100, 0)?,
    )?)
}
fn get(state: &AppState, id: &str) -> Result<Value> {
    Ok(serde_json::to_value(services(state)?.edit_document(id)?)?)
}
fn save(state: &AppState, id: &str, file: PathBuf) -> Result<Value> {
    let update: SaveEdit = serde_json::from_value(json_file(file)?)?;
    Ok(serde_json::to_value(
        services(state)?.save_edit_document(id, update)?,
    )?)
}
fn duplicate(state: &AppState, id: &str, name: &str) -> Result<Value> {
    Ok(serde_json::to_value(
        services(state)?.duplicate_edit_document(id, name)?,
    )?)
}
fn history(state: &AppState, id: &str) -> Result<Value> {
    Ok(serde_json::to_value(
        services(state)?.edit_history(id, 100, 0)?,
    )?)
}
fn cursor(
    state: &AppState,
    id: &str,
    revision: u64,
    request_id: String,
    redo: bool,
) -> Result<Value> {
    let command = EditCommand {
        expected_revision: revision,
        request_id,
    };
    let mut s = services(state)?;
    let record = if redo {
        s.redo_edit_document(id, command)?
    } else {
        s.undo_edit_document(id, command)?
    };
    Ok(serde_json::to_value(record)?)
}
fn export_status(state: &AppState, id: &str) -> Result<Value> {
    Ok(serde_json::to_value(services(state)?.rendered_export(id)?)?)
}
fn export(state: &AppState, file: PathBuf) -> Result<Value> {
    let request: RenderedExportRequest = serde_json::from_value(json_file(file)?)?;
    let admitted = services(state)?.queue_rendered_exports(&[request])?;
    let id = &admitted[0].id;
    match run_export(state, id) {
        Ok(value) => Ok(value),
        Err(error) => {
            services(state)?.fail_rendered_export(id, &error.to_string())?;
            Err(error)
        }
    }
}
fn run_export(state: &AppState, id: &str) -> Result<Value> {
    let job = services(state)?.prepare_rendered_export(id)?;
    let mut rendered = job.render(&state.render_context())?;
    services(state)?.authorize_rendered_export_publication(&mut rendered)?;
    let published = rendered.publish(&state.render_context())?;
    Ok(serde_json::to_value(
        services(state)?.finish_rendered_export(&published)?,
    )?)
}
