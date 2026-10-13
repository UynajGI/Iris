// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use anyhow::Context;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};
const MAX_RESOURCE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_RESOURCE_STORE_BYTES: i64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EditResourceKind {
    Lut,
    Icc,
    LensProfile,
}
impl EditResourceKind {
    fn key(self) -> &'static str {
        match self {
            Self::Lut => "lut",
            Self::Icc => "icc",
            Self::LensProfile => "lens_profile",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EditResource {
    pub hash: String,
    pub kind: EditResourceKind,
    pub size_bytes: u64,
}

fn validate_blob(kind: EditResourceKind, bytes: &[u8]) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() as u64 <= MAX_RESOURCE_BYTES,
        "invalid resource byte length"
    );
    match kind {
        EditResourceKind::Lut => {
            iris_render::lut::CubeLut::parse(std::str::from_utf8(bytes)?)?;
        }
        EditResourceKind::LensProfile => {
            iris_render::lens_profile::LensProfile::parse(bytes)?;
        }
        EditResourceKind::Icc => {
            iris_render::color::validate_icc(bytes)?;
        }
    }
    Ok(())
}

fn admit_resource(
    tx: &rusqlite::Connection,
    hash: &str,
    kind: EditResourceKind,
    size_bytes: i64,
) -> Result<()> {
    let existing: Option<(String, i64)> = tx
        .query_row(
            "SELECT kind,size_bytes FROM edit_resources WHERE hash=?1",
            [hash],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((existing_kind, size)) = existing {
        ensure!(
            existing_kind == kind.key() && size == size_bytes,
            "resource kind/size conflict"
        );
    } else {
        let total: i64 = tx.query_row(
            "SELECT COALESCE(SUM(size_bytes),0) FROM edit_resources",
            [],
            |r| r.get(0),
        )?;
        ensure!(
            total + size_bytes <= MAX_RESOURCE_STORE_BYTES,
            "edit resource store quota reached"
        );
    }
    Ok(())
}
fn publish_resource(root: &std::path::Path, target: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if !target.exists() {
        let temporary = root.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            match crate::cache_publish::publish_noclobber(&temporary, target) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(e) => Err(e.into()),
            }
        })();
        let _ = fs::remove_file(&temporary);
        result?;
    }
    Ok(())
}

impl Services {
    fn edit_resource_root(&self) -> Result<PathBuf> {
        let parent = self
            .store
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let parent = parent.canonicalize()?;
        let root = parent.join("edit-resources-v1");
        fs::create_dir_all(&root)?;
        let canonical = root.canonicalize()?;
        ensure!(
            canonical.starts_with(&parent),
            "resource directory escapes database storage"
        );
        Ok(canonical)
    }
    /// Imports bytes, never a client-controlled filesystem path. The exact bytes
    /// are immutable, addressed by SHA-256, and verified again on every read.
    pub fn import_edit_resource(
        &mut self,
        kind: EditResourceKind,
        bytes: &[u8],
    ) -> Result<EditResource> {
        validate_blob(kind, bytes)?;
        let hash = hash_bytes(bytes);
        let root = self.edit_resource_root()?;
        let target = root.join(&hash);
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        admit_resource(&tx, &hash, kind, bytes.len() as i64)?;
        publish_resource(&root, &target, bytes)?;
        verify_file(&target, &hash, bytes.len() as u64)?;
        tx.execute("INSERT OR IGNORE INTO edit_resources(hash,kind,size_bytes,created_at) VALUES(?1,?2,?3,?4)",params![hash,kind.key(),bytes.len() as i64,now()])?;
        tx.commit()?;
        Ok(EditResource {
            hash,
            kind,
            size_bytes: bytes.len() as u64,
        })
    }
    pub fn edit_resource(&self, hash: &str) -> Result<EditResource> {
        iris_render::document::validate_hash(hash)?;
        let (kind, size): (String, i64) = self.store.conn.query_row(
            "SELECT kind,size_bytes FROM edit_resources WHERE hash=?1",
            [hash],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let kind = match kind.as_str() {
            "lut" => EditResourceKind::Lut,
            "icc" => EditResourceKind::Icc,
            "lens_profile" => EditResourceKind::LensProfile,
            _ => anyhow::bail!("invalid stored resource kind"),
        };
        ensure!(
            size > 0 && size as u64 <= MAX_RESOURCE_BYTES,
            "invalid stored resource size"
        );
        Ok(EditResource {
            hash: hash.into(),
            kind,
            size_bytes: size as u64,
        })
    }
    pub fn read_edit_resource(&self, hash: &str) -> Result<Vec<u8>> {
        let resource = self.edit_resource(hash)?;
        let bytes = verify_file(
            &self.edit_resource_root()?.join(hash),
            hash,
            resource.size_bytes,
        )?;
        validate_blob(resource.kind, &bytes)?;
        Ok(bytes)
    }
    pub(in crate::services) fn verify_edit_resources(&self, document: &EditDocument) -> Result<()> {
        for hash in document.resources() {
            let resource = self.edit_resource(&hash)?;
            ensure!(
                resource.kind == EditResourceKind::Lut,
                "graph LUT references a non-LUT resource"
            );
            self.read_edit_resource(&hash)?;
        }
        Ok(())
    }
}

fn verify_file(path: &std::path::Path, hash: &str, size: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).context("edit resource is missing")?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() == size,
        "invalid edit resource file"
    );
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_RESOURCE_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == size && hash_bytes(&bytes) == hash,
        "edit resource hash mismatch"
    );
    Ok(bytes)
}
