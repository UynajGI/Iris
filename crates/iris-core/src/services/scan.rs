use super::*;

type PriorPhoto = (i64, i64, u64, bool);
struct ScanFile<'a> {
    project_id: i64,
    root: &'a Path,
    entry: &'a walkdir::DirEntry,
    relative: String,
    format: String,
    mtime: i64,
    size: u64,
    prior: Option<PriorPhoto>,
}

impl Services {
    pub fn scan(&mut self, project_id: i64) -> Result<ScanReport> {
        self.scan_with_cancel(project_id, &AtomicBool::new(false), |_| {})
    }
    pub fn scan_with_cancel(
        &mut self,
        project_id: i64,
        cancel: &AtomicBool,
        progress: impl FnMut(&ScanReport),
    ) -> Result<ScanReport> {
        self.scan_scoped_with_cancel(project_id, None, cancel, progress)
    }

    pub fn scan_scoped_with_cancel(
        &mut self,
        project_id: i64,
        selected: Option<&[String]>,
        cancel: &AtomicBool,
        mut progress: impl FnMut(&ScanReport),
    ) -> Result<ScanReport> {
        let project = self.project(project_id)?;
        let root = Path::new(&project.root);
        let mut report = ScanReport::default();
        if let Err(error) = fs::read_dir(root) {
            report.root_unavailable = true;
            report.failed_paths.push(String::new());
            report.errors.push(format!("{}: {error}", root.display()));
            progress(&report);
            return Ok(report);
        }
        let roots = scan_roots(root, selected)?;
        let mut seen = HashSet::new();
        for entry in roots.iter().flat_map(|start| {
            WalkDir::new(start)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    e.file_name() != ".iris-quarantine"
                        && e.file_name() != ".iris-cache"
                        && e.file_name() != ".iris"
                })
        }) {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                break;
            }
            match entry {
                Ok(entry) => self.scan_entry(
                    project_id,
                    root,
                    &entry,
                    &mut seen,
                    &mut report,
                    &mut progress,
                )?,
                Err(error) => {
                    record_walk_error(root, error, &mut report);
                    progress(&report);
                }
            }
        }
        self.mark_missing_scanned_photos(project_id, selected, &seen, &mut report)?;
        if report.changed > 0 || report.missing > 0 {
            self.store
                .conn
                .execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
        }
        Ok(report)
    }

    fn scan_entry(
        &mut self,
        project_id: i64,
        root: &Path,
        entry: &walkdir::DirEntry,
        seen: &mut HashSet<String>,
        report: &mut ScanReport,
        progress: &mut impl FnMut(&ScanReport),
    ) -> Result<()> {
        if !entry.file_type().is_file() {
            return Ok(());
        }
        let path = entry.path();
        let Some(format) = crate::vision::supported_format(path) else {
            report.skipped += 1;
            return Ok(());
        };
        let rel = relative_text(path.strip_prefix(root)?);
        if !seen.insert(rel.clone()) {
            return Ok(());
        }
        let Some((size, mtime)) = scan_metadata(path, &rel, report) else {
            progress(report);
            return Ok(());
        };
        let prior: Option<PriorPhoto> = self.store.conn.query_row(
            "SELECT id,mtime,size_bytes,quarantined FROM photos WHERE project_id=?1 AND path=?2",
            params![project_id,rel], |r| Ok((r.get(0)?,r.get(1)?,r.get::<_,i64>(2)? as u64,r.get(3)?))
        ).optional()?;
        let file = ScanFile {
            project_id,
            root,
            entry,
            relative: rel,
            format: format.to_owned(),
            mtime,
            size,
            prior,
        };
        if self.scan_file_unchanged(&file)? {
            report.unchanged += 1;
            progress(report);
            return Ok(());
        }
        if let Some((width, height)) = self.scan_dimensions(&file, report)? {
            self.persist_scanned_file(&file, width, height, report)?;
        }
        progress(report);
        Ok(())
    }

    fn scan_file_unchanged(&self, file: &ScanFile<'_>) -> Result<bool> {
        if let Some((id, old_time, old_size, quarantined)) = file.prior {
            if old_time == file.mtime && old_size == file.size && !quarantined {
                self.store
                    .conn
                    .execute("UPDATE photos SET missing=0 WHERE id=?1", [id])?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn scan_dimensions(
        &mut self,
        file: &ScanFile<'_>,
        report: &mut ScanReport,
    ) -> Result<Option<(u32, u32)>> {
        match crate::vision::image_dimensions_with_media(
            file.entry.path(),
            self.media_runtime_dir.as_deref(),
        ) {
            Ok(dimensions) => Ok(Some(dimensions)),
            Err(error) => {
                report.failed_paths.push(file.relative.clone());
                report.errors.push(format!("{}: {error}", file.relative));
                if let Some((id, _, _, _)) = file.prior {
                    let tx = self.store.conn.transaction()?;
                    tx.execute("DELETE FROM analyses WHERE photo_id=?1", [id])?;
                    tx.execute("DELETE FROM faces WHERE photo_id=?1", [id])?;
                    tx.execute(
                        "DELETE FROM burst_groups WHERE project_id=?1",
                        [file.project_id],
                    )?;
                    tx.commit()?;
                }
                Ok(None)
            }
        }
    }

    fn persist_scanned_file(
        &mut self,
        file: &ScanFile<'_>,
        width: u32,
        height: u32,
        report: &mut ScanReport,
    ) -> Result<()> {
        let ScanFile {
            project_id,
            root,
            entry,
            relative: rel,
            format,
            mtime,
            size,
            prior,
        } = file;
        let path = entry.path();
        let taken_at = read_taken_at(path);
        let variant = relative_text(&path.strip_prefix(root)?.with_extension(""));
        let tx = self.store.conn.transaction()?;
        tx.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height,taken_at,capture_variant_id) VALUES(?1,?2,?3,?10,?4,?5,?6,?7,?8,?9) ON CONFLICT(project_id,path) DO UPDATE SET format=excluded.format,mtime=excluded.mtime,size_bytes=excluded.size_bytes,width=excluded.width,height=excluded.height,taken_at=excluded.taken_at,missing=0",params![project_id,rel,entry.file_name().to_string_lossy(),mtime,i64::try_from(*size)?,width,height,taken_at,variant,format])?;
        if let Some((id, _, _, _)) = prior {
            tx.execute("DELETE FROM analyses WHERE photo_id=?1", [id])?;
            tx.execute("DELETE FROM faces WHERE photo_id=?1", [id])?;
            report.changed += 1;
        } else {
            report.added += 1;
        }
        tx.commit()?;
        Ok(())
    }

    fn mark_missing_scanned_photos(
        &self,
        project_id: i64,
        selected: Option<&[String]>,
        seen: &HashSet<String>,
        report: &mut ScanReport,
    ) -> Result<()> {
        if !report.cancelled && report.errors.is_empty() {
            for photo in self.photos(
                project_id,
                PhotoFilter {
                    include_missing: Some(true),
                    ..Default::default()
                },
            )? {
                let in_scope = selected.is_none_or(|paths| {
                    paths
                        .iter()
                        .any(|path| Path::new(&photo.path).starts_with(path))
                });
                if in_scope && !seen.contains(&photo.path) && !photo.quarantined {
                    self.store
                        .conn
                        .execute("UPDATE photos SET missing=1 WHERE id=?1", [photo.id])?;
                    report.missing += 1;
                }
            }
        }
        Ok(())
    }
}

fn scan_roots(root: &Path, selected: Option<&[String]>) -> Result<Vec<PathBuf>> {
    let roots = match selected {
        None => vec![root.to_path_buf()],
        Some(paths) => paths
            .iter()
            .map(|relative| {
                anyhow::ensure!(
                    Path::new(relative)
                        .components()
                        .all(|part| matches!(part, Component::Normal(_))),
                    "scan retry path must be relative to the project"
                );
                let path = root.join(relative);
                if path.exists() {
                    anyhow::ensure!(
                        path.canonicalize()?.starts_with(root.canonicalize()?),
                        "scan retry path leaves the project"
                    );
                }
                Ok(path)
            })
            .collect::<Result<Vec<_>>>()?,
    };
    Ok(roots)
}

fn record_walk_error(root: &Path, error: walkdir::Error, report: &mut ScanReport) {
    let failed = error
        .path()
        .and_then(|path| path.strip_prefix(root).ok())
        .map(relative_text)
        .unwrap_or_default();
    if failed.is_empty() {
        report.root_unavailable = true;
    }
    report.failed_paths.push(failed);
    report.errors.push(error.to_string());
}

fn scan_metadata(path: &Path, relative: &str, report: &mut ScanReport) -> Option<(u64, i64)> {
    match fs::metadata(path)
        .map_err(anyhow::Error::from)
        .and_then(|meta| Ok((meta.len(), modified(&meta)?)))
    {
        Ok(value) => Some(value),
        Err(error) => {
            report.failed_paths.push(relative.to_owned());
            report.errors.push(format!("{relative}: {error}"));
            None
        }
    }
}
