use super::scan::{record_walk_error, scan_roots};
use super::*;

type Prior = (i64, i64, u64, bool);
struct FileInfo {
    relative: String,
    path: PathBuf,
    format: String,
    prior: Option<Prior>,
    metadata: Result<(u64, i64)>,
    details: Option<Result<(u32, u32, Option<String>)>>,
}

impl Services {
    pub(super) fn scan_parallel(
        &mut self,
        project_id: i64,
        selected: Option<&[String]>,
        cancel: &AtomicBool,
        progress: &mut impl FnMut(&ScanReport),
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
        let prior = read_prior(&self.store.conn, project_id)?;
        let mut seen = HashSet::new();
        let roots = scan_roots(root, selected)?;
        let mut batch = Vec::new();
        // Publish the first file immediately; later batches amortize durable commits.
        let mut batch_limit = 1;
        for entry in scan_entries(&roots) {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                break;
            }
            collect_entry(entry, root, &mut seen, &mut batch, &mut report)?;
            if batch.len() >= batch_limit {
                self.scan_batch(project_id, root, &mut batch, &prior, cancel, &mut report)?;
                progress(&report);
                batch_limit = 32;
            }
        }
        self.finish_scan(
            project_id,
            selected,
            root,
            &seen,
            &prior,
            &mut batch,
            cancel,
            &mut report,
            progress,
        )?;
        Ok(report)
    }

    fn finish_scan(
        &mut self,
        project_id: i64,
        selected: Option<&[String]>,
        root: &Path,
        seen: &HashSet<String>,
        prior: &HashMap<String, Prior>,
        batch: &mut Vec<(PathBuf, String, String)>,
        cancel: &AtomicBool,
        report: &mut ScanReport,
        progress: &mut impl FnMut(&ScanReport),
    ) -> Result<()> {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
        }
        if !report.cancelled && !batch.is_empty() {
            self.scan_batch(project_id, root, batch, prior, cancel, report)?;
            progress(report);
        }
        self.mark_missing_scanned_photos(project_id, selected, seen, report)?;
        if report.changed > 0 || report.missing > 0 {
            self.store
                .conn
                .execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
        }
        Ok(())
    }

    fn scan_batch(
        &mut self,
        project_id: i64,
        root: &Path,
        batch: &mut Vec<(PathBuf, String, String)>,
        prior: &HashMap<String, Prior>,
        cancel: &AtomicBool,
        report: &mut ScanReport,
    ) -> Result<()> {
        let inputs = std::mem::take(batch);
        let workers = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .saturating_sub(2)
            .clamp(1, 8)
            .min(inputs.len());
        let runtime = self.media_runtime_dir.as_deref();
        let mut files = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|worker| {
                    let inputs = &inputs;
                    scope.spawn(move || {
                        inputs
                            .iter()
                            .enumerate()
                            .skip(worker)
                            .step_by(workers)
                            .filter_map(|(index, (path, relative, format))| {
                                if cancel.load(Ordering::Relaxed) {
                                    return None;
                                }
                                Some((
                                    index,
                                    inspect_file(
                                        path,
                                        relative,
                                        format,
                                        prior.get(relative).copied(),
                                        runtime,
                                    ),
                                ))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("scan worker panicked"))
                .collect::<Vec<_>>()
        });
        files.sort_by_key(|(index, _)| *index);
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            return Ok(());
        }
        let tx = self.store.conn.transaction()?;
        for (_, file) in files {
            persist_file(&tx, project_id, root, file, report)?;
        }
        tx.commit()?;
        Ok(())
    }
}
fn inspect_file(
    path: &Path,
    relative: &str,
    format: &str,
    prior: Option<Prior>,
    runtime: Option<&Path>,
) -> FileInfo {
    let metadata = fs::metadata(path)
        .map_err(anyhow::Error::from)
        .and_then(|m| Ok((m.len(), modified(&m)?)));
    let unchanged = metadata.as_ref().ok().is_some_and(|(size, mtime)| {
        prior.is_some_and(|(_, t, s, q)| t == *mtime && s == *size && !q)
    });
    let details = if unchanged || metadata.is_err() {
        None
    } else {
        Some((|| {
            let (w, h) = crate::vision::image_dimensions_with_media(path, runtime)?;
            let taken_at = read_taken_at(path);
            let current = fs::metadata(path)?;
            let (size, mtime) = metadata.as_ref().unwrap();
            anyhow::ensure!(
                current.len() == *size && modified(&current)? == *mtime,
                "file changed during scan"
            );
            Ok((w, h, taken_at))
        })())
    };
    FileInfo {
        path: path.to_owned(),
        relative: relative.into(),
        format: format.into(),
        prior,
        metadata,
        details,
    }
}
fn persist_file(
    tx: &rusqlite::Transaction<'_>,
    project_id: i64,
    root: &Path,
    mut file: FileInfo,
    report: &mut ScanReport,
) -> Result<()> {
    let (size, mtime) = match &file.metadata {
        Ok(metadata) => *metadata,
        Err(error) => {
            report.failed_paths.push(file.relative.clone());
            report.errors.push(format!("{}: {error}", file.relative));
            return Ok(());
        }
    };
    match file.details.take() {
        None => {
            if let Some((id, _, _, _)) = file.prior {
                tx.execute("UPDATE photos SET missing=0 WHERE id=?1", [id])?;
            }
            report.unchanged += 1;
        }
        Some(Err(error)) => {
            report.failed_paths.push(file.relative.clone());
            report.errors.push(format!("{}: {error}", file.relative));
            if let Some((id, _, _, _)) = file.prior {
                tx.execute("DELETE FROM analyses WHERE photo_id=?1", [id])?;
                tx.execute("DELETE FROM faces WHERE photo_id=?1", [id])?;
                tx.execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
            }
        }
        Some(Ok(dimensions)) => {
            upsert_file(tx, project_id, root, &file, size, mtime, dimensions, report)?;
        }
    }
    Ok(())
}

fn collect_entry(
    entry: std::result::Result<walkdir::DirEntry, walkdir::Error>,
    root: &Path,
    seen: &mut HashSet<String>,
    batch: &mut Vec<(PathBuf, String, String)>,
    report: &mut ScanReport,
) -> Result<()> {
    match entry {
        Ok(entry) if entry.file_type().is_file() => {
            let Some(format) = crate::vision::supported_format(entry.path()) else {
                report.skipped += 1;
                return Ok(());
            };
            let relative = relative_text(entry.path().strip_prefix(root)?);
            if seen.insert(relative.clone()) {
                batch.push((entry.into_path(), relative, format.to_owned()));
            }
        }
        Ok(_) => {}
        Err(error) => {
            record_walk_error(root, error, report);
        }
    }
    Ok(())
}
fn read_prior(conn: &rusqlite::Connection, project_id: i64) -> Result<HashMap<String, Prior>> {
    let mut query = conn
        .prepare("SELECT path,id,mtime,size_bytes,quarantined FROM photos WHERE project_id=?1")?;
    let rows = query
        .query_map([project_id], |row| {
            Ok((
                row.get(0)?,
                (
                    row.get(1)?,
                    row.get(2)?,
                    row.get::<_, i64>(3)? as u64,
                    row.get(4)?,
                ),
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}
fn scan_entries(
    roots: &[PathBuf],
) -> impl Iterator<Item = std::result::Result<walkdir::DirEntry, walkdir::Error>> + '_ {
    roots.iter().flat_map(|start| {
        WalkDir::new(start)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                ![".iris-quarantine", ".iris-cache", ".iris"]
                    .iter()
                    .any(|name| e.file_name() == *name)
            })
    })
}

fn upsert_file(
    tx: &rusqlite::Transaction<'_>,
    project_id: i64,
    root: &Path,
    file: &FileInfo,
    size: u64,
    mtime: i64,
    dimensions: (u32, u32, Option<String>),
    report: &mut ScanReport,
) -> Result<()> {
    let (width, height, taken_at) = dimensions;
    let variant = relative_text(&file.path.strip_prefix(root)?.with_extension(""));
    tx.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height,taken_at,capture_variant_id) VALUES(?1,?2,?3,?10,?4,?5,?6,?7,?8,?9) ON CONFLICT(project_id,path) DO UPDATE SET format=excluded.format,mtime=excluded.mtime,size_bytes=excluded.size_bytes,width=excluded.width,height=excluded.height,taken_at=excluded.taken_at,missing=0",
                        params![project_id,file.relative,file.path.file_name().unwrap().to_string_lossy(),mtime,i64::try_from(size)?,width,height,taken_at,variant,file.format])?;
    if let Some((id, _, _, _)) = file.prior {
        tx.execute("DELETE FROM analyses WHERE photo_id=?1", [id])?;
        tx.execute("DELETE FROM faces WHERE photo_id=?1", [id])?;
        tx.execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
        report.changed += 1;
    } else {
        report.added += 1;
    }
    Ok(())
}
