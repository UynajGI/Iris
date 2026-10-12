//! Human group intent is durable; recommendations never mutate decisions implicitly.
use super::*;
use crate::vision::{comparison, VisionAnalysis};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FaceChoice {
    pub photo_id: i64,
    /// Null explicitly means that the main person is absent in this photo.
    pub face_index: Option<usize>,
    /// Echo the current key returned by group selection; stale corrections are rejected.
    pub analysis_key: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GroupIntent {
    pub target_keep: Option<usize>,
    pub locked_photo_ids: Vec<i64>,
    // The lock snapshot returned by GET. Only changes relative to it are applied.
    pub lock_baseline: Vec<i64>,
    pub main_face: Option<FaceChoice>,
    pub face_choices: Vec<FaceChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct GroupFace {
    pub photo_id: i64,
    pub analysis_key: Option<String>,
    pub face_index: Option<usize>,
    /// reference, manual, absent, geometry_candidate, uncertain, or stale.
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct GroupSelection {
    pub group_id: String,
    pub intent: GroupIntent,
    pub recommended_photo_ids: Vec<i64>,
    pub pending_keep_photo_ids: Vec<i64>,
    pub remaining_slots: usize,
    pub over_target: usize,
    pub needs_review: bool,
    pub faces: Vec<GroupFace>,
}

struct ContextData {
    photos: Vec<Photo>,
    analyses: Vec<(i64, Option<VisionAnalysis>)>,
    keys: HashMap<i64, String>,
    members_key: String,
}

impl Services {
    fn selection_context(&self, project: i64, group: &str) -> Result<ContextData> {
        let project_info = self.project(project)?;
        anyhow::ensure!(
            !project_info.groups_dirty,
            "groups require current analysis"
        );
        let members: String = self.store.conn.query_row(
            "SELECT members FROM burst_groups WHERE project_id=?1 AND id=?2",
            params![project, group],
            |row| row.get(0),
        )?;
        let ids: Vec<i64> = serde_json::from_str(&members)?;
        self.require_available_photos(project, &ids)?;
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        let members_key = format!("{:x}", Sha256::digest(serde_json::to_vec(&sorted)?));
        let photos: Vec<Photo> = ids
            .into_iter()
            .map(|id| self.photo(id))
            .collect::<Result<_>>()?;
        let mut keys = HashMap::new();
        let analyses = photos
            .iter()
            .map(|photo| {
                let analysis = if photo.analysis_status == AnalysisStatus::Current {
                    photo
                        .analysis
                        .as_ref()
                        .and_then(|value| serde_json::from_value(value.clone()).ok())
                } else {
                    None
                };
                if analysis.is_some() {
                    let value = photo.analysis.as_ref().unwrap();
                    // Search-vector updates cannot invalidate a face correction.
                    let text = json!({"mtime": photo.mtime, "size": photo.size_bytes,
                    "version": value["version"], "settings": value["settings"],
                    "width": value["width"], "height": value["height"],
                    "orientation": value["orientation"], "faces": value["faces"]})
                    .to_string();
                    keys.insert(photo.id, format!("{:x}", Sha256::digest(text.as_bytes())));
                }
                (photo.id, analysis)
            })
            .collect();
        Ok(ContextData {
            photos,
            analyses,
            keys,
            members_key,
        })
    }

    fn saved_group_intent(&self, project: i64, group: &str, key: &str) -> Result<GroupIntent> {
        let value: Option<String> = self.store.conn.query_row(
            "SELECT data FROM group_selection_intents WHERE project_id=?1 AND group_id=?2 AND members_key=?3",
            params![project, group, key], |row| row.get(0)).optional()?;
        value
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .unwrap_or_else(|| Ok(GroupIntent::default()))
    }

    pub fn group_selection(&self, project: i64, group: &str) -> Result<GroupSelection> {
        let context = self.selection_context(project, group)?;
        let mut intent = self.saved_group_intent(project, group, &context.members_key)?;
        let mut query = self.store.conn.prepare("SELECT l.photo_id FROM selection_locks l JOIN photos p ON p.id=l.photo_id WHERE p.project_id=?1")?;
        let locks: HashSet<i64> = query
            .query_map([project], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        intent.locked_photo_ids = context
            .photos
            .iter()
            .filter(|photo| locks.contains(&photo.id))
            .map(|photo| photo.id)
            .collect();
        intent.lock_baseline = intent.locked_photo_ids.clone();
        Ok(plan(group, intent, &context))
    }

    pub fn set_group_intent(
        &mut self,
        project: i64,
        group: &str,
        mut intent: GroupIntent,
    ) -> Result<GroupSelection> {
        let context = self.selection_context(project, group)?;
        validate_intent(&intent, &context)?;
        intent.locked_photo_ids.sort_unstable();
        intent.locked_photo_ids.dedup();
        let tx = self.store.conn.transaction()?;
        for photo in &context.photos {
            let requested = intent.locked_photo_ids.contains(&photo.id);
            let previous = intent.lock_baseline.contains(&photo.id);
            if requested && !previous {
                tx.execute(
                    "INSERT OR IGNORE INTO selection_locks(photo_id) VALUES(?1)",
                    [photo.id],
                )?;
            } else if previous && !requested {
                tx.execute("DELETE FROM selection_locks WHERE photo_id=?1", [photo.id])?;
            }
        }
        intent.lock_baseline.clear();
        tx.execute("INSERT INTO group_selection_intents(project_id,group_id,members_key,data) VALUES(?1,?2,?3,?4) ON CONFLICT(project_id,group_id) DO UPDATE SET members_key=excluded.members_key,data=excluded.data",
            params![project, group, context.members_key, serde_json::to_string(&intent)?])?;
        tx.commit()?;
        self.group_selection(project, group)
    }

    pub fn accept_group_selection(&mut self, project: i64, group: &str) -> Result<DecisionBatch> {
        let selection = self.group_selection(project, group)?;
        // Recheck original source metadata before publishing one undoable batch.
        for id in &selection.pending_keep_photo_ids {
            let photo = self.photo(*id)?;
            let metadata = fs::metadata(self.photo_path(*id)?)?;
            anyhow::ensure!(
                metadata.len() == photo.size_bytes && modified(&metadata)? == photo.mtime,
                "photo changed; rescan before adopting group suggestions"
            );
        }
        self.apply_decisions(
            project,
            selection
                .pending_keep_photo_ids
                .into_iter()
                .map(|id| (id, Action::Keep))
                .collect(),
            "batch_adopt",
            vec![],
        )
    }

    /// Project-wide adoption also respects configured, current group targets.
    pub(super) fn group_keep_constraints(
        &self,
        project: i64,
        candidates: &[Photo],
    ) -> Result<HashMap<i64, bool>> {
        let mut query = self.store.conn.prepare("SELECT g.id,g.members FROM burst_groups g JOIN group_selection_intents i ON i.project_id=g.project_id AND i.group_id=g.id WHERE g.project_id=?1")?;
        let groups = query
            .query_map([project], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let scoped: HashSet<i64> = candidates.iter().map(|photo| photo.id).collect();
        let mut constraints = HashMap::new();
        for (group, data) in groups {
            let ids: Vec<i64> = serde_json::from_str(&data)?;
            if !ids.iter().any(|id| scoped.contains(id)) {
                continue;
            }
            let selection = self.group_selection(project, &group)?;
            if selection.intent.target_keep.is_none()
                && selection.intent.main_face.is_none()
                && selection.intent.locked_photo_ids.is_empty()
            {
                continue;
            }
            for id in ids {
                let allowed = selection.recommended_photo_ids.contains(&id);
                constraints
                    .entry(id)
                    .and_modify(|prior| *prior &= allowed)
                    .or_insert(allowed);
            }
        }
        Ok(constraints)
    }

    pub(super) fn selection_locked(&self, photo: i64) -> Result<bool> {
        Ok(self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM selection_locks WHERE photo_id=?1)",
            [photo],
            |row| row.get(0),
        )?)
    }
}

fn validate_intent(intent: &GroupIntent, context: &ContextData) -> Result<()> {
    anyhow::ensure!(
        intent
            .target_keep
            .is_none_or(|count| count > 0 && count <= context.photos.len()),
        "keep target must be between one and group size"
    );
    anyhow::ensure!(
        intent.locked_photo_ids.len() <= context.photos.len()
            && intent.lock_baseline.len() <= context.photos.len()
            && intent.face_choices.len() <= context.photos.len(),
        "too many group choices"
    );
    anyhow::ensure!(
        intent
            .lock_baseline
            .iter()
            .all(|id| context.photos.iter().any(|photo| photo.id == *id)),
        "lock baseline must belong to this group"
    );
    for id in &intent.locked_photo_ids {
        let photo = context
            .photos
            .iter()
            .find(|photo| photo.id == *id)
            .context("lock must belong to this group")?;
        anyhow::ensure!(
            matches!(photo.decision, Action::Pending | Action::Keep),
            "clear the existing reject or flag before locking this photo"
        );
    }
    let mut seen = HashSet::new();
    for choice in &intent.face_choices {
        anyhow::ensure!(seen.insert(choice.photo_id), "duplicate face correction");
        validate_choice(choice, context)?;
    }
    if let Some(main) = &intent.main_face {
        anyhow::ensure!(main.face_index.is_some(), "choose a visible main face");
        validate_choice(main, context)?;
    }
    Ok(())
}

fn validate_choice(choice: &FaceChoice, context: &ContextData) -> Result<()> {
    anyhow::ensure!(
        context.keys.get(&choice.photo_id) == Some(&choice.analysis_key),
        "face correction is stale; reload the group"
    );
    if let Some(index) = choice.face_index {
        anyhow::ensure!(
            context
                .analyses
                .iter()
                .find(|(id, _)| *id == choice.photo_id)
                .and_then(|(_, a)| a.as_ref())
                .is_some_and(|a| a.faces.iter().any(|face| face.index == index)),
            "face is no longer available"
        );
    }
    Ok(())
}

fn group_faces(intent: &GroupIntent, context: &ContextData) -> Vec<GroupFace> {
    let anchor = intent.main_face.as_ref();
    let valid_anchor = anchor.filter(|choice| validate_choice(choice, context).is_ok());
    let borrowed: Vec<_> = context
        .analyses
        .iter()
        .map(|(id, a)| (*id, a.as_ref()))
        .collect();
    let report = valid_anchor.map(|choice| comparison::compare(&borrowed, Some(choice.photo_id)));
    context
        .photos
        .iter()
        .map(|photo| {
            let key = context.keys.get(&photo.id).cloned();
            let manual = intent
                .face_choices
                .iter()
                .find(|choice| choice.photo_id == photo.id);
            let selected = valid_anchor
                .filter(|choice| choice.photo_id == photo.id)
                .or(manual);
            let (index, state) = if anchor.is_some() && valid_anchor.is_none() {
                (None, "stale")
            } else if let Some(choice) = selected {
                if validate_choice(choice, context).is_err() {
                    (None, "stale")
                } else if choice.face_index.is_none() {
                    (None, "absent")
                } else {
                    (
                        choice.face_index,
                        if valid_anchor.is_some_and(|a| a.photo_id == photo.id) {
                            "reference"
                        } else {
                            "manual"
                        },
                    )
                }
            } else if let (Some(anchor), Some(report)) = (valid_anchor, &report) {
                let index = report
                    .members
                    .iter()
                    .find(|member| member.photo_id == photo.id)
                    .and_then(|member| {
                        member
                            .face_correspondences
                            .iter()
                            .find(|pair| Some(pair.reference_face_index) == anchor.face_index)
                    })
                    .and_then(|pair| pair.face_index);
                (
                    index,
                    if index.is_some() {
                        "geometry_candidate"
                    } else {
                        "uncertain"
                    },
                )
            } else {
                (
                    None,
                    if anchor.is_some() {
                        "stale"
                    } else {
                        "uncertain"
                    },
                )
            };
            GroupFace {
                photo_id: photo.id,
                analysis_key: key,
                face_index: index,
                state: state.into(),
            }
        })
        .collect()
}

fn plan(group: &str, intent: GroupIntent, context: &ContextData) -> GroupSelection {
    let faces = group_faces(&intent, context);
    let target = intent.target_keep.unwrap_or(1);
    let mut recommended: Vec<i64> = context
        .photos
        .iter()
        .filter(|photo| {
            photo.decision == Action::Keep || intent.locked_photo_ids.contains(&photo.id)
        })
        .map(|photo| photo.id)
        .collect();
    let mut candidates: Vec<(i64, f64, f64)> = context
        .photos
        .iter()
        .filter_map(|photo| ranked_candidate(photo, &recommended, &intent, &faces, context))
        .collect();
    candidates.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then(b.2.total_cmp(&a.2))
            .then(a.0.cmp(&b.0))
    });
    recommended.extend(
        candidates
            .into_iter()
            .take(target.saturating_sub(recommended.len()))
            .map(|(id, _, _)| id),
    );
    let pending = recommended
        .iter()
        .copied()
        .filter(|id| {
            context
                .photos
                .iter()
                .any(|photo| photo.id == *id && photo.decision == Action::Pending)
        })
        .collect();
    let remaining_slots = target.saturating_sub(recommended.len());
    let over_target = recommended.len().saturating_sub(target);
    let needs_review = remaining_slots > 0
        || over_target > 0
        || (intent.main_face.is_some()
            && faces
                .iter()
                .any(|face| face.state != "reference" && face.state != "manual"));
    GroupSelection {
        group_id: group.into(),
        intent,
        recommended_photo_ids: recommended,
        pending_keep_photo_ids: pending,
        remaining_slots,
        over_target,
        needs_review,
        faces,
    }
}

fn ranked_candidate(
    photo: &Photo,
    recommended: &[i64],
    intent: &GroupIntent,
    faces: &[GroupFace],
    context: &ContextData,
) -> Option<(i64, f64, f64)> {
    if photo.decision != Action::Pending || recommended.contains(&photo.id) {
        return None;
    }
    let analysis = context
        .analyses
        .iter()
        .find(|(id, _)| *id == photo.id)?
        .1
        .as_ref()?;
    if crate::vision::assess_group(&[(photo.id, Some(analysis))])
        .reference_photo_id
        .is_none()
    {
        return None;
    }
    let main_score = if intent.main_face.is_some() {
        let face = faces.iter().find(|face| face.photo_id == photo.id)?;
        let index = face.face_index?;
        // Geometry remains an explicit candidate, never an identity assertion.
        analysis
            .faces
            .iter()
            .find(|face| face.index == index)?
            .quality
            .as_ref()?
            .score
    } else {
        0.
    };
    (main_score.is_finite() && analysis.composite_score.is_finite()).then_some((
        photo.id,
        main_score,
        analysis.composite_score,
    ))
}
