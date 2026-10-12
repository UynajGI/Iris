use super::*;

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
        self.scan_parallel(project_id, selected, cancel, &mut progress)
    }

    pub(super) fn mark_missing_scanned_photos(
        &mut self,
        project_id: i64,
        selected: Option<&[String]>,
        seen: &HashSet<String>,
        report: &mut ScanReport,
    ) -> Result<()> {
        if !report.cancelled && report.errors.is_empty() {
            let photos = self.photos(
                project_id,
                PhotoFilter {
                    include_missing: Some(true),
                    ..Default::default()
                },
            )?;
            let tx = self.store.conn.transaction()?;
            for photo in photos {
                let in_scope = selected.is_none_or(|paths| {
                    paths
                        .iter()
                        .any(|path| Path::new(&photo.path).starts_with(path))
                });
                if in_scope && !seen.contains(&photo.path) && !photo.quarantined {
                    tx.execute("UPDATE photos SET missing=1 WHERE id=?1", [photo.id])?;
                    report.missing += 1;
                }
            }
            tx.commit()?;
        }
        Ok(())
    }
}

pub(super) fn scan_roots(root: &Path, selected: Option<&[String]>) -> Result<Vec<PathBuf>> {
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

pub(super) fn record_walk_error(root: &Path, error: walkdir::Error, report: &mut ScanReport) {
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
