use super::*;

impl Services {
    pub fn cache_status(&self, project_id: i64) -> Result<CacheStatus> {
        let project = self.project(project_id)?;
        let mut status = CacheStatus {
            root: project.cache_root.clone(),
            files: 0,
            bytes: 0,
        };
        if Path::new(&project.cache_root).exists() {
            for item in WalkDir::new(&project.cache_root).follow_links(false) {
                let item = item?;
                if item.file_type().is_file() {
                    status.files += 1;
                    status.bytes += item.metadata()?.len();
                }
            }
        }
        Ok(status)
    }
    pub fn cache_cleanup(&mut self, project_id: i64) -> Result<CacheStatus> {
        let status = self.cache_status(project_id)?;
        let root = Path::new(&status.root);
        if root.exists() {
            let canonical = root.canonicalize()?;
            self.guard_cache_storage(&canonical, Some(project_id), Some(project_id))?;
            for entry in WalkDir::new(&canonical)
                .follow_links(false)
                .min_depth(1)
                .contents_first(true)
            {
                let entry = entry?;
                if entry.file_type().is_dir() {
                    fs::remove_dir(entry.path())?;
                } else {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        self.cache_status(project_id)
    }
    pub fn cache_migrations(&self, project_id: i64) -> Result<Vec<CacheMigration>> {
        self.project(project_id)?;
        let mut query = self.store.conn.prepare("SELECT data FROM cache_migrations WHERE project_id=?1 ORDER BY created_at DESC,rowid DESC")?;
        let rows = query.query_map([project_id], |r| r.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub(super) fn persist_cache_migration(&self, migration: &CacheMigration) -> Result<()> {
        self.store.conn.execute("INSERT INTO cache_migrations(id,project_id,state,data,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET state=excluded.state,data=excluded.data",
            params![migration.id,migration.project_id,migration.state,serde_json::to_string(migration)?,migration.created_at])?;
        Ok(())
    }
    pub(super) fn guard_cache_storage(
        &self,
        candidate: &Path,
        own_cache: Option<i64>,
        managed_project: Option<i64>,
    ) -> Result<()> {
        // Include hidden projects: hiding history never relinquishes file protection.
        let mut query = self
            .store
            .conn
            .prepare("SELECT id,root,cache_root FROM projects")?;
        let rows = query.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, photos, cache) = row?;
            let photos = resolve_storage_root(Path::new(&photos))?;
            if paths_overlap(candidate, &photos) {
                // The CLI's default .iris/cache lives inside scan('.'), but the
                // scanner excludes this managed subtree. Do not exempt any
                // other project or any directory containing registered photos.
                let relative = candidate.strip_prefix(&photos).ok();
                let managed = relative.is_some_and(|p| {
                    p.starts_with(Path::new(".iris").join("cache")) || p.starts_with(".iris-cache")
                });
                anyhow::ensure!(
                    Some(id) == managed_project && managed,
                    "cache path overlaps a photo project"
                );
                let mut tracked = self
                    .store
                    .conn
                    .prepare("SELECT path FROM photos WHERE project_id=?1")?;
                for path in tracked.query_map([id], |r| r.get::<_, String>(0))? {
                    let path = photos.join(checked_relative(&path?)?);
                    anyhow::ensure!(
                        !path.starts_with(candidate)
                            && !resolve_storage_root(&path)?.starts_with(candidate),
                        "cache path contains a registered photo"
                    );
                }
            }
            if Some(id) != own_cache {
                let cache = resolve_storage_root(Path::new(&cache))?;
                anyhow::ensure!(
                    !paths_overlap(candidate, &cache),
                    "cache path overlaps a current project cache"
                );
            }
        }
        Ok(())
    }
    pub fn cache_migrate(&mut self, project_id: i64, destination: &Path) -> Result<CacheStatus> {
        let project = self.project(project_id)?;
        let source = resolve_storage_root(Path::new(&project.cache_root))?;
        let destination = resolve_storage_root(destination)?;
        if source == destination {
            return self.cache_status(project_id);
        }
        anyhow::ensure!(
            !paths_overlap(&source, &destination),
            "cache folders must not overlap"
        );
        self.guard_cache_storage(&source, Some(project_id), Some(project_id))?;
        self.guard_cache_storage(&destination, None, Some(project_id))?;
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&destination)?;
        anyhow::ensure!(
            fs::read_dir(&destination)?.next().is_none(),
            "cache destination must be empty"
        );
        let mut files = Vec::new();
        for entry in WalkDir::new(&source).follow_links(false) {
            let entry = entry?;
            anyhow::ensure!(!entry.file_type().is_symlink(), "cache contains a symlink");
            if entry.file_type().is_file() {
                files.push(CacheMigrationFile {
                    path: relative_text(entry.path().strip_prefix(&source)?),
                    size_bytes: entry.metadata()?.len(),
                    sha256: digest(entry.path())?,
                    cleaned: false,
                });
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut migration = CacheMigration {
            id: uuid::Uuid::new_v4().to_string(),
            project_id,
            source_root: source.to_string_lossy().into_owned(),
            destination_root: destination.to_string_lossy().into_owned(),
            created_at: now(),
            state: "copying".into(),
            files,
            error: None,
        };
        self.persist_cache_migration(&migration)?;
        let result = (|| -> Result<()> {
            for file in &migration.files {
                let src = source.join(checked_relative(&file.path)?);
                anyhow::ensure!(
                    restore_file_exists(&source, &src, &file.sha256)?,
                    "cache source disappeared"
                );
                anyhow::ensure!(
                    fs::metadata(&src)?.len() == file.size_bytes,
                    "cache source size changed"
                );
                let dst = safe_destination(&destination, &file.path)?;
                copy_new_verified(&src, &dst)?;
                anyhow::ensure!(
                    restore_file_exists(&destination, &dst, &file.sha256)?,
                    "migrated cache missing"
                );
            }
            let tx = self.store.conn.unchecked_transaction()?;
            migration.state = "ready".into();
            self.persist_cache_migration(&migration)?;
            self.store.conn.execute(
                "UPDATE projects SET cache_root=?1 WHERE id=?2",
                params![migration.destination_root, project_id],
            )?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = result {
            migration.state = "copy_failed".into();
            migration.error = Some(format!("{error:#}"));
            self.persist_cache_migration(&migration)?;
            return Err(error.context(format!(
                "cache migration failed; original cache retained; migration {}",
                migration.id
            )));
        }
        self.cache_status(project_id)
    }
    pub fn cache_cleanup_old(
        &mut self,
        project_id: i64,
        migration_id: &str,
    ) -> Result<CacheMigration> {
        let data: String = self.store.conn.query_row(
            "SELECT data FROM cache_migrations WHERE id=?1 AND project_id=?2",
            params![migration_id, project_id],
            |r| r.get(0),
        )?;
        let mut migration: CacheMigration = serde_json::from_str(&data)?;
        anyhow::ensure!(
            migration.id == migration_id && migration.project_id == project_id,
            "invalid cache migration ownership"
        );
        if migration.state == "cleaned" {
            return Ok(migration);
        }
        anyhow::ensure!(
            ["ready", "cleanup_in_progress"].contains(&migration.state.as_str()),
            "cache migration is not ready for cleanup"
        );
        // Keep project/cache assignments stable while verifying and removing files.
        // Missing listed files on retry cover a process exit after unlink but before commit.
        let tx = self.store.conn.unchecked_transaction()?;
        migration.state = "cleanup_in_progress".into();
        migration.error = None;
        self.persist_cache_migration(&migration)?;
        let result = (|| -> Result<()> {
            let source = resolve_storage_root(Path::new(&migration.source_root))?;
            let destination = resolve_storage_root(Path::new(&migration.destination_root))?;
            anyhow::ensure!(
                source == Path::new(&migration.source_root)
                    && destination == Path::new(&migration.destination_root),
                "cache migration root changed through a link"
            );
            anyhow::ensure!(
                !paths_overlap(&source, &destination),
                "cache migration roots overlap"
            );
            self.guard_cache_storage(&source, None, Some(project_id))?;
            let validate = |file: &CacheMigrationFile| -> Result<Option<PathBuf>> {
                let path = source.join(checked_relative(&file.path)?);
                if !restore_file_exists(&source, &path, &file.sha256)? {
                    return Ok(None);
                }
                anyhow::ensure!(
                    fs::metadata(&path)?.len() == file.size_bytes,
                    "old cache size changed"
                );
                let copied = destination.join(checked_relative(&file.path)?);
                anyhow::ensure!(
                    restore_file_exists(&destination, &copied, &file.sha256)?,
                    "verified migrated copy is missing"
                );
                anyhow::ensure!(
                    fs::metadata(&copied)?.len() == file.size_bytes,
                    "migrated cache size changed"
                );
                Ok(Some(path))
            };
            // Known conflicts must fail before any deletion.
            for file in migration.files.iter().filter(|file| !file.cleaned) {
                validate(file)?;
            }
            for index in 0..migration.files.len() {
                if migration.files[index].cleaned {
                    continue;
                }
                if let Some(path) = validate(&migration.files[index])? {
                    fs::remove_file(path)?;
                }
                migration.files[index].cleaned = true;
                self.persist_cache_migration(&migration)?;
            }
            // Unlisted files and directories are deliberately never removed.
            migration.state = "cleaned".into();
            self.persist_cache_migration(&migration)?;
            Ok(())
        })();
        if let Err(error) = result {
            migration.error = Some(format!("{error:#}"));
            self.persist_cache_migration(&migration)?;
            tx.commit()?;
            return Err(error.context(format!(
                "old cache cleanup incomplete; retry migration {migration_id}"
            )));
        }
        tx.commit()?;
        Ok(migration)
    }
}
