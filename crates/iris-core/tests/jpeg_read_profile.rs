//! Opt-in microbenchmark only. Production decoder and dependencies are unchanged.
use anyhow::{bail, Context, Result};
use image::{imageops, Rgb, RgbImage};
use iris_core::vision::{decode_jpeg_preview, DecodedPreview};
use jpeg_decoder::{Decoder, PixelFormat};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    time::Instant,
};

const INPUT_CAP: u64 = 256 * 1024 * 1024;
const MODES: [&str; 3] = ["bufreader_8k", "bufreader_256k", "bounded_memory_slice"];

// Deliberately mirrors production's decoder, scale, allocation bound and RGB
// conversion. Only the Read implementation differs; comparisons below also
// check every output byte against the actual production API.
fn decode_reader<R: Read>(reader: R, max_edge: u32) -> Result<DecodedPreview> {
    if max_edge == 0 || max_edge > 2560 {
        bail!("preview max_edge must be in 1..=2560");
    }
    let mut decoder = Decoder::new(reader);
    decoder.read_info().context("JPEG header invalid")?;
    let original = decoder.info().context("missing JPEG dimensions")?;
    let (w, h) = (u32::from(original.width), u32::from(original.height));
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 120_000_000 {
        bail!("JPEG exceeds pixel safety limit");
    }
    let denominator = [1, 2, 4, 8]
        .into_iter()
        .find(|d| w.div_ceil(*d).max(h.div_ceil(*d)) <= max_edge)
        .context("JPEG too large for bounded 1/8 IDCT preview")?;
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
        _ => bail!("Unsupported JPEG color space"),
    };
    Ok(DecodedPreview {
        image,
        original_width: w,
        original_height: h,
        orientation: 1,
        source: "jpeg_idct",
    })
}

fn decode_mode(path: &Path, mode: usize) -> Result<DecodedPreview> {
    let mut output = match mode {
        0 => decode_reader(BufReader::new(File::open(path)?), 1280)?,
        1 => decode_reader(
            BufReader::with_capacity(256 * 1024, File::open(path)?),
            1280,
        )?,
        2 => {
            let file = File::open(path)?;
            let length = file.metadata()?.len();
            if length > INPUT_CAP {
                bail!("microbenchmark input exceeds 256 MiB limit");
            }
            let mut bytes = Vec::with_capacity(length as usize);
            file.take(INPUT_CAP + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > INPUT_CAP {
                bail!("microbenchmark input grew beyond 256 MiB limit");
            }
            decode_reader(bytes.as_slice(), 1280)?
        }
        _ => unreachable!(),
    };
    // Hold EXIF I/O identical across all three modes to isolate decoder input.
    output.orientation = exif::Reader::new()
        .read_from_container(&mut BufReader::new(File::open(path)?))
        .ok()
        .and_then(|e| {
            e.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
        })
        .unwrap_or(1);
    output.image = match output.orientation {
        2 => imageops::flip_horizontal(&output.image),
        3 => imageops::rotate180(&output.image),
        4 => imageops::flip_vertical(&output.image),
        5 => imageops::rotate90(&imageops::flip_vertical(&output.image)),
        6 => imageops::rotate90(&output.image),
        7 => imageops::rotate90(&imageops::flip_horizontal(&output.image)),
        8 => imageops::rotate270(&output.image),
        _ => output.image,
    };
    Ok(output)
}

fn median(samples: &[f64]) -> f64 {
    let mut samples = samples.to_vec();
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

#[test]
#[ignore = "five local JPEGs x three rounds; release only, RAYON_NUM_THREADS=1"]
fn jpeg_read_modes_single_thread() {
    assert!(
        !cfg!(debug_assertions),
        "run this microbenchmark in release mode"
    );
    assert_eq!(std::env::var("RAYON_NUM_THREADS").as_deref(), Ok("1"));
    let directory =
        PathBuf::from(std::env::var_os("IRIS_PROFILE_PHOTOS").expect("IRIS_PROFILE_PHOTOS"));
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg")
                })
        })
        .collect();
    paths.sort();
    paths.truncate(5);
    assert_eq!(paths.len(), 5);
    let orders = [[0, 1, 2], [1, 2, 0], [2, 0, 1]];
    let mut by_mode: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
    let mut per_photo = Vec::new();
    for path in paths {
        // One production reference warms each file. File-cache cold-start speed
        // is not measured; output comparison and hashing are outside all timers.
        let reference = decode_jpeg_preview(&path, 1280).unwrap();
        let mut samples: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
        for order in orders {
            for mode in order {
                let start = Instant::now();
                let candidate = decode_mode(&path, mode).unwrap();
                let elapsed = start.elapsed().as_secs_f64() * 1000.;
                assert_eq!(
                    (
                        candidate.original_width,
                        candidate.original_height,
                        candidate.orientation
                    ),
                    (
                        reference.original_width,
                        reference.original_height,
                        reference.orientation
                    )
                );
                assert_eq!(candidate.image.dimensions(), reference.image.dimensions());
                assert_eq!(
                    candidate.image.as_raw(),
                    reference.image.as_raw(),
                    "pixel mismatch: {} {}",
                    path.display(),
                    MODES[mode]
                );
                samples[mode].push(elapsed);
                by_mode[mode].push(elapsed);
            }
        }
        per_photo.push(serde_json::json!({
            "filename":path.file_name().unwrap().to_string_lossy(),
            "input_bytes":std::fs::metadata(&path).unwrap().len(),
            "original":[reference.original_width, reference.original_height],
            "preview":[reference.image.width(), reference.image.height()],
            "orientation":reference.orientation,
            "pixel_sha256":format!("{:x}", Sha256::digest(reference.image.as_raw())),
            "samples_ms":samples,
            "median_ms":samples.iter().map(|v|median(v)).collect::<Vec<_>>()
        }));
    }
    println!(
        "IRIS_JPEG_READ_PROFILE={}",
        serde_json::json!({
            "modes":MODES,"photos":5,"repetitions_per_photo_per_mode":3,"orders":orders,
            "rayon_num_threads":1,"test_threads":1,"sequential":true,"release":true,
            "warmups":"one production reference per photo","cache":"warm filesystem cache; no eviction",
            "scope":"open/input read/decode/RGB conversion/identical EXIF file read/orientation; comparison and hash excluded",
            "input_memory_cap_bytes":INPUT_CAP,"max_edge":1280,"decoder":"jpeg-decoder 0.3.2 unchanged",
            "all_pixels_dimensions_orientation_identical":true,
            "median_ms":by_mode.iter().map(|v|median(v)).collect::<Vec<_>>(),
            "sum_ms":by_mode.iter().map(|v|v.iter().sum::<f64>()).collect::<Vec<_>>(),
            "per_photo":per_photo
        })
    );
}
