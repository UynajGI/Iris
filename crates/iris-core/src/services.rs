use crate::{domain::*, Store};
use anyhow::{bail, Context, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use walkdir::WalkDir;

pub struct Services {
    pub store: Store,
    media_runtime_dir: Option<PathBuf>,
}

// Private durable recovery details remain additive to the public quarantine plan.
#[derive(serde::Serialize, serde::Deserialize)]
struct RestoreJournal {
    #[serde(flatten)]
    plan: QuarantinePlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restore_rollback: Option<RestoreRollback>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct RestoreRollback {
    prior_state: String,
    steps: Vec<RestoreStep>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct RestoreStep {
    index: usize,
    source_existed: bool,
    quarantine_existed: bool,
    missing: bool,
    quarantined: bool,
    prior_item_state: String,
}

/// Shared cache/adoption gate. `settings` must be the canonical current settings.
pub fn analysis_matches_settings(analysis: &Value, settings: &Value) -> bool {
    if analysis.get("source_invalid").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    if analysis.get("version").and_then(Value::as_str) != Some(crate::vision::ANALYSIS_VERSION) {
        return false;
    }
    analysis
        .get("settings")
        .and_then(|value| {
            let parsed: crate::vision::AnalysisSettings =
                serde_json::from_value(value.clone()).ok()?;
            parsed.validate().ok()?;
            // Quality inference and semantic vectors have separate freshness.
            // Candidate selection may legitimately leave this photo without a vector.
            serde_json::to_value(parsed).ok()
        })
        .as_ref()
        == Some(settings)
}
impl Services {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            media_runtime_dir: None,
        }
    }
    pub fn with_media_runtime_dir(mut self, directory: PathBuf) -> Self {
        self.media_runtime_dir = Some(directory);
        self
    }
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
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    let failed = e
                        .path()
                        .and_then(|path| path.strip_prefix(root).ok())
                        .map(relative_text)
                        .unwrap_or_default();
                    if failed.is_empty() {
                        report.root_unavailable = true;
                    }
                    report.failed_paths.push(failed);
                    report.errors.push(e.to_string());
                    progress(&report);
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let Some(format) = crate::vision::supported_format(path) else {
                report.skipped += 1;
                continue;
            };
            let rel = relative_text(path.strip_prefix(root)?);
            if !seen.insert(rel.clone()) {
                continue;
            }
            let (meta, mtime) =
                match fs::metadata(path)
                    .map_err(anyhow::Error::from)
                    .and_then(|meta| {
                        let mtime = modified(&meta)?;
                        Ok((meta, mtime))
                    }) {
                    Ok(value) => value,
                    Err(error) => {
                        report.failed_paths.push(rel.clone());
                        report.errors.push(format!("{rel}: {error}"));
                        progress(&report);
                        continue;
                    }
                };
            let size = meta.len();
            let prior:Option<(i64,i64,u64,bool)>=self.store.conn.query_row("SELECT id,mtime,size_bytes,quarantined FROM photos WHERE project_id=?1 AND path=?2",params![project_id,rel],|r|Ok((r.get(0)?,r.get(1)?,r.get::<_,i64>(2)? as u64,r.get(3)?))).optional()?;
            if let Some((id, old_time, old_size, quarantined)) = prior {
                if old_time == mtime && old_size == size && !quarantined {
                    self.store
                        .conn
                        .execute("UPDATE photos SET missing=0 WHERE id=?1", [id])?;
                    report.unchanged += 1;
                    progress(&report);
                    continue;
                }
            }
            let (width, height) = match crate::vision::image_dimensions_with_media(
                path,
                self.media_runtime_dir.as_deref(),
            ) {
                Ok(d) => d,
                Err(e) => {
                    report.failed_paths.push(rel.clone());
                    report.errors.push(format!("{rel}: {e}"));
                    if let Some((id, _, _, _)) = prior {
                        let tx = self.store.conn.transaction()?;
                        tx.execute("DELETE FROM analyses WHERE photo_id=?1", [id])?;
                        tx.execute("DELETE FROM faces WHERE photo_id=?1", [id])?;
                        tx.execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
                        tx.commit()?;
                    }
                    progress(&report);
                    continue;
                }
            };
            let taken_at = read_taken_at(path);
            let variant = relative_text(&path.strip_prefix(root)?.with_extension(""));
            let tx = self.store.conn.transaction()?;
            tx.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height,taken_at,capture_variant_id) VALUES(?1,?2,?3,?10,?4,?5,?6,?7,?8,?9) ON CONFLICT(project_id,path) DO UPDATE SET format=excluded.format,mtime=excluded.mtime,size_bytes=excluded.size_bytes,width=excluded.width,height=excluded.height,taken_at=excluded.taken_at,missing=0",params![project_id,rel,entry.file_name().to_string_lossy(),mtime,i64::try_from(size)?,width,height,taken_at,variant,format])?;
            if let Some((id, _, _, _)) = prior {
                tx.execute("DELETE FROM analyses WHERE photo_id=?1", [id])?;
                tx.execute("DELETE FROM faces WHERE photo_id=?1", [id])?;
                report.changed += 1;
            } else {
                report.added += 1;
            }
            tx.commit()?;
            progress(&report);
        }
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
        if report.changed > 0 || report.missing > 0 {
            self.store
                .conn
                .execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
        }
        Ok(report)
    }
    pub fn photos(&self, project_id: i64, filter: PhotoFilter) -> Result<Vec<Photo>> {
        if filter.rating.is_some_and(|rating| rating > 5) {
            bail!("rating must be between 0 and 5");
        }
        self.project(project_id)?;
        let mut q = self
            .store
            .conn
            .prepare(&format!("{} WHERE p.project_id=?1", PHOTO_SELECT))?;
        let mut photos = q
            .query_map([project_id], photo_row)?
            .collect::<Result<Vec<_>, _>>()?;
        let settings = self.settings(project_id)?;
        for photo in &mut photos {
            photo.analysis_status = analysis_status(photo.analysis.as_ref(), &settings);
        }
        photos.retain(|p| {
            (filter.include_missing.unwrap_or(false) || (!p.missing && !p.quarantined))
                && filter.decision.is_none_or(|d| d == p.decision)
                && filter.rating.is_none_or(|rating| rating == p.rating)
                && filter
                    .color_label
                    .is_none_or(|color| color == p.color_label)
                && filter.format.as_ref().is_none_or(|f| f == &p.format)
                && filter.verdict.as_ref().is_none_or(|v| {
                    p.analysis_status == AnalysisStatus::Current
                        && p.analysis
                            .as_ref()
                            .and_then(|a| a.get("verdict"))
                            .and_then(Value::as_str)
                            == Some(v)
                })
        });
        match filter.sort.as_deref().unwrap_or("name") {
            "score" | "suggestion" => {
                photos.sort_by(|a, b| score(b).total_cmp(&score(a)).then(a.id.cmp(&b.id)))
            }
            "size" => photos.sort_by_key(|p| p.size_bytes),
            "date" => photos.sort_by_key(|p| (p.taken_at.clone().unwrap_or_default(), p.mtime)),
            "name" => photos.sort_by(|a, b| a.path.cmp(&b.path)),
            _ => bail!("unsupported photo sort"),
        }
        if filter.descending.unwrap_or(false) {
            photos.reverse();
        }
        Ok(photos
            .into_iter()
            .skip(filter.offset.unwrap_or(0))
            .take(filter.limit.unwrap_or(usize::MAX))
            .collect())
    }
    pub fn photo(&self, id: i64) -> Result<Photo> {
        let mut photo = self.store.conn.query_row(
            &format!("{} WHERE p.id=?1", PHOTO_SELECT),
            [id],
            photo_row,
        )?;
        photo.analysis_status =
            analysis_status(photo.analysis.as_ref(), &self.settings(photo.project_id)?);
        Ok(photo)
    }
    pub fn photo_path(&self, id: i64) -> Result<PathBuf> {
        // Path resolution needs neither analysis JSON nor project-wide statistics.
        let (root, path, missing, quarantined): (String, String, bool, bool) =
            self.store.conn.query_row(
                "SELECT pr.root,p.path,p.missing,p.quarantined FROM photos p JOIN projects pr ON pr.id=p.project_id WHERE p.id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
        if quarantined || missing {
            bail!("photo is unavailable");
        }
        existing_within(Path::new(&root), &path)
    }
    /// Retain historical scores when source verification fails, but stop using
    /// them for recommendations, grouping, or cache reuse until a fresh analysis.
    pub fn invalidate_source_analysis(&mut self, photo_id: i64) -> Result<()> {
        self.store.conn.execute("UPDATE analyses SET data=json_set(data,'$.source_invalid',json('true')) WHERE photo_id=?1",[photo_id])?;
        Ok(())
    }
    pub fn save_analysis(
        &mut self,
        photo_id: i64,
        data: impl std::borrow::Borrow<Value>,
    ) -> Result<()> {
        let data = data.borrow();
        let exists: bool = self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM photos WHERE id=?1)",
            [photo_id],
            |r| r.get(0),
        )?;
        anyhow::ensure!(exists, "photo does not exist");
        let tx = self.store.conn.transaction()?;
        tx.execute("INSERT INTO analyses(photo_id,data,version,analyzed_at) VALUES(?1,?2,?3,?4) ON CONFLICT(photo_id) DO UPDATE SET data=excluded.data,version=excluded.version,analyzed_at=excluded.analyzed_at",params![photo_id,data.to_string(),data.get("version").and_then(Value::as_str),now()])?;
        tx.execute("DELETE FROM faces WHERE photo_id=?1", [photo_id])?;
        if let Some(faces) = data.get("faces").and_then(Value::as_array) {
            for (i, face) in faces.iter().enumerate() {
                tx.execute(
                    "INSERT INTO faces(photo_id,face_index,data) VALUES(?1,?2,?3)",
                    params![photo_id, i as i64, face.to_string()],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    /// Add optional semantic evidence without reserializing stored quality numbers.
    pub fn save_semantic_embedding(
        &mut self,
        photo_id: i64,
        analysis: &crate::vision::VisionAnalysis,
    ) -> Result<()> {
        let photo = self.photo(photo_id)?;
        anyhow::ensure!(
            photo.analysis_status == AnalysisStatus::Current,
            "embedding requires current quality analysis"
        );
        let settings: crate::vision::AnalysisSettings =
            serde_json::from_value(self.settings(photo.project_id)?)?;
        anyhow::ensure!(analysis.embedding.is_some(), "embedding is missing");
        let embedding = serde_json::to_value(&analysis.embedding)?;
        anyhow::ensure!(
            crate::vision::embedding_matches_settings(&json!({"embedding":embedding}), &settings),
            "embedding identity does not match current settings"
        );
        let changed = self.store.conn.execute("UPDATE analyses SET data=json_set(data,'$.embedding',json(?1),'$.warnings',json(?2)) WHERE photo_id=?3", params![embedding.to_string(), serde_json::to_string(&analysis.warnings)?, photo_id])?;
        anyhow::ensure!(changed == 1, "analysis disappeared during embedding");
        Ok(())
    }
    pub fn save_groups(&mut self, project_id: i64, groups: Vec<BurstGroup>) -> Result<()> {
        self.project(project_id)?;
        let allowed: HashSet<i64> = {
            let mut query = self.store.conn.prepare(
                "SELECT id FROM photos WHERE project_id=?1 AND missing=0 AND quarantined=0",
            )?;
            let ids = query
                .query_map([project_id], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            ids
        };
        for g in &groups {
            if g.project_id != project_id
                || g.member_photo_ids.iter().any(|id| !allowed.contains(id))
            {
                bail!("invalid group membership");
            }
        }
        let tx = self.store.conn.transaction()?;
        tx.execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
        for g in groups {
            tx.execute(
                "INSERT INTO burst_groups(id,project_id,kind,members) VALUES(?1,?2,?3,?4)",
                params![
                    g.id,
                    project_id,
                    g.kind,
                    serde_json::to_string(&g.member_photo_ids)?
                ],
            )?;
        }
        tx.execute(
            "UPDATE projects SET groups_dirty=0 WHERE id=?1",
            [project_id],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn groups(&self, project_id: i64) -> Result<Vec<BurstGroup>> {
        let mut q = self
            .store
            .conn
            .prepare("SELECT id,kind,members FROM burst_groups WHERE project_id=?1")?;
        let rows = q.query_map([project_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, kind, members) = row?;
            out.push(BurstGroup {
                id,
                project_id,
                kind,
                member_photo_ids: serde_json::from_str(&members)?,
            });
        }
        Ok(out)
    }
    pub fn decisions(
        &mut self,
        project_id: i64,
        ids: &[i64],
        action: Action,
        source: &str,
        link_variants: bool,
    ) -> Result<DecisionBatch> {
        let mut changes: HashMap<i64, Action> = HashMap::new();
        let mut conflicts = Vec::new();
        for id in ids {
            let p = self.photo(*id)?;
            if p.project_id != project_id {
                bail!("photo belongs to another project");
            }
            changes.insert(*id, action);
            if link_variants {
                if let Some(variant) = p.capture_variant_id {
                    for sibling in self.photos(
                        project_id,
                        PhotoFilter {
                            include_missing: Some(true),
                            ..Default::default()
                        },
                    )? {
                        if sibling.id != *id
                            && sibling.capture_variant_id.as_ref() == Some(&variant)
                        {
                            if sibling.decision != Action::Pending && sibling.decision != action {
                                conflicts.push(sibling.id);
                            } else {
                                changes.insert(sibling.id, action);
                            }
                        }
                    }
                }
            }
        }
        self.apply_decisions(project_id, changes, source, conflicts)
    }
    fn apply_decisions(
        &mut self,
        project_id: i64,
        changes: HashMap<i64, Action>,
        source: &str,
        conflicts: Vec<i64>,
    ) -> Result<DecisionBatch> {
        self.project(project_id)?;
        if !["human", "batch_adopt", "csv_import"].contains(&source) {
            bail!("invalid decision source");
        }
        let mut actions = Vec::new();
        for (id, action) in changes {
            let p = self.photo(id)?;
            if p.project_id != project_id {
                bail!("photo belongs to another project");
            }
            if p.decision != action {
                actions.push((id, action, p.decision));
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let changed = actions.len();
        if changed > 0 {
            let tx = self.store.conn.transaction()?;
            tx.execute(
                "INSERT INTO sessions(id,project_id,created_at) VALUES(?1,?2,?3)",
                params![id, project_id, now()],
            )?;
            for (photo_id, action, old) in actions {
                tx.execute("INSERT INTO decisions(photo_id,session_id,action,previous_action,source,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![photo_id,id,action.as_str(),old.as_str(),source,now()])?;
            }
            tx.commit()?;
        }
        Ok(DecisionBatch {
            id,
            changed,
            conflicts,
        })
    }
    /// An explicit selection changes only its IDs; all fields share one undo session.
    pub fn mark_photos(&mut self, project_id: i64, request: MarkRequest) -> Result<DecisionBatch> {
        self.mark_photos_as(project_id, request, "human")
    }
    pub fn mark_photos_as(
        &mut self,
        project_id: i64,
        request: MarkRequest,
        source: &str,
    ) -> Result<DecisionBatch> {
        self.project(project_id)?;
        if request.rating.is_some_and(|rating| rating > 5) {
            bail!("rating must be between 0 and 5");
        }
        let mut seen = HashSet::new();
        let mut changes = Vec::new();
        for id in request.photo_ids {
            if !seen.insert(id) {
                continue;
            }
            let photo = self.photo(id)?;
            if photo.project_id != project_id {
                bail!("photo belongs to another project");
            }
            let action = request.decision.unwrap_or(photo.decision);
            let rating = request.rating.unwrap_or(photo.rating);
            let color = request.color_label.unwrap_or(photo.color_label);
            if action != photo.decision || rating != photo.rating || color != photo.color_label {
                changes.push((photo, action, rating, color));
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let changed = changes.len();
        if changed > 0 {
            let tx = self.store.conn.transaction()?;
            tx.execute(
                "INSERT INTO sessions(id,project_id,created_at) VALUES(?1,?2,?3)",
                params![id, project_id, now()],
            )?;
            for (photo, action, rating, color) in changes {
                if action != photo.decision {
                    tx.execute("INSERT INTO decisions(photo_id,session_id,action,previous_action,source,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![photo.id,id,action.as_str(),photo.decision.as_str(),source,now()])?;
                }
                if rating != photo.rating || color != photo.color_label {
                    tx.execute("INSERT INTO photo_marks(photo_id,session_id,rating,color_label) VALUES(?1,?2,?3,?4)",params![photo.id,id,rating,color.as_str()])?;
                }
            }
            tx.commit()?;
        }
        Ok(DecisionBatch {
            id,
            changed,
            conflicts: vec![],
        })
    }
    pub fn undo(&mut self, project_id: i64) -> Result<DecisionBatch> {
        let id:Option<String>=self.store.conn.query_row("SELECT id FROM sessions WHERE project_id=?1 AND undone_at IS NULL ORDER BY rowid DESC LIMIT 1",[project_id],|r|r.get(0)).optional()?;
        let Some(id) = id else {
            return Ok(DecisionBatch {
                id: String::new(),
                changed: 0,
                conflicts: vec![],
            });
        };
        let tx = self.store.conn.transaction()?;
        let changed: i64 = tx.query_row("SELECT COUNT(*) FROM (SELECT photo_id FROM decisions WHERE session_id=?1 UNION SELECT photo_id FROM photo_marks WHERE session_id=?1)", [&id], |r| r.get(0))?;
        tx.execute(
            "UPDATE decisions SET undone_at=?1 WHERE session_id=?2",
            params![now(), id],
        )?;
        tx.execute(
            "UPDATE photo_marks SET undone_at=?1 WHERE session_id=?2",
            params![now(), id],
        )?;
        tx.execute(
            "UPDATE sessions SET undone_at=?1 WHERE id=?2",
            params![now(), id],
        )?;
        tx.commit()?;
        Ok(DecisionBatch {
            id,
            changed: usize::try_from(changed)?,
            conflicts: vec![],
        })
    }
    pub fn accept(&mut self, project_id: i64) -> Result<DecisionBatch> {
        self.accept_scoped(project_id, AcceptRequest::default())
    }
    pub fn accept_scoped(
        &mut self,
        project_id: i64,
        request: AcceptRequest,
    ) -> Result<DecisionBatch> {
        let settings = self.settings(project_id)?;
        let candidates = if let Some(ids) = request.photo_ids {
            let mut seen = HashSet::new();
            let mut photos = Vec::new();
            for id in ids {
                if !seen.insert(id) {
                    continue;
                }
                let photo = self.photo(id)?;
                anyhow::ensure!(
                    photo.project_id == project_id,
                    "photo belongs to another project"
                );
                anyhow::ensure!(!photo.missing && !photo.quarantined, "photo is unavailable");
                photos.push(photo);
            }
            photos
        } else {
            self.photos(project_id, PhotoFilter::default())?
        };
        let mut changes = HashMap::new();
        for p in candidates {
            if p.decision != Action::Pending {
                continue;
            }
            let Some(analysis) = p.analysis.as_ref() else {
                continue;
            };
            let verdict = analysis.get("verdict").and_then(Value::as_str);
            let action = match (request.category, verdict) {
                (AcceptCategory::All | AcceptCategory::Recommend, Some("recommend")) => {
                    Action::Keep
                }
                (AcceptCategory::All | AcceptCategory::RejectSuggest, Some("reject_suggest")) => {
                    Action::Reject
                }
                _ => continue,
            };
            let closed = analysis
                .get("faces")
                .and_then(Value::as_array)
                .is_some_and(|faces| {
                    faces.iter().any(|face| {
                        ["left_eye", "right_eye"].iter().any(|eye| {
                            face.get(*eye)
                                .and_then(|v| v.get("state"))
                                .and_then(Value::as_str)
                                == Some("closed")
                        })
                    })
                });
            if closed {
                continue;
            }
            anyhow::ensure!(analysis_matches_settings(analysis, &settings),
                    "{} has stale or unknown analysis; analyze the project before accepting suggestions", p.filename);
            let metadata = fs::metadata(self.photo_path(p.id)?)?;
            anyhow::ensure!(
                metadata.len() == p.size_bytes && modified(&metadata)? == p.mtime,
                "{} changed since analysis; rescan before accepting suggestions",
                p.filename
            );
            changes.insert(p.id, action);
        }
        self.apply_decisions(project_id, changes, "batch_adopt", vec![])
    }
    pub fn settings(&self, project_id: i64) -> Result<Value> {
        // Called before each analysis write. Do not build a Project here: its
        // readiness count scans all analysis rows and makes a batch quadratic.
        self.require_project(project_id)?;
        let data: Option<String> = self
            .store
            .conn
            .query_row(
                "SELECT data FROM settings WHERE project_id=?1",
                [project_id],
                |r| r.get(0),
            )
            .optional()?;
        let settings: crate::vision::AnalysisSettings = data
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        Ok(serde_json::to_value(settings)?)
    }
    pub fn set_settings(&mut self, project_id: i64, settings: Value) -> Result<Value> {
        self.project(project_id)?;
        let parsed: crate::vision::AnalysisSettings = serde_json::from_value(settings)?;
        parsed.validate()?;
        let settings = serde_json::to_value(&parsed)?;
        let prior = self.settings(project_id)?;
        if prior == settings {
            return Ok(settings);
        }
        let detection_changed = prior["face_confidence"] != settings["face_confidence"]
            || prior["execution_provider"] != settings["execution_provider"]
            || prior["directml_device_id"] != settings["directml_device_id"]
            || prior["max_faces"] != settings["max_faces"]
            || prior["enable_niqe"] != settings["enable_niqe"]
            || prior["face_detector"] != settings["face_detector"]
            || prior["scrfd_model_sha256"] != settings["scrfd_model_sha256"]
            || prior["occlusion_provider"] != settings["occlusion_provider"]
            || prior["occlusion_model_sha256"] != settings["occlusion_model_sha256"]
            || prior["occlusion_min_visible_fraction"]
                != settings["occlusion_min_visible_fraction"];
        let mut rescored = Vec::new();
        let mut invalidate = Vec::new();
        let mut semantic_only = Vec::new();
        let mut quality_prior = prior.clone();
        let mut quality_next = settings.clone();
        for key in [
            "embedding_provider",
            "embedding_model_sha256",
            "semantic_similarity_threshold",
        ] {
            quality_prior.as_object_mut().unwrap().remove(key);
            quality_next.as_object_mut().unwrap().remove(key);
        }
        for photo in self.photos(
            project_id,
            PhotoFilter {
                include_missing: Some(true),
                ..Default::default()
            },
        )? {
            if detection_changed {
                invalidate.push(photo.id);
                continue;
            }
            if let Some(value) = photo.analysis {
                if !analysis_matches_settings(&value, &prior) {
                    invalidate.push(photo.id);
                    continue;
                }
                if quality_prior == quality_next {
                    // Preserve the stored numeric representation, not just f32
                    // equivalence. A Value parse/serialize can round f64 digits.
                    semantic_only.push(photo.id);
                    continue;
                }
                if let Ok(analysis) = serde_json::from_value::<crate::vision::VisionAnalysis>(value)
                {
                    let mut updated =
                        serde_json::to_value(crate::vision::rescore(&analysis, &parsed))?;
                    updated["settings"] = settings.clone();
                    rescored.push((photo.id, updated));
                } else {
                    invalidate.push(photo.id);
                }
            }
        }
        let tx = self.store.conn.transaction()?;
        tx.execute("INSERT INTO settings(project_id,data) VALUES(?1,?2) ON CONFLICT(project_id) DO UPDATE SET data=excluded.data",params![project_id,settings.to_string()])?;
        for id in semantic_only {
            tx.execute(
                "UPDATE analyses SET data=json_set(data,'$.settings',json(?1)) WHERE photo_id=?2",
                params![settings.to_string(), id],
            )?;
        }
        for (id, analysis) in rescored {
            tx.execute(
                "UPDATE analyses SET data=?1,analyzed_at=?2 WHERE photo_id=?3",
                params![analysis.to_string(), now(), id],
            )?;
        }
        for id in invalidate {
            tx.execute("DELETE FROM analyses WHERE photo_id=?1", [id])?;
            tx.execute("DELETE FROM faces WHERE photo_id=?1", [id])?;
        }
        // Group rank also depends on scoring; discard outdated ranks for the next analysis pass.
        tx.execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
        tx.commit()?;
        Ok(settings)
    }
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        let mut q = self
            .store
            .conn
            .prepare("SELECT name,data FROM profiles ORDER BY name")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut profiles = Vec::new();
        for row in rows {
            let (name, data) = row?;
            let settings: crate::vision::AnalysisSettings = serde_json::from_str(&data)?;
            profiles.push(Profile {
                name,
                settings: serde_json::to_value(settings)?,
            });
        }
        Ok(profiles)
    }
    pub fn save_profile(&mut self, name: &str, settings: Value) -> Result<Profile> {
        if name.trim().is_empty() || name.len() > 128 {
            bail!("invalid profile name");
        }
        let parsed: crate::vision::AnalysisSettings = serde_json::from_value(settings)?;
        parsed.validate()?;
        let settings = serde_json::to_value(parsed)?;
        self.store.conn.execute("INSERT INTO profiles(name,data) VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET data=excluded.data",params![name,settings.to_string()])?;
        Ok(Profile {
            name: name.to_owned(),
            settings,
        })
    }
    pub fn delete_profile(&mut self, name: &str) -> Result<bool> {
        Ok(self
            .store
            .conn
            .execute("DELETE FROM profiles WHERE name=?1", [name])?
            > 0)
    }
    pub fn apply_profile(&mut self, project_id: i64, name: &str) -> Result<Value> {
        let profile = self
            .profiles()?
            .into_iter()
            .find(|p| p.name == name)
            .context("profile not found")?;
        self.set_settings(project_id, profile.settings)
    }
    pub fn estimate_profile(&self, project_id: i64, name: &str) -> Result<Value> {
        let mut profile = self
            .profiles()?
            .into_iter()
            .find(|p| p.name == name)
            .context("profile not found")?;
        let normalized: crate::vision::AnalysisSettings = serde_json::from_value(profile.settings)?;
        profile.settings = serde_json::to_value(normalized)?;
        let current = self.settings(project_id)?;
        let keys: Vec<_> = current
            .as_object()
            .unwrap()
            .keys()
            .chain(profile.settings.as_object().unwrap().keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|key| current.get(key.as_str()) != profile.settings.get(key.as_str()))
            .cloned()
            .collect();
        let target: crate::vision::AnalysisSettings = serde_json::from_value(profile.settings)?;
        let old: crate::vision::AnalysisSettings = serde_json::from_value(current.clone())?;
        let requires_inference = keys.iter().any(|k| {
            matches!(
                k.as_str(),
                "face_confidence"
                    | "execution_provider"
                    | "directml_device_id"
                    | "max_faces"
                    | "enable_niqe"
                    | "face_detector"
                    | "scrfd_model_sha256"
                    | "occlusion_provider"
                    | "occlusion_model_sha256"
                    | "occlusion_min_visible_fraction"
            )
        });
        let mut before = HashMap::<String, usize>::new();
        let mut after = HashMap::<String, usize>::new();
        let mut changed = 0;
        let mut unavailable = 0;
        let photos = self.photos(project_id, PhotoFilter::default())?;
        for p in &photos {
            if requires_inference {
                unavailable += 1;
                continue;
            }
            let Some(analysis) = p
                .analysis
                .as_ref()
                .filter(|v| analysis_matches_settings(v, &current))
                .and_then(|v| {
                    serde_json::from_value::<crate::vision::VisionAnalysis>(v.clone()).ok()
                })
            else {
                unavailable += 1;
                continue;
            };
            let a = crate::vision::rescore(&analysis, &old);
            let b = crate::vision::rescore(&analysis, &target);
            let av = serde_json::to_value(a.verdict)?
                .as_str()
                .unwrap()
                .to_owned();
            let bv = serde_json::to_value(b.verdict)?
                .as_str()
                .unwrap()
                .to_owned();
            *before.entry(av.clone()).or_default() += 1;
            *after.entry(bv.clone()).or_default() += 1;
            if av != bv {
                changed += 1;
            }
        }
        Ok(
            json!({"changed_keys":keys,"photos_requiring_refresh":if keys.is_empty(){unavailable}else{photos.len()},"requires_analysis":requires_inference || unavailable > 0,"estimated_photos":photos.len()-unavailable,"unavailable_photos":unavailable,"verdict_changes":changed,"before":before,"after":after}),
        )
    }
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
    fn export_selection(&self, project_id: i64, scope: &str) -> Result<Vec<Photo>> {
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
    pub fn quarantine_preview(&mut self, project_id: i64) -> Result<QuarantinePlan> {
        let project = self.project(project_id)?;
        let id = uuid::Uuid::new_v4().to_string();
        let root = Path::new(&project.root);
        let mut items = Vec::new();
        for p in self.export_selection(project_id, "reject")? {
            let source = self.photo_path(p.id)?;
            let dest = root
                .join(".iris-quarantine")
                .join(&id)
                .join(checked_relative(&p.path)?);
            items.push(QuarantineItem {
                photo_id: p.id,
                source: source.to_string_lossy().into_owned(),
                destination: dest.to_string_lossy().into_owned(),
                size_bytes: p.size_bytes,
                sha256: digest(&source)?,
                state: "planned".into(),
            });
        }
        let plan = QuarantinePlan {
            id,
            project_id,
            state: "preview".into(),
            items,
        };
        self.store.conn.execute(
            "INSERT INTO quarantine(id,project_id,state,data,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![
                plan.id,
                project_id,
                plan.state,
                serde_json::to_string(&plan)?,
                now()
            ],
        )?;
        Ok(plan)
    }
    pub fn quarantine_plan(&self, id: &str) -> Result<QuarantinePlan> {
        let data: String =
            self.store
                .conn
                .query_row("SELECT data FROM quarantine WHERE id=?1", [id], |r| {
                    r.get(0)
                })?;
        Ok(serde_json::from_str(&data)?)
    }
    pub fn quarantine_history(&self, project_id: i64) -> Result<Vec<QuarantinePlan>> {
        let mut q = self
            .store
            .conn
            .prepare("SELECT data FROM quarantine WHERE project_id=?1 ORDER BY rowid DESC")?;
        let data = q
            .query_map([project_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        data.into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    fn persist_plan(&self, plan: &QuarantinePlan) -> Result<()> {
        self.store.conn.execute(
            "UPDATE quarantine SET state=?1,data=?2 WHERE id=?3",
            params![plan.state, serde_json::to_string(plan)?, plan.id],
        )?;
        Ok(())
    }
    pub fn quarantine_commit(&mut self, id: &str) -> Result<QuarantinePlan> {
        let mut plan = self.quarantine_plan(id)?;
        if plan.state != "preview" {
            bail!("quarantine requires an uncommitted preview");
        }
        let project = self.project(plan.project_id)?;
        let root = Path::new(&project.root).canonicalize()?;
        // Validate every file and current decision before the first move.
        for item in &plan.items {
            let p = self.photo(item.photo_id)?;
            if p.decision != Action::Reject || p.quarantined {
                bail!("preview is stale: photo decision changed");
            }
            let src = self.photo_path(item.photo_id)?;
            if src != Path::new(&item.source)
                || fs::metadata(&src)?.len() != item.size_bytes
                || digest(&src)? != item.sha256
            {
                bail!("preview is stale: source changed");
            }
            let expected = root
                .join(".iris-quarantine")
                .join(&plan.id)
                .join(checked_relative(&p.path)?);
            if expected != Path::new(&item.destination) || expected.exists() {
                bail!("invalid quarantine destination");
            }
            safe_destination(&root, &relative_text(expected.strip_prefix(&root)?))?;
        }
        plan.state = "moving".into();
        self.persist_plan(&plan)?;
        for i in 0..plan.items.len() {
            plan.items[i].state = "moving".into();
            self.persist_plan(&plan)?;
            let result = (|| -> Result<()> {
                let item = &plan.items[i];
                move_verified(
                    Path::new(&item.source),
                    Path::new(&item.destination),
                    &item.sha256,
                )?;
                self.store.conn.execute(
                    "UPDATE photos SET quarantined=1 WHERE id=?1",
                    [item.photo_id],
                )?;
                Ok(())
            })();
            if let Err(e) = result {
                plan.state = "interrupted".into();
                self.persist_plan(&plan)?;
                return Err(e.context(format!("quarantine interrupted; restore plan {}", plan.id)));
            }
            plan.items[i].state = "moved".into();
            self.persist_plan(&plan)?;
        }
        plan.state = "committed".into();
        self.persist_plan(&plan)?;
        Ok(plan)
    }
    fn persist_restore_journal(&self, journal: &RestoreJournal) -> Result<()> {
        self.store.conn.execute(
            "UPDATE quarantine SET state=?1,data=?2 WHERE id=?3",
            params![
                journal.plan.state,
                serde_json::to_string(journal)?,
                journal.plan.id
            ],
        )?;
        Ok(())
    }
    fn inspect_restore_item(
        &self,
        root: &Path,
        plan: &QuarantinePlan,
        index: usize,
    ) -> Result<RestoreStep> {
        let item = plan
            .items
            .get(index)
            .context("invalid restore journal index")?;
        let photo = self.photo(item.photo_id)?;
        anyhow::ensure!(
            photo.project_id == plan.project_id,
            "restore photo belongs to another project"
        );
        let expected_src = root.join(checked_relative(&photo.path)?);
        let expected_dst = root
            .join(".iris-quarantine")
            .join(&plan.id)
            .join(checked_relative(&photo.path)?);
        anyhow::ensure!(
            Path::new(&item.source) == expected_src && Path::new(&item.destination) == expected_dst,
            "invalid restore journal paths"
        );
        safe_destination(root, &photo.path)?;
        safe_destination(root, &relative_text(expected_dst.strip_prefix(root)?))?;
        let source_existed = restore_file_exists(root, &expected_src, &item.sha256)?;
        let quarantine_existed = restore_file_exists(root, &expected_dst, &item.sha256)?;
        anyhow::ensure!(
            source_existed || quarantine_existed,
            "restore source missing or changed"
        );
        Ok(RestoreStep {
            index,
            source_existed,
            quarantine_existed,
            missing: photo.missing,
            quarantined: photo.quarantined,
            prior_item_state: item.state.clone(),
        })
    }
    fn compensate_restore(&self, root: &Path, journal: &mut RestoreJournal) -> Result<()> {
        journal.plan.state = "restore_rollback".into();
        self.persist_restore_journal(journal)?;
        while let Some(step) = journal
            .restore_rollback
            .as_ref()
            .and_then(|r| r.steps.last())
        {
            let current = self.inspect_restore_item(root, &journal.plan, step.index)?;
            let item = &journal.plan.items[step.index];
            let source = Path::new(&item.source);
            let quarantined = Path::new(&item.destination);
            if step.quarantine_existed && !current.quarantine_existed {
                // Create and verify the original quarantine copy before removing
                // any restored source. This is idempotent across interrupted moves.
                copy_new_verified(source, quarantined)?;
            } else if !step.quarantine_existed && current.quarantine_existed {
                bail!("unexpected quarantine file during restore compensation");
            }
            if !step.source_existed && current.source_existed {
                anyhow::ensure!(
                    restore_file_exists(root, quarantined, &item.sha256)?,
                    "rollback quarantine copy missing"
                );
                anyhow::ensure!(
                    restore_file_exists(root, source, &item.sha256)?,
                    "rollback source missing"
                );
                fs::remove_file(source)?;
            } else if step.source_existed && !current.source_existed {
                bail!("original source missing during restore compensation");
            }
            self.store.conn.execute(
                "UPDATE photos SET quarantined=?1,missing=?2 WHERE id=?3",
                params![step.quarantined, step.missing, item.photo_id],
            )?;
            journal.plan.items[step.index].state = step.prior_item_state.clone();
            journal.restore_rollback.as_mut().unwrap().steps.pop();
            self.persist_restore_journal(journal)?;
        }
        journal.plan.state = journal
            .restore_rollback
            .as_ref()
            .unwrap()
            .prior_state
            .clone();
        // Persist an empty journal first. If removing it fails, a restart can
        // safely finish this already-completed compensation.
        self.persist_restore_journal(journal)?;
        journal.restore_rollback = None;
        self.persist_restore_journal(journal)
    }
    pub fn quarantine_restore(&mut self, id: &str) -> Result<QuarantinePlan> {
        let data: String =
            self.store
                .conn
                .query_row("SELECT data FROM quarantine WHERE id=?1", [id], |r| {
                    r.get(0)
                })?;
        let mut journal: RestoreJournal = serde_json::from_str(&data)?;
        if ![
            "committed",
            "interrupted",
            "moving",
            "restoring",
            "restore_rollback",
            "restore_rollback_failed",
        ]
        .contains(&journal.plan.state.as_str())
        {
            bail!("quarantine plan is not restorable");
        }
        let project = self.project(journal.plan.project_id)?;
        let root = Path::new(&project.root).canonicalize()?;
        if journal.restore_rollback.is_some() {
            if let Err(error) = self.compensate_restore(&root, &mut journal) {
                journal.plan.state = "restore_rollback_failed".into();
                let _ = self.persist_restore_journal(&journal);
                return Err(error.context(format!(
                    "restore compensation incomplete; retry plan {}",
                    id
                )));
            }
        } else if journal.plan.state.starts_with("restore_rollback") {
            bail!("restore compensation journal missing");
        }
        // Discover every known conflict before restoring any file or changing flags.
        for index in 0..journal.plan.items.len() {
            self.inspect_restore_item(&root, &journal.plan, index)?;
        }
        journal.restore_rollback = Some(RestoreRollback {
            prior_state: journal.plan.state.clone(),
            steps: vec![],
        });
        journal.plan.state = "restoring".into();
        self.persist_restore_journal(&journal)?;
        let result = (|| -> Result<()> {
            for index in (0..journal.plan.items.len()).rev() {
                let step = self.inspect_restore_item(&root, &journal.plan, index)?;
                journal.restore_rollback.as_mut().unwrap().steps.push(step);
                // Write-ahead state includes the currently executing item, even
                // if a file move or its subsequent database update fails.
                self.persist_restore_journal(&journal)?;
                let item = &journal.plan.items[index];
                let source = Path::new(&item.source);
                let quarantined = Path::new(&item.destination);
                if quarantined.exists() {
                    if source.exists() {
                        anyhow::ensure!(
                            restore_file_exists(&root, source, &item.sha256)?,
                            "restore source disappeared"
                        );
                        anyhow::ensure!(
                            restore_file_exists(&root, quarantined, &item.sha256)?,
                            "quarantine source disappeared"
                        );
                        fs::remove_file(quarantined)?;
                    } else {
                        move_verified(quarantined, source, &item.sha256)?;
                    }
                }
                self.store.conn.execute(
                    "UPDATE photos SET quarantined=0,missing=0 WHERE id=?1",
                    [item.photo_id],
                )?;
                journal.plan.items[index].state = "restored".into();
                self.persist_restore_journal(&journal)?;
            }
            // Commit the public plan and remove the rollback journal together.
            journal.plan.state = "restored".into();
            self.persist_plan(&journal.plan)?;
            Ok(())
        })();
        if let Err(error) = result {
            if let Err(compensation) = self.compensate_restore(&root, &mut journal) {
                journal.plan.state = "restore_rollback_failed".into();
                let _ = self.persist_restore_journal(&journal);
                return Err(anyhow::anyhow!("restore failed: {error:#}; compensation incomplete: {compensation:#}; retry plan {id}"));
            }
            return Err(error.context(format!(
                "restore failed; completed operations rolled back; retry plan {id}"
            )));
        }
        Ok(journal.plan)
    }
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
    fn persist_cache_migration(&self, migration: &CacheMigration) -> Result<()> {
        self.store.conn.execute("INSERT INTO cache_migrations(id,project_id,state,data,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET state=excluded.state,data=excluded.data",
            params![migration.id,migration.project_id,migration.state,serde_json::to_string(migration)?,migration.created_at])?;
        Ok(())
    }
    fn guard_cache_storage(
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

const PHOTO_SELECT:&str="SELECT p.id,p.project_id,p.path,p.filename,p.format,p.mtime,p.size_bytes,p.width,p.height,p.taken_at,p.capture_variant_id,p.missing,p.quarantined,COALESCE((SELECT action FROM decisions d WHERE d.photo_id=p.id AND d.undone_at IS NULL ORDER BY d.id DESC LIMIT 1),'pending'),a.data,COALESCE((SELECT rating FROM photo_marks m WHERE m.photo_id=p.id AND m.undone_at IS NULL ORDER BY m.id DESC LIMIT 1),0),COALESCE((SELECT color_label FROM photo_marks m WHERE m.photo_id=p.id AND m.undone_at IS NULL ORDER BY m.id DESC LIMIT 1),'none') FROM photos p LEFT JOIN analyses a ON a.photo_id=p.id";
const PROJECT_SELECT: &str = "SELECT p.id,p.root,p.name,p.created_at,p.cache_root,p.groups_dirty,(SELECT COUNT(*) FROM photos ph LEFT JOIN analyses a ON a.photo_id=ph.id WHERE ph.project_id=p.id AND ph.missing=0 AND ph.quarantined=0 AND (a.photo_id IS NULL OR a.version IS NULL OR a.version!=?1 OR COALESCE(json_extract(a.data,'$.source_invalid'),0)=1)),p.last_opened_at,p.hidden FROM projects p";
fn project_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        root: r.get(1)?,
        name: r.get(2)?,
        created_at: r.get(3)?,
        cache_root: r.get(4)?,
        groups_dirty: r.get(5)?,
        pending_analysis: r.get::<_, i64>(6)? as u64,
        last_opened_at: r.get(7)?,
        hidden: r.get(8)?,
    })
}
fn photo_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Photo> {
    let action: String = r.get(13)?;
    let data: Option<String> = r.get(14)?;
    let decision = action.parse().map_err(|e: anyhow::Error| {
        rusqlite::Error::FromSqlConversionFailure(
            13,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    let analysis = data
        .map(|s| {
            serde_json::from_str(&s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    14,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })
        .transpose()?;
    Ok(Photo {
        rating: r.get(15)?,
        color_label: serde_json::from_value(Value::String(r.get(16)?)).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                16,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        id: r.get(0)?,
        project_id: r.get(1)?,
        path: r.get(2)?,
        filename: r.get(3)?,
        format: r.get(4)?,
        mtime: r.get(5)?,
        size_bytes: r.get::<_, i64>(6)? as u64,
        width: r.get(7)?,
        height: r.get(8)?,
        taken_at: r.get(9)?,
        capture_variant_id: r.get(10)?,
        missing: r.get(11)?,
        quarantined: r.get(12)?,
        decision,
        analysis_status: AnalysisStatus::Missing,
        analysis,
    })
}
fn analysis_status(analysis: Option<&Value>, settings: &Value) -> AnalysisStatus {
    match analysis {
        None => AnalysisStatus::Missing,
        Some(value) if analysis_matches_settings(value, settings) => AnalysisStatus::Current,
        Some(_) => AnalysisStatus::Stale,
    }
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn score(p: &Photo) -> f64 {
    if p.analysis_status != AnalysisStatus::Current {
        return -1.;
    }
    p.analysis
        .as_ref()
        .and_then(|a| a.get("composite_score"))
        .and_then(Value::as_f64)
        .unwrap_or(-1.)
}
fn modified(meta: &fs::Metadata) -> Result<i64> {
    Ok(meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos()
        .min(i64::MAX as u128) as i64)
}
fn relative_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
fn read_taken_at(path: &Path) -> Option<String> {
    let mut file = std::io::BufReader::new(File::open(path).ok()?);
    let exif = exif::Reader::new().read_from_container(&mut file).ok()?;
    exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string())
}
fn checked_relative(rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if p.as_os_str().is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        bail!("invalid relative path");
    }
    Ok(p.to_path_buf())
}
fn existing_within(root: &Path, rel: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let path = root.join(checked_relative(rel)?).canonicalize()?;
    if !path.starts_with(&root) {
        bail!("path escapes project folder");
    }
    Ok(path)
}
fn digest(path: &Path) -> Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn safe_destination(root: &Path, rel: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let dest = root.join(checked_relative(rel)?);
    let parent = dest.parent().context("destination has no parent")?;
    let mut existing = parent;
    while !existing.exists() {
        existing = existing.parent().context("invalid destination")?;
    }
    if !existing.canonicalize()?.starts_with(&root) {
        bail!("destination escapes root through a link");
    }
    fs::create_dir_all(parent)?;
    if !parent.canonicalize()?.starts_with(&root) {
        bail!("destination parent escapes root");
    }
    Ok(dest)
}
fn copy_new_verified(src: &Path, dst: &Path) -> Result<()> {
    let mut input = File::open(src)?;
    let source_modified = input.metadata()?.modified()?;
    let mut output = OpenOptions::new().write(true).create_new(true).open(dst)?;
    let copied = (|| -> std::io::Result<()> {
        std::io::copy(&mut input, &mut output)?;
        // Quarantine/restore use verified copies; preserve the scan fingerprint
        // when the bytes are unchanged, including during compensation.
        output.set_times(fs::FileTimes::new().set_modified(source_modified))?;
        output.sync_all()
    })();
    if let Err(e) = copied {
        drop(output);
        let _ = fs::remove_file(dst);
        return Err(e.into());
    }
    drop(output);
    if digest(src)? != digest(dst)? {
        let _ = fs::remove_file(dst);
        bail!("copy verification failed");
    }
    Ok(())
}
fn move_verified(src: &Path, dst: &Path, expected_hash: &str) -> Result<()> {
    if dst.exists() {
        bail!("destination exists");
    }
    if digest(src)? != expected_hash {
        bail!("source changed since preview");
    }
    copy_new_verified(src, dst)?;
    if digest(dst)? != expected_hash {
        let _ = fs::remove_file(dst);
        bail!("move verification failed");
    }
    fs::remove_file(src)?;
    Ok(())
}

fn restore_file_exists(root: &Path, path: &Path, expected_hash: &str) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "restore path is not a regular file"
            );
            anyhow::ensure!(
                path.canonicalize()?.starts_with(root),
                "restore path escapes project"
            );
            anyhow::ensure!(
                digest(path)? == expected_hash,
                "restore file has different contents; no files overwritten"
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn paths_overlap(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn resolve_storage_root(path: &Path) -> Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(path.canonicalize()?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let absolute = std::path::absolute(path)?;
            let parent = absolute.parent().context("storage path has no parent")?;
            Ok(resolve_storage_root(parent)?
                .join(absolute.file_name().context("invalid storage path")?))
        }
        Err(error) => Err(error.into()),
    }
}
