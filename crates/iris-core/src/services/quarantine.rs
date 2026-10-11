use super::*;

impl Services {
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
    pub(super) fn persist_plan(&self, plan: &QuarantinePlan) -> Result<()> {
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
            self.validate_quarantine_item(&root, &plan.id, item)?;
        }
        plan.state = "moving".into();
        self.persist_plan(&plan)?;
        for i in 0..plan.items.len() {
            plan.items[i].state = "moving".into();
            self.persist_plan(&plan)?;
            let result = self.move_quarantine_item(&plan.items[i]);
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
    fn validate_quarantine_item(
        &self,
        root: &Path,
        plan_id: &str,
        item: &QuarantineItem,
    ) -> Result<()> {
        let p = self.photo(item.photo_id)?;
        if p.decision != Action::Reject || p.quarantined {
            bail!("preview is stale: photo decision changed");
        }
        let src = self.photo_path(item.photo_id)?;
        validate_quarantine_source(&src, item)?;
        let expected = root
            .join(".iris-quarantine")
            .join(plan_id)
            .join(checked_relative(&p.path)?);
        if expected != Path::new(&item.destination) || expected.exists() {
            bail!("invalid quarantine destination");
        }
        safe_destination(root, &relative_text(expected.strip_prefix(root)?))?;
        Ok(())
    }
    fn move_quarantine_item(&self, item: &QuarantineItem) -> Result<()> {
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
    }
    pub(super) fn persist_restore_journal(&self, journal: &RestoreJournal) -> Result<()> {
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
    pub(super) fn inspect_restore_item(
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
    pub(super) fn compensate_restore(
        &self,
        root: &Path,
        journal: &mut RestoreJournal,
    ) -> Result<()> {
        journal.plan.state = "restore_rollback".into();
        self.persist_restore_journal(journal)?;
        while let Some(step) = journal
            .restore_rollback
            .as_ref()
            .and_then(|r| r.steps.last())
        {
            let current = self.inspect_restore_item(root, &journal.plan, step.index)?;
            let item = &journal.plan.items[step.index];
            compensate_restore_files(root, item, step, &current)?;
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
        self.resume_restore_compensation(&root, &mut journal, id)?;
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
        let result = self.restore_quarantine_items(&root, &mut journal);
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
    fn resume_restore_compensation(
        &self,
        root: &Path,
        journal: &mut RestoreJournal,
        id: &str,
    ) -> Result<()> {
        if journal.restore_rollback.is_some() {
            if let Err(error) = self.compensate_restore(root, journal) {
                journal.plan.state = "restore_rollback_failed".into();
                let _ = self.persist_restore_journal(journal);
                return Err(error.context(format!(
                    "restore compensation incomplete; retry plan {}",
                    id
                )));
            }
        } else if journal.plan.state.starts_with("restore_rollback") {
            bail!("restore compensation journal missing");
        }
        Ok(())
    }
    fn restore_quarantine_items(&self, root: &Path, journal: &mut RestoreJournal) -> Result<()> {
        for index in (0..journal.plan.items.len()).rev() {
            let step = self.inspect_restore_item(root, &journal.plan, index)?;
            journal.restore_rollback.as_mut().unwrap().steps.push(step);
            // Write-ahead state includes the currently executing item, even
            // if a file move or its subsequent database update fails.
            self.persist_restore_journal(journal)?;
            let item = &journal.plan.items[index];
            restore_quarantine_file(root, item)?;
            self.store.conn.execute(
                "UPDATE photos SET quarantined=0,missing=0 WHERE id=?1",
                [item.photo_id],
            )?;
            journal.plan.items[index].state = "restored".into();
            self.persist_restore_journal(journal)?;
        }
        // Commit the public plan and remove the rollback journal together.
        journal.plan.state = "restored".into();
        self.persist_plan(&journal.plan)?;
        Ok(())
    }
}

fn validate_quarantine_source(src: &Path, item: &QuarantineItem) -> Result<()> {
    if src != Path::new(&item.source)
        || fs::metadata(src)?.len() != item.size_bytes
        || digest(src)? != item.sha256
    {
        bail!("preview is stale: source changed");
    }
    Ok(())
}

fn compensate_restore_files(
    root: &Path,
    item: &QuarantineItem,
    step: &RestoreStep,
    current: &RestoreStep,
) -> Result<()> {
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
    Ok(())
}

fn restore_quarantine_file(root: &Path, item: &QuarantineItem) -> Result<()> {
    let source = Path::new(&item.source);
    let quarantined = Path::new(&item.destination);
    if quarantined.exists() {
        if source.exists() {
            anyhow::ensure!(
                restore_file_exists(root, source, &item.sha256)?,
                "restore source disappeared"
            );
            anyhow::ensure!(
                restore_file_exists(root, quarantined, &item.sha256)?,
                "quarantine source disappeared"
            );
            fs::remove_file(quarantined)?;
        } else {
            move_verified(quarantined, source, &item.sha256)?;
        }
    }
    Ok(())
}
