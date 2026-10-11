use super::*;

impl Services {
    pub fn export_xmp(
        &self,
        project_id: i64,
        scope: &str,
        overwrite: bool,
    ) -> Result<ExportReport> {
        let photos = self.export_selection(project_id, scope)?;
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
        let mut report = ExportReport {
            written: 0,
            skipped: 0,
            paths: vec![],
        };
        for (p, dest) in planned {
            let rating = match p.decision {
                Action::Keep => 5,
                Action::Reject => -1,
                _ => 0,
            };
            let label = if p.decision == Action::Flag {
                "Select"
            } else {
                ""
            };
            let body=format!("<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n<x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><rdf:Description rdf:about='' xmlns:xmp='http://ns.adobe.com/xap/1.0/' xmp:Rating='{rating}' xmp:Label='{label}'/></rdf:RDF></x:xmpmeta>\n<?xpacket end='w'?>\n");
            if dest.exists() && !overwrite {
                report.skipped += 1;
                continue;
            }
            // Refuse links even in explicit overwrite mode; sidecars never modify image bytes.
            if fs::symlink_metadata(&dest).is_ok_and(|m| m.file_type().is_symlink()) {
                bail!("refusing symlink sidecar");
            }
            let temp = dest.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp)?;
            f.write_all(body.as_bytes())?;
            f.sync_all()?;
            drop(f);
            if dest.exists() && overwrite {
                let backup = dest.with_extension(format!("xmp.{}.bak", uuid::Uuid::new_v4()));
                fs::rename(&dest, &backup)?;
                if let Err(e) = fs::rename(&temp, &dest) {
                    let _ = fs::rename(&backup, &dest);
                    return Err(e.into());
                }
            } else {
                if dest.exists() {
                    let _ = fs::remove_file(&temp);
                    report.skipped += 1;
                    continue;
                }
                fs::rename(&temp, &dest)?;
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
