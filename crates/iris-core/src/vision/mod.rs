//! Offline, bounded-preview vision. Scores are heuristics, never calibrated probabilities.
mod composition;
mod decode;
pub mod dinov3;
mod eye_quality;
mod faceocc;
mod metrics;
mod models;
mod niqe;
pub mod runtime;
mod scoring;
mod scrfd;
mod semantic;
use anyhow::{bail, Result};
pub use decode::{
    decode_jpeg_preview, decode_preview, decode_preview_with_media, image_dimensions_with_media,
    supported_format, DecodedPreview, PreviewDecoder,
};
pub use faceocc::{occlusion_model_status, validate_occlusion_model, OcclusionModelStatus};
pub use metrics::perceptual_distance;
pub use niqe::NiqeModel;
pub use scoring::{FaceQuality, ScoreBreakdown, ScoreComponent, ScoreInput, ScoreTerm};
pub use scrfd::{
    detector_model_status, validate_detector_model, DetectorModelStatus, ModelAvailability,
};
pub use semantic::{
    embedding_matches_settings, embedding_model_status, group_semantic, semantic_candidates,
    validate_embedding_model, EmbeddingModelStatus, SemanticEmbedding,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;
use utoipa::ToSchema;
pub const ANALYSIS_VERSION: &str = "iris-vision-v6-local-pipeline-2026-10-07";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionProvider {
    #[default]
    Cpu,
    Directml,
    Auto,
}
impl ExecutionProvider {
    fn is_cpu(&self) -> bool {
        *self == Self::Cpu
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FaceDetectorProvider {
    #[default]
    Yunet,
    #[serde(rename = "scrfd_500m")]
    Scrfd500m,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OcclusionProvider {
    #[default]
    None,
    Faceocc,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProvider {
    #[default]
    None,
    Dinov3Vits16,
}
impl EmbeddingProvider {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AnalysisSettings {
    #[serde(default, skip_serializing_if = "ExecutionProvider::is_cpu")]
    #[schema(required = false)]
    pub execution_provider: ExecutionProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub directml_device_id: Option<u32>,
    pub eyes_weight: f64,
    pub sharpness_weight: f64,
    pub face_weight: f64,
    pub exposure_weight: f64,
    pub smile_weight: f64,
    pub face_confidence: f32,
    pub max_faces: usize,
    pub recommend_threshold: f64,
    pub reject_threshold: f64,
    pub enable_niqe: bool,
    pub niqe_weight: f64,
    pub face_detector: FaceDetectorProvider,
    pub scrfd_model_sha256: Option<String>,
    pub occlusion_provider: OcclusionProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occlusion_model_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occlusion_min_visible_fraction: Option<f32>,
    #[serde(default, skip_serializing_if = "EmbeddingProvider::is_none")]
    #[schema(required = false)]
    pub embedding_provider: EmbeddingProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub embedding_model_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub semantic_similarity_threshold: Option<f32>,
}
impl Default for AnalysisSettings {
    fn default() -> Self {
        Self {
            execution_provider: ExecutionProvider::Cpu,
            directml_device_id: None,
            eyes_weight: 25.,
            sharpness_weight: 25.,
            face_weight: 20.,
            exposure_weight: 20.,
            smile_weight: 10.,
            face_confidence: 0.55,
            max_faces: 10,
            recommend_threshold: 70.,
            reject_threshold: 25.,
            enable_niqe: true,
            niqe_weight: 0.20,
            face_detector: FaceDetectorProvider::Yunet,
            scrfd_model_sha256: None,
            occlusion_provider: OcclusionProvider::None,
            occlusion_model_sha256: None,
            occlusion_min_visible_fraction: None,
            embedding_provider: EmbeddingProvider::None,
            embedding_model_sha256: None,
            semantic_similarity_threshold: None,
        }
    }
}
impl AnalysisSettings {
    pub fn validate(&self) -> Result<()> {
        if self
            .directml_device_id
            .is_some_and(|id| id > i32::MAX as u32)
        {
            bail!("directml_device_id must fit a nonnegative 32-bit device index");
        }
        if let Some(hash) = &self.embedding_model_sha256 {
            if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                bail!("embedding_model_sha256 must contain exactly 64 hexadecimal characters");
            }
        }
        if let Some(value) = self.semantic_similarity_threshold {
            if !value.is_finite() || value <= 0. || value > 1. {
                bail!("semantic_similarity_threshold must be finite and satisfy 0 < value <= 1");
            }
        }
        if self.embedding_provider == EmbeddingProvider::Dinov3Vits16
            && (self.embedding_model_sha256.is_none()
                || self.semantic_similarity_threshold.is_none())
        {
            bail!("DINOv3 requires an explicit model SHA-256 and semantic similarity threshold");
        }
        if let Some(hash) = &self.occlusion_model_sha256 {
            if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                bail!("occlusion_model_sha256 must contain exactly 64 hexadecimal characters");
            }
        }
        if let Some(value) = self.occlusion_min_visible_fraction {
            if !value.is_finite() || value <= 0. || value > 1. {
                bail!("occlusion_min_visible_fraction must be finite and satisfy 0 < value <= 1");
            }
        }
        if self.occlusion_provider == OcclusionProvider::Faceocc
            && (self.occlusion_model_sha256.is_none()
                || self.occlusion_min_visible_fraction.is_none())
        {
            bail!("FaceOcc requires an explicit model SHA-256 and minimum visible fraction; no calibrated default exists");
        }
        if let Some(hash) = &self.scrfd_model_sha256 {
            if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                bail!("scrfd_model_sha256 must contain exactly 64 hexadecimal characters");
            }
        }
        if self.face_detector == FaceDetectorProvider::Scrfd500m
            && self.scrfd_model_sha256.is_none()
        {
            bail!("SCRFD requires an explicit scrfd_model_sha256");
        }
        let weights = [
            self.eyes_weight,
            self.sharpness_weight,
            self.face_weight,
            self.exposure_weight,
            self.smile_weight,
        ];
        if weights
            .iter()
            .any(|w| !w.is_finite() || *w < 0. || *w > 100.)
            || weights.iter().sum::<f64>() <= 0.
        {
            bail!("weights must be finite 0..100 with positive total");
        }
        if !(0.1..=0.99).contains(&self.face_confidence) || !(1..=10).contains(&self.max_faces) {
            bail!("invalid face confidence or face limit");
        }
        if !(0.0..=1.0).contains(&self.niqe_weight) {
            bail!("NIQE weight must be a fraction in 0..1");
        }
        if !(0.0..=100.0).contains(&self.reject_threshold)
            || !(0.0..=100.0).contains(&self.recommend_threshold)
            || self.reject_threshold >= self.recommend_threshold
        {
            bail!("verdict thresholds must satisfy 0 <= reject < recommend <= 100");
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Recommend,
    Review,
    RejectSuggest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EyeState {
    Open,
    Closed,
    Uncertain,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Eye {
    pub state: EyeState,
    pub blink_score: Option<f32>,
    pub ear: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HeadPose {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub method: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Face {
    pub index: usize,
    pub bbox: [f32; 4],
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<FaceQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_unavailable_reason: Option<String>,
    pub head_pose: Option<HeadPose>,
    pub left_eye: Option<Eye>,
    pub right_eye: Option<Eye>,
    #[serde(default)]
    pub left_eye_unreliable_reason: Option<String>,
    #[serde(default)]
    pub right_eye_unreliable_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_eye_visibility: Option<EyeVisibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_eye_visibility: Option<EyeVisibility>,
    pub smile_score: Option<f32>,
    pub landmark_confidence: Option<f32>,
    pub landmarks: Vec<[f32; 3]>,
    pub unreliable_reason: Option<String>,
}
/// Visible-face mask measurements in a fixed eye-corner ROI, not calibrated occlusion probabilities.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EyeVisibility {
    pub visible_fraction: f32,
    pub mean_probability: f32,
    pub sampled_pixels: usize,
    pub method: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Exposure {
    pub mean: f64,
    pub shadow_clip: f64,
    pub highlight_clip: f64,
    pub verdict: String,
}
/// Explainable portrait-placement geometry, not a learned aesthetic assessment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Composition {
    pub subject: [f32; 2],
    pub thirds_distance: f64,
    pub center_distance: f64,
    pub score: f64,
    pub method: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VisionAnalysis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_source: Option<String>,
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
    pub orientation: u32,
    pub faces: Vec<Face>,
    pub sharpness_lap: f64,
    pub sharpness_fft: f64,
    pub niqe: Option<f64>,
    pub exposure: Exposure,
    #[serde(default)]
    pub composition: Option<Composition>,
    pub composite_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_breakdown: Option<ScoreBreakdown>,
    pub verdict: Verdict,
    pub phash: String,
    pub structure: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<SemanticEmbedding>,
    pub warnings: Vec<String>,
    pub version: String,
}
pub struct VisionEngine {
    models: models::Models,
    niqe: Option<NiqeModel>,
    niqe_error: Option<String>,
    model_dir: std::path::PathBuf,
    embedding_model: Option<(String, dinov3::DinoV3)>,
}
/// Optional local diagnostics; never part of VisionAnalysis, API schemas or caches.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnalysisTimings {
    pub decode_ms: f64,
    pub metrics_ms: f64,
    pub niqe_ms: f64,
    pub faces_ms: f64,
    /// Includes the tiny grouping structure, pHash, composition and rescoring.
    pub phash_rescore_ms: f64,
    pub embedding_ms: f64,
}

fn finish_stage(clock: &mut Option<Instant>) -> f64 {
    match clock {
        Some(start) => {
            let now = Instant::now();
            let milliseconds = now.duration_since(*start).as_secs_f64() * 1000.;
            *start = now;
            milliseconds
        }
        None => 0.,
    }
}
impl VisionEngine {
    pub fn new(model_dir: &Path) -> Result<Self> {
        let (niqe, niqe_error) = match NiqeModel::load(model_dir) {
            Ok(m) => (Some(m), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Ok(Self {
            models: models::Models::load(model_dir)?,
            niqe,
            niqe_error,
            model_dir: model_dir.to_path_buf(),
            embedding_model: None,
        })
    }
    pub fn analyze(&mut self, path: &Path, settings: &AnalysisSettings) -> Result<VisionAnalysis> {
        let result = self
            .analyze_pipeline(path, settings, false)
            .map(|(analysis, _)| analysis);
        let result = match result {
            Err(error)
                if runtime::directml_active() && error.chain().any(|e| e.is::<ort::Error>()) =>
            {
                runtime::force_cpu(format!(
                    "DirectML pipeline failed; retrying on CPU: {error}"
                ));
                self.models = models::Models::load(&self.model_dir)?;
                self.embedding_model = None;
                self.analyze_pipeline(path, settings, false)
                    .map(|(analysis, _)| analysis)
            }
            other => other,
        };
        result.map(|mut analysis| {
            analysis.warnings.extend(runtime::warnings());
            analysis
        })
    }
    /// Compute only the optional vector while preserving existing quality evidence.
    pub fn embed_analysis(
        &mut self,
        path: &Path,
        settings: &AnalysisSettings,
        mut analysis: VisionAnalysis,
    ) -> Result<VisionAnalysis> {
        settings.validate()?;
        let hash = settings
            .embedding_model_sha256
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("DINOv3 hash required"))?;
        if self
            .embedding_model
            .as_ref()
            .is_none_or(|(loaded, _)| loaded != hash)
        {
            self.embedding_model =
                Some((hash.clone(), dinov3::DinoV3::load(&self.model_dir, hash)?));
        }
        let preview = decode_preview_with_media(path, 1280, Some(&self.model_dir.join("media")))?;
        let vector = match self
            .embedding_model
            .as_mut()
            .unwrap()
            .1
            .embed(&preview.image)
        {
            Ok(vector) => vector,
            Err(error)
                if runtime::directml_active() && error.chain().any(|e| e.is::<ort::Error>()) =>
            {
                runtime::force_cpu(format!(
                    "DirectML embedding failed; retrying on CPU: {error}"
                ));
                self.embedding_model =
                    Some((hash.clone(), dinov3::DinoV3::load(&self.model_dir, hash)?));
                self.embedding_model
                    .as_mut()
                    .unwrap()
                    .1
                    .embed(&preview.image)?
            }
            Err(error) => return Err(error),
        };
        analysis.embedding = Some(SemanticEmbedding {
            model_sha256: hash.clone(),
            preprocessing: dinov3::PREPROCESSING_ID.into(),
            vector,
        });
        for warning in runtime::warnings() {
            if !analysis.warnings.contains(&warning) {
                analysis.warnings.push(warning);
            }
        }
        Ok(analysis)
    }
    pub fn analyze_with_timings(
        &mut self,
        path: &Path,
        settings: &AnalysisSettings,
    ) -> Result<(VisionAnalysis, AnalysisTimings)> {
        self.analyze_pipeline(path, settings, true)
    }
    fn analyze_pipeline(
        &mut self,
        path: &Path,
        settings: &AnalysisSettings,
        timed: bool,
    ) -> Result<(VisionAnalysis, AnalysisTimings)> {
        settings.validate()?;
        self.models.prepare_detector(settings)?;
        self.models.prepare_occlusion(settings)?;
        if settings.embedding_provider == EmbeddingProvider::Dinov3Vits16 {
            let hash = settings
                .embedding_model_sha256
                .as_ref()
                .expect("validated DINOv3 hash");
            if self
                .embedding_model
                .as_ref()
                .is_none_or(|(loaded, _)| loaded != hash)
            {
                self.embedding_model =
                    Some((hash.clone(), dinov3::DinoV3::load(&self.model_dir, hash)?));
            }
        }
        let mut clock = timed.then(Instant::now);
        let mut timings = AnalysisTimings::default();
        let preview = decode_preview_with_media(path, 1280, Some(&self.model_dir.join("media")))?;
        let im = &preview.image;
        timings.decode_ms = finish_stage(&mut clock);
        let (sharpness_lap, sharpness_fft, exposure) = metrics::metrics(im);
        timings.metrics_ms = finish_stage(&mut clock);
        let mut warnings =
            vec!["Smile score is experimental; eye thresholds require labeled evaluation".into()];
        let niqe = if settings.enable_niqe {
            match &self.niqe {
                Some(model) => match model.score(im) {
                    Ok(value) => Some(value),
                    Err(e) => {
                        warnings.push(format!("NIQE unavailable for this image: {e}"));
                        None
                    }
                },
                None => {
                    warnings.push(format!(
                        "NIQE unavailable: {}",
                        self.niqe_error.as_deref().unwrap_or("missing statistics")
                    ));
                    None
                }
            }
        } else {
            None
        };
        timings.niqe_ms = finish_stage(&mut clock);
        let mut faces = self.models.faces(im, settings, &mut warnings)?;
        for face in &mut faces {
            match scoring::measure_face(im, face.bbox) {
                Ok(quality) => face.quality = Some(quality),
                Err(reason) => face.quality_unavailable_reason = Some(reason.into()),
            }
        }
        timings.faces_ms = finish_stage(&mut clock);
        let embedding = if settings.embedding_provider == EmbeddingProvider::Dinov3Vits16 {
            let (hash, model) = self
                .embedding_model
                .as_mut()
                .expect("prepared DINOv3 model");
            Some(SemanticEmbedding {
                model_sha256: hash.clone(),
                preprocessing: dinov3::PREPROCESSING_ID.into(),
                vector: model.embed(im)?,
            })
        } else {
            None
        };
        timings.embedding_ms = finish_stage(&mut clock);
        let structure = image::imageops::resize(im, 8, 8, image::imageops::FilterType::Triangle)
            .pixels()
            .map(|p| {
                (0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])) / 255.
            })
            .collect();
        let analysis = rescore(
            &VisionAnalysis {
                preview_source: matches!(
                    preview.source,
                    "raw_embedded_jpeg" | "raw_developed" | "heic_thumbnail"
                )
                .then(|| preview.source.into()),
                width: im.width(),
                height: im.height(),
                original_width: preview.original_width,
                original_height: preview.original_height,
                orientation: preview.orientation,
                faces,
                sharpness_lap,
                sharpness_fft,
                niqe,
                exposure,
                composition: None,
                composite_score: 0.,
                score_breakdown: None,
                verdict: Verdict::Review,
                phash: metrics::phash(im),
                structure,
                embedding,
                warnings,
                version: ANALYSIS_VERSION.into(),
            },
            settings,
        );
        timings.phash_rescore_ms = finish_stage(&mut clock);
        Ok((analysis, timings))
    }
}

/// Withheld eye measurements do not earn an eyes score; missing factors are omitted.
/// Observation-quality gates are not a general occlusion detector.
/// Closed eyes can only force Review, never RejectSuggest, regardless of weights.
pub fn rescore(analysis: &VisionAnalysis, settings: &AnalysisSettings) -> VisionAnalysis {
    let mut a = analysis.clone();
    // Old records may contain definite EAR-only states. Rescoring must not turn
    // those into recommendations while their stale inference awaits a refresh.
    // Preserve measured EAR; withheld observations remain absent.
    for face in &mut a.faces {
        for eye in [&mut face.left_eye, &mut face.right_eye]
            .into_iter()
            .flatten()
        {
            if !eye.blink_score.is_some_and(|score| score.is_finite()) {
                eye.state = EyeState::Uncertain;
            }
        }
    }
    a.composition = composition::measure(&a.faces, a.width, a.height);
    let eyes: Vec<_> = a
        .faces
        .iter()
        .flat_map(|f| [f.left_eye.as_ref(), f.right_eye.as_ref()])
        .flatten()
        .collect();
    let all_open = !a.faces.is_empty()
        && eyes.len() == a.faces.len() * 2
        && eyes.iter().all(|e| e.state == EyeState::Open)
        && !a.warnings.iter().any(|w| w.contains("omitted faces"));
    let closed = eyes.iter().any(|e| e.state == EyeState::Closed);
    let breakdown = scoring::breakdown(&a, settings);
    a.composite_score = breakdown.components.iter().map(|c| c.contribution).sum();
    a.score_breakdown = Some(breakdown);
    // Reject only when independent technical defects corroborate a low score.
    let severe_quality = a.sharpness_lap < 8.
        && (a.exposure.mean < 35.
            || a.exposure.mean > 225.
            || a.exposure.shadow_clip > 0.65
            || a.exposure.highlight_clip > 0.65);
    a.verdict = if closed {
        Verdict::Review
    } else if severe_quality && a.composite_score < settings.reject_threshold {
        Verdict::RejectSuggest
    } else if all_open
        && a.composite_score >= settings.recommend_threshold
        && a.exposure.verdict == "normal"
    {
        Verdict::Recommend
    } else {
        Verdict::Review
    };
    a
}

/// Blink is an uncalibrated blendshape coefficient. EAR alone cannot decide.
pub fn classify_eye(blink: Option<f32>, ear: f32) -> EyeState {
    if !ear.is_finite() {
        return EyeState::Uncertain;
    }
    match blink {
        Some(b) if b.is_finite() && b > 0.58 && ear < 0.18 => EyeState::Closed,
        Some(b) if b.is_finite() && b < 0.42 && ear > 0.23 => EyeState::Open,
        _ => EyeState::Uncertain,
    }
}

/// Conservative grouping; callers supply ids, analyses and capture timestamps (seconds).
/// Each member must match the first member, preventing similarity-chain drift.
pub fn group_similar(items: &[(String, VisionAnalysis, Option<i64>)]) -> Vec<Vec<String>> {
    use std::collections::{HashMap, HashSet};
    let mut groups: Vec<Vec<usize>> = Vec::new();
    // Nine disjoint pHash bands provide an exact candidate index at Hamming <=8:
    // eight differing bits cannot touch all nine bands (pigeonhole principle).
    let mut index: HashMap<(usize, u64), Vec<usize>> = HashMap::new();
    let bands = |hash: u64| {
        (0..9).map(move |band| {
            let mask = if band == 8 { 255 } else { 127 };
            (band, (hash >> (band * 7)) & mask)
        })
    };
    for (i, (_, a, time)) in items.iter().enumerate() {
        let hash = u64::from_str_radix(&a.phash, 16).ok();
        let mut candidates = HashSet::new();
        if let (Some(hash), Some(_)) = (hash, time) {
            for key in bands(hash) {
                if let Some(ids) = index.get(&key) {
                    candidates.extend(ids.iter().copied());
                }
            }
        }
        let mut candidates: Vec<usize> = candidates.into_iter().collect();
        candidates.sort_unstable();
        let found = candidates.into_iter().find(|group| {
            let g = &groups[*group];
            let (_, b, bt) = &items[g[0]];
            let close_time = matches!((time,bt),(Some(t),Some(u))if t.abs_diff(*u)<=8);
            if !close_time || !perceptual_distance(&a.phash, &b.phash).is_some_and(|d| d <= 8) {
                return false;
            }
            let aspect_a = a.width as f64 / a.height.max(1) as f64;
            let aspect_b = b.width as f64 / b.height.max(1) as f64;
            let structure = a.structure.len() == 64
                && b.structure.len() == 64
                && a.structure
                    .iter()
                    .zip(&b.structure)
                    .map(|(x, y)| (x - y).abs())
                    .sum::<f32>()
                    / 64.
                    < 0.10;
            (aspect_a - aspect_b).abs() < 0.05 && structure
        });
        if let Some(g) = found {
            groups[g].push(i);
        } else {
            if let (Some(hash), Some(_)) = (hash, time) {
                for key in bands(hash) {
                    index.entry(key).or_default().push(groups.len());
                }
            }
            groups.push(vec![i]);
        }
    }
    groups
        .into_iter()
        .filter(|g| g.len() > 1)
        .map(|mut g| {
            g.sort_by(|x, y| {
                let a = &items[*x].1;
                let b = &items[*y].1;
                let blink = |a: &VisionAnalysis| {
                    a.faces.iter().any(|f| {
                        [f.left_eye.as_ref(), f.right_eye.as_ref()]
                            .into_iter()
                            .flatten()
                            .any(|e| e.state == EyeState::Closed)
                    })
                };
                blink(a)
                    .cmp(&blink(b))
                    .then_with(|| b.composite_score.total_cmp(&a.composite_score))
            });
            g.into_iter().map(|i| items[i].0.clone()).collect()
        })
        .collect()
}

/// Near-duplicate candidates independent of capture time. This is perceptual
/// corroboration, NOT proof of byte equality or exact decoded-pixel equality.
/// Callers must exclude linked capture variants and never delete automatically.
/// Every candidate matches its group's anchor; similarity chains do not merge.
pub fn group_duplicates(items: &[(String, VisionAnalysis, Option<i64>)]) -> Vec<Vec<String>> {
    use std::collections::{HashMap, HashSet};
    let mut groups: Vec<Vec<usize>> = Vec::new();
    // Three disjoint bands retain every Hamming-distance <=2 candidate.
    let bands = |hash: u64| {
        (0..3).map(move |band| {
            let bits = if band == 2 { 22 } else { 21 };
            (band, (hash >> (band * 21)) & ((1u64 << bits) - 1))
        })
    };
    let mut index: HashMap<(usize, u64), Vec<usize>> = HashMap::new();
    for (item_index, (_, analysis, _)) in items.iter().enumerate() {
        let Some(hash) = u64::from_str_radix(&analysis.phash, 16).ok() else {
            continue;
        };
        if analysis.structure.len() != 64
            || analysis.width == 0
            || analysis.height == 0
            || analysis.structure.iter().any(|x| !x.is_finite())
        {
            continue;
        }
        let mut candidates = HashSet::new();
        for key in bands(hash) {
            if let Some(ids) = index.get(&key) {
                candidates.extend(ids.iter().copied());
            }
        }
        let mut candidates: Vec<usize> = candidates.into_iter().collect();
        candidates.sort_unstable();
        let found = candidates.into_iter().find(|group| {
            let anchor = &items[groups[*group][0]].1;
            if !perceptual_distance(&analysis.phash, &anchor.phash).is_some_and(|d| d <= 2) {
                return false;
            }
            let aspect = analysis.width as f64 / analysis.height as f64;
            let anchor_aspect = anchor.width as f64 / anchor.height as f64;
            if (aspect / anchor_aspect - 1.).abs() > 0.005 {
                return false;
            }
            let (mean, maximum) = analysis
                .structure
                .iter()
                .zip(&anchor.structure)
                .map(|(a, b)| (a - b).abs())
                .fold((0f32, 0f32), |(sum, max), d| (sum + d, max.max(d)));
            mean / 64. <= 0.015 && maximum <= 0.06
        });
        if let Some(group) = found {
            groups[group].push(item_index);
        } else {
            for key in bands(hash) {
                index.entry(key).or_default().push(groups.len());
            }
            groups.push(vec![item_index]);
        }
    }
    groups
        .into_iter()
        .filter(|g| g.len() > 1)
        .map(|mut group| {
            group.sort_by(|a, b| {
                items[*b]
                    .1
                    .composite_score
                    .total_cmp(&items[*a].1.composite_score)
                    .then_with(|| items[*a].0.cmp(&items[*b].0))
            });
            group.into_iter().map(|i| items[i].0.clone()).collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn channels_must_agree() {
        assert_eq!(classify_eye(Some(0.9), 0.3), EyeState::Uncertain);
        assert_eq!(classify_eye(Some(0.9), 0.1), EyeState::Closed);
        assert_eq!(classify_eye(Some(0.1), 0.3), EyeState::Open);
        assert_eq!(classify_eye(Some(0.5), 0.3), EyeState::Uncertain);
    }
    #[test]
    fn settings_validation() {
        assert!(AnalysisSettings::default().validate().is_ok());
        let mut s = AnalysisSettings::default();
        s.eyes_weight = f64::NAN;
        assert!(s.validate().is_err());
    }
}
