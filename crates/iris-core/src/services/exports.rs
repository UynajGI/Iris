use super::*;

mod xmp;

impl Services {
    pub fn export_xmp(
        &self,
        project_id: i64,
        scope: &str,
        overwrite: bool,
    ) -> Result<ExportReport> {
        let photos = self.export_selection(project_id, scope)?;
        let planned = self.plan_xmp_destinations(photos)?;
        self.write_xmp_sidecars(planned, overwrite)
    }
    fn plan_xmp_destinations(&self, photos: Vec<Photo>) -> Result<Vec<(Photo, PathBuf)>> {
        // Distinct JPEG names can share a sidecar (a.jpg and a.jpeg). Check the
        // whole selected set before writing, including when overwrite is enabled.
        let mut destinations = HashMap::<_, usize>::new();
        let mut planned: Vec<(Photo, PathBuf)> = Vec::with_capacity(photos.len());
        for photo in photos {
            let destination = self.photo_path(photo.id)?.with_extension("xmp");
            #[cfg(windows)]
            let key = destination.to_string_lossy().to_lowercase();
            #[cfg(not(windows))]
            let key = destination.clone();
            if let Some(index) = destinations.get(&key).copied() {
                let previous: &Photo = &planned[index].0;
                let raw_jpeg = matches!(
                    (previous.format.as_str(), photo.format.as_str()),
                    ("raw", "jpeg") | ("jpeg", "raw")
                );
                anyhow::ensure!(
                    raw_jpeg
                        && previous.decision == photo.decision
                        && previous.rating == photo.rating
                        && previous.color_label == photo.color_label
                        && previous.capture_variant_id.is_some()
                        && previous.capture_variant_id == photo.capture_variant_id,
                    "multiple selected photos share XMP destination: {}",
                    destination.display()
                );
                continue;
            }
            destinations.insert(key, planned.len());
            planned.push((photo, destination));
        }
        Ok(planned)
    }
    fn write_xmp_sidecars(
        &self,
        planned: Vec<(Photo, PathBuf)>,
        overwrite: bool,
    ) -> Result<ExportReport> {
        let mut report = ExportReport {
            written: 0,
            skipped: 0,
            paths: vec![],
        };
        for (p, dest) in planned {
            if !write_sidecar(&dest, &p, overwrite)? {
                report.skipped += 1;
                continue;
            }
            report.written += 1;
            report.paths.push(dest.to_string_lossy().into_owned());
        }
        Ok(report)
    }
    pub(super) fn export_selection(&self, project_id: i64, scope: &str) -> Result<Vec<Photo>> {
        let decision = match scope {
            "keep" => Some(Action::Keep),
            "reject" => Some(Action::Reject),
            "all" => None,
            _ => bail!("scope must be keep, reject, or all"),
        };
        self.photos(
            project_id,
            PhotoFilter {
                decision,
                ..Default::default()
            },
        )
    }
    pub fn export_copy(
        &self,
        project_id: i64,
        destination: &Path,
        scope: &str,
    ) -> Result<ExportReport> {
        let project = self.project(project_id)?;
        fs::create_dir_all(destination)?;
        let destination = destination.canonicalize()?;
        let source_root = Path::new(&project.root).canonicalize()?;
        if destination == source_root || destination.starts_with(&source_root) {
            bail!("copy destination must be outside the project to prevent rescanning exported copies");
        }
        let mut report = ExportReport {
            written: 0,
            skipped: 0,
            paths: vec![],
        };
        for p in self.export_selection(project_id, scope)? {
            let src = self.photo_path(p.id)?;
            let dest = safe_destination(&destination, &p.path)?;
            if dest.exists() {
                report.skipped += 1;
                continue;
            }
            copy_new_verified(&src, &dest)?;
            report.written += 1;
            report.paths.push(dest.to_string_lossy().into_owned());
        }
        Ok(report)
    }
    pub fn export_csv(&self, project_id: i64, destination: &Path) -> Result<ExportReport> {
        let photos = self.photos(
            project_id,
            PhotoFilter {
                include_missing: Some(true),
                ..Default::default()
            },
        )?;
        let f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)?;
        let mut w = csv::Writer::from_writer(f);
        w.write_record(["path", "action"])?;
        for p in &photos {
            w.write_record([p.path.as_str(), p.decision.as_str()])?;
        }
        w.flush()?;
        w.get_ref().sync_all()?;
        Ok(ExportReport {
            written: photos.len(),
            skipped: 0,
            paths: vec![destination.to_string_lossy().into_owned()],
        })
    }
    pub fn import_csv(&mut self, project_id: i64, source: &Path) -> Result<DecisionBatch> {
        let photos: HashMap<String, i64> = self
            .photos(
                project_id,
                PhotoFilter {
                    include_missing: Some(true),
                    ..Default::default()
                },
            )?
            .into_iter()
            .map(|p| (p.path, p.id))
            .collect();
        let mut reader = csv::Reader::from_path(source)?;
        let headers = reader.headers()?.clone();
        let path_idx = headers
            .iter()
            .position(|s| s == "path")
            .context("CSV missing path column")?;
        let action_idx = headers
            .iter()
            .position(|s| s == "action")
            .context("CSV missing action column")?;
        let mut changes = HashMap::new();
        for row in reader.records() {
            let row = row?;
            let path = row.get(path_idx).context("missing path")?;
            checked_relative(path)?;
            let id = *photos
                .get(path)
                .with_context(|| format!("unknown photo path: {path}"))?;
            let action = row.get(action_idx).context("missing action")?.parse()?;
            if changes.insert(id, action).is_some() {
                bail!("duplicate CSV path: {path}");
            }
        }
        self.apply_decisions(project_id, changes, "csv_import", vec![])
    }
}

fn read_sidecar(dest: &Path) -> Result<Option<String>> {
    let metadata = match fs::symlink_metadata(dest) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "refusing non-regular or symlink sidecar"
    );
    let mut bytes = Vec::new();
    File::open(dest)?
        .take(xmp::MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() as u64 <= xmp::MAX_BYTES,
        "XMP sidecar exceeds size limit"
    );
    Ok(Some(
        String::from_utf8(bytes).context("XMP sidecar is not UTF-8")?,
    ))
}

fn write_sidecar(dest: &Path, photo: &Photo, overwrite: bool) -> Result<bool> {
    // Skip existing files without parsing when overwrite was not requested.
    if !overwrite && fs::symlink_metadata(dest).is_ok() {
        anyhow::ensure!(
            !fs::symlink_metadata(dest)?.file_type().is_symlink(),
            "refusing symlink sidecar"
        );
        return Ok(false);
    }
    let original = read_sidecar(dest)?;
    let body = xmp::merge(
        photo,
        original.as_deref(),
        &dest.with_file_name(&photo.filename),
    )?;
    let mut temp =
        tempfile::NamedTempFile::new_in(dest.parent().context("sidecar has no parent")?)?;
    temp.write_all(body.as_bytes())?;
    temp.as_file().sync_all()?;
    if let Some(original) = original {
        return replace_sidecar(dest, temp, &original);
    }
    match temp.persist_noclobber(dest) {
        Ok(_) => Ok(true),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.error.into()),
    }
}

fn replace_sidecar(dest: &Path, temp: tempfile::NamedTempFile, original: &str) -> Result<bool> {
    anyhow::ensure!(
        read_sidecar(dest)?.as_deref() == Some(original),
        "XMP changed during export; retry"
    );
    let backup = dest.with_extension(format!("xmp.{}.bak", uuid::Uuid::new_v4()));
    fs::rename(dest, &backup)?;
    if read_sidecar(&backup)?.as_deref() != Some(original) {
        restore_backup_if_absent(&backup, dest)?;
        bail!("XMP changed during export; retry");
    }
    if let Err(error) = temp.persist_noclobber(dest) {
        // Never replace a file created by another writer during the export.
        restore_backup_if_absent(&backup, dest)?;
        return Err(error.error).context("XMP export failed; original retained in backup");
    }
    Ok(true)
}

fn restore_backup_if_absent(backup: &Path, dest: &Path) -> Result<()> {
    match fs::symlink_metadata(dest) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::rename(backup, dest).context("restore XMP backup after failed export")?
        }
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    Ok(())
}
