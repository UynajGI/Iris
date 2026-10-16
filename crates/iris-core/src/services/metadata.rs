// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

/// Capture catalogue identity under the service lock; read outside it in the bounded media queue.
pub struct PhotoMetadataJob {
    source: PathBuf,
    mtime: i64,
    size: u64,
}
impl PhotoMetadataJob {
    pub fn read(&self, cancel: &AtomicBool) -> Result<Vec<u8>> {
        self.verify(cancel)?;
        let metadata = crate::metadata::read(&self.source, cancel)?;
        self.verify(cancel)?;
        Ok(serde_json::to_vec(&metadata)?)
    }
    fn verify(&self, cancel: &AtomicBool) -> Result<()> {
        anyhow::ensure!(
            !cancel.load(Ordering::Relaxed),
            "metadata request cancelled"
        );
        let metadata = fs::metadata(&self.source)?;
        anyhow::ensure!(
            modified(&metadata)? == self.mtime && metadata.len() == self.size,
            "metadata source changed; rescan the project"
        );
        Ok(())
    }
}
impl Services {
    pub fn prepare_photo_metadata(&self, id: i64) -> Result<PhotoMetadataJob> {
        let source = self.photo_path(id)?;
        let (mtime, size): (i64, i64) = self.store.conn.query_row(
            "SELECT mtime,size_bytes FROM photos WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(PhotoMetadataJob {
            source,
            mtime,
            size: u64::try_from(size)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_job_rejects_changed_missing_and_unavailable_sources() {
        let temp = tempfile::tempdir().unwrap();
        let photos = temp.path().join("photos");
        fs::create_dir(&photos).unwrap();
        let path = photos.join("synthetic.jpg");
        fs::write(&path, [0xff, 0xd8, 0xff, 0xd9]).unwrap();
        let source = fs::metadata(&path).unwrap();
        let mut services = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
        let project = services.create_project(&photos).unwrap();
        services.store.conn.execute(
            "INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'synthetic.jpg','synthetic.jpg','jpeg',?2,?3,32,24)",
            params![project.id, modified(&source).unwrap(), source.len()],
        ).unwrap();
        let id = services.store.conn.last_insert_rowid();
        let job = services.prepare_photo_metadata(id).unwrap();
        let data: crate::metadata::PhotoMetadata =
            serde_json::from_slice(&job.read(&AtomicBool::new(false)).unwrap()).unwrap();
        assert_eq!(data.status, crate::metadata::PhotoMetadataStatus::NotFound);
        assert!(job.read(&AtomicBool::new(true)).is_err());
        assert!(services.prepare_photo_metadata(id + 1).is_err());
        fs::write(&path, b"changed").unwrap();
        assert!(job.read(&AtomicBool::new(false)).is_err());
        fs::remove_file(&path).unwrap();
        assert!(job.read(&AtomicBool::new(false)).is_err());
        assert!(services.prepare_photo_metadata(id).is_err());
    }
}
