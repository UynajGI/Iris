use super::*;

impl Services {
    pub fn cached_image(&self, photo_id: i64, edge: u32) -> Result<Vec<u8>> {
        if edge != 320 && edge != 1280 && edge != 2560 {
            bail!("supported preview edges are 320, 1280 and 2560");
        }
        let photo = self.photo(photo_id)?;
        let source = self.photo_path(photo_id)?;
        let project = self.project(photo.project_id)?;
        let root = Path::new(&project.cache_root);
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
        if destination.exists() {
            return Ok(fs::read(destination)?);
        }
        let preview = crate::vision::decode_preview_with_media(
            &source,
            edge.max(1280),
            self.media_runtime_dir.as_deref(),
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
        // A competing renderer may populate the cache; never truncate that file.
        let temporary = destination.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        // Publish atomically without replacing a competing renderer's file.
        let publication = crate::cache_publish::publish_noclobber(&temporary, &destination);
        let _ = fs::remove_file(&temporary);
        match publication {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        };
        Ok(bytes)
    }
}
