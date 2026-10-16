// SPDX-License-Identifier: GPL-3.0-or-later
//! Sensor white balance from a single D65 camera matrix. No DCP dual-illuminant
//! interpolation, profile look tables or creative RGB temperature are implied.
//! Daylight equations: https://docs.acescentral.com/white-point/ (CIE 15:2004).
//! Planckian approximation: Kang et al., JKPS 41(6), 865-871 (2002), cross-checked
//! against Colour's CCT_to_xy_Kang2002; see white_balance/COLOUR-LICENSE.txt.
//! Tint is an explicit additive CIE 1960 v coordinate offset, not vendor tint or
//! a perpendicular Duv distance: u=4x/(-2x+12y+3), v=6y/(-2x+12y+3).
//! Definitions: https://pmc.ncbi.nlm.nih.gov/articles/PMC6753040/ (NBS, 1976).
//! Coefficient reference: https://colour.readthedocs.io/en/develop/_modules/colour/temperature/kang2002.html
use crate::{region::PixelRect, Frame, RenderContext};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RawCalibration {
    pub version: u32,
    pub xyz_to_camera_d65: [[f64; 3]; 3],
    /// Maps WB-multiplied camera RGB to linear Rec.2020; D65 neutral row normalized.
    pub camera_to_working: [[f64; 3]; 3],
    pub as_shot_gains: Option<[f32; 3]>,
    pub applied_gains: [f32; 3],
}

impl RawCalibration {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "unsupported RAW calibration version");
        inverse(self.xyz_to_camera_d65)?;
        inverse(self.camera_to_working)?;
        validate_gains(self.applied_gains)?;
        if let Some(gains) = self.as_shot_gains {
            validate_gains(gains)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WhiteBalanceLocus {
    /// CIE daylight locus, 4000..=25000 K, not a blackbody approximation.
    Daylight,
    /// Kang (2002) Planckian approximation, 1667..=25000 K.
    Planckian,
}

/// Coordinates in Iris's illuminant model, not vendor-specific temperature/tint.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct WhiteBalanceTemperature {
    pub kelvin: f64,
    pub tint_delta_v: f64,
    pub locus: WhiteBalanceLocus,
}

/// Recover editable coordinates from sensor gains. An illuminant outside the
/// supported locus/tint domain has no reading; never substitute a default white.
pub fn temperature_from_gains(
    calibration: &RawCalibration,
    gains: [f32; 3],
    locus: WhiteBalanceLocus,
) -> Result<Option<WhiteBalanceTemperature>> {
    calibration.validate()?;
    validate_gains(gains)?;
    let xyz = product(
        inverse(calibration.xyz_to_camera_d65)?,
        gains.map(|gain| 1.0 / gain as f64),
    );
    let sum: f64 = xyz.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        return Ok(None);
    }
    let xy = [xyz[0] / sum, xyz[1] / sum];
    if validate_xy(xy).is_err() {
        return Ok(None);
    }
    let target = xy_to_uv(xy);
    // Polynomial pieces have tiny discontinuities. Solve each continuous piece
    // independently, then prefer the valid solution nearest its locus.
    let pieces: &[(f64, f64)] = match locus {
        WhiteBalanceLocus::Daylight => &[(4000.0, 7000.0), (7000.000001, 25000.0)],
        WhiteBalanceLocus::Planckian => &[
            (1667.0, 2222.0),
            (2222.000001, 4000.0),
            (4000.000001, 25000.0),
        ],
    };
    let mut best: Option<WhiteBalanceTemperature> = None;
    for &(low, high) in pieces {
        if let Some(reading) = solve_temperature(target, locus, low, high)? {
            if best.is_none_or(|old| reading.tint_delta_v.abs() < old.tint_delta_v.abs()) {
                best = Some(reading);
            }
        }
    }
    Ok(best)
}

fn xy_to_uv([x, y]: [f64; 2]) -> [f64; 2] {
    let denominator = -2.0 * x + 12.0 * y + 3.0;
    [4.0 * x / denominator, 6.0 * y / denominator]
}

fn solve_temperature(
    target: [f64; 2],
    locus: WhiteBalanceLocus,
    mut low: f64,
    mut high: f64,
) -> Result<Option<WhiteBalanceTemperature>> {
    let uv = |kelvin| illuminant_xy(kelvin, 0.0, locus).map(xy_to_uv);
    // Allow only f32 gain roundoff at endpoints, not out-of-range clamping.
    if target[0] > uv(low)?[0] + 1e-8 || target[0] < uv(high)?[0] - 1e-8 {
        return Ok(None);
    }
    for _ in 0..50 {
        let middle = (low + high) * 0.5;
        if uv(middle)?[0] > target[0] {
            low = middle;
        } else {
            high = middle;
        }
    }
    let kelvin = (low + high) * 0.5;
    let tint = target[1] - uv(kelvin)?[1];
    if tint.abs() > 0.02 + 1e-8 {
        return Ok(None);
    }
    Ok(Some(WhiteBalanceTemperature {
        kelvin,
        tint_delta_v: tint.clamp(-0.02, 0.02),
        locus,
    }))
}

pub fn validate_gains(gains: [f32; 3]) -> Result<()> {
    ensure!(
        gains
            .iter()
            .all(|v| v.is_finite() && *v > 0.0 && *v <= 1024.0),
        "invalid sensor white balance gains"
    );
    ensure!(
        (gains[1] - 1.0).abs() < 1e-5,
        "sensor white balance must be green normalized"
    );
    Ok(())
}

fn neutral_gains(camera: [f64; 3]) -> Result<[f32; 3]> {
    ensure!(
        camera.iter().all(|v| v.is_finite() && *v > 1e-12),
        "illuminant has nonpositive camera response"
    );
    let gains = camera.map(|value| (camera[1] / value) as f32);
    validate_gains(gains)?;
    Ok(gains)
}

pub fn illuminant_xy(kelvin: f64, tint_delta_v: f64, locus: WhiteBalanceLocus) -> Result<[f64; 2]> {
    ensure!(
        tint_delta_v.is_finite() && tint_delta_v.abs() <= 0.02,
        "tint delta v must be in [-0.02,0.02]"
    );
    let lower = if locus == WhiteBalanceLocus::Daylight {
        4000.0
    } else {
        1667.0
    };
    ensure!(
        kelvin.is_finite() && (lower..=25000.0).contains(&kelvin),
        "temperature outside selected locus range"
    );
    let [x, y] = match locus {
        WhiteBalanceLocus::Daylight => daylight_xy(kelvin),
        WhiteBalanceLocus::Planckian => planckian_xy(kelvin),
    };
    let denominator = -2.0 * x + 12.0 * y + 3.0;
    let u = 4.0 * x / denominator;
    let v = 6.0 * y / denominator + tint_delta_v;
    let denominator = 2.0 * u - 8.0 * v + 4.0;
    let xy = [3.0 * u / denominator, 2.0 * v / denominator];
    validate_xy(xy)?;
    Ok(xy)
}

fn validate_xy([x, y]: [f64; 2]) -> Result<()> {
    ensure!(
        x.is_finite() && y.is_finite() && x > 0.0 && y > 0.0 && x + y < 1.0,
        "invalid illuminant chromaticity"
    );
    Ok(())
}

fn daylight_xy(t: f64) -> [f64; 2] {
    let r = 1.0 / t;
    let x = if t <= 7000.0 {
        ((-4.6070e9 * r + 2.9678e6) * r + 99.11) * r + 0.244063
    } else {
        ((-2.0064e9 * r + 1.9018e6) * r + 247.48) * r + 0.237040
    };
    [x, (-3.0 * x + 2.870) * x - 0.275]
}

// Polynomial coefficients cross-checked with Colour, Copyright 2013 Colour
// Developers (BSD-3-Clause). Full redistribution notice accompanies this module.
fn planckian_xy(t: f64) -> [f64; 2] {
    let r = 1.0 / t;
    let x = if t <= 4000.0 {
        ((-0.2661239e9 * r - 0.2343589e6) * r + 877.6956) * r + 0.179910
    } else {
        ((-3.0258469e9 * r + 2.1070379e6) * r + 222.6347) * r + 0.24039
    };
    let c = if t <= 2222.0 {
        [-1.1063814, -1.34811020, 2.18555832, -0.20219683]
    } else if t <= 4000.0 {
        [-0.9549476, -1.37418593, 2.09137015, -0.16748867]
    } else {
        [3.0817580, -5.8733867, 3.75112997, -0.37001483]
    };
    [x, ((c[0] * x + c[1]) * x + c[2]) * x + c[3]]
}

pub fn temperature_gains(
    calibration: &RawCalibration,
    kelvin: f64,
    tint_delta_v: f64,
    locus: WhiteBalanceLocus,
) -> Result<[f32; 3]> {
    calibration.validate()?;
    gains_for_xy(calibration, illuminant_xy(kelvin, tint_delta_v, locus)?)
}

pub fn gains_for_xy(calibration: &RawCalibration, xy: [f64; 2]) -> Result<[f32; 3]> {
    calibration.validate()?;
    validate_xy(xy)?;
    let [x, y] = xy;
    neutral_gains(product(
        calibration.xyz_to_camera_d65,
        [x / y, 1.0, (1.0 - x - y) / y],
    ))
}

/// Source coordinates are orientation-normalized RAW default-crop pixels,
/// before edit geometry, masks, LUTs or display mapping. All selected pixels must
/// be opaque and have recovered camera RGB in (1e-6,0.98); otherwise reject the
/// patch. Demosaic samples are used, not independent original CFA photosites.
pub fn neutral_patch_gains(
    frame: &Frame,
    calibration: &RawCalibration,
    rect: PixelRect,
    context: &RenderContext,
) -> Result<[f32; 3]> {
    calibration.validate()?;
    frame.validate()?;
    rect.validate_bounds(frame.width, frame.height)?;
    ensure!(
        rect.width <= 64 && rect.height <= 64,
        "neutral patch maximum is 64x64"
    );
    let inverse = inverse(calibration.camera_to_working)?;
    let mut sum = [0.0; 3];
    for y in rect.y..rect.y + rect.height {
        context.check_cancelled()?;
        for x in rect.x..rect.x + rect.width {
            let p = frame.pixels[y as usize * frame.width as usize + x as usize];
            let camera = unsaturated_camera(p, inverse, calibration.applied_gains)?;
            for c in 0..3 {
                sum[c] += camera[c];
            }
        }
    }
    neutral_gains(sum)
}

fn unsaturated_camera(
    pixel: [f32; 4],
    inverse: [[f64; 3]; 3],
    gains: [f32; 3],
) -> Result<[f64; 3]> {
    ensure!(pixel[3] == 1.0, "neutral patch must be opaque");
    let camera = product(inverse, [pixel[0] as f64, pixel[1] as f64, pixel[2] as f64]);
    let camera = std::array::from_fn(|c| camera[c] / gains[c] as f64);
    ensure!(
        camera
            .iter()
            .all(|v| v.is_finite() && *v > 1e-6 && *v < 0.98),
        "neutral patch contains dark, negative or saturated camera samples"
    );
    Ok(camera)
}

fn product(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    matrix.map(|row| row[0] * vector[0] + row[1] * vector[1] + row[2] * vector[2])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn inverse(m: [[f64; 3]; 3]) -> Result<[[f64; 3]; 3]> {
    ensure!(
        m.iter().flatten().all(|v| v.is_finite() && v.abs() <= 1e6),
        "invalid camera matrix"
    );
    let columns = [cross(m[1], m[2]), cross(m[2], m[0]), cross(m[0], m[1])];
    let det = m[0][0] * columns[0][0] + m[0][1] * columns[0][1] + m[0][2] * columns[0][2];
    ensure!(
        det.is_finite() && det.abs() > 1e-10,
        "singular camera matrix"
    );
    let result: [[f64; 3]; 3] =
        std::array::from_fn(|r| std::array::from_fn(|c| columns[c][r] / det));
    ensure!(
        result
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() <= 1e6),
        "unstable camera matrix inverse"
    );
    Ok(result)
}
