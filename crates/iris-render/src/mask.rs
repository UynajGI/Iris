// SPDX-License-Identifier: GPL-3.0-or-later
//! Traditional masks. Coordinates are normalized image-edge coordinates; pixel
//! centers are `(x+0.5)/width`. Brush radii use the shorter image dimension.
//! Recipes are evaluated per pixel (no intermediate full-frame allocations).
use crate::{Frame, RenderContext};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BrushStroke {
    /// Polyline in normalized image coordinates, connected by continuous capsules.
    pub points: Vec<[f32; 2]>,
    pub radius: f32,
    pub hardness: f32,
    pub flow: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MaskOperation {
    Add,
    Subtract,
    Intersect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MaskDefinition {
    Brush {
        strokes: Vec<BrushStroke>,
    },
    LinearGradient {
        start: [f32; 2],
        end: [f32; 2],
    },
    /// Elliptical falloff: full weight within hardness, zero at radii.
    RadialGradient {
        center: [f32; 2],
        radii: [f32; 2],
        hardness: f32,
    },
    /// Linear Rec.2020 luminance; feather is in linear luminance units.
    LuminanceRange {
        low: f32,
        high: f32,
        feather: f32,
    },
    /// Euclidean distance in linear Rec.2020 RGB, not perceptual Delta E.
    ColorRange {
        color: [f32; 3],
        radius: f32,
        feather: f32,
    },
    Invert {
        #[schema(no_recursion)]
        mask: Box<MaskDefinition>,
    },
    Combine {
        operation: MaskOperation,
        #[schema(no_recursion)]
        left: Box<MaskDefinition>,
        #[schema(no_recursion)]
        right: Box<MaskDefinition>,
    },
    Opacity {
        #[schema(no_recursion)]
        mask: Box<MaskDefinition>,
        opacity: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    pub values: Vec<f32>,
}

fn unit(v: f32) -> bool {
    v.is_finite() && (0.0..=1.0).contains(&v)
}
fn point(p: &[f32; 2]) -> bool {
    p.iter().all(|v| unit(*v))
}
fn positive(v: f32) -> bool {
    v.is_finite() && v > 0.0 && v <= 16.0
}
impl MaskDefinition {
    fn work_units(&self) -> u64 {
        match self {
            Self::Brush { strokes } => strokes
                .iter()
                .map(|s| s.points.len() as u64)
                .sum::<u64>()
                .max(1),
            Self::Invert { mask } | Self::Opacity { mask, .. } => 1 + mask.work_units(),
            Self::Combine { left, right, .. } => 1 + left.work_units() + right.work_units(),
            _ => 1,
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.validate_inner(0, &mut 0, &mut 0)
    }
    fn validate_inner(&self, depth: usize, nodes: &mut usize, points: &mut usize) -> Result<()> {
        *nodes += 1;
        ensure!(
            depth <= 16 && *nodes <= 256,
            "mask recipe exceeds nesting/node budget"
        );
        match self {
            Self::Brush { strokes } => validate_brush(strokes, points),
            Self::LinearGradient { start, end } => validate_linear(start, end),
            Self::RadialGradient {
                center,
                radii,
                hardness,
            } => validate_radial(center, radii, *hardness),
            Self::LuminanceRange { low, high, feather } => {
                validate_luminance(*low, *high, *feather)
            }
            Self::ColorRange {
                color,
                radius,
                feather,
            } => validate_color(color, *radius, *feather),
            Self::Invert { mask } => mask.validate_inner(depth + 1, nodes, points),
            Self::Opacity { mask, opacity } => {
                ensure!(unit(*opacity), "invalid mask opacity");
                mask.validate_inner(depth + 1, nodes, points)
            }
            Self::Combine { left, right, .. } => {
                left.validate_inner(depth + 1, nodes, points)?;
                right.validate_inner(depth + 1, nodes, points)
            }
        }
    }
}

fn validate_brush(strokes: &[BrushStroke], points: &mut usize) -> Result<()> {
    ensure!(strokes.len() <= 1024, "too many brush strokes");
    for s in strokes {
        *points += s.points.len();
        ensure!(
            *points <= 4096 && !s.points.is_empty(),
            "brush point budget exceeded or empty stroke"
        );
        ensure!(
            s.points.iter().all(point) && positive(s.radius) && unit(s.hardness) && unit(s.flow),
            "invalid brush stroke"
        );
    }
    Ok(())
}
fn validate_linear(start: &[f32; 2], end: &[f32; 2]) -> Result<()> {
    ensure!(
        point(start) && point(end) && start != end,
        "invalid linear gradient"
    );
    Ok(())
}
fn validate_radial(center: &[f32; 2], radii: &[f32; 2], hardness: f32) -> Result<()> {
    ensure!(
        point(center) && radii.iter().all(|v| positive(*v)) && unit(hardness),
        "invalid radial gradient"
    );
    Ok(())
}
fn validate_luminance(low: f32, high: f32, feather: f32) -> Result<()> {
    ensure!(
        low.is_finite()
            && high.is_finite()
            && low <= high
            && low.abs() <= 65536.0
            && high.abs() <= 65536.0
            && feather.is_finite()
            && (0.0..=65536.0).contains(&feather),
        "invalid luminance range"
    );
    Ok(())
}
fn validate_color(color: &[f32; 3], radius: f32, feather: f32) -> Result<()> {
    ensure!(
        color.iter().all(|v| v.is_finite() && v.abs() <= 65536.0)
            && radius.is_finite()
            && (0.0..=65536.0).contains(&radius)
            && feather.is_finite()
            && (0.0..=65536.0).contains(&feather),
        "invalid color range"
    );
    Ok(())
}

fn falloff(distance: f64, inner: f64, outer: f64) -> f32 {
    if distance <= inner {
        return 1.0;
    }
    if distance >= outer || outer <= inner {
        return 0.0;
    }
    let t = ((distance - inner) / (outer - inner)) as f32;
    1.0 - t * t * (3.0 - 2.0 * t)
}

fn stroke_weight(s: &BrushStroke, p: [f64; 2], scale: [f64; 2]) -> f32 {
    let distance = |a: [f32; 2], b: [f32; 2]| {
        let a = [a[0] as f64 * scale[0], a[1] as f64 * scale[1]];
        let b = [b[0] as f64 * scale[0], b[1] as f64 * scale[1]];
        let d = [b[0] - a[0], b[1] - a[1]];
        let len = d[0] * d[0] + d[1] * d[1];
        let t = if len > 0.0 {
            (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
    };
    let d = if s.points.len() == 1 {
        distance(s.points[0], s.points[0])
    } else {
        s.points
            .windows(2)
            .map(|w| distance(w[0], w[1]))
            .fold(f64::INFINITY, f64::min)
    };
    falloff(d, (s.radius * s.hardness) as f64, s.radius as f64) * s.flow
}

fn evaluate(m: &MaskDefinition, p: [f64; 2], rgb: [f32; 4], scale: [f64; 2]) -> f32 {
    match m {
        MaskDefinition::Brush { strokes } => strokes.iter().fold(0.0, |a, s| {
            let b = stroke_weight(s, [p[0] * scale[0], p[1] * scale[1]], scale);
            a + b - a * b
        }),
        MaskDefinition::LinearGradient { start, end } => {
            let d = [
                (end[0] - start[0]) as f64 * scale[0],
                (end[1] - start[1]) as f64 * scale[1],
            ];
            (((p[0] - start[0] as f64) * scale[0] * d[0]
                + (p[1] - start[1] as f64) * scale[1] * d[1])
                / (d[0] * d[0] + d[1] * d[1]))
                .clamp(0.0, 1.0) as f32
        }
        MaskDefinition::RadialGradient {
            center,
            radii,
            hardness,
        } => falloff(
            ((p[0] - center[0] as f64) / radii[0] as f64)
                .hypot((p[1] - center[1] as f64) / radii[1] as f64),
            *hardness as f64,
            1.0,
        ),
        MaskDefinition::LuminanceRange { low, high, feather } => {
            let l = 0.2627 * rgb[0] as f64 + 0.6780 * rgb[1] as f64 + 0.0593 * rgb[2] as f64;
            let d = (*low as f64 - l).max(l - *high as f64).max(0.0);
            falloff(d, 0.0, *feather as f64)
        }
        MaskDefinition::ColorRange {
            color,
            radius,
            feather,
        } => {
            let d = (0..3)
                .map(|i| (rgb[i] as f64 - color[i] as f64).powi(2))
                .sum::<f64>()
                .sqrt();
            falloff(d, *radius as f64, *radius as f64 + *feather as f64)
        }
        MaskDefinition::Invert { mask } => 1.0 - evaluate(mask, p, rgb, scale),
        MaskDefinition::Opacity { mask, opacity } => evaluate(mask, p, rgb, scale) * opacity,
        MaskDefinition::Combine {
            operation,
            left,
            right,
        } => {
            let a = evaluate(left, p, rgb, scale);
            let b = evaluate(right, p, rgb, scale);
            match operation {
                MaskOperation::Add => (a + b).min(1.0),
                MaskOperation::Subtract => (a - b).max(0.0),
                MaskOperation::Intersect => a.min(b),
            }
        }
    }
}

pub fn render(input: &Frame, definition: &MaskDefinition, context: &RenderContext) -> Result<Mask> {
    input.validate()?;
    definition.validate()?;
    context.check_cancelled()?;
    ensure!(
        (input.pixels.len() as u64).saturating_mul(definition.work_units()) <= 2_000_000_000,
        "mask exceeds evaluation work budget"
    );
    let _memory = context.reserve(input.pixels.len() as u64 * 4)?;
    ensure!(
        input.pixels.len() <= 120_000_000,
        "mask exceeds pixel budget"
    );
    let mut values = Vec::new();
    values.try_reserve_exact(input.pixels.len())?;
    let short = input.width.min(input.height).max(1) as f64;
    let scale = [input.width as f64 / short, input.height as f64 / short];
    for (i, pixel) in input.pixels.iter().enumerate() {
        if i % 1024 == 0 {
            context.check_cancelled()?;
        }
        let p = [
            (i as u32 % input.width) as f64 + 0.5,
            (i as u32 / input.width) as f64 + 0.5,
        ];
        values.push(
            evaluate(
                definition,
                [p[0] / input.width as f64, p[1] / input.height as f64],
                *pixel,
                scale,
            )
            .clamp(0.0, 1.0),
        );
    }
    Ok(Mask {
        width: input.width,
        height: input.height,
        values,
    })
}
