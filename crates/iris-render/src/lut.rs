// SPDX-License-Identifier: GPL-3.0-or-later
//! Strict Iridas .cube reader: 1D or 3D tables, red index varying fastest.
//! Trilinear 3D interpolation. Outside the declared domain, preserve the input
//! rather than silently clipping scene highlights. LUT input space is explicit.
use crate::{
    color::{self, OutputSpace},
    Frame, RenderContext,
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LutDomain {
    LinearRec2020,
    EncodedSrgb,
}

pub struct CubeLut {
    size: usize,
    is_3d: bool,
    min: [f32; 3],
    max: [f32; 3],
    values: Vec<[f32; 3]>,
}

impl CubeLut {
    pub fn parse(text: &str) -> Result<Self> {
        ensure!(text.len() <= 32 * 1024 * 1024, "LUT text exceeds limit");
        let mut size = None;
        let mut is_3d = false;
        let mut min = None;
        let mut max = None;
        let mut values = Vec::new();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let fields: Vec<_> = line.split_whitespace().collect();
            match fields[0] {
                "TITLE" => {
                    ensure!(values.is_empty() && line.len() <= 1024, "invalid LUT title");
                }
                "LUT_1D_SIZE" | "LUT_3D_SIZE" => {
                    let (n, dimensional) = parse_size(&fields, size, values.is_empty())?;
                    is_3d = dimensional;
                    size = Some(n);
                }
                "DOMAIN_MIN" | "DOMAIN_MAX" => {
                    ensure!(fields.len() == 4 && values.is_empty(), "invalid LUT domain");
                    let triple = triple(&fields[1..])?;
                    let target = if fields[0] == "DOMAIN_MIN" {
                        &mut min
                    } else {
                        &mut max
                    };
                    ensure!(target.is_none(), "duplicate LUT domain");
                    *target = Some(triple);
                }
                _ => {
                    let n = size.context("LUT data appeared before size")?;
                    let expected = if is_3d { n * n * n } else { n };
                    ensure!(
                        fields.len() == 3 && values.len() < expected,
                        "invalid LUT row count"
                    );
                    values.push(triple(&fields)?);
                }
            }
        }
        Self::finish_table(size, is_3d, min, max, values)
    }

    fn finish_table(
        size: Option<usize>,
        is_3d: bool,
        min: Option<[f32; 3]>,
        max: Option<[f32; 3]>,
        values: Vec<[f32; 3]>,
    ) -> Result<Self> {
        let size = size.context("missing LUT size")?;
        let expected = if is_3d { size * size * size } else { size };
        ensure!(values.len() == expected, "incomplete LUT table");
        let min = min.unwrap_or([0.0; 3]);
        let max = max.unwrap_or([1.0; 3]);
        ensure!(
            (0..3).all(|c| max[c] > min[c] && (max[c] - min[c]).is_finite()),
            "empty LUT domain"
        );
        Ok(Self {
            size,
            is_3d,
            min,
            max,
            values,
        })
    }

    pub fn sample(&self, rgb: [f32; 3]) -> [f32; 3] {
        if (0..3).any(|c| rgb[c] < self.min[c] || rgb[c] > self.max[c]) {
            return rgb;
        }
        let mut index = [0; 3];
        let mut t = [0.0; 3];
        for c in 0..3 {
            let position =
                (rgb[c] - self.min[c]) / (self.max[c] - self.min[c]) * (self.size - 1) as f32;
            index[c] = (position.floor() as usize).min(self.size - 2);
            t[c] = position - index[c] as f32;
        }
        if !self.is_3d {
            return std::array::from_fn(|c| {
                self.values[index[c]][c] * (1.0 - t[c]) + self.values[index[c] + 1][c] * t[c]
            });
        }
        let mut result = [0.0; 3];
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..2 {
                    let weight = (if x == 0 { 1.0 - t[0] } else { t[0] })
                        * (if y == 0 { 1.0 - t[1] } else { t[1] })
                        * (if z == 0 { 1.0 - t[2] } else { t[2] });
                    let entry = self.values[(index[2] + z) * self.size * self.size
                        + (index[1] + y) * self.size
                        + index[0]
                        + x];
                    for c in 0..3 {
                        result[c] += entry[c] * weight;
                    }
                }
            }
        }
        result
    }

    fn map_pixels(&self, result: &mut Frame, context: &RenderContext) -> Result<()> {
        for row in result.pixels.chunks_mut(result.width as usize) {
            context.check_cancelled()?;
            for pixel in row {
                let mapped = self.sample([pixel[0], pixel[1], pixel[2]]);
                pixel[..3].copy_from_slice(&mapped);
            }
        }
        Ok(())
    }

    pub fn apply(
        &self,
        input: &Frame,
        strength: f32,
        domain: LutDomain,
        context: &RenderContext,
    ) -> Result<Frame> {
        input.validate()?;
        ensure!(
            strength.is_finite() && (0.0..=1.0).contains(&strength),
            "invalid LUT strength"
        );
        let _budget = context.reserve(input.byte_len() * 2)?;
        context.check_cancelled()?;
        if strength == 0.0 {
            return Ok(input.clone());
        }
        let working = color::profile(OutputSpace::LinearRec2020, true)?;
        let srgb = color::profile(OutputSpace::Srgb, false)?;
        let mut result = match domain {
            LutDomain::LinearRec2020 => input.clone(),
            LutDomain::EncodedSrgb => color::transform(input, &working, &srgb, context)?,
        };
        self.map_pixels(&mut result, context)?;
        if domain == LutDomain::EncodedSrgb {
            result = color::transform(&result, &srgb, &working, context)?;
        }
        blend_strength(&mut result, input, strength, context)?;
        result.validate()?;
        Ok(result)
    }
}

fn triple(parts: &[&str]) -> Result<[f32; 3]> {
    if parts.len() != 3 {
        bail!("expected LUT RGB triple");
    }
    let values = [
        parts[0].parse::<f32>()?,
        parts[1].parse()?,
        parts[2].parse()?,
    ];
    ensure!(
        values.iter().all(|v| v.is_finite() && v.abs() <= 65536.0),
        "invalid LUT value"
    );
    Ok(values)
}

fn parse_size(fields: &[&str], previous: Option<usize>, empty: bool) -> Result<(usize, bool)> {
    ensure!(
        previous.is_none() && fields.len() == 2 && empty,
        "duplicate or invalid LUT size"
    );
    let n: usize = fields[1].parse()?;
    let is_3d = fields[0] == "LUT_3D_SIZE";
    ensure!(
        (2..=if is_3d { 65 } else { 65536 }).contains(&n),
        "unsupported LUT size"
    );
    Ok((n, is_3d))
}

fn blend_strength(
    result: &mut Frame,
    input: &Frame,
    strength: f32,
    context: &RenderContext,
) -> Result<()> {
    // Strength blending is in working linear light, not encoded LUT space.
    for (i, row) in result.pixels.chunks_mut(input.width as usize).enumerate() {
        context.check_cancelled()?;
        for (x, pixel) in row.iter_mut().enumerate() {
            let original = input.pixels[i * input.width as usize + x];
            for c in 0..3 {
                pixel[c] = original[c] * (1.0 - strength) + pixel[c] * strength;
            }
        }
    }
    Ok(())
}
