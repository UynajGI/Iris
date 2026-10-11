//! Selection evidence is technical observation, never a calibrated probability.
use super::{EyeState, Verdict, VisionAnalysis};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const POLICY: &str = "selection_preview_evidence_v1";
pub const CLOSE_SCORE_GAP: f64 = 5.;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SelectionReason {
    pub code: String,
    /// observation, limitation, or context; missing evidence is not a defect.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SelectionEvidence {
    pub method: String,
    pub basis: String,
    pub subject: String,
    pub reasons: Vec<SelectionReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupComparison {
    pub photo_id: i64,
    pub score_gap: Option<f64>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupAssessment {
    pub method: String,
    /// Comparison reference only. Never an instruction to discard other members.
    pub reference_photo_id: Option<i64>,
    pub candidate_photo_ids: Vec<i64>,
    pub close_score_gap: f64,
    pub needs_review: bool,
    pub comparisons: Vec<GroupComparison>,
}

fn reason(code: &str, kind: &str) -> SelectionReason {
    SelectionReason {
        code: code.into(),
        kind: kind.into(),
    }
}

pub(super) fn evidence(a: &VisionAnalysis) -> SelectionEvidence {
    let mut reasons = vec![reason("preview_measurement", "context")];
    if a.faces.is_empty() {
        reasons.push(reason("no_face_detected", "context"));
    }
    face_reasons(a, &mut reasons);
    exposure_reasons(a, &mut reasons);
    if a.sharpness_lap < 8. {
        reasons.push(reason("low_preview_detail", "observation"));
    }
    if !measurements_valid(a) {
        reasons.push(reason("invalid_measurements", "limitation"));
    }
    if a.warnings.iter().any(|w| w.contains("omitted faces")) {
        reasons.push(reason("omitted_faces", "limitation"));
    }
    SelectionEvidence {
        method: POLICY.into(),
        basis: "preview".into(),
        subject: if a.faces.is_empty() {
            "scene"
        } else {
            "people"
        }
        .into(),
        reasons,
    }
}

fn face_reasons(a: &VisionAnalysis, reasons: &mut Vec<SelectionReason>) {
    let closed = a.faces.iter().any(|f| {
        [f.left_eye.as_ref(), f.right_eye.as_ref()]
            .into_iter()
            .flatten()
            .any(|e| e.state == EyeState::Closed)
    });
    if closed {
        reasons.push(reason("closed_eye_review", "observation"));
    }
    let uncertain = a.faces.iter().any(|f| {
        [f.left_eye.as_ref(), f.right_eye.as_ref()]
            .into_iter()
            .any(|e| e.is_none_or(|e| e.state == EyeState::Uncertain))
    });
    if uncertain {
        reasons.push(reason("eye_evidence_limited", "limitation"));
    }
    if a.faces.iter().any(|f| {
        f.quality
            .as_ref()
            .is_none_or(|q| q.crop_width.min(q.crop_height) < 32)
    }) {
        reasons.push(reason("face_detail_limited", "limitation"));
    }
}

fn exposure_reasons(a: &VisionAnalysis, reasons: &mut Vec<SelectionReason>) {
    if a.exposure.mean < 60. || a.exposure.mean > 200. {
        reasons.push(reason("tonal_style_review", "context"));
    }
    if a.exposure.shadow_clip > 0.1 {
        reasons.push(reason("preview_shadow_clip", "observation"));
    }
    if a.exposure.highlight_clip > 0.1 {
        reasons.push(reason("preview_highlight_clip", "observation"));
    }
    if raw_preview(a) {
        reasons.push(reason("raw_recovery_unknown", "limitation"));
    }
}

pub(super) fn raw_preview(a: &VisionAnalysis) -> bool {
    a.preview_source
        .as_deref()
        .is_some_and(|s| s.starts_with("raw_"))
}

pub(super) fn measurements_valid(a: &VisionAnalysis) -> bool {
    [
        a.sharpness_lap,
        a.sharpness_fft,
        a.exposure.mean,
        a.exposure.shadow_clip,
        a.exposure.highlight_clip,
        a.composite_score,
    ]
    .iter()
    .all(|n| n.is_finite())
        && (0. ..=1.).contains(&a.exposure.shadow_clip)
        && (0. ..=1.).contains(&a.exposure.highlight_clip)
}

pub(super) fn can_recommend(a: &VisionAnalysis, threshold: f64, all_open: bool) -> bool {
    (all_open || a.faces.is_empty())
        && !a.warnings.iter().any(|w| w.contains("omitted faces"))
        && measurements_valid(a)
        && a.sharpness_lap >= 8.
        && a.composite_score >= threshold
        && a.exposure.shadow_clip <= 0.1
        && a.exposure.highlight_clip <= 0.1
}

fn usable(a: &VisionAnalysis) -> bool {
    measurements_valid(a)
        && a.sharpness_lap >= 8.
        && a.verdict != Verdict::RejectSuggest
        && !evidence(a).reasons.iter().any(|r| {
            (r.kind == "limitation" && r.code != "raw_recovery_unknown")
                || r.code == "closed_eye_review"
        })
}

/// Callers supply only current, available analyses; absent entries stay visible as unknown.
pub fn assess_group(items: &[(i64, Option<&VisionAnalysis>)]) -> GroupAssessment {
    let reference = items
        .iter()
        .filter_map(|(id, a)| a.filter(|a| usable(a)).map(|a| (*id, a)))
        .max_by(|(aid, a), (bid, b)| {
            a.composite_score
                .total_cmp(&b.composite_score)
                .then(bid.cmp(aid))
        });
    let candidate_photo_ids = items
        .iter()
        .filter_map(|(id, a)| {
            let a = a.filter(|a| usable(a))?;
            let (_, best) = reference?;
            (best.composite_score - a.composite_score <= CLOSE_SCORE_GAP).then_some(*id)
        })
        .collect();
    GroupAssessment {
        method: "group_technical_comparison_v1".into(),
        reference_photo_id: reference.map(|(id, _)| id),
        candidate_photo_ids,
        close_score_gap: CLOSE_SCORE_GAP,
        needs_review: items
            .iter()
            .any(|(_, a)| a.is_none_or(|a| !usable(a) || a.verdict != Verdict::Recommend)),
        comparisons: items
            .iter()
            .map(|(id, a)| compare(*id, *a, reference))
            .collect(),
    }
}

fn compare(
    id: i64,
    analysis: Option<&VisionAnalysis>,
    reference: Option<(i64, &VisionAnalysis)>,
) -> GroupComparison {
    let Some(a) = analysis.filter(|a| measurements_valid(a)) else {
        return GroupComparison {
            photo_id: id,
            score_gap: None,
            reasons: vec!["analysis_unavailable".into()],
        };
    };
    let mut reasons = Vec::new();
    if !usable(a) || a.verdict == Verdict::Review {
        reasons.push("review_observations".into());
    }
    let gap = reference.map(|(_, best)| (best.composite_score - a.composite_score).max(0.));
    if let Some((best_id, best)) = reference {
        if id == best_id {
            reasons.push("technical_reference".into());
        } else if gap.is_some_and(|v| v <= CLOSE_SCORE_GAP) && usable(a) {
            reasons.push("close_quality".into());
        }
        compare_components(a, best, &mut reasons);
        if a.faces.len() != best.faces.len() {
            reasons.push("different_face_count".into());
        }
    }
    GroupComparison {
        photo_id: id,
        score_gap: gap,
        reasons,
    }
}

fn compare_components(a: &VisionAnalysis, best: &VisionAnalysis, reasons: &mut Vec<String>) {
    let component = |v: &VisionAnalysis, id: &str| {
        v.score_breakdown
            .as_ref()
            .and_then(|b| b.components.iter().find(|c| c.id == id))
            .and_then(|c| c.score)
    };
    for (id, lower, higher) in [
        ("face", "face_detail_lower", "face_detail_higher"),
        ("sharpness", "preview_detail_lower", "preview_detail_higher"),
    ] {
        if let (Some(left), Some(right)) = (component(a, id), component(best, id)) {
            if left + 10. < right {
                reasons.push(lower.into());
            }
            if right + 10. < left {
                reasons.push(higher.into());
            }
        }
    }
}
