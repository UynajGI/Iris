use super::*;

fn matches_photo_filter(p: &Photo, filter: &PhotoFilter) -> bool {
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
}

impl Services {
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
        photos.retain(|p| matches_photo_filter(p, &filter));
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
}
