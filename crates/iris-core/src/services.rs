mod cache;
mod decisions;
mod exports;
mod media;
mod photos;
mod projects;
mod quarantine;
mod scan;
mod scan_pipeline;
mod settings;

pub use media::CachedImageJob;

use crate::{domain::*, Store};
use anyhow::{bail, Context, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use walkdir::WalkDir;

pub struct Services {
    pub store: Store,
    media_runtime_dir: Option<PathBuf>,
}

// Private durable recovery details remain additive to the public quarantine plan.
#[derive(serde::Serialize, serde::Deserialize)]
struct RestoreJournal {
    #[serde(flatten)]
    plan: QuarantinePlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restore_rollback: Option<RestoreRollback>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct RestoreRollback {
    prior_state: String,
    steps: Vec<RestoreStep>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct RestoreStep {
    index: usize,
    source_existed: bool,
    quarantine_existed: bool,
    missing: bool,
    quarantined: bool,
    prior_item_state: String,
}

/// Shared cache/adoption gate. `settings` must be the canonical current settings.
pub fn analysis_matches_settings(analysis: &Value, settings: &Value) -> bool {
    if analysis.get("source_invalid").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    if analysis.get("version").and_then(Value::as_str) != Some(crate::vision::ANALYSIS_VERSION) {
        return false;
    }
    analysis
        .get("settings")
        .and_then(|value| {
            let parsed: crate::vision::AnalysisSettings =
                serde_json::from_value(value.clone()).ok()?;
            parsed.validate().ok()?;
            // Quality inference and semantic vectors have separate freshness.
            // Candidate selection may legitimately leave this photo without a vector.
            serde_json::to_value(parsed).ok()
        })
        .as_ref()
        == Some(settings)
}
impl Services {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            media_runtime_dir: None,
        }
    }
    pub fn with_media_runtime_dir(mut self, directory: PathBuf) -> Self {
        self.media_runtime_dir = Some(directory);
        self
    }
}

const PHOTO_SELECT:&str="SELECT p.id,p.project_id,p.path,p.filename,p.format,p.mtime,p.size_bytes,p.width,p.height,p.taken_at,p.capture_variant_id,p.missing,p.quarantined,COALESCE((SELECT action FROM decisions d WHERE d.photo_id=p.id AND d.undone_at IS NULL ORDER BY d.id DESC LIMIT 1),'pending'),a.data,COALESCE((SELECT rating FROM photo_marks m WHERE m.photo_id=p.id AND m.undone_at IS NULL ORDER BY m.id DESC LIMIT 1),0),COALESCE((SELECT color_label FROM photo_marks m WHERE m.photo_id=p.id AND m.undone_at IS NULL ORDER BY m.id DESC LIMIT 1),'none') FROM photos p LEFT JOIN analyses a ON a.photo_id=p.id";
const PROJECT_SELECT: &str = "SELECT p.id,p.root,p.name,p.created_at,p.cache_root,p.groups_dirty,(SELECT COUNT(*) FROM photos ph LEFT JOIN analyses a ON a.photo_id=ph.id WHERE ph.project_id=p.id AND ph.missing=0 AND ph.quarantined=0 AND (a.photo_id IS NULL OR a.version IS NULL OR a.version!=?1 OR COALESCE(json_extract(a.data,'$.source_invalid'),0)=1)),p.last_opened_at,p.hidden FROM projects p";
fn project_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        root: r.get(1)?,
        name: r.get(2)?,
        created_at: r.get(3)?,
        cache_root: r.get(4)?,
        groups_dirty: r.get(5)?,
        pending_analysis: r.get::<_, i64>(6)? as u64,
        last_opened_at: r.get(7)?,
        hidden: r.get(8)?,
    })
}
fn photo_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Photo> {
    let action: String = r.get(13)?;
    let data: Option<String> = r.get(14)?;
    let decision = action.parse().map_err(|e: anyhow::Error| {
        rusqlite::Error::FromSqlConversionFailure(
            13,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    let analysis = data
        .map(|s| {
            serde_json::from_str(&s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    14,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })
        .transpose()?;
    let rating = r.get(15)?;
    let color_label = serde_json::from_value(Value::String(r.get(16)?)).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(16, rusqlite::types::Type::Text, Box::new(error))
    })?;
    photo_metadata_row(r, decision, analysis, rating, color_label)
}

fn photo_metadata_row(
    r: &rusqlite::Row<'_>,
    decision: Action,
    analysis: Option<Value>,
    rating: u8,
    color_label: ColorLabel,
) -> rusqlite::Result<Photo> {
    Ok(Photo {
        rating,
        color_label,
        id: r.get(0)?,
        project_id: r.get(1)?,
        path: r.get(2)?,
        filename: r.get(3)?,
        format: r.get(4)?,
        mtime: r.get(5)?,
        size_bytes: r.get::<_, i64>(6)? as u64,
        width: r.get(7)?,
        height: r.get(8)?,
        taken_at: r.get(9)?,
        capture_variant_id: r.get(10)?,
        missing: r.get(11)?,
        quarantined: r.get(12)?,
        decision,
        analysis_status: AnalysisStatus::Missing,
        analysis,
    })
}
fn analysis_status(analysis: Option<&Value>, settings: &Value) -> AnalysisStatus {
    match analysis {
        None => AnalysisStatus::Missing,
        Some(value) if analysis_matches_settings(value, settings) => AnalysisStatus::Current,
        Some(_) => AnalysisStatus::Stale,
    }
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn score(p: &Photo) -> f64 {
    if p.analysis_status != AnalysisStatus::Current {
        return -1.;
    }
    p.analysis
        .as_ref()
        .and_then(|a| a.get("composite_score"))
        .and_then(Value::as_f64)
        .unwrap_or(-1.)
}
fn modified(meta: &fs::Metadata) -> Result<i64> {
    Ok(meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos()
        .min(i64::MAX as u128) as i64)
}
fn relative_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
fn read_taken_at(path: &Path) -> Option<String> {
    let mut file = std::io::BufReader::new(File::open(path).ok()?);
    let exif = exif::Reader::new().read_from_container(&mut file).ok()?;
    exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string())
}
fn checked_relative(rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if p.as_os_str().is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        bail!("invalid relative path");
    }
    Ok(p.to_path_buf())
}
fn existing_within(root: &Path, rel: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let path = root.join(checked_relative(rel)?).canonicalize()?;
    if !path.starts_with(&root) {
        bail!("path escapes project folder");
    }
    Ok(path)
}
fn digest(path: &Path) -> Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn safe_destination(root: &Path, rel: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let dest = root.join(checked_relative(rel)?);
    let parent = dest.parent().context("destination has no parent")?;
    let mut existing = parent;
    while !existing.exists() {
        existing = existing.parent().context("invalid destination")?;
    }
    if !existing.canonicalize()?.starts_with(&root) {
        bail!("destination escapes root through a link");
    }
    fs::create_dir_all(parent)?;
    if !parent.canonicalize()?.starts_with(&root) {
        bail!("destination parent escapes root");
    }
    Ok(dest)
}
fn copy_new_verified(src: &Path, dst: &Path) -> Result<()> {
    let mut input = File::open(src)?;
    let source_modified = input.metadata()?.modified()?;
    let mut output = OpenOptions::new().write(true).create_new(true).open(dst)?;
    let copied = (|| -> std::io::Result<()> {
        std::io::copy(&mut input, &mut output)?;
        // Quarantine/restore use verified copies; preserve the scan fingerprint
        // when the bytes are unchanged, including during compensation.
        output.set_times(fs::FileTimes::new().set_modified(source_modified))?;
        output.sync_all()
    })();
    if let Err(e) = copied {
        drop(output);
        let _ = fs::remove_file(dst);
        return Err(e.into());
    }
    drop(output);
    if digest(src)? != digest(dst)? {
        let _ = fs::remove_file(dst);
        bail!("copy verification failed");
    }
    Ok(())
}
fn move_verified(src: &Path, dst: &Path, expected_hash: &str) -> Result<()> {
    if dst.exists() {
        bail!("destination exists");
    }
    if digest(src)? != expected_hash {
        bail!("source changed since preview");
    }
    copy_new_verified(src, dst)?;
    if digest(dst)? != expected_hash {
        let _ = fs::remove_file(dst);
        bail!("move verification failed");
    }
    fs::remove_file(src)?;
    Ok(())
}

fn restore_file_exists(root: &Path, path: &Path, expected_hash: &str) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "restore path is not a regular file"
            );
            anyhow::ensure!(
                path.canonicalize()?.starts_with(root),
                "restore path escapes project"
            );
            anyhow::ensure!(
                digest(path)? == expected_hash,
                "restore file has different contents; no files overwritten"
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn paths_overlap(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn resolve_storage_root(path: &Path) -> Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(path.canonicalize()?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let absolute = std::path::absolute(path)?;
            let parent = absolute.parent().context("storage path has no parent")?;
            Ok(resolve_storage_root(parent)?
                .join(absolute.file_name().context("invalid storage path")?))
        }
        Err(error) => Err(error.into()),
    }
}
