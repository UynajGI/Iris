use super::*;

impl Services {
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
        let photos = self.photos(
            project_id,
            PhotoFilter {
                include_missing: Some(true),
                ..Default::default()
            },
        )?;
        let changes = SettingsChanges::prepare(photos, &prior, &settings, &parsed)?;
        let tx = self.store.conn.transaction()?;
        changes.apply(&tx, project_id, &settings)?;
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
        let profile = self.normalized_profile(name)?;
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
        let requires_inference = keys
            .iter()
            .any(|k| DETECTION_SETTINGS.contains(&k.as_str()));
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
    fn normalized_profile(&self, name: &str) -> Result<Profile> {
        let mut profile = self
            .profiles()?
            .into_iter()
            .find(|p| p.name == name)
            .context("profile not found")?;
        let normalized: crate::vision::AnalysisSettings = serde_json::from_value(profile.settings)?;
        profile.settings = serde_json::to_value(normalized)?;
        Ok(profile)
    }
}

const DETECTION_SETTINGS: &[&str] = &[
    "face_confidence",
    "execution_provider",
    "directml_device_id",
    "max_faces",
    "enable_niqe",
    "face_detector",
    "scrfd_model_sha256",
    "occlusion_provider",
    "occlusion_model_sha256",
    "occlusion_min_visible_fraction",
];

fn detection_settings_changed(prior: &Value, next: &Value) -> bool {
    DETECTION_SETTINGS
        .iter()
        .any(|key| prior[*key] != next[*key])
}

fn quality_settings(mut settings: Value) -> Value {
    for key in [
        "embedding_provider",
        "embedding_model_sha256",
        "semantic_similarity_threshold",
    ] {
        settings.as_object_mut().unwrap().remove(key);
    }
    settings
}

#[derive(Default)]
struct SettingsChanges {
    rescored: Vec<(i64, Value)>,
    invalidate: Vec<i64>,
    semantic_only: Vec<i64>,
}

impl SettingsChanges {
    fn prepare(
        photos: Vec<Photo>,
        prior: &Value,
        next: &Value,
        parsed: &crate::vision::AnalysisSettings,
    ) -> Result<Self> {
        let detection_changed = detection_settings_changed(prior, next);
        let semantic_only = quality_settings(prior.clone()) == quality_settings(next.clone());
        let mut changes = Self::default();
        for photo in photos {
            if detection_changed {
                changes.invalidate.push(photo.id);
                continue;
            }
            if let Some(value) = photo.analysis {
                if !analysis_matches_settings(&value, prior) {
                    changes.invalidate.push(photo.id);
                    continue;
                }
                if semantic_only {
                    // Preserve the stored numeric representation, not just f32 equivalence.
                    // A Value parse/serialize can round f64 digits.
                    changes.semantic_only.push(photo.id);
                    continue;
                }
                if let Ok(analysis) = serde_json::from_value::<crate::vision::VisionAnalysis>(value)
                {
                    let mut updated =
                        serde_json::to_value(crate::vision::rescore(&analysis, parsed))?;
                    updated["settings"] = next.clone();
                    changes.rescored.push((photo.id, updated));
                } else {
                    changes.invalidate.push(photo.id);
                }
            }
        }
        Ok(changes)
    }

    fn apply(
        self,
        tx: &rusqlite::Transaction<'_>,
        project_id: i64,
        settings: &Value,
    ) -> Result<()> {
        tx.execute("INSERT INTO settings(project_id,data) VALUES(?1,?2) ON CONFLICT(project_id) DO UPDATE SET data=excluded.data",params![project_id,settings.to_string()])?;
        for id in self.semantic_only {
            tx.execute(
                "UPDATE analyses SET data=json_set(data,'$.settings',json(?1)) WHERE photo_id=?2",
                params![settings.to_string(), id],
            )?;
        }
        for (id, analysis) in self.rescored {
            tx.execute(
                "UPDATE analyses SET data=?1,analyzed_at=?2 WHERE photo_id=?3",
                params![analysis.to_string(), now(), id],
            )?;
        }
        for id in self.invalidate {
            tx.execute("DELETE FROM analyses WHERE photo_id=?1", [id])?;
            tx.execute("DELETE FROM faces WHERE photo_id=?1", [id])?;
        }
        // Group rank also depends on scoring; discard outdated ranks for the next analysis pass.
        tx.execute("DELETE FROM burst_groups WHERE project_id=?1", [project_id])?;
        Ok(())
    }
}

#[cfg(test)]
mod settings_change_tests {
    use super::*;

    #[test]
    fn detection_keys_each_invalidate_but_scoring_and_semantic_keys_do_not() {
        let original = serde_json::to_value(crate::vision::AnalysisSettings::default()).unwrap();
        assert!(!detection_settings_changed(&original, &original));
        for key in DETECTION_SETTINGS {
            let mut next = original.clone();
            next[*key] = json!("changed");
            assert!(detection_settings_changed(&original, &next), "{key}");
        }
        for key in [
            "eyes_weight",
            "reject_threshold",
            "embedding_provider",
            "embedding_model_sha256",
            "semantic_similarity_threshold",
        ] {
            let mut next = original.clone();
            next[key] = json!("changed");
            assert!(!detection_settings_changed(&original, &next), "{key}");
        }
    }

    #[test]
    fn semantic_only_comparison_retains_exact_quality_numbers() {
        let prior = json!({"eyes_weight":25.00000000001,"embedding_provider":"none"});
        let next = json!({"eyes_weight":25.00000000001,"embedding_provider":"dinov3_vits16", "embedding_model_sha256":"hash", "semantic_similarity_threshold":0.8});
        assert_eq!(quality_settings(prior.clone()), quality_settings(next));
        assert_ne!(
            quality_settings(prior),
            quality_settings(json!({"eyes_weight":25.00000000002}))
        );
    }
}
