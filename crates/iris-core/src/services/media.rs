use super::*;

/// File-only work prepared while the caller owns the database, executed without it.
pub struct CachedImageJob {
    source: PathBuf,
    destination: PathBuf,
    edge: u32,
    media_runtime: Option<PathBuf>,
    mtime: i64,
    size: u64,
}

impl CachedImageJob {
    pub fn render(&self, cancelled: &std::sync::atomic::AtomicBool) -> Result<Vec<u8>> {
        let check = || -> Result<()> {
            anyhow::ensure!(
                !cancelled.load(std::sync::atomic::Ordering::Relaxed),
                "preview cancelled"
            );
            let metadata = fs::metadata(&self.source)?;
            anyhow::ensure!(
                modified(&metadata)? == self.mtime && metadata.len() == self.size,
                "preview source changed"
            );
            Ok(())
        };
        check()?;
        if self.destination.exists() {
            let bytes = fs::read(&self.destination)?;
            check()?;
            return Ok(bytes);
        }
        let bytes = render_cached_image(
            &self.source,
            self.edge,
            self.media_runtime.as_deref(),
            cancelled,
        )?;
        check()?;
        publish_cached_image(&self.destination, &bytes)?;
        Ok(bytes)
    }
}

impl Services {
    pub fn cached_image(&self, photo_id: i64, edge: u32) -> Result<Vec<u8>> {
        self.prepare_cached_image(photo_id, edge)?
            .render(&std::sync::atomic::AtomicBool::new(false))
    }

    pub fn prepare_cached_image(&self, photo_id: i64, edge: u32) -> Result<CachedImageJob> {
        if edge != 320 && edge != 1280 && edge != 2560 {
            bail!("supported preview edges are 320, 1280 and 2560");
        }
        let source = self.photo_path(photo_id)?;
        let cache_root: String = self.store.conn.query_row(
            "SELECT projects.cache_root FROM projects JOIN photos ON photos.project_id=projects.id WHERE photos.id=?1",
            [photo_id], |row| row.get(0))?;
        let root = Path::new(&cache_root);
        fs::create_dir_all(root)?;
        let metadata = fs::metadata(&source)?;
        let filename = format!(
            "v2-{}-{}-{}-{}.jpg",
            photo_id,
            modified(&metadata)?,
            metadata.len(),
            edge
        );
        let destination = safe_destination(root, &filename)?;
        Ok(CachedImageJob {
            source,
            destination,
            edge,
            media_runtime: self.media_runtime_dir.clone(),
            mtime: modified(&metadata)?,
            size: metadata.len(),
        })
    }
}

fn render_cached_image(
    source: &Path,
    edge: u32,
    media_runtime: Option<&Path>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>> {
    let preview = crate::vision::decode_preview_cancellable(
        source,
        edge.max(1280),
        media_runtime,
        Some(cancel),
    )?;
    let image = if preview.image.width().max(preview.image.height()) > edge {
        image::DynamicImage::ImageRgb8(preview.image)
            .resize(edge, edge, image::imageops::FilterType::Triangle)
            .to_rgb8()
    } else {
        preview.image
    };
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85).encode_image(&image)?;
    Ok(bytes)
}

pub(super) fn publish_cached_image(destination: &Path, bytes: &[u8]) -> Result<()> {
    // A competing renderer may populate the cache; never truncate that file.
    let temporary = destination.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    // Publish atomically without replacing a competing renderer's file.
    let publication = crate::cache_publish::publish_noclobber(&temporary, destination);
    let _ = fs::remove_file(&temporary);
    match publication {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    };
    Ok(())
}
