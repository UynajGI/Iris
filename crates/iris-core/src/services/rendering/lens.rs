// SPDX-License-Identifier: GPL-3.0-or-later
use super::{EditRenderJob, Services};
use crate::metadata::{rational_field, text_field};
use crate::services::EditResourceKind;
use anyhow::{ensure, Result};
use iris_render::{
    lens_profile::{LensCaptureMetadata, LensProfile, LensProfileMatch},
    RenderContext,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Cursor, Read},
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LensProfileSuggestion {
    pub document_id: String,
    pub expected_revision: u64,
    pub source_fingerprint: String,
    pub resource: String,
    pub capture: LensCaptureMetadata,
    pub matching: LensProfileMatch,
    /// Merge optical coefficients into a node using the uncropped source extent.
    /// This response never mutates a document or replaces crop/rotation/perspective.
    pub placement: String,
}
pub struct LensProfileJob {
    render: EditRenderJob,
    profile: LensProfile,
    resource: String,
}
impl Services {
    pub fn prepare_lens_profile_suggestion(
        &self,
        id: &str,
        expected_revision: u64,
        resource: &str,
    ) -> Result<LensProfileJob> {
        ensure!(
            self.edit_resource(resource)?.kind == EditResourceKind::LensProfile,
            "lens suggestion requires a lens_profile resource"
        );
        let profile = LensProfile::parse(&self.read_edit_resource(resource)?)?;
        Ok(LensProfileJob {
            render: self.prepare_edit_render(id, expected_revision)?,
            profile,
            resource: resource.into(),
        })
    }
}
impl LensProfileJob {
    /// File hashing and EXIF parsing run outside the shared service lock.
    pub fn suggest(&self, context: &RenderContext) -> Result<LensProfileSuggestion> {
        self.render.verify_source(context)?;
        let source = &self.render.record.document.source;
        let capture =
            capture_metadata(&self.render.source, [source.width, source.height], context)?;
        self.render.verify_source(context)?;
        Ok(LensProfileSuggestion {
            document_id: self.render.record.id.clone(),
            expected_revision: self.render.record.revision,
            source_fingerprint: source.fingerprint.clone(),
            resource: self.resource.clone(),
            matching: self.profile.match_capture(&capture)?,
            capture,
            placement: "optical_coefficients_only_on_uncropped_source_extent".into(),
        })
    }
}

/// Reads at most 16 MiB of the original container, without external tools or
/// catalogue/user overrides. EXIF beyond that prefix or unsupported containers
/// simply lack matching evidence; the caller must not apply a guessed profile.
fn capture_metadata(
    path: &std::path::Path,
    dimensions: [u32; 2],
    context: &RenderContext,
) -> Result<LensCaptureMetadata> {
    let mut capture = LensCaptureMetadata {
        dimensions,
        ..Default::default()
    };
    let file = File::open(path)?;
    let length = file.metadata()?.len().min(16 * 1024 * 1024);
    let _memory = context.reserve(length * 2)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length as usize)?;
    file.take(length).read_to_end(&mut bytes)?;
    context.check_cancelled()?;
    if let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        capture.camera_make = text_field(&exif, exif::Tag::Make);
        capture.camera_model = text_field(&exif, exif::Tag::Model);
        capture.lens_model = text_field(&exif, exif::Tag::LensModel);
        capture.focal_mm = rational_field(&exif, exif::Tag::FocalLength);
        capture.f_number = rational_field(&exif, exif::Tag::FNumber);
    }
    context.check_cancelled()?;
    Ok(capture)
}
