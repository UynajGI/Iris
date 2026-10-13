// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{ensure, Result};

/// A bounded, orientation-normalized, linear Rec.2020 D65 image.
/// RGB is unbounded finite FP32; alpha is straight, finite, in 0..=1.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[f32; 4]>,
}

pub const MAX_PIXELS: u64 = 120_000_000;

pub fn pixel_count(width: u32, height: u32) -> Result<usize> {
    let count = u64::from(width) * u64::from(height);
    ensure!(
        width > 0 && height > 0 && count <= MAX_PIXELS,
        "image dimensions exceed render limits"
    );
    Ok(usize::try_from(count)?)
}

impl Frame {
    pub fn new(width: u32, height: u32, pixels: Vec<[f32; 4]>) -> Result<Self> {
        let frame = Self {
            width,
            height,
            pixels,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            pixel_count(self.width, self.height)? == self.pixels.len(),
            "pixel count mismatch"
        );
        ensure!(
            self.pixels
                .iter()
                .all(|p| p.iter().all(|v| v.is_finite()) && (0.0..=1.0).contains(&p[3])),
            "invalid image values"
        );
        Ok(())
    }

    pub fn byte_len(&self) -> u64 {
        self.pixels.len() as u64 * 16
    }

    /// Spatial reduction keeps FP32 data. Area averaging uses premultiplied
    /// alpha so invisible RGB cannot bleed into visible edges.
    pub fn proxy(&self, max_edge: u32, context: &crate::RenderContext) -> Result<Self> {
        self.validate()?;
        ensure!(max_edge > 0, "proxy edge must be positive");
        context.check_cancelled()?;
        let largest = self.width.max(self.height);
        if largest <= max_edge {
            let _budget = context.reserve(self.byte_len())?;
            return Ok(self.clone());
        }
        let w = (u64::from(self.width) * u64::from(max_edge) / u64::from(largest)).max(1) as u32;
        let h = (u64::from(self.height) * u64::from(max_edge) / u64::from(largest)).max(1) as u32;
        let count = pixel_count(w, h)?;
        let _budget = context.reserve(count as u64 * 16)?;
        let mut pixels = Vec::with_capacity(count);
        // Exact area coverage, including fractional source pixel boundaries.
        for y in 0..h {
            context.check_cancelled()?;
            let y0 = f64::from(y) * f64::from(self.height) / f64::from(h);
            let y1 = f64::from(y + 1) * f64::from(self.height) / f64::from(h);
            for x in 0..w {
                let x0 = f64::from(x) * f64::from(self.width) / f64::from(w);
                let x1 = f64::from(x + 1) * f64::from(self.width) / f64::from(w);
                let mut sum = [0.0_f64; 4];
                let area = (x1 - x0) * (y1 - y0);
                for sy in y0.floor() as u32..(y1.ceil() as u32).min(self.height) {
                    let wy = (y1.min(f64::from(sy + 1)) - y0.max(f64::from(sy))).max(0.0);
                    for sx in x0.floor() as u32..(x1.ceil() as u32).min(self.width) {
                        let weight =
                            wy * (x1.min(f64::from(sx + 1)) - x0.max(f64::from(sx))).max(0.0);
                        let p = self.pixels[(sy * self.width + sx) as usize];
                        for c in 0..3 {
                            sum[c] += f64::from(p[c]) * f64::from(p[3]) * weight;
                        }
                        sum[3] += f64::from(p[3]) * weight;
                    }
                }
                let rgb_weight = sum[3];
                pixels.push(if rgb_weight > 0.0 {
                    [
                        (sum[0] / rgb_weight) as f32,
                        (sum[1] / rgb_weight) as f32,
                        (sum[2] / rgb_weight) as f32,
                        (sum[3] / area).clamp(0.0, 1.0) as f32,
                    ]
                } else {
                    [0.0; 4]
                });
            }
        }
        Self::new(w, h, pixels)
    }

    fn oriented_source(&self, orientation: u8, x: u32, y: u32) -> (u32, u32) {
        match orientation {
            2 => (self.width - 1 - x, y),
            3 => (self.width - 1 - x, self.height - 1 - y),
            4 => (x, self.height - 1 - y),
            5 => (y, x),
            6 => (y, self.height - 1 - x),
            7 => (self.width - 1 - y, self.height - 1 - x),
            8 => (self.width - 1 - y, x),
            _ => (x, y),
        }
    }

    /// Apply the EXIF transform exactly once at the source boundary.
    pub fn oriented(&self, orientation: u8, context: &crate::RenderContext) -> Result<Self> {
        self.validate()?;
        ensure!((1..=8).contains(&orientation), "invalid EXIF orientation");
        context.check_cancelled()?;
        if orientation == 1 {
            let _budget = context.reserve(self.byte_len())?;
            return Ok(self.clone());
        }
        let (w, h) = if orientation >= 5 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        let _budget = context.reserve(self.byte_len())?;
        let mut out = vec![[0.0; 4]; self.pixels.len()];
        for y in 0..h {
            context.check_cancelled()?;
            for x in 0..w {
                let (sx, sy) = self.oriented_source(orientation, x, y);
                out[(y * w + x) as usize] = self.pixels[(sy * self.width + sx) as usize];
            }
        }
        Self::new(w, h, out)
    }
}
