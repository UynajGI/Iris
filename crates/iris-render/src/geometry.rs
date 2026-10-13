// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit manual geometry; no camera/lens profile database. Output has the
//! rounded crop dimensions, with no rotation expansion. Pixel-center inverse
//! mapping applies crop, inverse rotation about image center, output-to-input
//! homography, then radial distortion. Coordinates use image edges [0,1].
//! Bilinear interpolation uses premultiplied alpha and transparent borders.
use crate::{Frame, RenderContext};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Geometry {
    /// [left, top, width, height], normalized to the input frame.
    pub crop: [f32; 4],
    /// Clockwise displayed rotation/straightening, degrees.
    pub rotation_degrees: f32,
    /// Row-major inverse homography in normalized image coordinates.
    pub perspective: [f32; 9],
    /// Inverse radial mapping r -> r*(1+k1*r²+k2*r⁴), radius in half short edge.
    pub radial: [f32; 2],
    /// Additional red/blue radial scale relative to green, bounded to +/- 0.1.
    pub chromatic_aberration: [f32; 2],
    /// Exposure lift at radius 1 in stops (quadratic radial rolloff).
    pub vignette_stops: f32,
}
impl Default for Geometry {
    fn default() -> Self {
        Self {
            crop: [0.0, 0.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            perspective: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            radial: [0.0, 0.0],
            chromatic_aberration: [0.0, 0.0],
            vignette_stops: 0.0,
        }
    }
}
impl Geometry {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.crop
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                && self.crop[2] > 0.0
                && self.crop[3] > 0.0
                && self.crop[0] as f64 + self.crop[2] as f64 <= 1.000001
                && self.crop[1] as f64 + self.crop[3] as f64 <= 1.000001,
            "invalid crop"
        );
        ensure!(
            self.rotation_degrees.is_finite() && self.rotation_degrees.abs() <= 360.0,
            "invalid rotation"
        );
        ensure!(
            self.perspective
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 100.0),
            "invalid homography"
        );
        let h = self.perspective.map(f64::from);
        let det = h[0] * (h[4] * h[8] - h[5] * h[7]) - h[1] * (h[3] * h[8] - h[5] * h[6])
            + h[2] * (h[3] * h[7] - h[4] * h[6]);
        ensure!(det.abs() > 1e-10, "singular homography");
        ensure!(
            self.radial.iter().all(|v| v.is_finite() && v.abs() <= 2.0)
                && self
                    .chromatic_aberration
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 0.1)
                && self.vignette_stops.is_finite()
                && self.vignette_stops.abs() <= 8.0,
            "invalid manual lens correction"
        );
        Ok(())
    }
    /// Green-channel input anchor for an output normalized position. A projective
    /// horizon returns None. Positions beyond the frame map to transparent pixels.
    /// Call validate before using this mapping independently of apply.
    pub fn map_to_input(&self, position: [f64; 2], width: u32, height: u32) -> Option<[f64; 2]> {
        if width == 0 || height == 0 || !position.iter().all(|v| v.is_finite()) {
            return None;
        }
        let x = (self.crop[0] as f64 + position[0] * self.crop[2] as f64 - 0.5) * width as f64;
        let y = (self.crop[1] as f64 + position[1] * self.crop[3] as f64 - 0.5) * height as f64;
        let (s, c) = (-(self.rotation_degrees as f64).to_radians()).sin_cos();
        let (x, y) = (
            (c * x - s * y) / width as f64 + 0.5,
            (s * x + c * y) / height as f64 + 0.5,
        );
        let h = self.perspective.map(f64::from);
        let w = h[6] * x + h[7] * y + h[8];
        if w.abs() < 1e-10 {
            return None;
        }
        let p = [
            (h[0] * x + h[1] * y + h[2]) / w,
            (h[3] * x + h[4] * y + h[5]) / w,
        ];
        let short = width.min(height) as f64;
        let r2 = ((p[0] - 0.5) * 2.0 * width as f64 / short).powi(2)
            + ((p[1] - 0.5) * 2.0 * height as f64 / short).powi(2);
        let factor = 1.0 + self.radial[0] as f64 * r2 + self.radial[1] as f64 * r2 * r2;
        let out = [0.5 + (p[0] - 0.5) * factor, 0.5 + (p[1] - 0.5) * factor];
        out.iter().all(|v| v.is_finite()).then_some(out)
    }
}

fn sample_coordinate_valid(value: f64, extent: u32) -> bool {
    value.is_finite() && value >= -1.0 && value <= extent as f64
}

fn sample(input: &Frame, p: [f64; 2]) -> [f32; 4] {
    let x = p[0] * input.width as f64 - 0.5;
    let y = p[1] * input.height as f64 - 0.5;
    if !sample_coordinate_valid(x, input.width) || !sample_coordinate_valid(y, input.height) {
        return [0.0; 4];
    }
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let fx = x - ix as f64;
    let fy = y - iy as f64;
    let mut out = [0.0f64; 4];
    for dy in 0..2 {
        for dx in 0..2 {
            let sx = ix + dx;
            let sy = iy + dy;
            if sx < 0 || sy < 0 || sx >= input.width as i64 || sy >= input.height as i64 {
                continue;
            }
            let w = if dx == 0 { 1.0 - fx } else { fx } * if dy == 0 { 1.0 - fy } else { fy };
            let p = input.pixels[sy as usize * input.width as usize + sx as usize];
            out[3] += w * p[3] as f64;
            for c in 0..3 {
                out[c] += w * p[c] as f64 * p[3] as f64;
            }
        }
    }
    if out[3] > 0.0 {
        for c in 0..3 {
            out[c] /= out[3];
        }
    }
    out.map(|v| v.clamp(-(f32::MAX as f64), f32::MAX as f64) as f32)
}

fn correct_lens(input: &Frame, params: &Geometry, p: [f64; 2]) -> [f32; 4] {
    let mut pixel = sample(input, p);
    for (channel, coefficient) in [
        (0, params.chromatic_aberration[0]),
        (2, params.chromatic_aberration[1]),
    ] {
        if coefficient != 0.0 {
            let scale = 1.0 + coefficient as f64;
            let shifted = sample(
                input,
                [0.5 + (p[0] - 0.5) * scale, 0.5 + (p[1] - 0.5) * scale],
            );
            // Green defines coverage; absent shifted samples contribute zero.
            pixel[channel] = if shifted[3] > 0.0 {
                shifted[channel]
            } else {
                0.0
            };
        }
    }
    let short = input.width.min(input.height).max(1) as f64;
    let r2 = (((p[0] - 0.5) * 2.0 * input.width as f64 / short).powi(2)
        + ((p[1] - 0.5) * 2.0 * input.height as f64 / short).powi(2))
    .min(1.0);
    let gain = 2.0f64.powf(params.vignette_stops as f64 * r2);
    for value in &mut pixel[..3] {
        *value = (*value as f64 * gain).clamp(-(f32::MAX as f64), f32::MAX as f64) as f32;
    }
    pixel
}

pub fn apply(input: &Frame, params: &Geometry, context: &RenderContext) -> Result<Frame> {
    input.validate()?;
    params.validate()?;
    context.check_cancelled()?;
    if *params == Geometry::default() {
        let _memory = context.reserve(input.byte_len())?;
        return Ok(input.clone());
    }
    let width = (input.width as f64 * params.crop[2] as f64)
        .round()
        .max(1.0) as u32;
    let height = (input.height as f64 * params.crop[3] as f64)
        .round()
        .max(1.0) as u32;
    let count = width as u64 * height as u64;
    ensure!(count <= 120_000_000, "geometry exceeds pixel budget");
    let _memory = context.reserve(count * 16)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(count as usize)?;
    for y in 0..height {
        context.check_cancelled()?;
        for x in 0..width {
            if x % 1024 == 0 {
                context.check_cancelled()?;
            }
            let Some(p) = params.map_to_input(
                [
                    (x as f64 + 0.5) / width as f64,
                    (y as f64 + 0.5) / height as f64,
                ],
                input.width,
                input.height,
            ) else {
                pixels.push([0.0; 4]);
                continue;
            };
            let pixel = correct_lens(input, params, p);
            pixels.push(pixel);
        }
    }
    Frame::new(width, height, pixels)
}
