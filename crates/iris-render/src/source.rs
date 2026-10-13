// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    color::{self, OutputSpace, ReferenceKind},
    frame::pixel_count,
    Frame, RenderContext,
};
use anyhow::{ensure, Context, Result};
use image::{DynamicImage, ImageDecoder, ImageReader};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Read, Seek},
    path::Path,
};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SourceMetadata {
    pub width: u32,
    pub height: u32,
    /// Encoded source/sensor precision, not the FP32 working buffer precision.
    /// None means the RAW worker could not establish sensor precision.
    pub source_bit_depth: Option<u16>,
    pub reference_kind: ReferenceKind,
    pub assumed_srgb: bool,
    pub embedded_icc: bool,
    #[serde(default)]
    pub raw_calibration: Option<crate::white_balance::RawCalibration>,
}

#[derive(Debug)]
pub struct DecodedSource {
    pub frame: Frame,
    pub metadata: SourceMetadata,
}

pub fn decode_file(path: &Path, context: &RenderContext) -> Result<DecodedSource> {
    let file = std::fs::File::open(path).context("open high precision source")?;
    ensure!(
        file.metadata()?.len() <= 1024 * 1024 * 1024,
        "source file too large"
    );
    decode(std::io::BufReader::new(file), context)
}

/// Decode tagged RGB sources through their ICC profile. Untagged raster input
/// explicitly assumes sRGB. Tagged grayscale uses its gray transfer function.
/// CMYK raster decoding fails explicitly because image decoders may already
/// convert ink samples to RGB; CMYK ICC remains supported for soft proofing.
pub fn decode<R: BufRead + Seek>(mut reader: R, context: &RenderContext) -> Result<DecodedSource> {
    context.check_cancelled()?;
    validate_stream_size(&mut reader)?;
    let (mut reader, tiff_metadata) = reader_with_metadata(reader, context)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(context.memory_limit());
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    let count = pixel_count(width, height)?;
    let color_type = decoder.color_type();
    let depth = color_type.bits_per_pixel() / u16::from(color_type.channel_count());
    let _decode_budget = context.reserve(count as u64 * 32 + decoder.total_bytes())?;
    let (icc, orientation) = decoder_metadata(&mut decoder, tiff_metadata)?;
    let _icc_memory = context.reserve(icc.as_ref().map_or(0, |bytes| bytes.len() as u64))?;
    let from = match &icc {
        Some(bytes) => color::validate_icc(bytes)?,
        None => color::profile(OutputSpace::Srgb, false)?,
    };
    let gray_profile = validate_raster_profile(&from, color_type)?;
    let frame = decode_to_working(decoder, orientation, &from, gray_profile, context)?;
    let (width, height) = (frame.width, frame.height);
    Ok(DecodedSource {
        frame,
        metadata: SourceMetadata {
            width,
            height,
            source_bit_depth: Some(depth),
            reference_kind: ReferenceKind::Display,
            assumed_srgb: icc.is_none(),
            embedded_icc: icc.is_some(),
            raw_calibration: None,
        },
    })
}

#[derive(Debug, Deserialize)]
struct RawHeader {
    version: u32,
    width: u32,
    height: u32,
    channels: u32,
    color_space: String,
    reference_kind: String,
    orientation: u8,
    source_kind: String,
    sample_format: String,
    alpha: String,
    orientation_applied: bool,
    #[serde(default)]
    sensor_bit_depth: Option<u16>,
    #[serde(default)]
    raw_calibration: Option<crate::white_balance::RawCalibration>,
}

/// Decode the separate RAW worker's versioned FP32 stream, never a PPM/JPEG.
/// The process owner must provide timeout, kill-on-cancel and bounded stderr.
pub fn decode_raw_stream(mut reader: impl Read, context: &RenderContext) -> Result<DecodedSource> {
    context.check_cancelled()?;
    let mut magic = [0_u8; 8];
    reader.read_exact(&mut magic)?;
    ensure!(&magic == b"IRISF32\0", "unsupported RAW float protocol");
    let mut len = [0; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    ensure!((1..=65536).contains(&len), "invalid RAW header size");
    // The JSON bytes and owned string fields coexist during parsing.
    let _header_memory = context.reserve(len as u64 * 2)?;
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes)?;
    let header: RawHeader = serde_json::from_slice(&bytes)?;
    validate_raw_header(&header)?;
    let count = pixel_count(header.width, header.height)?;
    ensure!(
        count <= 64_000_000,
        "RAW float frame exceeds worker sensor limit"
    );
    let frame = read_raw_pixels(&mut reader, &header, count, context)?;
    let metadata = SourceMetadata {
        width: frame.width,
        height: frame.height,
        source_bit_depth: header.sensor_bit_depth,
        reference_kind: ReferenceKind::Scene,
        assumed_srgb: false,
        embedded_icc: false,
        raw_calibration: header.raw_calibration,
    };
    Ok(DecodedSource { frame, metadata })
}

fn validate_raw_header(header: &RawHeader) -> Result<()> {
    ensure!(
        header.version == 1
            && header.channels == 4
            && header.color_space == "linear_rec2020_d65"
            && header.reference_kind == "scene"
            && header.source_kind == "raw"
            && header.sample_format == "f32_le"
            && header.alpha == "straight"
            && !header.orientation_applied,
        "unsupported RAW float interpretation"
    );
    ensure!(
        (1..=8).contains(&header.orientation),
        "invalid RAW orientation"
    );
    ensure!(
        header
            .sensor_bit_depth
            .is_none_or(|depth| (1..=32).contains(&depth)),
        "invalid RAW sensor bit depth"
    );
    if let Some(calibration) = &header.raw_calibration {
        calibration.validate()?;
    }
    Ok(())
}

fn validate_raster_profile(from: &lcms2::Profile, color_type: image::ColorType) -> Result<bool> {
    let gray_profile = from.color_space() == lcms2::ColorSpaceSignature::GrayData;
    ensure!(
        from.color_space() != lcms2::ColorSpaceSignature::CmykData,
        "CMYK raster input is not supported; convert to tagged RGB first"
    );
    ensure!(
        !gray_profile
            || matches!(
                color_type,
                image::ColorType::L8
                    | image::ColorType::La8
                    | image::ColorType::L16
                    | image::ColorType::La16
            ),
        "gray ICC attached to non-gray pixels"
    );
    Ok(gray_profile)
}

fn validate_stream_size(reader: &mut impl Seek) -> Result<()> {
    let start = reader.stream_position()?;
    let end = reader.seek(std::io::SeekFrom::End(0))?;
    ensure!(
        end.saturating_sub(start) <= 1024 * 1024 * 1024,
        "source file too large"
    );
    reader.seek(std::io::SeekFrom::Start(start))?;
    Ok(())
}

fn read_raw_pixels(
    reader: &mut impl Read,
    header: &RawHeader,
    count: usize,
    context: &RenderContext,
) -> Result<Frame> {
    let row_bytes = u64::from(header.width) * 16;
    let _memory = context.reserve(count as u64 * 16 + row_bytes)?;
    let mut pixels = Vec::with_capacity(count);
    let mut row = vec![0; header.width as usize * 16];
    for _ in 0..header.height {
        context.check_cancelled()?;
        reader.read_exact(&mut row)?;
        for pixel in row.chunks_exact(16) {
            let mut rgba = [0.0; 4];
            for (c, bytes) in pixel.chunks_exact(4).enumerate() {
                rgba[c] = f32::from_le_bytes(bytes.try_into()?);
            }
            ensure!(
                rgba.iter().all(|value| value.is_finite()) && (0.0..=1.0).contains(&rgba[3]),
                "invalid RAW float sample"
            );
            pixels.push(rgba);
        }
    }
    let mut extra = [0];
    ensure!(reader.read(&mut extra)? == 0, "unexpected RAW payload tail");
    let raw = Frame::new(header.width, header.height, pixels)?;
    let frame = if header.orientation == 1 {
        raw
    } else {
        raw.oriented(header.orientation, context)?
    };
    Ok(frame)
}

fn decode_to_working(
    decoder: impl ImageDecoder,
    orientation: image::metadata::Orientation,
    from: &lcms2::Profile,
    gray_profile: bool,
    context: &RenderContext,
) -> Result<Frame> {
    let mut image = DynamicImage::from_decoder(decoder)?;
    context.check_cancelled()?;
    image.apply_orientation(orientation);
    let rgba = image.into_rgba32f();
    let (width, height) = rgba.dimensions();
    let encoded = Frame::new(width, height, rgba.pixels().map(|p| p.0).collect())?;
    drop(rgba);
    let working = color::profile(OutputSpace::LinearRec2020, true)?;
    let frame = if gray_profile {
        color::gray_to_working(&encoded, &from, context)?
    } else {
        color::transform(&encoded, &from, &working, context)?
    };
    Ok(frame)
}

// image 0.25.10 sets tiff's decoding_buffer_size to pixel bytes. tiff 0.11.3
// also uses it for IFD Value allocations, so even a valid ICC can exceed a tiny
// image's limit; image then swallows that error as None. Read only TIFF metadata
// independently with explicit bounds. Pixel decompression still occurs once.
type RasterMetadata = (Option<Vec<u8>>, image::metadata::Orientation);
fn reader_with_metadata<R: BufRead + Seek>(
    reader: R,
    context: &RenderContext,
) -> Result<(ImageReader<R>, Option<RasterMetadata>)> {
    let reader = ImageReader::new(reader).with_guessed_format()?;
    if reader.format() != Some(image::ImageFormat::Tiff) {
        return Ok((reader, None));
    }
    let mut input = reader.into_inner();
    let position = input.stream_position()?;
    let metadata = read_tiff_metadata(&mut input, context)?;
    input.seek(std::io::SeekFrom::Start(position))?;
    Ok((
        ImageReader::with_format(input, image::ImageFormat::Tiff),
        Some(metadata),
    ))
}

fn read_tiff_metadata(
    reader: &mut (impl BufRead + Seek),
    context: &RenderContext,
) -> Result<RasterMetadata> {
    context.check_cancelled()?;
    let mut limits = tiff::decoder::Limits::default();
    let cap = usize::try_from(context.memory_limit().min(16 * 1024 * 1024))?;
    limits.decoding_buffer_size = cap;
    limits.ifd_value_size = cap;
    limits.intermediate_buffer_size = cap;
    let mut decoder = tiff::decoder::Decoder::new(reader)?.with_limits(limits);
    let (width, height) = decoder.dimensions()?;
    pixel_count(width, height)?;
    let mut ifd = decoder.image_ifd();
    let orientation = ifd
        .find_tag_unsigned::<u16>(tiff::tags::Tag::Orientation)?
        .unwrap_or(1);
    ensure!((1..=8).contains(&orientation), "invalid TIFF orientation");
    let orientation = image::metadata::Orientation::from_exif(orientation as u8)
        .context("invalid TIFF orientation")?;
    let icc = read_tiff_icc(&mut ifd, context)?;
    Ok((icc, orientation))
}

fn read_tiff_icc(
    ifd: &mut tiff::decoder::IfdDecoder<'_>,
    context: &RenderContext,
) -> Result<Option<Vec<u8>>> {
    use tiff::tags::{Tag, Type};
    let Some(entry) = ifd.find_entry(Tag::IccProfile) else {
        return Ok(None);
    };
    ensure!(
        matches!(entry.field_type(), Type::BYTE | Type::UNDEFINED),
        "TIFF ICC must contain bytes"
    );
    let count = entry.count();
    ensure!(
        (1..=16 * 1024 * 1024).contains(&count),
        "TIFF ICC resource exceeds limits"
    );
    let _memory = context.reserve(count)?;
    let mut bytes = vec![0; usize::try_from(count)?];
    let read = ifd
        .find_tag_bytes(Tag::IccProfile, &mut bytes, 0)?
        .context("missing TIFF ICC data")?;
    ensure!(read == bytes.len(), "truncated TIFF ICC");
    context.check_cancelled()?;
    Ok(Some(bytes))
}

fn decoder_metadata(
    decoder: &mut impl ImageDecoder,
    tiff_metadata: Option<RasterMetadata>,
) -> Result<RasterMetadata> {
    match tiff_metadata {
        Some(metadata) => Ok(metadata),
        None => Ok((decoder.icc_profile()?, decoder.orientation()?)),
    }
}
