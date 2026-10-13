// SPDX-License-Identifier: GPL-3.0-or-later
//! Terminal encoders. `encode` accepts working-linear pixels; `encode_output`
//! accepts the already target-encoded result of `color::output`. Encoding never
//! opens a path or publishes a file. The caller owns atomic publication and
//! original protection. Exported pixels are already orientation-normalized;
//! stale source EXIF orientation is intentionally not copied.
use crate::{
    color::{self, OutputConfig, OutputSpace},
    Frame, RenderContext,
};
use anyhow::{ensure, Context, Result};
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    ExtendedColorType, ImageEncoder,
};
use serde::{Deserialize, Serialize};
use std::io::{self, Cursor, Seek, SeekFrom, Write};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "format", rename_all = "snake_case", deny_unknown_fields)]
pub enum OutputFormat {
    Jpeg8 { quality: u8 },
    Png16,
    Tiff16,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg8 { .. } => "jpg",
            Self::Png16 => "png",
            Self::Tiff16 => "tif",
        }
    }
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Jpeg8 { .. } => "image/jpeg",
            Self::Png16 => "image/png",
            Self::Tiff16 => "image/tiff",
        }
    }
    fn validate(self) -> Result<()> {
        if let Self::Jpeg8 { quality } = self {
            ensure!((1..=100).contains(&quality), "JPEG quality must be 1..100");
        }
        Ok(())
    }
}

fn byte_limit(frame: &Frame) -> Result<u64> {
    frame.validate()?;
    // Covers lossless payload, row/strip metadata and ICC with ample overhead.
    let limit = (frame.pixels.len() as u64)
        .checked_mul(12)
        .and_then(|v| v.checked_add(1024 * 1024))
        .context("encoded size overflow")?;
    ensure!(
        limit <= 1024 * 1024 * 1024,
        "encoded image exceeds output safety limit"
    );
    Ok(limit)
}

/// Convert a working-linear frame exactly once, then encode terminal output.
pub fn encode(
    frame: &Frame,
    config: &OutputConfig,
    format: OutputFormat,
    context: &RenderContext,
) -> Result<Vec<u8>> {
    let target = color::output(frame, config, context)?;
    let _target_memory = context.reserve(target.byte_len())?;
    encode_output(&target, config, format, context)
}

/// Encode an already target-encoded frame. `config.space` MUST describe those
/// pixels. This does not repeat tone mapping, gamut mapping or `color::output`.
pub fn encode_output(
    encoded: &Frame,
    config: &OutputConfig,
    format: OutputFormat,
    context: &RenderContext,
) -> Result<Vec<u8>> {
    let limit = byte_limit(encoded)?;
    let _bytes_memory = context.reserve(limit)?;
    let mut bytes = Cursor::new(Vec::with_capacity(usize::try_from(limit)?));
    write_output(&mut bytes, encoded, config, format, context)?;
    let mut bytes = bytes.into_inner();
    // Do not retain the conservative admission cap in small cached exports.
    let _shrink_memory = context.reserve(bytes.len() as u64)?;
    bytes.shrink_to_fit();
    Ok(bytes)
}

/// Write target-encoded pixels to caller-owned storage. The bounded writer
/// rejects seeks/writes beyond the image's cap. A cancelled/failed write may be
/// partial; publish only after this returns success. Codec work is cancellable
/// between writes, with conversion/quantization checked once per row.
pub fn write_output<W: Write + Seek>(
    writer: W,
    encoded: &Frame,
    config: &OutputConfig,
    format: OutputFormat,
    context: &RenderContext,
) -> Result<()> {
    context.check_cancelled()?;
    config.validate()?;
    format.validate()?;
    let limit = byte_limit(encoded)?;
    let _scratch_memory = context.reserve(encoded.pixels.len() as u64 * 24 + 2 * 1024 * 1024)?;
    let icc = color::profile(config.space, false)?.icc()?;
    ensure!(icc.len() <= 16 * 1024 * 1024, "output ICC too large");
    let _icc_memory = context.reserve(icc.len() as u64)?;
    let mut writer = BoundedWriter::new(writer, limit, context)?;
    encode_pixels(&mut writer, encoded, config, format, context, icc)?;
    writer.flush()?;
    context.check_cancelled()?;
    Ok(())
}

fn decode_channel(value: f32, space: OutputSpace) -> f64 {
    // Encoded working pixels may be any finite f32. Inverse transfer functions
    // can overflow f32 even when their eventual terminal JPEG value is bounded.
    let value = f64::from(value);
    match space {
        OutputSpace::Srgb | OutputSpace::DisplayP3 => {
            let magnitude = value.abs();
            value.signum()
                * if magnitude <= 0.04045 {
                    magnitude / 12.92
                } else {
                    ((magnitude + 0.055) / 1.055).powf(2.4)
                }
        }
        OutputSpace::AdobeRgb => value.signum() * value.abs().powf(563.0 / 256.0),
        OutputSpace::LinearRec2020 => value,
    }
}
fn encode_channel(value: f32, space: OutputSpace) -> f32 {
    match space {
        OutputSpace::Srgb | OutputSpace::DisplayP3 => color::srgb_encode(value),
        OutputSpace::AdobeRgb => value.signum() * value.abs().powf(256.0 / 563.0),
        OutputSpace::LinearRec2020 => value,
    }
}
fn background_in_target(config: &OutputConfig, context: &RenderContext) -> Result<[f32; 3]> {
    let source = Frame::new(
        1,
        1,
        vec![[
            config.background[0],
            config.background[1],
            config.background[2],
            1.0,
        ]],
    )?;
    let from = color::profile(OutputSpace::Srgb, false)?;
    let to = color::profile(config.space, true)?;
    let result = color::transform(&source, &from, &to, context)?;
    Ok([
        result.pixels[0][0],
        result.pixels[0][1],
        result.pixels[0][2],
    ])
}

struct BoundedWriter<'a, W> {
    inner: W,
    start: u64,
    position: u64,
    limit: u64,
    context: &'a RenderContext,
}
impl<'a, W: Write + Seek> BoundedWriter<'a, W> {
    fn new(mut inner: W, limit: u64, context: &'a RenderContext) -> io::Result<Self> {
        let start = inner.stream_position()?;
        Ok(Self {
            inner,
            start,
            position: 0,
            limit,
            context,
        })
    }
    fn check(&self) -> io::Result<()> {
        self.context
            .check_cancelled()
            .map_err(|error| io::Error::other(error.to_string()))
    }
}
impl<W: Write + Seek> Write for BoundedWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.check()?;
        if bytes.len() as u64 > self.limit.saturating_sub(self.position) {
            return Err(io::Error::other("encoded image exceeds byte limit"));
        }
        let written = self.inner.write(bytes)?;
        self.position += written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.check()?;
        self.inner.flush()
    }
}
impl<W: Write + Seek> Seek for BoundedWriter<'_, W> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.check()?;
        let position = match from {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(delta) => self
                .position
                .checked_add_signed(delta)
                .ok_or_else(|| io::Error::other("invalid output seek"))?,
            SeekFrom::End(_) => {
                return Err(io::Error::other("end-relative output seek unsupported"))
            }
        };
        if position > self.limit {
            return Err(io::Error::other("output seek exceeds byte limit"));
        }
        let absolute = self
            .start
            .checked_add(position)
            .ok_or_else(|| io::Error::other("output seek overflow"))?;
        self.inner.seek(SeekFrom::Start(absolute))?;
        self.position = position;
        Ok(position)
    }
}

fn jpeg_samples(
    encoded: &Frame,
    config: &OutputConfig,
    context: &RenderContext,
) -> Result<Vec<u8>> {
    let background = background_in_target(config, context)?;
    let mut rgb = Vec::with_capacity(encoded.pixels.len() * 3);
    for row in encoded.pixels.chunks(encoded.width as usize) {
        context.check_cancelled()?;
        for p in row {
            for c in 0..3 {
                let foreground = decode_channel(p[c], config.space);
                let alpha = f64::from(p[3]);
                let linear = foreground * alpha + f64::from(background[c]) * (1.0 - alpha);
                ensure!(linear.is_finite(), "JPEG compositing overflow");
                rgb.push(
                    (encode_channel(linear.clamp(0.0, 1.0) as f32, config.space) * 255.0).round()
                        as u8,
                );
            }
        }
    }
    Ok(rgb)
}

fn rgba16_samples(encoded: &Frame, context: &RenderContext) -> Result<Vec<u8>> {
    let mut rgba = Vec::with_capacity(encoded.pixels.len() * 8);
    for row in encoded.pixels.chunks(encoded.width as usize) {
        context.check_cancelled()?;
        for p in row {
            for channel in p {
                // Integer output is explicitly terminal bounded display data.
                rgba.extend_from_slice(
                    &((channel.clamp(0.0, 1.0) * 65535.0).round() as u16).to_ne_bytes(),
                );
            }
        }
    }
    Ok(rgba)
}

fn write_tiff<W: Write + Seek>(writer: W, encoded: &Frame, rgba: &[u8], icc: &[u8]) -> Result<()> {
    // image 0.25's TIFF wrapper omits ExtraSamples, so write the
    // unassociated-alpha tag explicitly through its TIFF backend.
    let samples: Vec<u16> = rgba
        .chunks_exact(2)
        .map(|v| u16::from_ne_bytes([v[0], v[1]]))
        .collect();
    let mut encoder = tiff::encoder::TiffEncoder::new(writer)?;
    let mut image =
        encoder.new_image::<tiff::encoder::colortype::RGBA16>(encoded.width, encoded.height)?;
    image
        .encoder()
        .write_tag(tiff::tags::Tag::ExtraSamples, &[2_u16][..])?;
    image
        .encoder()
        .write_tag(tiff::tags::Tag::Orientation, 1_u16)?;
    image
        .encoder()
        .write_tag(tiff::tags::Tag::IccProfile, icc)?;
    image.write_data(&samples)?;
    Ok(())
}

fn encode_pixels<W: Write + Seek>(
    mut writer: W,
    encoded: &Frame,
    config: &OutputConfig,
    format: OutputFormat,
    context: &RenderContext,
    icc: Vec<u8>,
) -> Result<()> {
    match format {
        OutputFormat::Jpeg8 { quality } => {
            let rgb = jpeg_samples(encoded, config, context)?;
            let mut encoder = JpegEncoder::new_with_quality(&mut writer, quality);
            encoder.set_icc_profile(icc)?;
            encoder.write_image(&rgb, encoded.width, encoded.height, ExtendedColorType::Rgb8)?;
        }
        OutputFormat::Png16 | OutputFormat::Tiff16 => {
            let rgba = rgba16_samples(encoded, context)?;
            if format == OutputFormat::Png16 {
                let mut encoder = PngEncoder::new(&mut writer);
                encoder.set_icc_profile(icc)?;
                encoder.write_image(
                    &rgba,
                    encoded.width,
                    encoded.height,
                    ExtendedColorType::Rgba16,
                )?;
            } else {
                write_tiff(&mut writer, encoded, &rgba, &icc)?;
            }
        }
    }
    Ok(())
}
