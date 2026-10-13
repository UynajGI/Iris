// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{color, Frame, RenderContext};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScopeDomain {
    LinearRec2020,
    EncodedSrgb,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Histogram {
    pub domain: ScopeDomain,
    pub lower: f32,
    pub upper: f32,
    /// RGB and luminance, excluding fully transparent pixels.
    pub bins: [Vec<u64>; 4],
    pub below: [u64; 4],
    pub above: [u64; 4],
    pub visible_pixels: u64,
}

pub fn histogram(
    input: &Frame,
    domain: ScopeDomain,
    lower: f32,
    upper: f32,
    bin_count: usize,
    context: &RenderContext,
) -> Result<Histogram> {
    input.validate()?;
    ensure!(
        lower.is_finite() && upper.is_finite() && upper > lower && (upper - lower).is_finite(),
        "invalid histogram range"
    );
    ensure!((2..=4096).contains(&bin_count), "invalid histogram size");
    let mut histogram = Histogram {
        domain,
        lower,
        upper,
        bins: std::array::from_fn(|_| vec![0; bin_count]),
        below: [0; 4],
        above: [0; 4],
        visible_pixels: 0,
    };
    for row in input.pixels.chunks(input.width as usize) {
        context.check_cancelled()?;
        for pixel in row {
            if pixel[3] == 0.0 {
                continue;
            }
            histogram.visible_pixels += 1;
            let rgb = [pixel[0], pixel[1], pixel[2]];
            let y = match domain {
                ScopeDomain::LinearRec2020 => color::luminance(rgb),
                // Encoded waveform/luma, not physical scene luminance.
                ScopeDomain::EncodedSrgb => {
                    pixel[0] * 0.2126 + pixel[1] * 0.7152 + pixel[2] * 0.0722
                }
            };
            for (channel, value) in [pixel[0], pixel[1], pixel[2], y].into_iter().enumerate() {
                if value < lower {
                    histogram.below[channel] += 1;
                } else if value > upper {
                    histogram.above[channel] += 1;
                } else {
                    let bin = (((value - lower) / (upper - lower)) * bin_count as f32) as usize;
                    histogram.bins[channel][bin.min(bin_count - 1)] += 1;
                }
            }
        }
    }
    Ok(histogram)
}

/// Alpha-weighted linear RGB sample over normalized rectangle [x,y,w,h].
pub fn sample(input: &Frame, rect: [f32; 4], context: &RenderContext) -> Result<[f32; 3]> {
    input.validate()?;
    ensure!(
        rect.iter().all(|v| v.is_finite())
            && rect[0] >= 0.0
            && rect[1] >= 0.0
            && rect[2] > 0.0
            && rect[3] > 0.0
            && rect[0] + rect[2] <= 1.0
            && rect[1] + rect[3] <= 1.0,
        "sample rectangle outside image"
    );
    let x0 = (rect[0] * input.width as f32).floor() as u32;
    let x1 = ((rect[0] + rect[2]) * input.width as f32)
        .ceil()
        .min(input.width as f32) as u32;
    let y0 = (rect[1] * input.height as f32).floor() as u32;
    let y1 = ((rect[1] + rect[3]) * input.height as f32)
        .ceil()
        .min(input.height as f32) as u32;
    let mut sum = [0.0_f64; 3];
    let mut weight = 0.0_f64;
    for y in y0..y1 {
        context.check_cancelled()?;
        for x in x0..x1 {
            let pixel = input.pixels[(y * input.width + x) as usize];
            for c in 0..3 {
                sum[c] += f64::from(pixel[c]) * f64::from(pixel[3]);
            }
            weight += f64::from(pixel[3]);
        }
    }
    ensure!(weight > 0.0, "sample has no visible pixels");
    Ok(sum.map(|value| (value / weight) as f32))
}
