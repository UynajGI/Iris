// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit, locally supplied calibration; this is not a Lensfun adapter.
//! Coefficients use Geometry's inverse map on the oriented, uncropped source.
//! They are constant across the declared focal/aperture range, without fitting
//! or interpolation. Matching EXIF is evidence of applicability, not proof that
//! the source has never had optical correction applied by another application.
use crate::geometry::Geometry;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LensCalibrationModel {
    IrisInverseRadialV1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LensCoordinates {
    OrientedUncroppedImageEdgesHalfShortEdgeV1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LensCorrection {
    /// Inverse r -> r*(1+k1*r^2+k2*r^4), radius in half the short image edge.
    pub radial: [f32; 2],
    /// Additional red/blue scale relative to green, after the radial mapping.
    pub chromatic_aberration: [f32; 2],
    /// Exposure lift: 2^(stops * min(r^2,1)), evaluated at mapped input position.
    pub vignette_stops: f32,
}
impl LensCorrection {
    /// Merge only optical coefficients. Existing crop/rotation/perspective stay
    /// intact. The node must operate on the oriented, uncropped source extent.
    pub fn merge(&self, existing: &Geometry) -> Result<Geometry> {
        let mut geometry = existing.clone();
        geometry.radial = self.radial;
        geometry.chromatic_aberration = self.chromatic_aberration;
        geometry.vignette_stops = self.vignette_stops;
        geometry.validate()?;
        Ok(geometry)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LensProfile {
    pub schema_version: u32,
    pub name: String,
    pub model: LensCalibrationModel,
    pub coordinates: LensCoordinates,
    pub camera_make: String,
    pub camera_model: String,
    pub lens_model: String,
    /// Exact developed source extent; transposed dimensions permit EXIF rotation.
    pub calibrated_dimensions: [u32; 2],
    pub focal_mm: [f64; 2],
    pub f_number: [f64; 2],
    pub correction: LensCorrection,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct LensCaptureMetadata {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_mm: Option<f64>,
    pub f_number: Option<f64>,
    pub dimensions: [u32; 2],
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct LensProfileMatch {
    pub applicable: bool,
    /// Stable reason codes; no fuzzy matching or fallback to file names.
    pub issues: Vec<String>,
    /// Optical coefficients only: never replace existing geometry wholesale.
    pub correction: Option<LensCorrection>,
}
impl LensProfile {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 64 * 1024, "lens profile exceeds 64 KiB");
        let profile: Self = serde_json::from_slice(bytes)?;
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "unsupported lens profile version");
        for text in [
            &self.name,
            &self.camera_make,
            &self.camera_model,
            &self.lens_model,
        ] {
            ensure!(valid_text(text), "invalid lens profile identifier");
        }
        let [width, height] = self.calibrated_dimensions;
        ensure!(
            width > 0 && height > 0 && u64::from(width) * u64::from(height) <= 120_000_000,
            "invalid lens calibration dimensions"
        );
        validate_range(self.focal_mm, 10000.0)?;
        validate_range(self.f_number, 128.0)?;
        self.correction.merge(&Geometry::default())?;
        Ok(())
    }
    pub fn match_capture(&self, capture: &LensCaptureMetadata) -> Result<LensProfileMatch> {
        self.validate()?;
        let mut issues = Vec::new();
        match_text(
            &mut issues,
            "camera_make",
            &self.camera_make,
            capture.camera_make.as_deref(),
        );
        match_text(
            &mut issues,
            "camera_model",
            &self.camera_model,
            capture.camera_model.as_deref(),
        );
        match_text(
            &mut issues,
            "lens_model",
            &self.lens_model,
            capture.lens_model.as_deref(),
        );
        match_range(&mut issues, "focal_mm", self.focal_mm, capture.focal_mm);
        match_range(&mut issues, "f_number", self.f_number, capture.f_number);
        let [width, height] = self.calibrated_dimensions;
        if capture.dimensions != [width, height] && capture.dimensions != [height, width] {
            issues.push("calibrated_dimensions_mismatch".into());
        }
        Ok(LensProfileMatch {
            applicable: issues.is_empty(),
            correction: issues.is_empty().then(|| self.correction.clone()),
            issues,
        })
    }
}
fn valid_text(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= 128 && !text.chars().any(char::is_control)
}
fn validate_range(range: [f64; 2], maximum: f64) -> Result<()> {
    ensure!(
        range
            .iter()
            .all(|v| v.is_finite() && *v >= 0.1 && *v <= maximum)
            && range[0] <= range[1],
        "invalid lens calibration range"
    );
    Ok(())
}
fn match_text(issues: &mut Vec<String>, field: &str, expected: &str, actual: Option<&str>) {
    match actual.filter(|v| valid_text(v)) {
        None => issues.push(format!("{field}_missing")),
        Some(value) if !expected.trim().eq_ignore_ascii_case(value.trim()) => {
            issues.push(format!("{field}_mismatch"));
        }
        _ => {}
    }
}
fn match_range(issues: &mut Vec<String>, field: &str, range: [f64; 2], actual: Option<f64>) {
    match actual.filter(|v| v.is_finite() && *v > 0.0) {
        None => issues.push(format!("{field}_missing")),
        Some(value) if !(range[0]..=range[1]).contains(&value) => {
            issues.push(format!("{field}_outside_calibration"));
        }
        _ => {}
    }
}
