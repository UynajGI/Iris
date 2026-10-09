//! Versioned, uncalibrated technical-quality heuristics, not portrait aesthetics.
use super::{metrics, AnalysisSettings, Exposure, EyeState, VisionAnalysis};
use image::RgbImage;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const METHOD: &str = "observed_technical_quality_v1_lap75_fft25_face50_30_20";
const LAP_MAPPING: &str = "clamp(100*ln(1+max(lap,0))/ln(1001),0,100)";
const EXPOSURE_MAPPING: &str =
    "clamp(100-abs(mean-128)/128*60-(shadow_clip+highlight_clip)*70,0,100)";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FaceQuality {
    pub method: String,
    pub crop_width: u32,
    pub crop_height: u32,
    pub sharpness_lap: f64,
    pub exposure: Exposure,
    pub sharpness_score: f64,
    pub exposure_score: f64,
    pub resolution_score: f64,
    pub score: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScoreInput {
    pub name: String,
    pub value: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScoreTerm {
    pub id: String,
    pub raw: Vec<ScoreInput>,
    pub score: Option<f64>,
    /// Actual normalized share inside this category; zero for missing terms.
    pub weight: f64,
    /// Points inside the category, before its top-level weight.
    pub contribution: f64,
    pub mapping: String,
    pub missing_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScoreComponent {
    pub id: String,
    pub score: Option<f64>,
    pub configured_weight: f64,
    /// Normalized share of the final score, in 0..1.
    pub effective_weight: f64,
    pub contribution: f64,
    pub observed_count: usize,
    pub total_count: usize,
    pub missing_reason: Option<String>,
    pub terms: Vec<ScoreTerm>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScoreBreakdown {
    pub method: String,
    pub effective_weight_total: f64,
    pub components: Vec<ScoreComponent>,
}

fn lap_score(lap: f64) -> f64 {
    (100. * lap.max(0.).ln_1p() / 1000f64.ln_1p()).clamp(0., 100.)
}
fn exposure_score(e: &Exposure) -> f64 {
    (100. - (e.mean - 128.).abs() / 128. * 60. - (e.shadow_clip + e.highlight_clip) * 70.)
        .clamp(0., 100.)
}
fn quality_scores(q: &mut FaceQuality) {
    q.sharpness_score = lap_score(q.sharpness_lap);
    q.exposure_score = exposure_score(&q.exposure);
    q.resolution_score = (f64::from(q.crop_width.min(q.crop_height)) / 128.).clamp(0., 1.) * 100.;
    q.score = q.sharpness_score * 0.5 + q.exposure_score * 0.3 + q.resolution_score * 0.2;
}

/// Intersect a finite positive box with the preview before integer conversion.
/// Clipped dimensions are measured, never upscaled to invent source resolution.
pub(super) fn measure_face(image: &RgbImage, bbox: [f32; 4]) -> Result<FaceQuality, &'static str> {
    if bbox.iter().any(|v| !v.is_finite()) || bbox[2] <= 0. || bbox[3] <= 0. {
        return Err("face quality unavailable: invalid bounding box");
    }
    let [x, y, w, h] = bbox.map(f64::from);
    let x0 = x.floor().clamp(0., f64::from(image.width())) as u32;
    let y0 = y.floor().clamp(0., f64::from(image.height())) as u32;
    let x1 = (x + w).ceil().clamp(0., f64::from(image.width())) as u32;
    let y1 = (y + h).ceil().clamp(0., f64::from(image.height())) as u32;
    let cw = x1.saturating_sub(x0);
    let ch = y1.saturating_sub(y0);
    if cw < 3 || ch < 3 {
        return Err("face quality unavailable: clipped preview crop is smaller than 3x3 pixels");
    }
    let crop = image::imageops::crop_imm(image, x0, y0, cw, ch).to_image();
    let (sharpness_lap, exposure) = metrics::spatial_metrics(&crop);
    let mut q = FaceQuality {
        method: "preview_bbox_clipped_no_upscale_v1_lap50_exposure30_min_side128_20".into(),
        crop_width: cw,
        crop_height: ch,
        sharpness_lap,
        exposure,
        sharpness_score: 0.,
        exposure_score: 0.,
        resolution_score: 0.,
        score: 0.,
    };
    quality_scores(&mut q);
    Ok(q)
}

fn raw(values: &[(&str, f64)]) -> Vec<ScoreInput> {
    values
        .iter()
        .filter(|(_, v)| v.is_finite())
        .map(|(n, v)| ScoreInput {
            name: (*n).into(),
            value: *v,
        })
        .collect()
}
fn exposure_raw(e: &Exposure) -> Vec<ScoreInput> {
    raw(&[
        ("mean", e.mean),
        ("shadow_clip", e.shadow_clip),
        ("highlight_clip", e.highlight_clip),
    ])
}
fn finite_exposure(e: &Exposure) -> bool {
    e.mean.is_finite() && e.shadow_clip.is_finite() && e.highlight_clip.is_finite()
}
fn term(
    id: impl Into<String>,
    inputs: Vec<ScoreInput>,
    score: Option<f64>,
    weight: f64,
    mapping: &str,
    reason: &str,
) -> ScoreTerm {
    let score = score.filter(|s| s.is_finite()).map(|s| s.clamp(0., 100.));
    ScoreTerm {
        id: id.into(),
        raw: inputs,
        score,
        weight,
        contribution: 0.,
        mapping: mapping.into(),
        missing_reason: score.is_none().then(|| reason.into()),
    }
}
fn component(
    id: &str,
    configured_weight: f64,
    mut terms: Vec<ScoreTerm>,
    observed_count: usize,
    total_count: usize,
) -> ScoreComponent {
    let sum: f64 = terms
        .iter()
        .filter(|t| t.score.is_some())
        .map(|t| t.weight)
        .sum();
    for t in &mut terms {
        t.weight = if t.score.is_some() && sum > 0. {
            t.weight / sum
        } else {
            0.
        };
        t.contribution = t.score.unwrap_or(0.) * t.weight;
    }
    let score = (sum > 0.).then(|| terms.iter().map(|t| t.contribution).sum());
    ScoreComponent {
        id: id.into(),
        score,
        configured_weight,
        effective_weight: 0.,
        contribution: 0.,
        observed_count,
        total_count,
        missing_reason: score
            .is_none()
            .then(|| "No usable observations for this category".into()),
        terms,
    }
}

pub(super) fn breakdown(a: &VisionAnalysis, s: &AnalysisSettings) -> ScoreBreakdown {
    let n = if s.enable_niqe {
        a.niqe.filter(|n| n.is_finite())
    } else {
        None
    };
    let nw = if n.is_some() { s.niqe_weight } else { 0. };
    let sharp_terms = vec![
        term("laplacian", raw(&[("lap", a.sharpness_lap)]), a.sharpness_lap.is_finite().then(|| lap_score(a.sharpness_lap)), 0.75*(1.-nw), LAP_MAPPING, "Non-finite Laplacian measurement"),
        term("fft", raw(&[("high_frequency_energy_ratio", a.sharpness_fft)]), a.sharpness_fft.is_finite().then(|| 100.*a.sharpness_fft.clamp(0.,1.).sqrt()), 0.25*(1.-nw), "100*sqrt(clamp(high_frequency_energy_ratio,0,1)); noise and texture can increase this score", "Non-finite FFT measurement"),
        term("niqe", a.niqe.map_or_else(Vec::new, |v| raw(&[("niqe",v)])), n.map(|n| (100.-(n-2.).max(0.)*12.5).clamp(0.,100.)), nw, "clamp(100-max(niqe-2,0)*12.5,0,100)", if s.enable_niqe {"NIQE unavailable; remaining budget allocated to Laplacian and FFT at 75:25"} else {"NIQE disabled; Laplacian and FFT use 75:25"}),
    ];
    let has_composition = a.composition.is_some();
    let framing_terms = vec![
        term(
            "exposure",
            exposure_raw(&a.exposure),
            finite_exposure(&a.exposure).then(|| exposure_score(&a.exposure)),
            if has_composition { 0.75 } else { 1. },
            EXPOSURE_MAPPING,
            "Non-finite exposure measurement",
        ),
        term(
            "composition",
            a.composition.as_ref().map_or_else(Vec::new, |c| {
                raw(&[
                    ("center_distance", c.center_distance),
                    ("thirds_distance", c.thirds_distance),
                    ("score", c.score),
                ])
            }),
            a.composition.as_ref().map(|c| c.score),
            if has_composition { 0.25 } else { 0. },
            "portrait_center_or_thirds_v1; see composition.method",
            "No observed portrait placement",
        ),
    ];
    let mut eye_terms = Vec::new();
    let mut face_terms = Vec::new();
    let mut smile_terms = Vec::new();
    for (i, face) in a.faces.iter().enumerate() {
        for (side, eye) in [("left", &face.left_eye), ("right", &face.right_eye)] {
            let score = eye.as_ref().map(|e| match e.state {
                EyeState::Open => 100.,
                EyeState::Uncertain => 50.,
                EyeState::Closed => 0.,
            });
            let inputs = eye.as_ref().map_or_else(Vec::new, |e| {
                let mut r = raw(&[
                    ("ear", f64::from(e.ear)),
                    (
                        "state_code",
                        match e.state {
                            EyeState::Open => 1.,
                            EyeState::Uncertain => 0.5,
                            EyeState::Closed => 0.,
                        },
                    ),
                ]);
                if let Some(b) = e.blink_score {
                    r.extend(raw(&[("blink_score", f64::from(b))]));
                }
                r
            });
            eye_terms.push(term(
                format!("face_{i}_{side}_eye"),
                inputs,
                score,
                1.,
                "open=100; uncertain=50; closed=0; withheld omitted",
                "Eye observation withheld or absent",
            ));
        }
        let quality = face.quality.as_ref().filter(|q| {
            q.sharpness_lap.is_finite()
                && finite_exposure(&q.exposure)
                && q.crop_width >= 3
                && q.crop_height >= 3
        });
        let mut inputs = Vec::new();
        let qscore = quality.map(|q| {
            let mut q = q.clone();
            quality_scores(&mut q);
            inputs = exposure_raw(&q.exposure);
            inputs.extend(raw(&[
                ("lap", q.sharpness_lap),
                ("crop_width", f64::from(q.crop_width)),
                ("crop_height", f64::from(q.crop_height)),
                ("sharpness_score", q.sharpness_score),
                ("exposure_score", q.exposure_score),
                ("resolution_score", q.resolution_score),
            ]));
            q.score
        });
        face_terms.push(term(format!("face_{i}_quality"), inputs, qscore, 1., &format!("0.50*L+0.30*E+0.20*R; L={LAP_MAPPING}; E={EXPOSURE_MAPPING}; R=100*clamp(min(crop_width,crop_height)/128,0,1)"), face.quality_unavailable_reason.as_deref().unwrap_or("Local face quality not measured; full feature reanalysis required")));
        smile_terms.push(term(
            format!("face_{i}_smile"),
            face.smile_score
                .map_or_else(Vec::new, |v| raw(&[("smile", f64::from(v))])),
            face.smile_score.map(|v| f64::from(v) * 100.),
            1.,
            "100*clamp(smile,0,1); experimental",
            "Smile observation absent",
        ));
    }
    let count = |terms: &[ScoreTerm]| terms.iter().filter(|t| t.score.is_some()).count();
    let mut components = vec![
        component(
            "eyes",
            s.eyes_weight,
            eye_terms.clone(),
            count(&eye_terms),
            a.faces.len() * 2,
        ),
        component(
            "sharpness",
            s.sharpness_weight,
            sharp_terms.clone(),
            count(&sharp_terms),
            3,
        ),
        component(
            "face",
            s.face_weight,
            face_terms.clone(),
            count(&face_terms),
            a.faces.len(),
        ),
        component(
            "exposure",
            s.exposure_weight,
            framing_terms.clone(),
            count(&framing_terms),
            2,
        ),
        component(
            "smile",
            s.smile_weight,
            smile_terms.clone(),
            count(&smile_terms),
            a.faces.len(),
        ),
    ];
    let total: f64 = components
        .iter()
        .filter(|c| c.score.is_some())
        .map(|c| c.configured_weight)
        .sum();
    for c in &mut components {
        c.effective_weight = if c.score.is_some() && total > 0. {
            c.configured_weight / total
        } else {
            0.
        };
        c.contribution = c.score.unwrap_or(0.) * c.effective_weight;
    }
    ScoreBreakdown {
        method: METHOD.into(),
        effective_weight_total: total,
        components,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn face_crop_uses_actual_resolution_and_rejects_bad_boxes() {
        let image = RgbImage::from_pixel(200, 200, Rgb([128; 3]));
        let large = measure_face(&image, [0., 0., 128., 128.]).unwrap();
        let small = measure_face(&image, [0., 0., 64., 64.]).unwrap();
        assert!((large.score - 50.).abs() < 1e-6);
        assert!((small.score - 40.).abs() < 1e-6);
        assert_eq!(small.resolution_score, 50.);
        let clipped = measure_face(&image, [-64., -64., 128., 128.]).unwrap();
        assert_eq!((clipped.crop_width, clipped.crop_height), (64, 64));
        assert_eq!(clipped.resolution_score, small.resolution_score);
        for bbox in [
            [f32::NAN, 0., 10., 10.],
            [0., 0., f32::INFINITY, 10.],
            [0., 0., -10., 10.],
            [500., 500., 10., 10.],
            [-f32::MAX, 0., 1., 20.],
            [f32::MAX, 0., f32::MAX, 20.],
            [0., 0., 2., 2.],
        ] {
            assert!(measure_face(&image, bbox).is_err(), "{bbox:?}");
        }
    }

    #[test]
    fn face_quality_tracks_controlled_blur_and_exposure() {
        let image = RgbImage::from_fn(128, 128, |x, y| {
            Rgb([if (x / 4 + y / 4) % 2 == 0 { 118 } else { 138 }; 3])
        });
        let blurred = image::imageops::blur(&image, 2.);
        let q = measure_face(&image, [0., 0., 128., 128.]).unwrap();
        let b = measure_face(&blurred, [0., 0., 128., 128.]).unwrap();
        assert!(q.sharpness_lap > b.sharpness_lap);
        assert!(q.sharpness_score > b.sharpness_score);
        assert!(q.score > b.score);
        let dark = RgbImage::from_pixel(128, 128, Rgb([0; 3]));
        let gray = RgbImage::from_pixel(128, 128, Rgb([128; 3]));
        let dark = measure_face(&dark, [0., 0., 128., 128.]).unwrap();
        let gray = measure_face(&gray, [0., 0., 128., 128.]).unwrap();
        assert!(dark.exposure_score < gray.exposure_score);
        assert!(dark.score < gray.score);
    }

    #[test]
    fn controlled_noise_exposes_known_sharpness_limit() {
        let flat = RgbImage::from_pixel(128, 128, Rgb([128; 3]));
        let mut seed = 7u32;
        let noisy = RgbImage::from_fn(128, 128, |_, _| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            Rgb([112 + (seed >> 27) as u8; 3])
        });
        let (l0, f0, _) = metrics::metrics(&flat);
        let (l1, f1, _) = metrics::metrics(&noisy);
        assert!(l1 > l0 && f1 > f0);
        let base = |l, f: f64| 0.75 * lap_score(l) + 0.25 * 100. * f.clamp(0., 1.).sqrt();
        assert!(base(l1, f1) > base(l0, f0));
        eprintln!("Known noise limitation: flat lap={l0:.4} fft={f0:.6} score={:.4}; noisy lap={l1:.4} fft={f1:.6} score={:.4}",base(l0,f0),base(l1,f1));
    }
}
