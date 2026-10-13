//! Independent, on-demand regional evidence; never rewrites preview scores or eye states.
use super::{metrics, Exposure};
use anyhow::Result;
use base64::Engine;
use image::{GenericImageView, ImageDecoder};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use utoipa::ToSchema;

pub const DETAIL_METHOD: &str = "iris-region-native-v2";

pub(crate) fn raw_converter_revision() -> String {
    super::decode::raw_converter_revision()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DetailRequest {
    pub face_index: Option<usize>,
    /// x, y, width, height in the oriented image, normalized to 0..1.
    pub region: Option<[f32; 4]>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct DetailEvidence {
    pub method: String,
    /// full_detail, preview_only, or unavailable. No automatic reject decision.
    pub state: String,
    pub region: [f32; 4],
    pub source_width: u32,
    pub source_height: u32,
    pub crop_width: u32,
    pub crop_height: u32,
    pub measured_width: u32,
    pub measured_height: u32,
    pub sharpness_lap: Option<f64>,
    pub exposure: Option<Exposure>,
    pub reason: Option<String>,
    /// Bounded JPEG of this original region; absent when native detail is unavailable.
    #[serde(default)]
    pub image_data_url: Option<String>,
    #[serde(default)]
    pub display_width: u32,
    #[serde(default)]
    pub display_height: u32,
}

pub fn validate_region(region: [f32; 4]) -> Result<()> {
    anyhow::ensure!(
        region.iter().all(|x| x.is_finite()),
        "region must be finite"
    );
    anyhow::ensure!(
        region[0] >= 0.
            && region[1] >= 0.
            && region[2] > 0.
            && region[3] > 0.
            && region[0] + region[2] <= 1.000001
            && region[1] + region[3] <= 1.000001,
        "region must be inside the oriented image"
    );
    Ok(())
}

pub fn measure(path: &Path, region: [f32; 4], cancel: &AtomicBool) -> Result<DetailEvidence> {
    validate_region(region)?;
    let mut evidence = DetailEvidence {
        method: DETAIL_METHOD.into(),
        state: "unavailable".into(),
        region,
        source_width: 0,
        source_height: 0,
        crop_width: 0,
        crop_height: 0,
        measured_width: 0,
        measured_height: 0,
        sharpness_lap: None,
        exposure: None,
        reason: None,
        image_data_url: None,
        display_width: 0,
        display_height: 0,
    };
    if super::supported_format(path) == Some("raw") {
        return measure_raw(path, evidence, cancel);
    }
    measure_raster(path, evidence, cancel)
}

fn measure_raw(
    path: &Path,
    mut evidence: DetailEvidence,
    cancel: &AtomicBool,
) -> Result<DetailEvidence> {
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "detail cancelled");
    match super::decode::native_raw_region(path, evidence.region, cancel) {
        Ok((rgb, width, height)) => {
            evidence.source_width = width;
            evidence.source_height = height;
            evidence.crop_width = rgb.width();
            evidence.crop_height = rgb.height();
            measured(evidence, rgb, cancel)
        }
        Err(error) => {
            anyhow::ensure!(!cancel.load(Ordering::Relaxed), "detail cancelled");
            evidence.state = "preview_only".into();
            evidence.reason = Some(format!("Native RAW region unavailable: {error:#}"));
            Ok(evidence)
        }
    }
}

fn measure_raster(
    path: &Path,
    mut evidence: DetailEvidence,
    cancel: &AtomicBool,
) -> Result<DetailEvidence> {
    let region = evidence.region;
    if !matches!(
        super::supported_format(path),
        Some("jpeg" | "png" | "webp" | "tiff")
    ) {
        evidence.state = "preview_only".into();
        evidence.reason=Some("Native regional detail is unavailable for this format; preview cannot establish original detail or recoverability".into());
        return Ok(evidence);
    }
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "detail cancelled");
    let mut reader = image::ImageReader::open(path)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > 40_000_000
        || decoder.total_bytes() > 256 * 1024 * 1024
    {
        evidence.reason =
            Some("Native detail exceeds the 40 megapixel / 256 MiB decoded buffer limit".into());
        return Ok(evidence);
    }
    let orientation = decoder.orientation()?;
    let mut image = image::DynamicImage::from_decoder(decoder)?;
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "detail cancelled");
    image.apply_orientation(orientation);
    evidence.source_width = image.width();
    evidence.source_height = image.height();
    let x = (region[0] * image.width() as f32).floor() as u32;
    let y = (region[1] * image.height() as f32).floor() as u32;
    let right = ((region[0] + region[2]) * image.width() as f32)
        .ceil()
        .min(image.width() as f32) as u32;
    let bottom = ((region[1] + region[3]) * image.height() as f32)
        .ceil()
        .min(image.height() as f32) as u32;
    evidence.crop_width = right.saturating_sub(x);
    evidence.crop_height = bottom.saturating_sub(y);
    if evidence.crop_width < 16 || evidence.crop_height < 16 {
        evidence.reason = Some(
            "Region has fewer than 16 native pixels on one side; detail remains uncertain".into(),
        );
        return Ok(evidence);
    }
    let crop = image
        .view(x, y, evidence.crop_width, evidence.crop_height)
        .to_image();
    drop(image);
    // Bound the displayed crop independently of the smaller measurement buffer.
    let crop = if crop.width().max(crop.height()) > 2048 {
        image::DynamicImage::ImageRgba8(crop)
            .resize(2048, 2048, image::imageops::FilterType::Triangle)
            .to_rgba8()
    } else {
        crop
    };
    let mut rgb = image::RgbImage::new(crop.width(), crop.height());
    for (out, pixel) in rgb.pixels_mut().zip(crop.pixels()) {
        let alpha = u32::from(pixel[3]);
        for channel in 0..3 {
            out[channel] =
                ((u32::from(pixel[channel]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
        }
    }
    measured(evidence, rgb, cancel)
}

fn measured(
    mut evidence: DetailEvidence,
    rgb: image::RgbImage,
    cancel: &AtomicBool,
) -> Result<DetailEvidence> {
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "detail cancelled");
    let rgb = if rgb.width().max(rgb.height()) > 2048 {
        image::imageops::resize(
            &rgb,
            (rgb.width() as f64 * 2048. / rgb.width().max(rgb.height()) as f64)
                .round()
                .max(1.) as u32,
            (rgb.height() as f64 * 2048. / rgb.width().max(rgb.height()) as f64)
                .round()
                .max(1.) as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        rgb
    };
    evidence.display_width = rgb.width();
    evidence.display_height = rgb.height();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 92).encode_image(&rgb)?;
    evidence.image_data_url = Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    ));
    let rgb = if rgb.width().max(rgb.height()) > 1024 {
        image::DynamicImage::ImageRgb8(rgb)
            .resize(1024, 1024, image::imageops::FilterType::Triangle)
            .to_rgb8()
    } else {
        rgb
    };
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "detail cancelled");
    let (lap, exposure) = metrics::spatial_metrics(&rgb);
    evidence.state = "full_detail".into();
    evidence.measured_width = rgb.width();
    evidence.measured_height = rgb.height();
    evidence.sharpness_lap = Some(lap);
    evidence.exposure = Some(exposure);
    evidence.reason=Some("Regional sharpness is a scale-dependent heuristic; this pass does not reclassify eye state or change human decisions".into());
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_detail_distinguishes_blur_and_reports_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.png");
        let sharp = image::RgbImage::from_fn(256, 256, |x, y| {
            image::Rgb([if (x / 4 + y / 4) % 2 == 0 { 0 } else { 255 }; 3])
        });
        sharp.save(&path).unwrap();
        let cancel = AtomicBool::new(false);
        let a = measure(&path, [0.25, 0.25, 0.5, 0.5], &cancel).unwrap();
        image::imageops::blur(&sharp, 3.).save(&path).unwrap();
        let b = measure(&path, [0.25, 0.25, 0.5, 0.5], &cancel).unwrap();
        assert_eq!(a.state, "full_detail");
        assert_eq!((a.crop_width, a.measured_width), (128, 128));
        assert_eq!((a.display_width, a.display_height), (128, 128));
        assert!(a
            .image_data_url
            .as_ref()
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        assert!(a.sharpness_lap.unwrap() > b.sharpness_lap.unwrap() * 10.);
        assert_eq!(
            measure(&path, [0., 0., 0.01, 0.01], &cancel).unwrap().state,
            "unavailable"
        );
        assert_eq!(
            measure(&dir.path().join("portrait.dng"), [0., 0., 1., 1.], &cancel)
                .unwrap()
                .state,
            "preview_only"
        );
        assert!(measure(&path, [0., 0., 1., 1.], &AtomicBool::new(true)).is_err());
        assert!(validate_region([0.9, 0., 0.2, 1.]).is_err());
        assert!(validate_region([f32::NAN, 0., 1., 1.]).is_err());
    }
}
