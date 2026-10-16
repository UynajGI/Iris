//! Iris linear adjustments v1, in straight-alpha linear Rec.2020.
//! These deliberately simple, documented creative operators are not camera white balance
//! or a reproduction of another editor. Rec.2020 Y uses ITU-R BT.2020 coefficients.
//! Tone selection uses clamped Y only for masks; RGB is never clipped to [0,1].
//! Curves preserve legacy linear interpolation or use monotone cubic interpolation.
//! Both extrapolate endpoint slopes without clipping negative or HDR input.
//! HSL is an artistic cylindrical transform of linear RGB, not perceptual HSL:
//! it operates on chromatic differences around Y and preserves negative/HDR offsets.
//! Grain is a seeded spatial hash, not a physical film emulsion simulation.
use crate::{Frame, RenderContext};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
mod curve;
use curve::PreparedCurve;
mod balance;
use balance::PreparedBalance;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurveInterpolation {
    #[default]
    Linear,
    MonotoneCubic,
}
impl CurveInterpolation {
    fn is_linear(&self) -> bool {
        *self == Self::Linear
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CurvePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Curve {
    /// Empty means identity. Otherwise 2..=64 points, strictly increasing x,
    /// nondecreasing y, all coordinates in [0,1]. Endpoints must have x=0 and x=1.
    pub points: Vec<CurvePoint>,
    /// Missing means legacy linear interpolation. Cubic preserves monotonicity.
    #[serde(skip_serializing_if = "CurveInterpolation::is_linear")]
    pub interpolation: CurveInterpolation,
}
impl Curve {
    pub fn validate(&self) -> Result<()> {
        if self.points.is_empty() {
            return Ok(());
        }
        ensure!(
            (2..=64).contains(&self.points.len()),
            "curve requires 2..64 points"
        );
        ensure!(
            self.points[0].x == 0.0 && self.points.last().unwrap().x == 1.0,
            "curve endpoints must span 0..1"
        );
        for p in &self.points {
            range(p.x, 0.0, 1.0, "curve x")?;
            range(p.y, 0.0, 1.0, "curve y")?;
        }
        for p in self.points.windows(2) {
            ensure!(
                p[1].x - p[0].x >= 0.0001 && p[1].y >= p[0].y,
                "curve must be monotone with x spacing >= 0.0001"
            );
        }
        Ok(())
    }
    #[cfg(test)]
    fn map(&self, x: f64) -> f64 {
        // Frozen legacy oracle for adjustments/reference.rs. Keep it independent
        // of the prepared evaluator so legacy pixel comparisons detect drift.
        assert_eq!(self.interpolation, CurveInterpolation::Linear);
        if self.points.is_empty() {
            return x;
        }
        let i = self
            .points
            .windows(2)
            .position(|p| x <= p[1].x as f64)
            .unwrap_or(self.points.len() - 2);
        let a = self.points[i];
        let b = self.points[i + 1];
        a.y as f64 + (x - a.x as f64) * (b.y - a.y) as f64 / (b.x - a.x) as f64
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ToneCurve {
    /// Log2 gains in four overlapping bands centred at Y=0, 1/3, 2/3, 1.
    pub shadows: f32,
    pub darks: f32,
    pub lights: f32,
    pub highlights: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HueBand {
    /// Centre in degrees [0,360]; wrap-around is supported.
    pub center: f32,
    /// Half-width in degrees (1..180), triangular selection weight.
    pub width: f32,
    pub hue_shift: f32,
    /// Relative chroma change [-1,1].
    pub saturation: f32,
    /// Additive linear luminance change [-1,1], scaled by chroma selection.
    pub luminance: f32,
}
impl Default for HueBand {
    fn default() -> Self {
        Self {
            center: 0.0,
            width: 45.0,
            hue_shift: 0.0,
            saturation: 0.0,
            luminance: 0.0,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ColorBalance {
    /// Per-channel log2 gains [-2,2], smoothly blended by linear Y.
    pub shadows: [f32; 3],
    pub midtones: [f32; 3],
    pub highlights: [f32; 3],
    /// Additional whole-image per-channel log2 gains [-2,2].
    #[serde(skip_serializing_if = "neutral_balance_gain")]
    pub global: [f32; 3],
    /// Range overlap [0,1]. Missing/0.5 retains the legacy triangular weighting.
    #[serde(skip_serializing_if = "default_balance_blending")]
    pub blending: f32,
    /// Range shift [-1,1]; positive extends highlights, negative extends shadows.
    #[serde(skip_serializing_if = "neutral_balance_shift")]
    pub balance: f32,
}
impl Default for ColorBalance {
    fn default() -> Self {
        Self {
            shadows: [0.0; 3],
            midtones: [0.0; 3],
            highlights: [0.0; 3],
            global: [0.0; 3],
            blending: 0.5,
            balance: 0.0,
        }
    }
}
fn neutral_balance_gain(value: &[f32; 3]) -> bool {
    *value == [0.0; 3]
}
fn default_balance_blending(value: &f32) -> bool {
    *value == 0.5
}
fn neutral_balance_shift(value: &f32) -> bool {
    *value == 0.0
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BlackAndWhite {
    pub enabled: bool,
    /// Linear RGB mix; coefficients in [-2,2], sum must be one.
    pub mix: [f32; 3],
}
impl Default for BlackAndWhite {
    fn default() -> Self {
        Self {
            enabled: false,
            mix: [0.2627, 0.6780, 0.0593],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Vignette {
    /// Edge exposure in stops [-4,4].
    pub amount: f32,
    /// Fraction of centre-to-corner radius at which falloff begins [0,0.99].
    pub midpoint: f32,
    /// Falloff exponent [0.1,4].
    pub feather: f32,
}
impl Default for Vignette {
    fn default() -> Self {
        Self {
            amount: 0.0,
            midpoint: 0.35,
            feather: 1.0,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Grain {
    /// Additive uniform noise amplitude in linear units [0,0.25].
    pub amount: f32,
    pub seed: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Adjustments {
    pub exposure: f32,
    /// Contrast exponent multiplier 2^contrast around pivot; signed power keeps negatives.
    pub contrast: f32,
    pub contrast_pivot: f32,
    /// Smooth masked log2 gains [-2,2], with separate broad/endpoint masks.
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    /// Creative RGB gains in stops [-1,1]; not Kelvin or RAW illuminant correction.
    pub temperature: f32,
    pub tint: f32,
    pub saturation: f32,
    pub vibrance: f32,
    pub master_curve: Curve,
    pub red_curve: Curve,
    pub green_curve: Curve,
    pub blue_curve: Curve,
    pub tone_curve: ToneCurve,
    pub hsl: Vec<HueBand>,
    pub color_balance: ColorBalance,
    pub black_and_white: BlackAndWhite,
    pub vignette: Vignette,
    pub grain: Grain,
}
impl Default for Adjustments {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            contrast_pivot: 0.18,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            temperature: 0.0,
            tint: 0.0,
            saturation: 0.0,
            vibrance: 0.0,
            master_curve: Curve::default(),
            red_curve: Curve::default(),
            green_curve: Curve::default(),
            blue_curve: Curve::default(),
            tone_curve: ToneCurve::default(),
            hsl: vec![],
            color_balance: ColorBalance::default(),
            black_and_white: BlackAndWhite::default(),
            vignette: Vignette::default(),
            grain: Grain::default(),
        }
    }
}
pub(crate) fn range(value: f32, min: f32, max: f32, name: &str) -> Result<()> {
    ensure!(
        value.is_finite() && value >= min && value <= max,
        "{name} must be finite and in [{min},{max}]"
    );
    Ok(())
}
impl Adjustments {
    pub fn validate(&self) -> Result<()> {
        range(self.exposure, -16.0, 16.0, "exposure")?;
        range(self.contrast, -2.0, 2.0, "contrast")?;
        range(self.contrast_pivot, 0.001, 1.0, "contrast pivot")?;
        for (n, v) in [
            ("highlights", self.highlights),
            ("shadows", self.shadows),
            ("whites", self.whites),
            ("blacks", self.blacks),
        ] {
            range(v, -2.0, 2.0, n)?;
        }
        for (n, v) in [
            ("temperature", self.temperature),
            ("tint", self.tint),
            ("saturation", self.saturation),
            ("vibrance", self.vibrance),
        ] {
            range(v, -1.0, 1.0, n)?;
        }
        self.validate_curves_and_color()?;
        for v in self.black_and_white.mix {
            range(v, -2.0, 2.0, "black and white mix")?;
        }
        ensure!(
            (self.black_and_white.mix.iter().sum::<f32>() - 1.0).abs() < 0.0001,
            "black and white mix must sum to one"
        );
        range(self.vignette.amount, -4.0, 4.0, "vignette")?;
        range(self.vignette.midpoint, 0.0, 0.99, "vignette midpoint")?;
        range(self.vignette.feather, 0.1, 4.0, "vignette feather")?;
        range(self.grain.amount, 0.0, 0.25, "grain")?;
        Ok(())
    }
    fn validate_curves_and_color(&self) -> Result<()> {
        for c in [
            &self.master_curve,
            &self.red_curve,
            &self.green_curve,
            &self.blue_curve,
        ] {
            c.validate()?;
        }
        for v in [
            self.tone_curve.shadows,
            self.tone_curve.darks,
            self.tone_curve.lights,
            self.tone_curve.highlights,
        ] {
            range(v, -2.0, 2.0, "tone curve")?;
        }
        self.validate_hue_and_balance()
    }
    fn validate_hue_and_balance(&self) -> Result<()> {
        ensure!(self.hsl.len() <= 24, "at most 24 hue bands");
        for h in &self.hsl {
            range(h.center, 0.0, 360.0, "hue centre")?;
            range(h.width, 1.0, 180.0, "hue width")?;
            range(h.hue_shift, -180.0, 180.0, "hue shift")?;
            range(h.saturation, -1.0, 1.0, "hue saturation")?;
            range(h.luminance, -1.0, 1.0, "hue luminance")?;
        }
        for a in [
            self.color_balance.shadows,
            self.color_balance.midtones,
            self.color_balance.highlights,
            self.color_balance.global,
        ] {
            for v in a {
                range(v, -2.0, 2.0, "color balance")?;
            }
        }
        range(
            self.color_balance.blending,
            0.0,
            1.0,
            "color grading blending",
        )?;
        range(
            self.color_balance.balance,
            -1.0,
            1.0,
            "color grading balance",
        )?;
        Ok(())
    }
}
pub(crate) fn luma(p: [f64; 3]) -> f64 {
    0.2627 * p[0] + 0.6780 * p[1] + 0.0593 * p[2]
}
fn smooth(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn hue(p: [f64; 3]) -> f64 {
    let hi = p[0].max(p[1]).max(p[2]);
    let lo = p[0].min(p[1]).min(p[2]);
    let d = hi - lo;
    if d < 1e-15 {
        return 0.0;
    }
    let h = if hi == p[0] {
        (p[1] - p[2]) / d
    } else if hi == p[1] {
        (p[2] - p[0]) / d + 2.0
    } else {
        (p[0] - p[1]) / d + 4.0
    };
    (h * 60.0).rem_euclid(360.0)
}
fn hue_rgb(h: f64) -> [f64; 3] {
    let t = h.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (t.rem_euclid(2.0) - 1.0).abs();
    match t as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}
fn noise(x: u32, y: u32, seed: u32) -> f64 {
    let mut h = x.wrapping_mul(0x9e3779b9) ^ y.wrapping_mul(0x85ebca6b) ^ seed;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846ca68b);
    h ^= h >> 16;
    h as f64 / u32::MAX as f64 * 2.0 - 1.0
}

/// Pipeline order: exposure/creative balance, contrast, tonal masks, curves, chroma,
/// hue bands, three-way balance, B&W, vignette, grain. Arithmetic uses f64 and
/// returns an error instead of silently clipping an unrepresentable f32 result.
/// Fully transparent pixels keep hidden RGB unchanged; other alpha values are copied.
pub fn apply(input: &Frame, params: &Adjustments, context: &RenderContext) -> Result<Frame> {
    params.validate()?;
    input.validate()?;
    context.check_cancelled()?;
    let _memory = context.reserve(input.byte_len())?;
    if *params == Adjustments::default() {
        return Ok(input.clone());
    }
    let mut pixels = input.pixels.clone();
    let pool = crate::parallel::pool()?;
    if *params
        == (Adjustments {
            exposure: params.exposure,
            ..Default::default()
        })
    {
        exposure_pixels(&mut pixels, params.exposure, context, pool)?;
    } else {
        full_pixels(&mut pixels, input, params, context, pool)?;
    }
    context.check_cancelled()?;
    Frame::new(input.width, input.height, pixels)
}

fn full_pixels(
    pixels: &mut [[f32; 4]],
    input: &Frame,
    params: &Adjustments,
    context: &RenderContext,
    pool: &rayon::ThreadPool,
) -> Result<()> {
    use rayon::prelude::*;
    let prepared = Prepared::new(params);
    pool.install(|| {
        pixels
            .par_chunks_mut(input.width as usize)
            .enumerate()
            .try_for_each(|(y, row)| -> Result<()> {
                context.check_cancelled()?;
                for (x, pixel) in row.iter_mut().enumerate() {
                    if x % 1024 == 0 {
                        context.check_cancelled()?;
                    }
                    *pixel =
                        prepared.pixel(*pixel, x as u32, y as u32, input.width, input.height)?;
                }
                Ok(())
            })
    })
}

fn exposure_pixels(
    pixels: &mut [[f32; 4]],
    exposure: f32,
    context: &RenderContext,
    pool: &rayon::ThreadPool,
) -> Result<()> {
    use rayon::prelude::*;
    let gain = (exposure as f64).exp2();
    pool.install(|| {
        pixels
            .par_chunks_mut(4096)
            .try_for_each(|chunk| -> Result<()> {
                context.check_cancelled()?;
                for pixel in chunk {
                    if pixel[3] == 0.0 {
                        continue;
                    }
                    for value in &mut pixel[..3] {
                        let adjusted = *value as f64 * gain + 0.0;
                        ensure!(
                            adjusted.is_finite() && adjusted.abs() <= f32::MAX as f64,
                            "adjustment exceeds finite f32 range"
                        );
                        *value = adjusted as f32;
                    }
                }
                Ok(())
            })
    })
}

/// Constants and effect switches are computed once per request. Disabled effects
/// skip their nonlinear math; operator order and full-image coordinates are fixed.
struct Prepared<'a> {
    p: &'a Adjustments,
    gains: [f64; 3],
    contrast_power: f64,
    tone: bool,
    curves: bool,
    prepared_curves: [PreparedCurve<'a>; 4],
    chroma: bool,
    prepared_balance: PreparedBalance<'a>,
    tone_stops: [f64; 4],
}
impl<'a> Prepared<'a> {
    fn new(p: &'a Adjustments) -> Self {
        Self {
            p,
            gains: [
                (p.exposure as f64 + p.temperature as f64).exp2(),
                (p.exposure as f64 - p.tint as f64).exp2(),
                (p.exposure as f64 - p.temperature as f64).exp2(),
            ],
            contrast_power: (p.contrast as f64).exp2(),
            tone: p.shadows != 0.0
                || p.highlights != 0.0
                || p.blacks != 0.0
                || p.whites != 0.0
                || p.tone_curve != ToneCurve::default(),
            curves: [&p.master_curve, &p.red_curve, &p.green_curve, &p.blue_curve]
                .iter()
                .any(|c| !c.points.is_empty()),
            prepared_curves: [&p.master_curve, &p.red_curve, &p.green_curve, &p.blue_curve]
                .map(PreparedCurve::new),
            chroma: p.saturation != 0.0 || p.vibrance != 0.0,
            prepared_balance: PreparedBalance::new(&p.color_balance),
            tone_stops: [
                p.tone_curve.shadows as f64,
                p.tone_curve.darks as f64,
                p.tone_curve.lights as f64,
                p.tone_curve.highlights as f64,
            ],
        }
    }
    fn tone_and_curves(&self, mut p: [f64; 3]) -> [f64; 3] {
        let params = self.p;
        if self.tone {
            let t = luma(p).clamp(0.0, 1.0);
            let s = 1.0 - smooth(t * 2.0);
            let h = smooth((t - 0.5) * 2.0);
            let v = params.shadows as f64 * s
                + params.highlights as f64 * h
                + params.blacks as f64 * (1.0 - smooth(t * 5.0))
                + params.whites as f64 * smooth((t - 0.8) * 5.0);
            let q = t * 3.0;
            let j = (q.floor() as usize).min(2);
            let k = q - j as f64;
            let gain = (v + self.tone_stops[j] * (1.0 - k) + self.tone_stops[j + 1] * k).exp2();
            for v in &mut p {
                *v *= gain;
            }
        }
        if self.curves {
            for c in 0..3 {
                p[c] = self.prepared_curves[c + 1].map(self.prepared_curves[0].map(p[c]));
            }
        }
        p
    }
    fn chroma(&self, mut p: [f64; 3]) -> [f64; 3] {
        let params = self.p;
        if self.chroma {
            let lum = luma(p);
            let chroma = p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]);
            let sat = chroma / (lum.abs() + chroma + 1e-12);
            let boost =
                (1.0 + params.saturation as f64) * (1.0 + params.vibrance as f64 * (1.0 - sat));
            if boost != 1.0 {
                for v in &mut p {
                    *v = lum + (*v - lum) * boost;
                }
            }
        }
        p
    }
    fn hue_bands(&self, mut p: [f64; 3]) -> [f64; 3] {
        let params = self.p;
        if !params.hsl.is_empty() {
            let lum = luma(p);
            let chroma = p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]);
            if chroma > 1e-15 {
                let original_hue = hue(p);
                let mut dh = 0.0;
                let mut ds = 0.0;
                let mut dl = 0.0;
                for b in &params.hsl {
                    let distance =
                        ((original_hue - b.center as f64 + 180.0).rem_euclid(360.0) - 180.0).abs();
                    let weight = (1.0 - distance / b.width as f64).max(0.0);
                    dh += weight * b.hue_shift as f64;
                    ds += weight * b.saturation as f64;
                    dl += weight * b.luminance as f64;
                }
                if dh != 0.0 || ds != 0.0 || dl != 0.0 {
                    let rgb = hue_rgb(original_hue + dh);
                    let base = luma(rgb);
                    for c in 0..3 {
                        p[c] = lum + dl * chroma + (rgb[c] - base) * chroma * (1.0 + ds).max(0.0);
                    }
                }
            }
        }
        p
    }
    fn balance(&self, mut p: [f64; 3]) -> [f64; 3] {
        let params = self.p;
        p = self.prepared_balance.apply_ranges(p);
        if params.black_and_white.enabled {
            let gray = (0..3)
                .map(|c| p[c] * params.black_and_white.mix[c] as f64)
                .sum::<f64>();
            p = [gray; 3];
        }
        self.prepared_balance.apply_global(p)
    }
    fn pixel(&self, src: [f32; 4], x: u32, y: u32, width: u32, height: u32) -> Result<[f32; 4]> {
        if src[3] == 0.0 {
            return Ok(src);
        }
        let params = self.p;
        let mut p = [src[0] as f64, src[1] as f64, src[2] as f64];
        for c in 0..3 {
            p[c] *= self.gains[c];
            if params.contrast != 0.0 {
                let pivot = params.contrast_pivot as f64;
                p[c] = p[c].signum() * pivot * (p[c].abs() / pivot).powf(self.contrast_power);
            }
        }
        p = self.tone_and_curves(p);
        p = self.chroma(p);
        p = self.hue_bands(p);
        p = self.balance(p);
        let vignette = if params.vignette.amount != 0.0 {
            let nx = (2.0 * (x as f64 + 0.5) / width as f64 - 1.0).abs();
            let ny = (2.0 * (y as f64 + 0.5) / height as f64 - 1.0).abs();
            let r = (nx * nx + ny * ny).sqrt() / 2.0f64.sqrt();
            let v = smooth(
                (r - params.vignette.midpoint as f64) / (1.0 - params.vignette.midpoint as f64),
            )
            .powf(params.vignette.feather as f64);
            (params.vignette.amount as f64 * v).exp2()
        } else {
            1.0
        };
        let grain = if params.grain.amount != 0.0 {
            noise(x, y, params.grain.seed) * params.grain.amount as f64
        } else {
            0.0
        };
        let mut out = src;
        for c in 0..3 {
            let value = p[c] * vignette + grain;
            ensure!(
                value.is_finite() && value.abs() <= f32::MAX as f64,
                "adjustment exceeds finite f32 range"
            );
            out[c] = value as f32;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod reference;
#[cfg(test)]
mod optimized_tests {
    use super::*;
    #[test]
    fn prepared_parallel_pipeline_matches_frozen_reference() {
        let input = Frame::new(
            41,
            19,
            (0..779)
                .map(|i| {
                    let t = (i % 43) as f32 / 42.0;
                    [
                        t * 1.3 - 0.1,
                        0.3 + t * 0.2,
                        0.7 - t * 0.3,
                        if i % 13 == 0 { 0.0 } else { 0.7 },
                    ]
                })
                .collect(),
        )
        .unwrap();
        let mut all = Adjustments {
            exposure: 0.73,
            contrast: -0.1,
            shadows: 0.2,
            highlights: -0.1,
            whites: 0.1,
            blacks: 0.05,
            temperature: 0.1,
            tint: -0.2,
            saturation: 0.2,
            vibrance: -0.15,
            ..Default::default()
        };
        all.hsl.push(HueBand {
            center: 15.0,
            hue_shift: 5.0,
            saturation: 0.1,
            luminance: 0.03,
            ..Default::default()
        });
        all.tone_curve.darks = 0.1;
        all.color_balance.midtones = [0.01, -0.02, 0.03];
        all.vignette.amount = -0.2;
        all.grain.amount = 0.01;
        all.grain.seed = 19;
        all.master_curve = Curve {
            points: vec![
                CurvePoint { x: 0.0, y: 0.0 },
                CurvePoint { x: 0.5, y: 0.45 },
                CurvePoint { x: 1.0, y: 1.0 },
            ],
            ..Default::default()
        };
        for p in [
            Adjustments {
                exposure: 1.137,
                ..Default::default()
            },
            Adjustments {
                contrast_pivot: 0.3,
                ..Default::default()
            },
            all,
        ] {
            let a = apply(&input, &p, &RenderContext::default()).unwrap();
            let b = reference::apply(&input, &p, &RenderContext::default()).unwrap();
            for (a, b) in a.pixels.iter().zip(b.pixels) {
                for c in 0..4 {
                    assert!(
                        (a[c] - b[c]).abs() <= b[c].abs().max(1.0) * 2.0 * f32::EPSILON,
                        "optimized {} reference {}",
                        a[c],
                        b[c]
                    );
                }
            }
        }
    }
}
