// SPDX-License-Identifier: GPL-3.0-or-later
//! ICC transforms use LittleCMS; chromaticities follow the ICC RGB registry:
//! https://registry.color.org/rgb-registry/bt2020 . No tone map is implicit.
use crate::{Frame, RenderContext};
use anyhow::{ensure, Result};
use lcms2::{CIExyY, CIExyYTRIPLE, Intent, PixelFormat, Profile, ToneCurve, Transform};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Scene,
    #[default]
    Display,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputSpace {
    #[default]
    Srgb,
    DisplayP3,
    AdobeRgb,
    LinearRec2020,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToneMap {
    /// Appropriate for already-rendered inputs and intentionally clipped output.
    #[default]
    None,
    /// Identity up to knee, then exponential shoulder approaching display white.
    Shoulder { knee: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct OutputConfig {
    pub space: OutputSpace,
    pub tone_map: ToneMap,
    /// Neutral-axis gamut compression in the target's linear RGB primaries.
    pub gamut_compression: bool,
    /// Opaque JPEG background, specified as encoded sRGB values.
    pub background: [f32; 3],
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            space: OutputSpace::Srgb,
            tone_map: ToneMap::None,
            gamut_compression: true,
            background: [1.0; 3],
        }
    }
}

impl OutputConfig {
    pub fn validate(&self) -> Result<()> {
        if let ToneMap::Shoulder { knee } = self.tone_map {
            ensure!(
                knee.is_finite() && (0.0..1.0).contains(&knee),
                "invalid tone shoulder knee"
            );
        }
        ensure!(
            self.background
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "invalid output background"
        );
        Ok(())
    }
}

pub fn luminance(rgb: [f32; 3]) -> f32 {
    rgb[0] * 0.2627 + rgb[1] * 0.6780 + rgb[2] * 0.0593
}
pub fn srgb_decode(v: f32) -> f32 {
    let a = v.abs();
    v.signum()
        * if a <= 0.04045 {
            a / 12.92
        } else {
            ((a + 0.055) / 1.055).powf(2.4)
        }
}
pub fn srgb_encode(v: f32) -> f32 {
    let a = v.abs();
    v.signum()
        * if a <= 0.0031308 {
            a * 12.92
        } else {
            1.055 * a.powf(1.0 / 2.4) - 0.055
        }
}

fn xy(x: f64, y: f64) -> CIExyY {
    CIExyY { x, y, Y: 1.0 }
}
pub fn profile(space: OutputSpace, linear: bool) -> Result<Profile> {
    if space == OutputSpace::Srgb && !linear {
        return Ok(Profile::new_srgb());
    }
    let primaries = match space {
        OutputSpace::Srgb => CIExyYTRIPLE {
            Red: xy(0.64, 0.33),
            Green: xy(0.30, 0.60),
            Blue: xy(0.15, 0.06),
        },
        OutputSpace::DisplayP3 => CIExyYTRIPLE {
            Red: xy(0.68, 0.32),
            Green: xy(0.265, 0.69),
            Blue: xy(0.15, 0.06),
        },
        OutputSpace::AdobeRgb => CIExyYTRIPLE {
            Red: xy(0.64, 0.33),
            Green: xy(0.21, 0.71),
            Blue: xy(0.15, 0.06),
        },
        OutputSpace::LinearRec2020 => CIExyYTRIPLE {
            Red: xy(0.708, 0.292),
            Green: xy(0.170, 0.797),
            Blue: xy(0.131, 0.046),
        },
    };
    let curve = if linear || space == OutputSpace::LinearRec2020 {
        ToneCurve::new(1.0)
    } else if space == OutputSpace::AdobeRgb {
        ToneCurve::new(563.0 / 256.0)
    } else {
        ToneCurve::new_parametric(4, &[2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045])?
    };
    Ok(Profile::new_rgb(
        &xy(0.3127, 0.3290),
        &primaries,
        &[&curve, &curve, &curve],
    )?)
}

pub fn validate_icc(bytes: &[u8]) -> Result<Profile> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= 16 * 1024 * 1024,
        "ICC resource exceeds limits"
    );
    let profile = Profile::new_icc(bytes)?;
    ensure!(
        matches!(
            profile.color_space(),
            lcms2::ColorSpaceSignature::RgbData
                | lcms2::ColorSpaceSignature::GrayData
                | lcms2::ColorSpaceSignature::CmykData
        ),
        "ICC must describe RGB, grayscale or CMYK"
    );
    Ok(profile)
}

/// Transform bounded chunks to keep ICC scratch memory independent of image size.
pub fn transform(
    frame: &Frame,
    from: &Profile,
    to: &Profile,
    context: &RenderContext,
) -> Result<Frame> {
    ensure!(
        from.color_space() == lcms2::ColorSpaceSignature::RgbData
            && to.color_space() == lcms2::ColorSpaceSignature::RgbData,
        "RGB pixel transform requires RGB profiles"
    );
    frame.validate()?;
    let _memory = context.reserve(frame.byte_len() + 8192 * 24)?;
    let conversion: Transform<[f32; 3], [f32; 3]> = Transform::new_flags(
        from,
        PixelFormat::RGB_FLT,
        to,
        PixelFormat::RGB_FLT,
        Intent::RelativeColorimetric,
        lcms2::Flags::NO_OPTIMIZE,
    )?;
    let mut output = frame.clone();
    for pixels in output.pixels.chunks_mut(8192) {
        context.check_cancelled()?;
        let input: Vec<_> = pixels.iter().map(|p| [p[0], p[1], p[2]]).collect();
        let mut converted = vec![[0.0; 3]; input.len()];
        conversion.transform_pixels(&input, &mut converted);
        for (p, rgb) in pixels.iter_mut().zip(converted) {
            p[..3].copy_from_slice(&rgb);
        }
    }
    output.validate()?;
    Ok(output)
}

/// Grayscale ICC conversion uses its single-channel transfer function. The
/// image decoder expands gray to equal RGB; only the original gray is consumed.
pub fn gray_to_working(frame: &Frame, from: &Profile, context: &RenderContext) -> Result<Frame> {
    ensure!(
        from.color_space() == lcms2::ColorSpaceSignature::GrayData,
        "gray ICC required"
    );
    frame.validate()?;
    let to = profile(OutputSpace::LinearRec2020, true)?;
    let conversion: Transform<[f32; 1], [f32; 3]> = Transform::new_flags(
        from,
        PixelFormat::GRAY_FLT,
        &to,
        PixelFormat::RGB_FLT,
        Intent::RelativeColorimetric,
        lcms2::Flags::NO_OPTIMIZE,
    )?;
    let _memory = context.reserve(frame.byte_len() + 8192 * 16)?;
    let mut result = frame.clone();
    for pixels in result.pixels.chunks_mut(8192) {
        context.check_cancelled()?;
        let input: Vec<_> = pixels.iter().map(|p| [p[0]]).collect();
        let mut output = vec![[0.0; 3]; input.len()];
        conversion.transform_pixels(&input, &mut output);
        for (pixel, rgb) in pixels.iter_mut().zip(output) {
            pixel[..3].copy_from_slice(&rgb);
        }
    }
    result.validate()?;
    Ok(result)
}

/// Display transformation. Compression is terminal and never feeds editing caches.
/// Result is target-encoded RGBA, despite sharing the pixel container type.
pub fn output(frame: &Frame, config: &OutputConfig, context: &RenderContext) -> Result<Frame> {
    config.validate()?;
    frame.validate()?;
    let _mapped_memory = context.reserve(frame.byte_len())?;
    let mut mapped = frame.clone();
    apply_tone_map(&mut mapped, config.tone_map, context)?;
    let working = profile(OutputSpace::LinearRec2020, true)?;
    let destination = profile(config.space, true)?;
    let mut target = transform(&mapped, &working, &destination, context)?;
    encode_target(&mut target, config, context)?;
    target.validate()?;
    Ok(target)
}

/// Soft proof an already output-encoded image. The simulation result is sRGB
/// for browser display; it is never used as a replacement source for export.
pub fn soft_proof(
    encoded: &Frame,
    output_space: OutputSpace,
    proof_icc: &[u8],
    context: &RenderContext,
) -> Result<Frame> {
    encoded.validate()?;
    let proof = validate_icc(proof_icc)?;
    let from = profile(output_space, false)?;
    let display = profile(OutputSpace::Srgb, false)?;
    let conversion: Transform<[f32; 3], [f32; 3]> = Transform::new_proofing(
        &from,
        PixelFormat::RGB_FLT,
        &display,
        PixelFormat::RGB_FLT,
        &proof,
        Intent::RelativeColorimetric,
        Intent::RelativeColorimetric,
        lcms2::Flags::SOFT_PROOFING,
    )?;
    let _memory = context.reserve(encoded.byte_len() + 8192 * 24)?;
    let mut result = encoded.clone();
    for pixels in result.pixels.chunks_mut(8192) {
        context.check_cancelled()?;
        let input: Vec<_> = pixels.iter().map(|p| [p[0], p[1], p[2]]).collect();
        let mut output = vec![[0.0; 3]; input.len()];
        conversion.transform_pixels(&input, &mut output);
        for (pixel, rgb) in pixels.iter_mut().zip(output) {
            pixel[..3].copy_from_slice(&rgb);
        }
    }
    result.validate()?;
    Ok(result)
}

fn apply_tone_map(mapped: &mut Frame, tone_map: ToneMap, context: &RenderContext) -> Result<()> {
    if let ToneMap::Shoulder { knee } = tone_map {
        for row in mapped.pixels.chunks_mut(mapped.width as usize) {
            context.check_cancelled()?;
            for p in row {
                let y = luminance([p[0], p[1], p[2]]);
                if y > knee {
                    let compressed =
                        knee + (1.0 - knee) * (1.0 - (-(y - knee) / (1.0 - knee)).exp());
                    for value in &mut p[..3] {
                        *value *= compressed / y;
                    }
                }
            }
        }
    }
    Ok(())
}

fn compress_gamut(p: &mut [f32; 4], weights: [f32; 3]) {
    let y = (p[0] * weights[0] + p[1] * weights[1] + p[2] * weights[2]).clamp(0.0, 1.0);
    let mut scale = 1.0_f32;
    for &v in &p[..3] {
        let d = v - y;
        if d > 0.0 {
            scale = scale.min((1.0 - y) / d);
        }
        if d < 0.0 {
            scale = scale.min(-y / d);
        }
    }
    for v in &mut p[..3] {
        *v = y + (*v - y) * scale;
    }
}

fn encode_target(target: &mut Frame, config: &OutputConfig, context: &RenderContext) -> Result<()> {
    let weights = match config.space {
        OutputSpace::Srgb => [0.2126, 0.7152, 0.0722],
        OutputSpace::DisplayP3 => [0.228975, 0.691739, 0.079287],
        OutputSpace::AdobeRgb => [0.297377, 0.627349, 0.075274],
        OutputSpace::LinearRec2020 => [0.2627, 0.6780, 0.0593],
    };
    for row in target.pixels.chunks_mut(target.width as usize) {
        context.check_cancelled()?;
        for p in row {
            if config.gamut_compression {
                compress_gamut(p, weights);
            }
            for v in &mut p[..3] {
                *v = match config.space {
                    OutputSpace::Srgb | OutputSpace::DisplayP3 => srgb_encode(*v),
                    OutputSpace::AdobeRgb => v.signum() * v.abs().powf(256.0 / 563.0),
                    OutputSpace::LinearRec2020 => *v,
                };
            }
        }
    }
    Ok(())
}
