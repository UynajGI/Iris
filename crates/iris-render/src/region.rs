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

/// A native-pixel viewport centered in the final, edited image. Normalized
/// coordinates avoid guessing dimensions after rotation, perspective or crop.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegionView {
    pub center: [f32; 2],
    pub width: u32,
    pub height: u32,
}

impl RegionView {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.center
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "region center must be finite and within 0..=1"
        );
        PixelRect {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        }
        .validate()
    }

    pub fn resolve(&self, width: u32, height: u32) -> Result<PixelRect> {
        self.validate()?;
        ensure!(width > 0 && height > 0, "empty output image");
        let w = self.width.min(width);
        let h = self.height.min(height);
        let origin = |center: f32, full: u32, size: u32| {
            (f64::from(center) * f64::from(full) - f64::from(size) / 2.0)
                .round()
                .clamp(0.0, f64::from(full - size)) as u32
        };
        Ok(PixelRect {
            x: origin(self.center[0], width, w),
            y: origin(self.center[1], height, h),
            width: w,
            height: h,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_view_clamps_native_pixels_without_scaling() {
        let mut view = RegionView {
            center: [0.5, 0.5],
            width: 200,
            height: 100,
        };
        assert_eq!(
            view.resolve(1000, 600).unwrap(),
            PixelRect {
                x: 400,
                y: 250,
                width: 200,
                height: 100
            }
        );
        view.center = [0.0, 1.0];
        assert_eq!(
            view.resolve(1000, 600).unwrap(),
            PixelRect {
                x: 0,
                y: 500,
                width: 200,
                height: 100
            }
        );
        assert_eq!(
            view.resolve(32, 24).unwrap(),
            PixelRect {
                x: 0,
                y: 0,
                width: 32,
                height: 24
            }
        );
        for center in [[-0.1, 0.5], [1.1, 0.5], [f32::NAN, 0.5]] {
            view.center = center;
            assert!(view.resolve(1000, 600).is_err());
        }
        view.center = [0.5, 0.5];
        view.width = 2049;
        assert!(view.resolve(32, 24).is_err());
        view.width = 0;
        assert!(view.validate().is_err());
    }
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
