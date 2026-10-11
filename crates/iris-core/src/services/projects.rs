use super::*;

impl Services {
    pub fn create_project(&mut self, root: impl AsRef<Path>) -> Result<Project> {
        self.create_project_with_execution(root, crate::vision::ExecutionProvider::Cpu)
    }
    pub fn create_project_with_execution(
        &mut self,
        root: impl AsRef<Path>,
        provider: crate::vision::ExecutionProvider,
    ) -> Result<Project> {
        let root = root
            .as_ref()
            .canonicalize()
            .context("project folder must exist")?;
        if !root.is_dir() {
            bail!("project root is not a directory");
        }
        let root_text = root.to_string_lossy().into_owned();
        let cache = self
            .store
            .path
            .parent()
            .unwrap_or(Path::new("."))
            .join("cache")
            .join(uuid::Uuid::new_v4().to_string());
        let inserted = self.store.conn.execute(
            "INSERT OR IGNORE INTO projects(root,name,created_at,cache_root,groups_dirty) VALUES(?1,?2,?3,?4,0)",
            params![
                root_text,
                root.file_name().unwrap_or_default().to_string_lossy(),
                now(),
                cache.to_string_lossy()
            ],
        )?;
        let id = self.store.conn.query_row(
            "SELECT id FROM projects WHERE root=?1",
            [root_text],
            |r| r.get(0),
        )?;
        if inserted > 0 && provider != crate::vision::ExecutionProvider::Cpu {
            let settings = crate::vision::AnalysisSettings {
                execution_provider: provider,
                ..Default::default()
            };
            self.store.conn.execute(
                "INSERT INTO settings(project_id,data) VALUES(?1,?2)",
                params![id, serde_json::to_string(&settings)?],
            )?;
            self.store
                .conn
                .execute("UPDATE projects SET groups_dirty=0 WHERE id=?1", [id])?;
        }
        self.open_project(id)
    }
    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut q = self.store.conn.prepare(&format!(
            "{PROJECT_SELECT} WHERE p.hidden=0 ORDER BY p.last_opened_at DESC,p.id DESC"
        ))?;
        let projects = q
            .query_map([crate::vision::ANALYSIS_VERSION], project_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }
    pub fn project(&self, id: i64) -> Result<Project> {
        Ok(self.store.conn.query_row(
            &format!("{PROJECT_SELECT} WHERE p.id=?2"),
            params![crate::vision::ANALYSIS_VERSION, id],
            project_row,
        )?)
    }
    /// Check project existence without computing project-wide readiness counts.
    pub fn require_project(&self, project_id: i64) -> Result<()> {
        let exists: bool = self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            [project_id],
            |r| r.get(0),
        )?;
        anyhow::ensure!(exists, "project does not exist");
        Ok(())
    }
    pub fn save_analysis_run(&self, project_id: i64, report: &Value) -> Result<()> {
        self.require_project(project_id)?;
        self.store.conn.execute(
            "INSERT INTO analysis_runs(project_id,data,completed_at) VALUES(?1,?2,?3) ON CONFLICT(project_id) DO UPDATE SET data=excluded.data,completed_at=excluded.completed_at",
            params![project_id, report.to_string(), chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }
    pub fn analysis_run(&self, project_id: i64) -> Result<Option<Value>> {
        self.require_project(project_id)?;
        let text: Option<String> = self
            .store
            .conn
            .query_row(
                "SELECT data FROM analysis_runs WHERE project_id=?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()?;
        text.map(|text| serde_json::from_str(&text).map_err(Into::into))
            .transpose()
    }
    pub fn open_project(&mut self, id: i64) -> Result<Project> {
        let opened = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        anyhow::ensure!(
            self.store.conn.execute(
                "UPDATE projects SET last_opened_at=?1,hidden=0 WHERE id=?2",
                params![opened, id]
            )? == 1,
            "project not found"
        );
        self.project(id)
    }
    pub fn hide_project(&mut self, id: i64) -> Result<Project> {
        anyhow::ensure!(
            self.store
                .conn
                .execute("UPDATE projects SET hidden=1 WHERE id=?1", [id])?
                == 1,
            "project not found"
        );
        self.project(id)
    }
}
