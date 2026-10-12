use super::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SceneGroup {
    pub id: String,
    /// scene_candidate, manual, or unsequenced; never an exact-duplicate judgment.
    pub kind: String,
    pub member_photo_ids: Vec<i64>,
    pub burst_group_ids: Vec<String>,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneCorrection {
    pub photo_ids: Vec<i64>,
    #[serde(default)]
    pub reset: bool,
}

pub fn parse_capture_time(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|v| v.timestamp())
        .or_else(|| {
            ["%Y-%m-%d %H:%M:%S", "%Y:%m:%d %H:%M:%S"]
                .iter()
                .find_map(|format| chrono::NaiveDateTime::parse_from_str(value, format).ok())
                .map(|v| v.and_utc().timestamp())
        })
}

struct SceneItem {
    id: i64,
    time: Option<i64>,
    hash: Option<String>,
    structure: Vec<f32>,
    manual: Option<String>,
}
impl SceneItem {
    fn resembles(&self, other: &Self) -> bool {
        let hash = self
            .hash
            .as_deref()
            .zip(other.hash.as_deref())
            .and_then(|(a, b)| crate::vision::perceptual_distance(a, b));
        hash.is_some_and(|d| d <= 20)
            && self.structure.len() == 64
            && other.structure.len() == 64
            && self
                .structure
                .iter()
                .chain(&other.structure)
                .all(|v| v.is_finite())
            && self
                .structure
                .iter()
                .zip(&other.structure)
                .map(|(a, b)| f64::from(a - b).powi(2))
                .sum::<f64>()
                / 64.
                < 0.04
    }
}

impl Services {
    /// Reads only grouping evidence, never face landmarks or unrelated analysis payloads.
    pub fn scenes(&self, project_id: i64) -> Result<Vec<SceneGroup>> {
        let mut items = self.scene_items(project_id)?;
        items.sort_by_key(|item| (item.time.unwrap_or(i64::MAX), item.id));
        let mut scenes = partition(&items, project_id);
        self.attach_scene_bursts(project_id, &mut scenes)?;
        Ok(scenes)
    }
    fn scene_items(&self, project_id: i64) -> Result<Vec<SceneItem>> {
        self.require_project(project_id)?;
        let settings: crate::vision::AnalysisSettings =
            serde_json::from_value(self.settings(project_id)?)?;
        let mut query=self.store.conn.prepare("SELECT p.id,p.taken_at,
            CASE WHEN c.version=?2 AND c.settings_key=?3 AND c.source_invalid=0 THEN json_extract(a.data,'$.phash') END,
            CASE WHEN c.version=?2 AND c.settings_key=?3 AND c.source_invalid=0 THEN json_extract(a.data,'$.structure') END,
            s.scene_id FROM photos p LEFT JOIN analyses a ON a.photo_id=p.id LEFT JOIN analysis_catalog c ON c.photo_id=p.id
            LEFT JOIN scene_assignments s ON s.photo_id=p.id AND s.project_id=p.project_id
            WHERE p.project_id=?1 AND p.missing=0 AND p.quarantined=0 ORDER BY p.id")?;
        let items = query
            .query_map(
                params![
                    project_id,
                    crate::vision::ANALYSIS_VERSION,
                    serde_json::to_string(&settings)?
                ],
                |row| {
                    let taken: Option<String> = row.get(1)?;
                    let structure: Option<String> = row.get(3)?;
                    Ok(SceneItem {
                        id: row.get(0)?,
                        time: taken.as_deref().and_then(parse_capture_time),
                        hash: row.get(2)?,
                        structure: structure
                            .and_then(|v| serde_json::from_str(&v).ok())
                            .unwrap_or_default(),
                        manual: row.get(4)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }
    fn attach_scene_bursts(&self, project_id: i64, scenes: &mut [SceneGroup]) -> Result<()> {
        let mut bursts = self.store.conn.prepare(
            "SELECT id,members FROM burst_groups WHERE project_id=?1 AND kind='burst' ORDER BY id",
        )?;
        let bursts = bursts
            .query_map([project_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (id, members) = row?;
                Ok((id, serde_json::from_str::<Vec<i64>>(&members)?))
            })
            .collect::<Result<Vec<_>>>()?;
        // One membership lookup per photo, rather than every scene × every burst.
        let by_photo: HashMap<_, _> = scenes
            .iter()
            .enumerate()
            .flat_map(|(index, scene)| scene.member_photo_ids.iter().map(move |id| (*id, index)))
            .collect();
        for (id, members) in bursts {
            let Some(index) = members.first().and_then(|id| by_photo.get(id)).copied() else {
                continue;
            };
            if members.iter().all(|id| by_photo.get(id) == Some(&index)) {
                scenes[index].burst_group_ids.push(id);
            }
        }
        Ok(())
    }

    /// Assign selected photos together; one-photo assignment explicitly splits it out.
    /// Reset removes only the selected manual assignments, restoring automatic candidates.
    pub fn correct_scene(
        &mut self,
        project_id: i64,
        request: SceneCorrection,
    ) -> Result<Vec<SceneGroup>> {
        anyhow::ensure!(
            !request.photo_ids.is_empty() && request.photo_ids.len() <= 50_000,
            "select 1..=50000 photos"
        );
        let ids: HashSet<_> = request.photo_ids.iter().copied().collect();
        anyhow::ensure!(ids.len() == request.photo_ids.len(), "duplicate photo ids");
        self.require_available_photos(project_id, &request.photo_ids)?;
        let tx = self.store.conn.transaction()?;
        let scene_id = format!("manual-{}", uuid::Uuid::new_v4());
        for id in request.photo_ids {
            if request.reset {
                tx.execute(
                    "DELETE FROM scene_assignments WHERE photo_id=?1 AND project_id=?2",
                    params![id, project_id],
                )?;
            } else {
                tx.execute("INSERT INTO scene_assignments(photo_id,project_id,scene_id) VALUES(?1,?2,?3) ON CONFLICT(photo_id) DO UPDATE SET scene_id=excluded.scene_id",params![id,project_id,scene_id])?;
            }
        }
        tx.commit()?;
        self.scenes(project_id)
    }
}

fn partition(items: &[SceneItem], project: i64) -> Vec<SceneGroup> {
    let mut buckets: Vec<Vec<&SceneItem>> = Vec::new();
    let mut manual = HashMap::<&str, usize>::new();
    let mut active: Option<usize> = None;
    for item in items {
        if let Some(id) = item.manual.as_deref() {
            let index = *manual.entry(id).or_insert_with(|| {
                buckets.push(Vec::new());
                buckets.len() - 1
            });
            buckets[index].push(item);
            continue;
        }
        let candidate = active.filter(|index| {
            let group = &buckets[*index];
            let anchor = group[0];
            let last = group[group.len() - 1];
            item.time
                .zip(anchor.time)
                .zip(last.time)
                .is_some_and(|((now, start), previous)| {
                    now >= previous
                        && now - previous <= 90
                        && now - start <= 600
                        && item.resembles(anchor)
                })
        });
        if let Some(index) = candidate {
            buckets[index].push(item);
        } else {
            buckets.push(vec![item]);
            active = Some(buckets.len() - 1);
        }
    }
    buckets.into_iter().map(|members| {
        let manual=members[0].manual.clone();let timed=members[0].time.is_some();
        let ids:Vec<_>=members.iter().map(|p|p.id).collect();
        let id=manual.clone().unwrap_or_else(||format!("scene-{:x}",Sha256::digest(format!("scene-v1:{project}:{ids:?}").as_bytes())));
        SceneGroup {id,kind:if manual.is_some(){"manual"}else if timed{"scene_candidate"}else{"unsequenced"}.into(),
            member_photo_ids:ids,burst_group_ids:vec![],reason:if manual.is_some(){"User-corrected membership"}else if timed{"Conservative time and anchor appearance candidate; not a burst or duplicate claim"}else{"No valid capture time; kept separate for manual review"}.into()}
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: i64, time: Option<i64>, level: f32) -> SceneItem {
        SceneItem {
            id,
            time,
            hash: Some("0".into()),
            structure: vec![level; 64],
            manual: None,
        }
    }
    #[test]
    fn scene_boundaries_do_not_chain_across_time_or_appearance_and_missing_times_stay_separate() {
        let items = vec![
            item(1, Some(0), 0.),
            item(2, Some(60), 0.1),
            item(3, Some(120), 0.25),
            item(4, Some(300), 0.25),
            item(5, None, 0.),
            item(6, None, 0.),
        ];
        let a = partition(&items, 1);
        let b = partition(&items, 1);
        assert_eq!(
            a.iter()
                .map(|s| s.member_photo_ids.clone())
                .collect::<Vec<_>>(),
            vec![vec![1, 2], vec![3], vec![4], vec![5], vec![6]]
        );
        assert_eq!(a[0].id, b[0].id);
        assert_eq!(a[4].kind, "unsequenced");
        let long: Vec<_> = (0..12).map(|i| item(i, Some(i * 60), 0.)).collect();
        assert_eq!(partition(&long, 1).len(), 2);
    }
}
