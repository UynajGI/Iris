// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{ensure, Result};
use exif::{Exif, In, Tag, Value};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PhotoMetadataStatus {
    Available,
    NotFound,
    #[default]
    Unreadable,
    ReadLimit,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct PhotoMetadata {
    pub status: PhotoMetadataStatus,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_make: Option<String>,
    pub lens_model: Option<String>,
    pub captured_at: Option<String>,
    pub captured_offset: Option<String>,
    pub exposure_seconds: Option<f64>,
    pub f_number: Option<f64>,
    pub iso: Option<u32>,
    pub focal_mm: Option<f64>,
    pub focal_35mm: Option<u32>,
    pub exposure_bias_ev: Option<f64>,
    pub exposure_program: Option<u32>,
    pub metering_mode: Option<u32>,
    pub flash: Option<u32>,
    pub white_balance: Option<u32>,
    pub orientation: Option<u32>,
    pub artist: Option<String>,
    pub copyright: Option<String>,
}

/// Bounded, read-only container inspection. Never starts a decoder or external executable.
pub(crate) fn read(path: &Path, cancel: &AtomicBool) -> Result<PhotoMetadata> {
    ensure!(
        !cancel.load(Ordering::Relaxed),
        "metadata request cancelled"
    );
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    let limit = 16 * 1024 * 1024;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length.min(limit) as usize)?;
    file.take(limit).read_to_end(&mut bytes)?;
    ensure!(
        !cancel.load(Ordering::Relaxed),
        "metadata request cancelled"
    );
    let result = match exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        Ok(exif) => from_exif(&exif),
        Err(_) if length > limit => PhotoMetadata {
            status: PhotoMetadataStatus::ReadLimit,
            ..Default::default()
        },
        Err(exif::Error::NotFound(_)) => PhotoMetadata {
            status: PhotoMetadataStatus::NotFound,
            ..Default::default()
        },
        Err(_) => PhotoMetadata::default(),
    };
    ensure!(
        !cancel.load(Ordering::Relaxed),
        "metadata request cancelled"
    );
    Ok(result)
}

fn from_exif(exif: &Exif) -> PhotoMetadata {
    PhotoMetadata {
        status: PhotoMetadataStatus::Available,
        camera_make: text_field(exif, Tag::Make),
        camera_model: text_field(exif, Tag::Model),
        lens_make: text_field(exif, Tag::LensMake),
        lens_model: text_field(exif, Tag::LensModel),
        captured_at: text_field(exif, Tag::DateTimeOriginal),
        captured_offset: text_field(exif, Tag::OffsetTimeOriginal),
        exposure_seconds: rational_field(exif, Tag::ExposureTime),
        f_number: rational_field(exif, Tag::FNumber),
        iso: uint_field(exif, Tag::PhotographicSensitivity)
            .filter(|value| *value > 0 && *value < 65535)
            .or_else(|| uint_field(exif, Tag::ISOSpeed).filter(|value| *value > 0)),
        focal_mm: rational_field(exif, Tag::FocalLength),
        focal_35mm: uint_field(exif, Tag::FocalLengthIn35mmFilm).filter(|value| *value > 0),
        exposure_bias_ev: number_field(exif, Tag::ExposureBiasValue),
        exposure_program: uint_field(exif, Tag::ExposureProgram),
        metering_mode: uint_field(exif, Tag::MeteringMode),
        flash: uint_field(exif, Tag::Flash),
        white_balance: uint_field(exif, Tag::WhiteBalance),
        orientation: uint_field(exif, Tag::Orientation).filter(|value| (1..=8).contains(value)),
        artist: text_field(exif, Tag::Artist),
        copyright: text_field(exif, Tag::Copyright),
    }
}

pub(crate) fn text_field(exif: &Exif, tag: Tag) -> Option<String> {
    let Value::Ascii(values) = &exif.get_field(tag, In::PRIMARY)?.value else {
        return None;
    };
    if values.len() != 1 {
        return None;
    }
    let text = std::str::from_utf8(values.first()?)
        .ok()?
        .trim_matches('\0')
        .trim();
    if text.is_empty() || text.len() > 128 || text.chars().any(char::is_control) {
        return None;
    }
    Some(text.to_owned())
}

pub(crate) fn rational_field(exif: &Exif, tag: Tag) -> Option<f64> {
    let Value::Rational(values) = &exif.get_field(tag, In::PRIMARY)?.value else {
        return None;
    };
    if values.len() != 1 {
        return None;
    }
    let value = values.first()?.to_f64();
    (value.is_finite() && value > 0.0).then_some(value)
}

fn number_field(exif: &Exif, tag: Tag) -> Option<f64> {
    let value = match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Rational(values) if values.len() == 1 => values[0].to_f64(),
        Value::SRational(values) if values.len() == 1 => values[0].to_f64(),
        _ => return None,
    };
    value.is_finite().then_some(value)
}

fn uint_field(exif: &Exif, tag: Tag) -> Option<u32> {
    let value = &exif.get_field(tag, In::PRIMARY)?.value;
    if value.get_uint(1).is_some() {
        return None;
    }
    value.get_uint(0)
}

#[cfg(test)]
mod tests;
