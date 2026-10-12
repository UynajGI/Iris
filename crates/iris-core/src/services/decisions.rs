use super::*;

impl Services {
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
                            if self.linked_decision_conflicts(&sibling, action, ids)? {
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
    fn linked_decision_conflicts(
        &self,
        photo: &Photo,
        action: Action,
        explicit: &[i64],
    ) -> Result<bool> {
        Ok(
            (photo.decision != Action::Pending && photo.decision != action)
                || (!explicit.contains(&photo.id)
                    && action != Action::Keep
                    && action != photo.decision
                    && self.selection_locked(photo.id)?),
        )
    }
    pub(super) fn apply_decisions(
        &mut self,
        project_id: i64,
        changes: HashMap<i64, Action>,
        source: &str,
        mut conflicts: Vec<i64>,
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
            if !self.allow_locked_decision(&p, action, source)? {
                conflicts.push(id);
                continue;
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
    fn allow_locked_decision(&self, photo: &Photo, action: Action, source: &str) -> Result<bool> {
        if action == Action::Keep || action == photo.decision || !self.selection_locked(photo.id)? {
            return Ok(true);
        }
        anyhow::ensure!(
            source == "batch_adopt",
            "unlock this must-keep photo before changing its decision"
        );
        Ok(false)
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
        for id in &request.photo_ids {
            let id = *id;
            if !seen.insert(id) {
                continue;
            }
            let photo = self.photo(id)?;
            if photo.project_id != project_id {
                bail!("photo belongs to another project");
            }
            if request
                .decision
                .is_some_and(|action| action != Action::Keep && action != photo.decision)
                && self.selection_locked(id)?
            {
                bail!("unlock this must-keep photo before changing its decision");
            }
            if let Some(change) = photo_mark_change(photo, &request) {
                changes.push(change);
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let changed = changes.len();
        if changed > 0 {
            self.persist_photo_marks(project_id, &id, source, changes)?;
        }
        Ok(DecisionBatch {
            id,
            changed,
            conflicts: vec![],
        })
    }
    fn persist_photo_marks(
        &mut self,
        project_id: i64,
        id: &str,
        source: &str,
        changes: Vec<(Photo, Action, u8, ColorLabel)>,
    ) -> Result<()> {
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
        Ok(())
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
        let candidates = self.accept_candidates(project_id, request.photo_ids)?;
        let group_limits = self.group_keep_constraints(project_id, &candidates)?;
        let mut changes = HashMap::new();
        for p in candidates {
            if p.decision != Action::Pending {
                continue;
            }
            if self.adopts_locked_keep(p.id, request.category)? {
                self.verify_adoption_source(&p)?;
                changes.insert(p.id, Action::Keep);
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
            if action == Action::Keep && group_limits.get(&p.id) == Some(&false) {
                continue;
            }
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
            self.verify_adoption_source(&p)?;
            changes.insert(p.id, action);
        }
        self.apply_decisions(project_id, changes, "batch_adopt", vec![])
    }
    fn verify_adoption_source(&self, photo: &Photo) -> Result<()> {
        let metadata = fs::metadata(self.photo_path(photo.id)?)?;
        anyhow::ensure!(
            metadata.len() == photo.size_bytes && modified(&metadata)? == photo.mtime,
            "{} changed since analysis; rescan before accepting suggestions",
            photo.filename
        );
        Ok(())
    }
    fn adopts_locked_keep(&self, photo: i64, category: AcceptCategory) -> Result<bool> {
        Ok(
            matches!(category, AcceptCategory::All | AcceptCategory::Recommend)
                && self.selection_locked(photo)?,
        )
    }
    fn accept_candidates(&self, project_id: i64, ids: Option<Vec<i64>>) -> Result<Vec<Photo>> {
        let Some(ids) = ids else {
            return self.photos(project_id, PhotoFilter::default());
        };
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
        Ok(photos)
    }
}

fn photo_mark_change(
    photo: Photo,
    request: &MarkRequest,
) -> Option<(Photo, Action, u8, ColorLabel)> {
    let action = request.decision.unwrap_or(photo.decision);
    let rating = request.rating.unwrap_or(photo.rating);
    let color = request.color_label.unwrap_or(photo.color_label);
    if action != photo.decision || rating != photo.rating || color != photo.color_label {
        Some((photo, action, rating, color))
    } else {
        None
    }
}
