use super::*;
use crate::vision::{
    comparison::{self, ComparisonReport},
    VisionAnalysis,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ComparisonRequest {
    pub photo_ids: Vec<i64>,
    pub reference_photo_id: Option<i64>,
}
impl Services {
    pub fn compare_photos(
        &self,
        project: i64,
        request: ComparisonRequest,
    ) -> Result<ComparisonReport> {
        anyhow::ensure!(
            (2..=32).contains(&request.photo_ids.len()),
            "compare 2..=32 photos"
        );
        anyhow::ensure!(
            request
                .photo_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len()
                == request.photo_ids.len(),
            "duplicate comparison members"
        );
        anyhow::ensure!(
            request
                .reference_photo_id
                .is_none_or(|id| request.photo_ids.contains(&id)),
            "reference must be a comparison member"
        );
        self.require_available_photos(project, &request.photo_ids)?;
        let items: Vec<(i64, Option<VisionAnalysis>)> = request
            .photo_ids
            .iter()
            .map(|id| {
                let photo = self.photo(*id)?;
                let analysis = if photo.analysis_status == AnalysisStatus::Current {
                    photo.analysis.and_then(|a| serde_json::from_value(a).ok())
                } else {
                    None
                };
                Ok((*id, analysis))
            })
            .collect::<Result<_>>()?;
        let borrowed: Vec<_> = items.iter().map(|(id, a)| (*id, a.as_ref())).collect();
        Ok(comparison::compare(&borrowed, request.reference_photo_id))
    }
}
