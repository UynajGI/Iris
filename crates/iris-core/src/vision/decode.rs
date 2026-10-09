use anyhow::{bail, Context, Result};
use image::{imageops, ImageDecoder, Rgb, RgbImage};
use jpeg_decoder::{Decoder, PixelFormat};
use std::{fs::File, io::BufReader, path::Path};

#[path = "heic.rs"]
mod heic;
#[path = "raw.rs"]
mod raw;

pub(super) const MAX_RASTER_PIXELS: u64 = 60_000_000;
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;

pub struct DecodedPreview {
    pub source: &'static str,
    pub image: RgbImage,
    pub original_width: u32,
    pub original_height: u32,
    pub orientation: u32,
}

/// Extension boundary for bounded image previews.
pub trait PreviewDecoder: Send + Sync {
    fn supports(&self, path: &Path) -> bool;
    fn decode(&self, path: &Path, max_edge: u32) -> Result<DecodedPreview>;
}

pub fn supported_format(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("jpeg"),
        "png" => Some("png"),
        "webp" => Some("webp"),
        "heic" | "heif" => Some("heic"),
        "dng" | "cr2" | "cr3" | "nef" | "nrw" | "arw" | "srw" | "orf" | "rw2" | "pef" | "raf"
        | "rwl" | "raw" => Some("raw"),
        _ => None,
    }
}

pub fn image_dimensions_with_media(path: &Path, media_dir: Option<&Path>) -> Result<(u32, u32)> {
    match supported_format(path) {
        Some("heic") => heic::dimensions(path, media_dir),
        Some("raw") => raw::dimensions(path, media_dir),
        Some("png" | "webp") => {
            let dimensions = image::image_dimensions(path)?;
            check_raster_dimensions(dimensions.0, dimensions.1)?;
            Ok(dimensions)
        }
        Some(_) => Ok(image::image_dimensions(path)?),
        None => bail!("unsupported image format"),
    }
}

pub fn decode_preview(path: &Path, max_edge: u32) -> Result<DecodedPreview> {
    decode_preview_with_media(path, max_edge, None)
}

pub fn decode_preview_with_media(
    path: &Path,
    max_edge: u32,
    media_dir: Option<&Path>,
) -> Result<DecodedPreview> {
    anyhow::ensure!(
        (1..=2560).contains(&max_edge),
        "preview max_edge must be in 1..=2560"
    );
    match supported_format(path) {
        Some("jpeg") => decode_jpeg_preview(path, max_edge),
        Some("heic") => heic::decode(path, max_edge, media_dir),
        Some("raw") => raw::decode(path, max_edge, media_dir),
        Some("png" | "webp") => {
            let mut reader = image::ImageReader::open(path)?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(32768);
            limits.max_image_height = Some(32768);
            limits.max_alloc = Some(MAX_DECODE_BYTES);
            reader.limits(limits);
            let mut decoder = reader.into_decoder()?;
            let (width, height) = decoder.dimensions();
            check_raster_dimensions(width, height)?;
            anyhow::ensure!(
                decoder.total_bytes() <= MAX_DECODE_BYTES,
                "image exceeds decoded buffer safety limit"
            );
            let orientation = decoder.orientation()?.to_exif() as u32;
            let image = image::DynamicImage::from_decoder(decoder)?.to_rgba8();
            // Preview JPEGs have no alpha. Composite onto white consistently,
            // so invisible RGB payload never influences visual scoring.
            let mut rgb = RgbImage::new(width, height);
            for (out, pixel) in rgb.pixels_mut().zip(image.pixels()) {
                let alpha = u32::from(pixel[3]);
                for channel in 0..3 {
                    out[channel] = ((u32::from(pixel[channel]) * alpha + 255 * (255 - alpha) + 127)
                        / 255) as u8;
                }
            }
            Ok(DecodedPreview {
                source: "raster_primary",
                image: resize_preview(orient(rgb, orientation), max_edge),
                original_width: width,
                original_height: height,
                orientation,
            })
        }
        _ => bail!("unsupported image format"),
    }
}

fn check_raster_dimensions(width: u32, height: u32) -> Result<()> {
    anyhow::ensure!(
        width > 0 && height > 0 && u64::from(width) * u64::from(height) <= MAX_RASTER_PIXELS,
        "image exceeds pixel safety limit"
    );
    Ok(())
}

fn resize_preview(image: RgbImage, max_edge: u32) -> RgbImage {
    if image.width().max(image.height()) <= max_edge {
        image
    } else {
        image::DynamicImage::ImageRgb8(image)
            .resize(max_edge, max_edge, imageops::FilterType::Triangle)
            .to_rgb8()
    }
}

/// Reduced IDCT decodes directly to <= max_edge. Never decodes a full RGB image
/// and then resizes it. JPEG's minimum scale is 1/8; larger inputs are rejected.
pub fn decode_jpeg_preview(path: &Path, max_edge: u32) -> Result<DecodedPreview> {
    if max_edge == 0 || max_edge > 2560 {
        bail!("preview max_edge must be in 1..=2560");
    }
    let mut decoder = Decoder::new(BufReader::new(File::open(path)?));
    decoder
        .read_info()
        .context("JPEG header invalid or unsupported color format")?;
    let original = decoder.info().context("missing JPEG dimensions")?;
    let (w, h) = (original.width as u32, original.height as u32);
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 120_000_000 {
        bail!("JPEG exceeds pixel safety limit");
    }
    let denominator = [1, 2, 4, 8]
        .into_iter()
        .find(|d| w.div_ceil(*d).max(h.div_ceil(*d)) <= max_edge)
        .context("JPEG too large for bounded 1/8 IDCT preview; embedded preview required")?;
    let (dw, dh) = decoder.scale(
        w.div_ceil(denominator) as u16,
        h.div_ceil(denominator) as u16,
    )?;
    if u32::from(dw).max(u32::from(dh)) > max_edge {
        bail!("decoder violated preview size bound");
    }
    decoder.set_max_decoding_buffer_size(256 * 1024 * 1024);
    let pixels = decoder.decode().context("JPEG decode failed")?;
    let image = match original.pixel_format {
        PixelFormat::RGB24 => {
            RgbImage::from_raw(dw.into(), dh.into(), pixels).context("invalid RGB JPEG")?
        }
        PixelFormat::L8 => {
            let mut image = RgbImage::new(dw.into(), dh.into());
            for (p, v) in image.pixels_mut().zip(pixels) {
                *p = Rgb([v, v, v]);
            }
            image
        }
        _ => bail!("Unsupported JPEG color space; only RGB and grayscale are supported"),
    };
    let orientation = exif::Reader::new()
        .read_from_container(&mut BufReader::new(File::open(path)?))
        .ok()
        .and_then(|e| {
            e.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
        })
        .unwrap_or(1);
    let image = orient(image, orientation);
    Ok(DecodedPreview {
        source: "jpeg_reduced_idct",
        image,
        original_width: w,
        original_height: h,
        orientation,
    })
}

fn orient(image: RgbImage, orientation: u32) -> RgbImage {
    match orientation {
        2 => imageops::flip_horizontal(&image),
        3 => imageops::rotate180(&image),
        4 => imageops::flip_vertical(&image),
        5 => imageops::rotate90(&imageops::flip_vertical(&image)),
        6 => imageops::rotate90(&image),
        7 => imageops::rotate90(&imageops::flip_horizontal(&image)),
        8 => imageops::rotate270(&image),
        _ => image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orientation_swaps_dimensions() {
        let im = RgbImage::new(2, 3);
        assert_eq!(orient(im, 6).dimensions(), (3, 2));
    }
    #[test]
    fn all_orientations_preserve_pixels() {
        for o in 1..=8 {
            let mut im = RgbImage::new(2, 3);
            for (i, p) in im.pixels_mut().enumerate() {
                *p = Rgb([i as u8, 0, 0]);
            }
            let oriented = orient(im, o);
            let mut vals: Vec<_> = oriented.pixels().map(|p| p[0]).collect();
            vals.sort();
            assert_eq!(vals, vec![0, 1, 2, 3, 4, 5]);
        }
    }
}
