//! Scene-referred, unclipped RAW development. The display-preview path is separate.
use anyhow::{bail, ensure, Context, Result};
use rawler::{
    cfa::{CFAColor, PlaneColor},
    imgop::{
        matrix::multiply,
        sensor::{bayer::ppg::PPGDemosaic, Demosaic, SensorType},
        xyz::Illuminant,
        Rect,
    },
    pixarray::{Color2D, PixF32},
    rawimage::{RawImageData, RawPhotometricInterpretation},
    RawImage, CFA,
};
use serde_json::{json, Value};
use std::io::Write;

/// rawler's DNG sample decoder does not apply camera-calibration chains or
/// required sensor opcodes. Reject these rather than silently miscalibrate.
pub fn validate_calibration(decoder: &dyn rawler::decoders::Decoder) -> Result<()> {
    use rawler::{decoders::WellKnownIFD, tags::DngTag};
    let Some(root) = decoder.ifd(WellKnownIFD::Root)? else {
        return Ok(());
    };
    if root.get_entry(DngTag::DNGVersion).is_none() {
        return Ok(());
    }
    let mut explicit_d65 = false;
    for (matrix, illuminant) in [
        (DngTag::ColorMatrix1, DngTag::CalibrationIlluminant1),
        (DngTag::ColorMatrix2, DngTag::CalibrationIlluminant2),
    ] {
        if root.get_entry_recursive(matrix).is_some()
            && root
                .get_entry_recursive(illuminant)
                .is_some_and(|entry| entry.count() == 1 && entry.force_u16(0) == 21)
        {
            explicit_d65 = true;
        }
    }
    ensure!(
        explicit_d65,
        "DNG float RAW requires explicitly tagged D65 calibration"
    );
    for kind in [WellKnownIFD::Root, WellKnownIFD::Raw] {
        if let Some(ifd) = decoder.ifd(kind)? {
            for tag in [
                DngTag::CameraCalibration1,
                DngTag::CameraCalibration2,
                DngTag::CameraCalibration3,
                DngTag::AnalogBalance,
            ] {
                if let Some(entry) = ifd.get_entry(tag) {
                    validate_identity_calibration(entry, tag == DngTag::AnalogBalance)?;
                }
            }
            for tag in [
                DngTag::LinearizationTable,
                DngTag::BlackLevelDeltaH,
                DngTag::BlackLevelDeltaV,
                DngTag::OpcodeList1,
                DngTag::OpcodeList2,
            ] {
                ensure!(
                    ifd.get_entry(tag).is_none(),
                    "DNG sensor linearization, level deltas or required sensor opcodes unsupported"
                );
            }
        }
    }
    Ok(())
}

const MAGIC: &[u8; 8] = b"IRISF32\0";
pub(super) const MAX_PIXELS: usize = 64_000_000;
// BT.2020 primaries, D65 white, linear-light RGB -> CIE XYZ.
const REC2020_TO_XYZ: [[f32; 3]; 3] = [
    [0.63695806, 0.14461690, 0.16888098],
    [0.26270021, 0.67799807, 0.05930172],
    [0.0, 0.02807269, 1.06098506],
];

pub fn parse_wb(option: Option<&str>) -> Result<Option<[f32; 3]>> {
    let Some(option) = option else {
        return Ok(None);
    };
    let text = option
        .strip_prefix("--wb=")
        .context("expected --wb=r,g,b")?;
    let values = text
        .split(',')
        .map(str::parse::<f32>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let gains: [f32; 3] = values
        .try_into()
        .map_err(|_| anyhow::anyhow!("white balance requires three gains"))?;
    Ok(Some(normalize_wb(gains)?))
}

fn normalize_wb(gains: [f32; 3]) -> Result<[f32; 3]> {
    ensure!(
        gains.iter().all(|v| v.is_finite() && *v > 0.0),
        "invalid white balance gains"
    );
    let normalized = gains.map(|v| v / gains[1]);
    ensure!(
        normalized
            .iter()
            .all(|v| v.is_finite() && *v > 0.0 && *v <= 1024.0),
        "white balance gains exceed safety limit"
    );
    Ok(normalized)
}

fn inverse(matrix: [[f32; 3]; 3]) -> Result<[[f32; 3]; 3]> {
    // Pivoted Gauss-Jordan, f64 arithmetic; reject degenerate calibration.
    let mut rows = [[0.0_f64; 6]; 3];
    for r in 0..3 {
        for c in 0..3 {
            rows[r][c] = f64::from(matrix[r][c]);
        }
        rows[r][r + 3] = 1.0;
    }
    for col in 0..3 {
        let mut pivot = col;
        for r in col + 1..3 {
            if rows[r][col].abs() > rows[pivot][col].abs() {
                pivot = r;
            }
        }
        ensure!(
            rows[pivot][col].is_finite() && rows[pivot][col].abs() > 1e-10,
            "singular camera matrix"
        );
        rows.swap(col, pivot);
        let divisor = rows[col][col];
        for value in &mut rows[col] {
            *value /= divisor;
        }
        let pivot_row = rows[col];
        for (r, row) in rows.iter_mut().enumerate() {
            if r == col {
                continue;
            }
            let factor = row[col];
            for c in 0..6 {
                row[c] -= factor * pivot_row[c];
            }
        }
    }
    let result = std::array::from_fn(|r| std::array::from_fn(|c| rows[r][c + 3] as f32));
    ensure!(
        result
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() <= 1e6),
        "unstable camera matrix"
    );
    Ok(result)
}

fn camera_to_rec2020(xyz_to_camera: [[f32; 3]; 3]) -> Result<[[f32; 3]; 3]> {
    let mut rgb_to_camera = multiply(&xyz_to_camera, &REC2020_TO_XYZ);
    // WB-balanced camera neutral [1,1,1] maps to Rec.2020 D65 [1,1,1].
    for row in &mut rgb_to_camera {
        let sum: f32 = row.iter().sum();
        ensure!(
            sum.is_finite() && sum > 1e-8,
            "invalid camera D65 neutral response"
        );
        for value in row {
            *value /= sum;
        }
    }
    inverse(rgb_to_camera)
}

fn transform(pixel: [f32; 3], wb: [f32; 3], matrix: &[[f32; 3]; 3]) -> [f32; 4] {
    let balanced = std::array::from_fn::<_, 3, _>(|c| pixel[c] * wb[c]);
    let rgb: [f32; 3] = std::array::from_fn(|r| (0..3).map(|c| matrix[r][c] * balanced[c]).sum());
    [rgb[0], rgb[1], rgb[2], 1.0]
}

fn validate_rect(rect: Rect, width: usize, height: usize) -> Result<()> {
    ensure!(
        rect.d.w >= 2
            && rect.d.h >= 2
            && rect.p.x.checked_add(rect.d.w).is_some_and(|v| v <= width)
            && rect.p.y.checked_add(rect.d.h).is_some_and(|v| v <= height),
        "invalid RAW crop bounds"
    );
    Ok(())
}

fn normalize_samples(raw: &RawImage) -> Result<Vec<f32>> {
    let black = &raw.blacklevel;
    ensure!(
        black.cpp == 1
            && ((black.width == 1 && black.height == 1) || (black.width == 2 && black.height == 2))
            && black.levels.len() == black.width * black.height,
        "unsupported black-level repeat pattern"
    );
    ensure!(
        matches!(raw.whitelevel.0.len(), 1 | 4),
        "unsupported white-level pattern"
    );
    let black_values = black.as_vec();
    let white_values = raw.whitelevel.as_vec();
    let length = match &raw.data {
        RawImageData::Integer(values) => values.len(),
        RawImageData::Float(values) => values.len(),
    };
    ensure!(
        length == raw.width * raw.height,
        "RAW sample count mismatch"
    );
    let mut pixels = Vec::with_capacity(length);
    for index in 0..length {
        let value = match &raw.data {
            RawImageData::Integer(values) => f32::from(values[index]),
            RawImageData::Float(values) => values[index],
        };
        let (x, y) = (index % raw.width, index / raw.width);
        let black_value = black_values[(y % black.height) * black.width + x % black.width];
        // rawler's four-value Bayer white level uses the same 2x2 site order.
        let white_value = white_values[if white_values.len() == 1 {
            0
        } else {
            (y % 2) * 2 + x % 2
        }];
        pixels.push(normalize_sample(value, black_value, white_value)?);
    }
    Ok(pixels)
}

fn normalize_sample(value: f32, black: f32, white: f32) -> Result<f32> {
    ensure!(
        value.is_finite() && black.is_finite() && white.is_finite() && white > black,
        "invalid sensor sample or black/white levels"
    );
    let normalized = (value - black) / (white - black);
    ensure!(normalized.is_finite(), "sensor normalization overflow");
    Ok(normalized)
}

fn bilinear_pixel(
    samples: &[f32],
    width: usize,
    cfa: &CFA,
    active: Rect,
    x: usize,
    y: usize,
) -> Result<[f32; 3]> {
    let mut sums = [0.0; 3];
    let mut weights = [0.0; 3];
    let known = cfa.color_at(y, x);
    // Separable tent interpolation; available-neighbor normalization at edges.
    for sy in y.saturating_sub(1).max(active.p.y)..=(y + 1).min(active.p.y + active.d.h - 1) {
        for sx in x.saturating_sub(1).max(active.p.x)..=(x + 1).min(active.p.x + active.d.w - 1) {
            let channel = cfa.color_at(sy, sx);
            let weight = if sx == x { 2.0 } else { 1.0 } * if sy == y { 2.0 } else { 1.0 };
            sums[channel] += samples[sy * width + sx] * weight;
            weights[channel] += weight;
        }
    }
    ensure!(
        weights.iter().all(|v| *v > 0.0),
        "CFA neighborhood lacks RGB samples"
    );
    let mut rgb = std::array::from_fn(|c| sums[c] / weights[c]);
    rgb[known] = samples[y * width + x];
    Ok(rgb)
}

fn demosaic(
    samples: Vec<f32>,
    width: usize,
    height: usize,
    cfa: &CFA,
    colors: &PlaneColor,
    active: Rect,
) -> Result<(Color2D<f32, 3>, &'static str)> {
    // rawler's direct PPG implementation has no range clamp/tone/gamma and
    // shifts CFA phase for ROI. Its interior loops subtract six, so tiny ROIs
    // need an explicit safe fallback. Never invoke RawDevelop's rescale/calibrate.
    let sensor = PixF32::new_with(samples, width, height);
    if active.d.w >= 6 && active.d.h >= 6 {
        Ok((
            PPGDemosaic::new().demosaic(&sensor, cfa, colors, active),
            "rawler_ppg_bayer_0.8.0",
        ))
    } else {
        let mut pixels = Vec::with_capacity(active.d.w * active.d.h);
        for y in active.p.y..active.p.y + active.d.h {
            for x in active.p.x..active.p.x + active.d.w {
                pixels.push(bilinear_pixel(&sensor.data, width, cfa, active, x, y)?);
            }
        }
        Ok((
            Color2D::new_with(pixels, active.d.w, active.d.h),
            "bilinear_tiny_roi_v1",
        ))
    }
}

pub fn develop(raw: RawImage, override_wb: Option<[f32; 3]>) -> Result<()> {
    let count = raw
        .width
        .checked_mul(raw.height)
        .context("RAW dimension overflow")?;
    ensure!(
        count > 0 && count <= MAX_PIXELS && raw.cpp == 1,
        "float RAW requires a bounded single-plane sensor"
    );
    ensure!(
        raw.fuji_rotation_width.is_none(),
        "rotated Fuji sensor unsupported by float RAW"
    );
    let RawPhotometricInterpretation::Cfa(config) = &raw.photometric else {
        bail!("float RAW requires RGB Bayer CFA; linear RAW and monochrome unsupported")
    };
    ensure!(
        config.sensor == SensorType::Bayer
            && config.cfa.width == 2
            && config.cfa.height == 2
            && config.colors.colors == vec![CFAColor::RED, CFAColor::GREEN, CFAColor::BLUE],
        "float RAW supports only standard RGB Bayer planes"
    );
    validate_bayer_pattern(&config.cfa)?;
    let (xyz_to_camera, matrix) = sensor_matrix(&raw)?;
    let wb = camera_white_balance(&raw, override_wb)?;
    let orientation = raw.orientation.to_u16();
    ensure!(
        (1..=8).contains(&orientation),
        "missing or invalid EXIF orientation"
    );
    let (active, crop) = sensor_crop(&raw)?;
    let samples = normalize_samples(&raw)?;
    let (pixels, algorithm) = demosaic(
        samples,
        raw.width,
        raw.height,
        &config.cfa,
        &config.colors,
        active,
    )?;
    let as_shot_gains = camera_white_balance(&raw, None).ok();
    // The normalized sensor was dropped by demosaic; release decoded samples
    // before serialization too. Only the RGB demosaic remains image-sized.
    drop(raw.data);
    let header = json!({
        "version": 1, "width": crop.d.w, "height": crop.d.h, "channels": 4,
        "color_space": "linear_rec2020_d65", "reference_kind": "scene",
        "orientation": orientation, "orientation_applied": false, "source_kind": "raw",
        "alpha": "straight", "sample_format": "f32_le",
        "raw_calibration": { "version": 1, "xyz_to_camera_d65": xyz_to_camera,
            "camera_to_working": matrix, "as_shot_gains": as_shot_gains, "applied_gains": wb },
        "sensor_bit_depth": if (1..=32).contains(&raw.bps) { Some(raw.bps) } else { None },
        "worker_memory_estimate": { "image_buffer_bytes": count as u64 * 20,
            "serialization_scratch_bytes": 65536,
            "compressed_input_limit_bytes": 512_u64*1024*1024,
            "excludes": "decoder scratch, allocator overhead, mapped input, parent renderer",
            "is_hard_limit": false },
        "input_processing": {
            "decoder": "rawler-0.8.0", "normalization": "(sample-black)/(white-black)",
            "black_levels": raw.blacklevel.as_vec(), "black_repeat": [raw.blacklevel.width, raw.blacklevel.height],
            "white_levels": raw.whitelevel.as_vec(), "white_level_order": "scalar_or_2x2_sensor_sites",
            "white_balance": if override_wb.is_some() { "explicit_camera_gains" } else { "as_shot_camera_gains" },
            "wb_gains_rgb": wb, "wb_normalization": "green=1", "demosaic": algorithm,
            "matrix_illuminant": "D65", "camera_to_working_matrix": matrix,
            "matrix_normalization": "D65_neutral_row_sum", "chromatic_adaptation": "none_D65_matrix_required",
            "crop_sensor_xywh": [crop.p.x, crop.p.y, crop.d.w, crop.d.h],
            "tone_mapping": false, "gamma_encoding": false, "clipping": false,
            "exposure_scale": 1.0, "profile_luts_applied": false
        }
    });
    write_frame_pixels(
        &mut std::io::BufWriter::new(std::io::stdout().lock()),
        &header,
        crop.d.w * crop.d.h,
        |index| {
            let y = crop.p.y - active.p.y + index / crop.d.w;
            let x = crop.p.x - active.p.x + index % crop.d.w;
            transform(*pixels.at(y, x), wb, &matrix)
        },
    )
}

fn frame_count(header: &Value) -> Result<usize> {
    let width = header["width"].as_u64().context("missing width")?;
    let height = header["height"].as_u64().context("missing height")?;
    let count = width.checked_mul(height).context("frame size overflow")?;
    ensure!(
        width > 0 && height > 0 && count <= MAX_PIXELS as u64,
        "invalid float frame dimensions"
    );
    Ok(count as usize)
}

fn write_frame_pixels(
    out: &mut impl Write,
    header: &Value,
    count: usize,
    pixel: impl Fn(usize) -> [f32; 4],
) -> Result<()> {
    ensure!(
        frame_count(header)? == count,
        "float frame pixel count mismatch"
    );
    // Two deterministic passes avoid a full RGBA intermediate while retaining
    // the guarantee that all semantic failures occur before writing the magic.
    for index in 0..count {
        ensure!(
            pixel(index).iter().all(|v| v.is_finite()),
            "float frame contains nonfinite samples"
        );
    }
    let bytes = serde_json::to_vec(header)?;
    ensure!(bytes.len() <= 65536, "float frame header exceeds limit");
    // All validation precedes the first stdout byte. I/O failure may truncate a
    // frame; consumers must require successful exit and the exact payload size.
    out.write_all(MAGIC)?;
    out.write_all(&(bytes.len() as u32).to_le_bytes())?;
    out.write_all(&bytes)?;
    let mut chunk = [0_u8; 65536];
    let mut used = 0;
    for index in 0..count {
        for (channel, value) in pixel(index).iter().enumerate() {
            chunk[used + channel * 4..used + channel * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        used += 16;
        if used == chunk.len() {
            out.write_all(&chunk)?;
            used = 0;
        }
    }
    out.write_all(&chunk[..used])?;
    out.flush()?;
    Ok(())
}

#[cfg(test)]
fn write_frame(out: &mut impl Write, header: &Value, pixels: &[[f32; 4]]) -> Result<()> {
    write_frame_pixels(out, header, pixels.len(), |index| pixels[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ppg_preserves_signed_overwhite_channels_and_odd_roi_phase() {
        let channels = [-0.2, 0.50001, 2.25];
        let active = Rect::new(
            rawler::imgop::Point::new(1, 3),
            rawler::imgop::Dim2::new(10, 10),
        );
        for pattern in ["RGGB", "BGGR", "GRBG", "GBRG"] {
            let cfa = CFA::new(pattern);
            let samples = (0..14 * 16)
                .map(|index| channels[cfa.color_at(index / 14, index % 14)])
                .collect();
            let (rgb, algorithm) =
                demosaic(samples, 14, 16, &cfa, &PlaneColor::new("RGB"), active).unwrap();
            assert_eq!(algorithm, "rawler_ppg_bayer_0.8.0");
            for y in 0..active.d.h {
                for x in 0..active.d.w {
                    let pixel = rgb.at(y, x);
                    for c in 0..3 {
                        assert!(
                            (pixel[c] - channels[c]).abs() < 2e-6,
                            "{pattern}: {x},{y}, channel {c}"
                        );
                    }
                    let known = cfa.color_at(y + active.p.y, x + active.p.x);
                    assert_eq!(pixel[known], channels[known]);
                }
            }
        }
    }

    #[test]
    fn tiny_sensor_uses_safe_bilinear_fallback() {
        let active = Rect::new(
            rawler::imgop::Point::new(0, 0),
            rawler::imgop::Dim2::new(2, 4),
        );
        let (pixels, algorithm) = demosaic(
            vec![2.0; 8],
            2,
            4,
            &CFA::new("RGGB"),
            &PlaneColor::new("RGB"),
            active,
        )
        .unwrap();
        assert_eq!(algorithm, "bilinear_tiny_roi_v1");
        assert_eq!(*pixels.at(0, 0), [2.0; 3]);
    }

    #[test]
    fn float_frame_limit_accepts_64mp_and_rejects_larger_before_allocation() {
        assert_eq!(
            frame_count(&json!({"width":8000,"height":8000})).unwrap(),
            64_000_000
        );
        assert!(frame_count(&json!({"width":8001,"height":8000})).is_err());
    }

    #[test]
    fn streaming_transform_matches_fp32_reference_and_checks_last_sample_first() {
        let header = json!({"width":2,"height":1});
        let matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let sensor = [[-0.2, 0.50001, 2.0], [0.3, 0.4, 0.5]];
        let reference = sensor.map(|p| transform(p, [2.0, 1.0, 1.5], &matrix));
        let mut expected = Vec::new();
        write_frame(&mut expected, &header, &reference).unwrap();
        let mut actual = Vec::new();
        write_frame_pixels(&mut actual, &header, 2, |i| {
            transform(sensor[i], [2.0, 1.0, 1.5], &matrix)
        })
        .unwrap();
        assert_eq!(actual, expected);
        let mut invalid = Vec::new();
        assert!(write_frame_pixels(&mut invalid, &header, 2, |i| if i == 1 {
            [f32::NAN; 4]
        } else {
            [0.0; 4]
        })
        .is_err());
        assert!(invalid.is_empty());
    }

    #[test]
    fn sensor_normalization_preserves_black_noise_and_overwhite() {
        assert_eq!(normalize_sample(90.0, 100.0, 200.0).unwrap(), -0.1);
        assert_eq!(normalize_sample(250.0, 100.0, 200.0).unwrap(), 1.5);
        assert!(normalize_sample(1.0, 100.0, 100.0).is_err());
        assert!(normalize_sample(f32::NAN, 0.0, 1.0).is_err());
    }

    #[test]
    fn protocol_preserves_negative_overwhite_and_sub_8bit_precision() {
        let pixels = [[-0.125, 2.5, 0.50001, 1.0]];
        let header = json!({"width":1,"height":1});
        let mut bytes = vec![];
        write_frame(&mut bytes, &header, &pixels).unwrap();
        assert_eq!(&bytes[..8], MAGIC);
        let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 12 + len + 16);
        for (i, sample) in pixels[0].iter().enumerate() {
            assert_eq!(
                &bytes[12 + len + i * 4..16 + len + i * 4],
                &sample.to_le_bytes()
            );
        }
    }

    #[test]
    fn invalid_frame_writes_nothing() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bytes = vec![];
            assert!(write_frame(
                &mut bytes,
                &json!({"width":1,"height":1}),
                &[[value, 0.0, 0.0, 1.0]]
            )
            .is_err());
            assert!(bytes.is_empty());
        }
        let mut bytes = vec![];
        assert!(write_frame(&mut bytes, &json!({"width":2,"height":1}), &[[0.0; 4]]).is_err());
        assert!(bytes.is_empty());
    }

    #[test]
    fn neutral_matrix_preserves_range_without_tone_mapping() {
        let matrix = camera_to_rec2020(inverse(REC2020_TO_XYZ).unwrap()).unwrap();
        for neutral in [-0.2, 0.0, 0.18, 1.0, 4.0] {
            let mapped = transform([neutral; 3], [1.0; 3], &matrix);
            for value in &mapped[..3] {
                assert!((*value - neutral).abs() < 2e-6);
            }
        }
        let mapped = transform([-0.2, 0.50001, 2.0], [1.0; 3], &matrix);
        assert!((mapped[0] + 0.2).abs() < 2e-6 && (mapped[2] - 2.0).abs() < 2e-6);
        assert!(inverse([[0.0; 3]; 3]).is_err());
    }

    #[test]
    fn bilinear_keeps_constant_sensor_channels_including_edges() {
        let cfa = CFA::new("RGGB");
        let channels = [-0.2, 0.50001, 2.0];
        let samples = (0..16)
            .map(|i| channels[cfa.color_at(i / 4, i % 4)])
            .collect::<Vec<_>>();
        let active = Rect::new(
            rawler::imgop::Point::new(0, 0),
            rawler::imgop::Dim2::new(4, 4),
        );
        for y in 0..4 {
            for x in 0..4 {
                let pixel = bilinear_pixel(&samples, 4, &cfa, active, x, y).unwrap();
                for c in 0..3 {
                    assert!((pixel[c] - channels[c]).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn white_balance_is_explicit_finite_and_green_normalized() {
        assert_eq!(parse_wb(Some("--wb=4,2,3")).unwrap(), Some([2.0, 1.0, 1.5]));
        for value in ["--wb=1,0,1", "--wb=NaN,1,1", "--wb=1,1", "--wb=-1,1,1"] {
            assert!(parse_wb(Some(value)).is_err());
        }
    }
}

fn validate_bayer_pattern(cfa: &CFA) -> Result<()> {
    let mut counts = [0; 3];
    for y in 0..2 {
        for x in 0..2 {
            let channel = cfa.color_at(y, x);
            ensure!(channel < 3, "unsupported Bayer color");
            counts[channel] += 1;
        }
    }
    ensure!(
        counts == [1, 2, 1]
            && cfa.color_at(0, 0) != cfa.color_at(1, 0)
            && cfa.color_at(0, 0) != cfa.color_at(0, 1),
        "invalid RGB Bayer pattern"
    );
    Ok(())
}

fn camera_white_balance(raw: &RawImage, override_wb: Option<[f32; 3]>) -> Result<[f32; 3]> {
    let wb = normalize_wb(override_wb.unwrap_or([
        raw.wb_coeffs[0],
        raw.wb_coeffs[1],
        raw.wb_coeffs[2],
    ]))?;
    // There is only one green plane in RGB Bayer; a distinct fourth green gain
    // requires a four-plane pipeline and must not be silently discarded.
    if override_wb.is_none() && raw.wb_coeffs[3].is_finite() && raw.wb_coeffs[3] > 0.0 {
        ensure!(
            (raw.wb_coeffs[3] / raw.wb_coeffs[1] - 1.0).abs() < 1e-4,
            "distinct fourth green white balance unsupported"
        );
    }
    Ok(wb)
}

fn validate_identity_calibration(entry: &rawler::formats::tiff::Entry, analog: bool) -> Result<()> {
    let expected = if analog { 3 } else { 9 };
    ensure!(
        entry.count() as usize == expected,
        "unsupported DNG calibration dimensions"
    );
    for i in 0..expected {
        let target = if analog || i % 4 == 0 { 1.0 } else { 0.0 };
        let value = entry.force_f32(i);
        ensure!(
            value.is_finite() && (value - target).abs() < 1e-6,
            "nonidentity DNG camera calibration/analog balance unsupported"
        );
    }
    Ok(())
}

fn sensor_crop(raw: &RawImage) -> Result<(Rect, Rect)> {
    let full = Rect::new(rawler::imgop::Point::new(0, 0), raw.dim());
    let active = raw.active_area.unwrap_or(full);
    let crop = raw.crop_area.unwrap_or(active);
    validate_rect(active, raw.width, raw.height)?;
    validate_rect(crop, raw.width, raw.height)?;
    ensure!(
        crop.p.x >= active.p.x
            && crop.p.y >= active.p.y
            && crop.p.x + crop.d.w <= active.p.x + active.d.w
            && crop.p.y + crop.d.h <= active.p.y + active.d.h,
        "default crop lies outside active sensor"
    );
    Ok((active, crop))
}

fn sensor_matrix(raw: &RawImage) -> Result<([[f32; 3]; 3], [[f32; 3]; 3])> {
    let (_, flat_matrix) = raw
        .color_matrix_find_first([Illuminant::D65])
        .context("float RAW requires explicit D65 color matrix; no identity fallback")?;
    ensure!(
        flat_matrix.len() == 9 && flat_matrix.iter().all(|v| v.is_finite()),
        "invalid D65 color matrix"
    );
    let xyz_to_camera = std::array::from_fn(|r| std::array::from_fn(|c| flat_matrix[r * 3 + c]));
    let matrix = camera_to_rec2020(xyz_to_camera)?;
    Ok((xyz_to_camera, matrix))
}
