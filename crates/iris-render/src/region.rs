// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact pixel extraction from a completed edit output. This is not a sensor
//! ROI decoder or a graph optimization: the caller supplies the full frame.
use crate::{Frame, RenderContext};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=2048).contains(&self.width),
            "region width must be 1..=2048"
        );
        ensure!(
            (1..=2048).contains(&self.height),
            "region height must be 1..=2048"
        );
        ensure!(
            u64::from(self.width) * u64::from(self.height) <= 4_000_000,
            "region exceeds 4MP"
        );
        Ok(())
    }
    pub fn validate_bounds(&self, width: u32, height: u32) -> Result<()> {
        self.validate()?;
        ensure!(
            u64::from(self.x) + u64::from(self.width) <= u64::from(width),
            "region exceeds output width"
        );
        ensure!(
            u64::from(self.y) + u64::from(self.height) <= u64::from(height),
            "region exceeds output height"
        );
        Ok(())
    }
}

/// Coordinates identify output pixels after orientation and edit geometry.
/// Samples, including hidden RGB and alpha, are copied exactly without scaling.
pub fn extract(frame: &Frame, rect: PixelRect, context: &RenderContext) -> Result<Frame> {
    context.check_cancelled()?;
    frame.validate()?;
    rect.validate_bounds(frame.width, frame.height)?;
    let count = rect.width as usize * rect.height as usize;
    let _memory = context.reserve(count as u64 * 16)?;
    let mut pixels = Vec::with_capacity(count);
    for y in rect.y..rect.y + rect.height {
        context.check_cancelled()?;
        let start = y as usize * frame.width as usize + rect.x as usize;
        pixels.extend_from_slice(&frame.pixels[start..start + rect.width as usize]);
    }
    Frame::new(rect.width, rect.height, pixels)
}
