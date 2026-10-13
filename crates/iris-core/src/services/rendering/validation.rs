// SPDX-License-Identifier: GPL-3.0-or-later
//! Warm interactive checks deliberately trade repeated full-file reads for an
//! identity check. External writes preserving every observed metadata field can
//! evade this cache. Full requests and exports always SHA-256 validate; refresh
//! invalidates the hint. This is not a security boundary or a replacement for a
//! content fingerprint, and a decoder cache miss still performs a strong check.
use super::source_digest;
use anyhow::{ensure, Result};
use iris_render::RenderContext;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

const CAPACITY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceValidation {
    StrongHash,
    MetadataCache,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceIdentity {
    canonical: PathBuf,
    length: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
    #[cfg(unix)]
    unix: (u64, u64, i64, i64),
    #[cfg(windows)]
    attributes: u32,
}
impl SourceIdentity {
    pub(super) fn capture(path: &Path) -> Result<Self> {
        let canonical = path.canonicalize()?;
        let metadata = fs::metadata(&canonical)?;
        ensure!(
            metadata.is_file() && metadata.len() <= 1024 * 1024 * 1024,
            "invalid editing source file"
        );
        #[cfg(unix)]
        let unix = {
            use std::os::unix::fs::MetadataExt;
            (
                metadata.dev(),
                metadata.ino(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            )
        };
        #[cfg(windows)]
        let attributes = {
            use std::os::windows::fs::MetadataExt;
            metadata.file_attributes()
        };
        Ok(Self {
            canonical,
            length: metadata.len(),
            modified: metadata.modified()?,
            created: metadata.created().ok(),
            #[cfg(unix)]
            unix,
            #[cfg(windows)]
            attributes,
        })
    }
}
struct Entry {
    identity: SourceIdentity,
    fingerprint: String,
    touched: u64,
}
#[derive(Default)]
struct Cache {
    entries: BTreeMap<PathBuf, Entry>,
    clock: u64,
}
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

pub(super) fn invalidate(path: &Path) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("source validation cache poisoned"))?
        .entries
        .remove(&canonical);
    Ok(())
}

pub(super) fn validate(
    path: &Path,
    fingerprint: &str,
    force: bool,
    context: &RenderContext,
) -> Result<(SourceIdentity, SourceValidation)> {
    context.check_cancelled()?;
    iris_render::document::validate_hash(fingerprint)?;
    let identity = SourceIdentity::capture(path)?;
    if !force && warm(&identity, fingerprint)? {
        return Ok((identity, SourceValidation::MetadataCache));
    }
    // Never keep a formerly valid hint after a failed forced validation.
    invalidate(&identity.canonical)?;
    ensure!(
        source_digest(&identity.canonical, context)? == fingerprint,
        "source changed since editing began"
    );
    context.check_cancelled()?;
    ensure!(
        SourceIdentity::capture(path)? == identity,
        "source changed during validation"
    );
    remember(&identity, fingerprint)?;
    Ok((identity, SourceValidation::StrongHash))
}

pub(super) fn check_unchanged(
    path: &Path,
    identity: &SourceIdentity,
    context: &RenderContext,
) -> Result<()> {
    context.check_cancelled()?;
    let current = SourceIdentity::capture(path);
    match current {
        Ok(current) if current == *identity => Ok(()),
        _ => {
            invalidate(&identity.canonical)?;
            anyhow::bail!("source changed during preview")
        }
    }
}

fn warm(identity: &SourceIdentity, fingerprint: &str) -> Result<bool> {
    let mut cache = cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("source validation cache poisoned"))?;
    cache.clock = cache.clock.saturating_add(1);
    let touched = cache.clock;
    match cache.entries.get_mut(&identity.canonical) {
        Some(entry) if entry.identity == *identity && entry.fingerprint == fingerprint => {
            entry.touched = touched;
            Ok(true)
        }
        _ => Ok(false),
    }
}
fn remember(identity: &SourceIdentity, fingerprint: &str) -> Result<()> {
    let mut cache = cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("source validation cache poisoned"))?;
    if cache.entries.len() >= CAPACITY && !cache.entries.contains_key(&identity.canonical) {
        if let Some(oldest) = cache
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.touched)
            .map(|(path, _)| path.clone())
        {
            cache.entries.remove(&oldest);
        }
    }
    cache.clock = cache.clock.saturating_add(1);
    let touched = cache.clock;
    cache.entries.insert(
        identity.canonical.clone(),
        Entry {
            identity: identity.clone(),
            fingerprint: fingerprint.into(),
            touched,
        },
    );
    Ok(())
}
