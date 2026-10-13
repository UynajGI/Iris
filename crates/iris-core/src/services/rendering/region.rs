// SPDX-License-Identifier: GPL-3.0-or-later
use super::EditRenderJob;
use anyhow::Result;
use iris_render::{
    color, output,
    region::{self, PixelRect},
    RenderContext,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EditRegionMetadata {
    pub rect: PixelRect,
    pub full_width: u32,
    pub full_height: u32,
    pub revision: u64,
    pub fingerprint: String,
    pub approximate: bool,
    /// True: all edit nodes are evaluated at full resolution before extraction.
    pub full_frame_evaluation: bool,
}

pub struct EncodedEditRegion {
    pub jpeg: Vec<u8>,
    pub metadata: EditRegionMetadata,
}

impl EditRenderJob {
    /// Native output-pixel view, never max-edge resized. This first-stage path
    /// evaluates the entire full-resolution graph and therefore needs its full
    /// memory/time budget. Only terminal sRGB conversion operates on the region.
    pub fn encode_edit_region(
        &self,
        context: &RenderContext,
        rect: PixelRect,
    ) -> Result<EncodedEditRegion> {
        rect.validate()?;
        context.check_cancelled()?;
        let full = self.full_resolution(context)?;
        let metadata = EditRegionMetadata {
            rect,
            full_width: full.width,
            full_height: full.height,
            revision: self.record.revision,
            fingerprint: self.record.document.fingerprint()?,
            approximate: false,
            full_frame_evaluation: true,
        };
        let region = region::extract(&full, rect, context)?;
        let _region_memory = context.reserve(region.byte_len())?;
        drop(full);
        let mut config = self.record.document.output.clone();
        config.space = color::OutputSpace::Srgb;
        let target = color::output(&region, &config, context)?;
        let _target_memory = context.reserve(target.byte_len())?;
        let jpeg = output::encode_output(
            &target,
            &config,
            output::OutputFormat::Jpeg8 { quality: 90 },
            context,
        )?;
        context.check_cancelled()?;
        Ok(EncodedEditRegion { jpeg, metadata })
    }
}
