//! Geometry candidates assist navigation, never identify people.
use super::{assess_group, GroupAssessment, VisionAnalysis};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct FaceCorrespondence {
    pub reference_face_index: usize,
    pub face_index: Option<usize>,
    /// geometry_candidate or uncertain; users must verify correspondence.
    pub state: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ComparisonMember {
    pub photo_id: i64,
    pub score_delta: Option<f64>,
    pub exposure_mean_delta: Option<f64>,
    pub face_count: Option<usize>,
    pub face_correspondences: Vec<FaceCorrespondence>,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ComparisonReport {
    pub method: String,
    pub reference_photo_id: Option<i64>,
    pub assessment: GroupAssessment,
    pub members: Vec<ComparisonMember>,
}
pub fn compare(
    items: &[(i64, Option<&VisionAnalysis>)],
    reference: Option<i64>,
) -> ComparisonReport {
    let assessment = assess_group(items);
    let reference = reference
        .or(assessment.reference_photo_id)
        .or_else(|| items.iter().find(|(_, a)| a.is_some()).map(|(id, _)| *id));
    let anchor = reference.and_then(|id| {
        items
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .and_then(|(_, a)| *a)
    });
    let members = items
        .iter()
        .map(|(id, analysis)| ComparisonMember {
            photo_id: *id,
            score_delta: analysis
                .zip(anchor)
                .and_then(|(a, b)| finite(a.composite_score - b.composite_score)),
            exposure_mean_delta: analysis
                .zip(anchor)
                .and_then(|(a, b)| finite(a.exposure.mean - b.exposure.mean)),
            face_count: analysis.map(|a| a.faces.len()),
            face_correspondences: anchor
                .map(|a| correspondences(a, *analysis))
                .unwrap_or_default(),
        })
        .collect();
    ComparisonReport {
        method: "selection-differences-geometry-v1".into(),
        reference_photo_id: reference,
        assessment,
        members,
    }
}
fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}
fn geometry(a: &VisionAnalysis) -> Vec<(usize, [f64; 3])> {
    if a.width == 0 || a.height == 0 {
        return vec![];
    }
    a.faces
        .iter()
        .filter_map(|f| {
            let [x, y, w, h] = f.bbox;
            (f.confidence >= 0.55 && [x, y, w, h].iter().all(|v| v.is_finite()) && w > 0. && h > 0.)
                .then_some((
                    f.index,
                    [
                        f64::from((x + w / 2.) / a.width as f32),
                        f64::from((y + h / 2.) / a.height as f32),
                        f64::from(w * h) / f64::from(a.width) / f64::from(a.height),
                    ],
                ))
        })
        .collect()
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let ratio = a[2] / b[2];
    if !(0.5..=2.).contains(&ratio) {
        return f64::INFINITY;
    }
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn nearest(target: [f64; 3], candidates: &[(usize, [f64; 3])]) -> Option<usize> {
    let mut distances: Vec<_> = candidates
        .iter()
        .map(|(id, point)| (*id, distance(target, *point)))
        .collect();
    distances.sort_by(|a, b| a.1.total_cmp(&b.1));
    let first = distances.first()?;
    (first.1 <= 0.15
        && distances
            .get(1)
            .is_none_or(|second| second.1 - first.1 >= 0.05))
    .then_some(first.0)
}
fn correspondences(
    reference: &VisionAnalysis,
    other: Option<&VisionAnalysis>,
) -> Vec<FaceCorrespondence> {
    let a = geometry(reference);
    let b = other.map(geometry).unwrap_or_default();
    reference
        .faces
        .iter()
        .map(|face| {
            let matched = a
                .iter()
                .find(|(id, _)| *id == face.index)
                .and_then(|(_, point)| nearest(*point, &b))
                .filter(|id| {
                    b.iter()
                        .find(|(candidate, _)| candidate == id)
                        .and_then(|(_, point)| nearest(*point, &a))
                        == Some(face.index)
                });
            FaceCorrespondence {
                reference_face_index: face.index,
                face_index: matched,
                state: if matched.is_some() {
                    "geometry_candidate"
                } else {
                    "uncertain"
                }
                .into(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matching_requires_separation_reciprocity_and_similar_size() {
        let center = [0.5, 0.5, 0.1];
        assert_eq!(
            nearest(center, &[(1, [0.52, 0.5, 0.11]), (2, [0.9, 0.9, 0.1])]),
            Some(1)
        );
        assert_eq!(
            nearest(center, &[(1, [0.49, 0.5, 0.1]), (2, [0.51, 0.5, 0.1])]),
            None
        );
        assert_eq!(nearest(center, &[(1, [0.5, 0.5, 0.01])]), None);
        assert_eq!(nearest(center, &[]), None);
        let report = compare(&[(1, None), (2, None)], None);
        assert!(report.reference_photo_id.is_none());
        assert!(report
            .members
            .iter()
            .all(|m| m.score_delta.is_none() && m.face_correspondences.is_empty()));
    }
}
