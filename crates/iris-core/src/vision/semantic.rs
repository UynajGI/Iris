//! Local optional semantic corroboration of bounded pHash/time candidates.
use super::{
    dinov3, AnalysisSettings, EmbeddingProvider, EyeState, ModelAvailability, VisionAnalysis,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SemanticEmbedding {
    pub model_sha256: String,
    pub preprocessing: String,
    pub vector: Vec<f32>,
}
impl SemanticEmbedding {
    fn valid(&self) -> bool {
        self.model_sha256.len() == 64
            && self.model_sha256.bytes().all(|x| x.is_ascii_hexdigit())
            && self.preprocessing == dinov3::PREPROCESSING_ID
            && self.vector.len() == dinov3::EMBEDDING_DIM
            && self.vector.iter().all(|v| v.is_finite())
            && (self
                .vector
                .iter()
                .map(|x| f64::from(*x).powi(2))
                .sum::<f64>()
                - 1.)
                .abs()
                < 0.001
    }
}

pub fn embedding_matches_settings(
    analysis: &serde_json::Value,
    settings: &AnalysisSettings,
) -> bool {
    if settings.embedding_provider == EmbeddingProvider::None {
        return true;
    }
    analysis
        .get("embedding")
        .and_then(|value| serde_json::from_value::<SemanticEmbedding>(value.clone()).ok())
        .is_some_and(|embedding| {
            embedding.valid()
                && settings
                    .embedding_model_sha256
                    .as_ref()
                    .is_some_and(|hash| hash.eq_ignore_ascii_case(&embedding.model_sha256))
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EmbeddingModelStatus {
    pub provider: EmbeddingProvider,
    pub state: ModelAvailability,
    /// SHA-256 verified against the pinned local artifact; absent on failure.
    pub sha256: Option<String>,
    pub dimensions: usize,
    pub preprocessing: String,
    pub reason: Option<String>,
}
pub fn validate_embedding_model(model_dir: &Path, settings: &AnalysisSettings) -> Result<()> {
    settings.validate()?;
    if settings.embedding_provider == EmbeddingProvider::Dinov3Vits16 {
        dinov3::validate_dinov3_model(
            model_dir,
            settings
                .embedding_model_sha256
                .as_deref()
                .expect("validated hash"),
        )?;
    }
    Ok(())
}
pub fn embedding_model_status(
    model_dir: &Path,
    settings: &AnalysisSettings,
) -> EmbeddingModelStatus {
    let mut status = EmbeddingModelStatus {
        provider: settings.embedding_provider,
        state: ModelAvailability::Disabled,
        sha256: None,
        dimensions: dinov3::EMBEDDING_DIM,
        preprocessing: dinov3::PREPROCESSING_ID.into(),
        reason: None,
    };
    if settings.embedding_provider == EmbeddingProvider::None {
        return status;
    }
    // Inspect the pinned installation even before the project opts into a threshold.
    let hash = settings
        .embedding_model_sha256
        .as_deref()
        .unwrap_or(dinov3::MODEL_SHA256);
    match dinov3::validate_dinov3_model(model_dir, hash) {
        Ok(()) => {
            status.state = ModelAvailability::Available;
            status.sha256 = Some(dinov3::MODEL_SHA256.into());
        }
        Err(error) => {
            status.state = if model_dir.join(dinov3::MODEL_FILE).exists() {
                ModelAvailability::Invalid
            } else {
                ModelAvailability::Missing
            };
            status.reason = Some(error.to_string());
        }
    }
    status
}

fn cosine(a: &SemanticEmbedding, b: &SemanticEmbedding) -> Option<f64> {
    if !a.valid() || !b.valid() || !a.model_sha256.eq_ignore_ascii_case(&b.model_sha256) {
        return None;
    }
    let dot: f64 = a
        .vector
        .iter()
        .zip(&b.vector)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
    Some((dot / (norm(&a.vector) * norm(&b.vector))).clamp(-1., 1.))
}

/// All members of possible semantic pairs, before any embedding is computed.
/// Sorting by time bounds the search; missing timestamps are never candidates.
pub fn semantic_candidates(items: &[(String, VisionAnalysis, Option<i64>)]) -> HashSet<String> {
    let mut timed: Vec<_> = items
        .iter()
        .filter_map(|item| item.2.map(|time| (time, item)))
        .collect();
    timed.sort_by_key(|(time, _)| *time);
    let mut selected = HashSet::new();
    for (i, (time, a)) in timed.iter().enumerate() {
        if a.1.width == 0 || a.1.height == 0 {
            continue;
        }
        for (other_time, b) in &timed[i + 1..] {
            if time.abs_diff(*other_time) > 8 {
                break;
            }
            if b.1.width == 0 || b.1.height == 0 {
                continue;
            }
            let ratio =
                (a.1.width as f64 / a.1.height as f64) / (b.1.width as f64 / b.1.height as f64);
            if (2. / 3. ..=1.5).contains(&ratio)
                && super::perceptual_distance(&a.1.phash, &b.1.phash).is_some_and(|d| d <= 16)
            {
                selected.insert(a.0.clone());
                selected.insert(b.0.clone());
            }
        }
    }
    selected
}

/// DINOv3 groups retain the capture-time boundary and anchor rule. The wider
/// Hamming <=16 candidate gate admits changes that the default <=8/structure
/// gate rejects; cosine is then required. This never declares exact duplicates.
/// The threshold is explicit and heuristic, not a calibrated quality guarantee.
pub fn group_semantic(
    items: &[(String, VisionAnalysis, Option<i64>)],
    minimum_cosine: f32,
) -> Vec<Vec<String>> {
    if !minimum_cosine.is_finite() || minimum_cosine <= 0. || minimum_cosine > 1. {
        return vec![];
    }
    let bands = |hash: u64| {
        (0..17).map(move |band| {
            let (shift, bits) = if band < 13 {
                (band * 4, 4)
            } else {
                (52 + (band - 13) * 3, 3)
            };
            (band, (hash >> shift) & ((1u64 << bits) - 1))
        })
    };
    let mut index: HashMap<(usize, u64), Vec<usize>> = HashMap::new();
    let mut groups: Vec<Vec<usize>> = vec![];
    for (i, (_, analysis, time)) in items.iter().enumerate() {
        let (Some(time), Some(embedding), Ok(hash)) = (
            time,
            analysis.embedding.as_ref(),
            u64::from_str_radix(&analysis.phash, 16),
        ) else {
            continue;
        };
        if !embedding.valid() || analysis.width == 0 || analysis.height == 0 {
            continue;
        }
        let mut candidates = HashSet::new();
        for band in bands(hash) {
            if let Some(ids) = index.get(&band) {
                candidates.extend(ids.iter().copied());
            }
        }
        let mut candidates: Vec<_> = candidates.into_iter().collect();
        candidates.sort_unstable();
        let found = candidates.into_iter().find(|g| {
            let (_, anchor, anchor_time) = &items[groups[*g][0]];
            if !anchor_time.is_some_and(|other| time.abs_diff(other) <= 8)
                || !super::perceptual_distance(&analysis.phash, &anchor.phash)
                    .is_some_and(|d| d <= 16)
            {
                return false;
            }
            let ratio = (analysis.width as f64 / analysis.height as f64)
                / (anchor.width as f64 / anchor.height as f64);
            (2. / 3. ..=1.5).contains(&ratio)
                && anchor
                    .embedding
                    .as_ref()
                    .and_then(|other| cosine(embedding, other))
                    .is_some_and(|value| value >= f64::from(minimum_cosine))
        });
        if let Some(group) = found {
            groups[group].push(i);
        } else {
            for band in bands(hash) {
                index.entry(band).or_default().push(groups.len());
            }
            groups.push(vec![i]);
        }
    }
    groups
        .into_iter()
        .filter(|g| g.len() > 1)
        .map(|mut group| {
            let blink = |a: &VisionAnalysis| {
                a.faces.iter().any(|f| {
                    [f.left_eye.as_ref(), f.right_eye.as_ref()]
                        .into_iter()
                        .flatten()
                        .any(|e| e.state == EyeState::Closed)
                })
            };
            group.sort_by(|x, y| {
                let a = &items[*x].1;
                let b = &items[*y].1;
                blink(a)
                    .cmp(&blink(b))
                    .then_with(|| b.composite_score.total_cmp(&a.composite_score))
                    .then(x.cmp(y))
            });
            group.into_iter().map(|i| items[i].0.clone()).collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str, hash: &str, time: i64, angle: f32) -> (String, VisionAnalysis, Option<i64>) {
        let mut vector = vec![0.; dinov3::EMBEDDING_DIM];
        vector[0] = angle.cos();
        vector[1] = angle.sin();
        let analysis:VisionAnalysis=serde_json::from_value(serde_json::json!({"width":100,"height":100,"original_width":100,"original_height":100,"orientation":1,"faces":[],"sharpness_lap":0.,"sharpness_fft":0.,"niqe":null,"exposure":{"mean":100.,"shadow_clip":0.,"highlight_clip":0.,"verdict":"normal"},"composite_score":50.,"verdict":"review","phash":hash,"structure":[],"warnings":[],"version":super::super::ANALYSIS_VERSION,"embedding":{"model_sha256":"a".repeat(64),"preprocessing":dinov3::PREPROCESSING_ID,"vector":vector}})).unwrap();
        (id.into(), analysis, Some(time))
    }
    #[test]
    fn semantics_corroborates_wider_candidates_without_chaining() {
        let a = item("a", "0", 0, 0.);
        let b = item("b", "fff", 1, 0.25);
        let c = item("c", "ffff", 2, 0.5);
        assert!(super::super::group_similar(&[a.clone(), b.clone()]).is_empty());
        assert_eq!(group_semantic(&[a, b, c], 0.95), vec![vec!["a", "b"]]);
    }
    #[test]
    fn semantic_gates_reject_missing_mixed_invalid_and_distant_evidence() {
        let a = item("a", "0", 0, 0.);
        for variant in 0..7 {
            let mut b = item("b", "0", 1, 0.);
            match variant {
                0 => b.1.embedding = None,
                1 => b.1.embedding.as_mut().unwrap().model_sha256 = "b".repeat(64),
                2 => b.1.embedding.as_mut().unwrap().vector[0] = f32::NAN,
                3 => b.2 = Some(9),
                4 => b.1.phash = "1ffff".into(),
                5 => b.1.embedding.as_mut().unwrap().preprocessing = "old".into(),
                _ => b.1.width = 400,
            }
            assert!(
                group_semantic(&[a.clone(), b], 0.9).is_empty(),
                "variant {variant}"
            );
        }
    }
    #[test]
    fn candidate_selection_does_not_require_vectors_and_obeys_all_gates() {
        let mut a = item("a", "0", 0, 0.);
        let mut b = item("b", "fff", 8, 0.);
        a.1.embedding = None;
        b.1.embedding = None;
        assert_eq!(
            semantic_candidates(&[a.clone(), b.clone()]),
            HashSet::from(["a".into(), "b".into()])
        );
        for variant in 0..4 {
            let mut c = b.clone();
            match variant {
                0 => c.2 = None,
                1 => c.2 = Some(9),
                2 => c.1.phash = "1ffff".into(),
                _ => c.1.width = 400,
            }
            assert!(semantic_candidates(&[a.clone(), c]).is_empty());
        }
    }
    #[test]
    fn enabled_embeddings_are_required_for_cache_reuse() {
        let mut settings = AnalysisSettings::default();
        let mut analysis = serde_json::to_value(item("a", "0", 0, 0.).1).unwrap();
        assert!(embedding_matches_settings(&analysis, &settings));
        settings.embedding_provider = EmbeddingProvider::Dinov3Vits16;
        settings.embedding_model_sha256 = Some("a".repeat(64));
        settings.semantic_similarity_threshold = Some(0.9);
        assert!(embedding_matches_settings(&analysis, &settings));
        analysis.as_object_mut().unwrap().remove("embedding");
        assert!(!embedding_matches_settings(&analysis, &settings));
    }
}
