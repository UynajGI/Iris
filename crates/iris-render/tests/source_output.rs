// SPDX-License-Identifier: GPL-3.0-or-later
use image::{ImageDecoder, ImageReader};
use iris_render::{
    color::{self, OutputConfig, OutputSpace, ReferenceKind},
    output::{self, OutputFormat},
    source, Frame, RenderContext,
};
use serde_json::{json, Value};
use std::io::Cursor;

fn context() -> RenderContext {
    RenderContext::new(32 * 1024 * 1024)
}
fn config(space: OutputSpace) -> OutputConfig {
    OutputConfig {
        space,
        gamut_compression: false,
        ..Default::default()
    }
}
fn raw_header() -> Value {
    json!({"version":1,"width":2,"height":1,"channels":4,"color_space":"linear_rec2020_d65","reference_kind":"scene","source_kind":"raw","orientation":1,"orientation_applied":false,"sample_format":"f32_le","alpha":"straight","sensor_bit_depth":14})
}
fn raw_stream(header: &Value, pixels: &[[f32; 4]]) -> Vec<u8> {
    let json = serde_json::to_vec(header).unwrap();
    let mut bytes = b"IRISF32\0".to_vec();
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&json);
    for pixel in pixels {
        for value in pixel {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[test]
fn raw_protocol_preserves_scene_range_and_applies_orientation_once() {
    let mut header = raw_header();
    header["orientation"] = json!(6);
    let pixels = [[-0.125, 2.0, 0.50001, 1.0], [1.5, 0.25, 0.0, 0.5]];
    let decoded =
        source::decode_raw_stream(Cursor::new(raw_stream(&header, &pixels)), &context()).unwrap();
    assert_eq!((decoded.frame.width, decoded.frame.height), (1, 2));
    assert_eq!(decoded.frame.pixels, pixels);
    assert_eq!(decoded.metadata.source_bit_depth, Some(14));
    assert_eq!(decoded.metadata.reference_kind, ReferenceKind::Scene);
    assert!(!decoded.metadata.assumed_srgb);
    header.as_object_mut().unwrap().remove("sensor_bit_depth");
    let unknown =
        source::decode_raw_stream(Cursor::new(raw_stream(&header, &pixels)), &context()).unwrap();
    assert_eq!(unknown.metadata.source_bit_depth, None);
}

#[test]
fn raw_calibration_is_optional_but_invalid_supplied_calibration_fails_closed() {
    let mut header = raw_header();
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    header["raw_calibration"] = json!({"version":1,"xyz_to_camera_d65":identity,"camera_to_working":identity,"as_shot_gains":[2.0,1.0,1.5],"applied_gains":[2.0,1.0,1.5]});
    let decoded = source::decode_raw_stream(
        Cursor::new(raw_stream(&header, &[[0.2, 0.3, 0.4, 1.0]; 2])),
        &context(),
    )
    .unwrap();
    assert_eq!(
        decoded.metadata.raw_calibration.unwrap().as_shot_gains,
        Some([2.0, 1.0, 1.5])
    );
    header["raw_calibration"]["camera_to_working"] = json!(vec![[0.0; 3]; 3]);
    // Calibration validation happens before a frame is allocated/read.
    assert!(source::decode_raw_stream(Cursor::new(raw_stream(&header, &[])), &context()).is_err());
}

#[test]
fn raw_protocol_rejects_interpretation_mismatch_before_payload() {
    for (key, value) in [
        ("version", json!(2)),
        ("channels", json!(3)),
        ("color_space", json!("srgb")),
        ("reference_kind", json!("display")),
        ("sample_format", json!("u16_le")),
        ("alpha", json!("premultiplied")),
        ("orientation_applied", json!(true)),
        ("orientation", json!(0)),
        ("sensor_bit_depth", json!(0)),
        ("width", json!(u32::MAX)),
        ("height", json!(0)),
    ] {
        let mut header = raw_header();
        header[key] = value;
        assert!(
            source::decode_raw_stream(Cursor::new(raw_stream(&header, &[])), &context()).is_err(),
            "accepted {key}"
        );
    }
    let mut bytes = b"IRISF32\0".to_vec();
    bytes.extend_from_slice(&65537_u32.to_le_bytes());
    assert!(source::decode_raw_stream(Cursor::new(bytes), &context()).is_err());
}

#[test]
fn raw_protocol_rejects_nonfinite_truncation_tail_and_budget_excess() {
    let header = raw_header();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let bytes = raw_stream(&header, &[[value, 0.0, 0.0, 1.0]; 2]);
        assert!(source::decode_raw_stream(Cursor::new(bytes), &context()).is_err());
    }
    let bytes = raw_stream(&header, &[[0.5; 4]; 2]);
    assert!(source::decode_raw_stream(Cursor::new(&bytes[..bytes.len() - 1]), &context()).is_err());
    let mut tail = bytes.clone();
    tail.push(0);
    assert!(source::decode_raw_stream(Cursor::new(tail), &context()).is_err());
    assert!(source::decode_raw_stream(Cursor::new(bytes), &RenderContext::new(16)).is_err());
}

#[test]
fn png_preserves_16bit_precision_alpha_and_icc() {
    assert_16bit_precision_alpha_and_icc(OutputFormat::Png16);
}

#[test]
fn tiff_preserves_16bit_precision_alpha_and_icc() {
    assert_16bit_precision_alpha_and_icc(OutputFormat::Tiff16);
}

fn assert_16bit_precision_alpha_and_icc(format: OutputFormat) {
    let pixels = [
        [10001.0 / 65535.0, 20002.0 / 65535.0, 30003.0 / 65535.0, 0.0],
        [10002.0 / 65535.0, 20003.0 / 65535.0, 30004.0 / 65535.0, 0.5],
    ];
    let target = Frame::new(2, 1, pixels.to_vec()).unwrap();
    {
        let bytes =
            output::encode_output(&target, &config(OutputSpace::Srgb), format, &context()).unwrap();
        let mut decoder = ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .unwrap()
            .into_decoder()
            .unwrap();
        let icc = if format == OutputFormat::Tiff16 {
            // image's TIFF ICC getter uses the pixel decode budget for tag
            // Values and silently returns None on tiny images. The file must
            // still contain a valid ICC; Iris source uses bounded raw metadata.
            let mut raw = tiff::decoder::Decoder::new(Cursor::new(bytes.as_slice())).unwrap();
            raw.get_tag_u8_vec(tiff::tags::Tag::IccProfile)
                .expect("TIFF raw IccProfile tag must decode as byte array")
        } else {
            decoder
                .icc_profile()
                .unwrap()
                .unwrap_or_else(|| panic!("{format:?}: missing embedded output ICC"))
        };
        color::validate_icc(&icc).unwrap();
        assert_eq!(decoder.color_type(), image::ColorType::Rgba16);
        assert_eq!(
            decoder.orientation().unwrap(),
            image::metadata::Orientation::NoTransforms
        );
        let raster = image::load_from_memory(&bytes).unwrap().to_rgba16();
        if format == OutputFormat::Tiff16 {
            let mut tags = tiff::decoder::Decoder::new(Cursor::new(&bytes)).unwrap();
            assert_eq!(
                tags.get_tag_u16_vec(tiff::tags::Tag::ExtraSamples).unwrap(),
                vec![2]
            );
            assert_eq!(tags.get_tag_u32(tiff::tags::Tag::Orientation).unwrap(), 1);
        }
        assert_eq!(raster.get_pixel(0, 0).0, [10001, 20002, 30003, 0]);
        assert_eq!(raster.get_pixel(1, 0).0, [10002, 20003, 30004, 32768]);
        let decoded = source::decode(Cursor::new(bytes.as_slice()), &context()).unwrap();
        assert_eq!(decoded.metadata.source_bit_depth, Some(16));
        assert!(decoded.metadata.embedded_icc && !decoded.metadata.assumed_srgb);
        assert!(decoded.frame.pixels[1][0] > decoded.frame.pixels[0][0]);
    }
}

#[test]
fn jpeg_composites_in_linear_light_and_embeds_target_profile() {
    let target = Frame::new(8, 8, vec![[0.0, 0.0, 0.0, 0.5]; 64]).unwrap();
    let bytes = output::encode_output(
        &target,
        &config(OutputSpace::Srgb),
        OutputFormat::Jpeg8 { quality: 100 },
        &context(),
    )
    .unwrap();
    let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();
    // Half black over white is linear 0.5 -> encoded sRGB ~188, not 128.
    for value in decoded.get_pixel(4, 4).0 {
        assert!((i32::from(value) - 188).abs() <= 3);
    }
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .unwrap()
        .into_decoder()
        .unwrap();
    assert!(decoder.icc_profile().unwrap().is_some());
}

#[test]
fn jpeg_background_is_transformed_from_srgb_to_target_primaries() {
    let target = Frame::new(8, 8, vec![[0.9, 0.1, 0.2, 0.0]; 64]).unwrap();
    let mut config = config(OutputSpace::DisplayP3);
    config.background = [1.0, 0.0, 0.0];
    let bytes = output::encode_output(
        &target,
        &config,
        OutputFormat::Jpeg8 { quality: 100 },
        &context(),
    )
    .unwrap();
    let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();
    let srgb = Frame::new(1, 1, vec![[1.0, 0.0, 0.0, 1.0]]).unwrap();
    let expected = color::transform(
        &srgb,
        &color::profile(OutputSpace::Srgb, false).unwrap(),
        &color::profile(OutputSpace::DisplayP3, false).unwrap(),
        &context(),
    )
    .unwrap();
    for c in 0..3 {
        assert!(
            (f32::from(decoded.get_pixel(4, 4).0[c]) - expected.pixels[0][c] * 255.0).abs() < 4.0
        );
    }
}

#[test]
fn jpeg_terminal_compositing_accepts_finite_extreme_encoded_samples() {
    for (pixel, expected) in [
        ([f32::MAX, -f32::MAX, f32::MAX, 0.0], 255),
        ([f32::MAX, f32::MAX, f32::MAX, 1.0], 255),
        ([-f32::MAX, -f32::MAX, -f32::MAX, 1.0], 0),
    ] {
        let frame = Frame::new(8, 8, vec![pixel; 64]).unwrap();
        let bytes = output::encode_output(
            &frame,
            &config(OutputSpace::Srgb),
            OutputFormat::Jpeg8 { quality: 100 },
            &context(),
        )
        .unwrap();
        let rgb = image::load_from_memory(&bytes).unwrap().to_rgb8();
        for value in rgb.get_pixel(4, 4).0 {
            assert!((i32::from(value) - expected).abs() <= 1);
        }
    }
}

#[test]
fn terminal_encoder_does_not_repeat_display_transform() {
    let target = Frame::new(1, 1, vec![[0.25, 0.25, 0.25, 1.0]]).unwrap();
    let bytes = output::encode_output(
        &target,
        &config(OutputSpace::Srgb),
        OutputFormat::Png16,
        &context(),
    )
    .unwrap();
    let sample = image::load_from_memory(&bytes)
        .unwrap()
        .to_rgba16()
        .get_pixel(0, 0)
        .0[0];
    assert_eq!(sample, 16384);
    let cancelled = context();
    cancelled.cancel();
    assert!(output::encode_output(
        &target,
        &config(OutputSpace::Srgb),
        OutputFormat::Png16,
        &cancelled
    )
    .is_err());
    assert!(output::encode_output(
        &target,
        &config(OutputSpace::Srgb),
        OutputFormat::Png16,
        &RenderContext::new(64)
    )
    .is_err());
}

#[test]
fn gray_icc_uses_single_channel_curve_and_preserves_16bit_alpha() {
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
    // lcms2's format/type boundary checks bytes per pixel; [f32;1] is its
    // correctly sized Pod representation of GRAY_FLT, not RGB_FLT.
    assert_eq!(
        lcms2::PixelFormat::GRAY_FLT.bytes_per_pixel(),
        std::mem::size_of::<[f32; 1]>()
    );
    let gray = lcms2::Profile::new_gray(
        &lcms2::CIExyY {
            x: 0.3127,
            y: 0.3290,
            Y: 1.0,
        },
        &lcms2::ToneCurve::new(2.0),
    )
    .unwrap();
    let samples = [32768_u16, 16384, 32769, 65535];
    let raw = samples
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect::<Vec<_>>();
    let mut bytes = Vec::new();
    let mut encoder = PngEncoder::new(&mut bytes);
    encoder.set_icc_profile(gray.icc().unwrap()).unwrap();
    encoder
        .write_image(&raw, 2, 1, ExtendedColorType::La16)
        .unwrap();
    let decoded = source::decode(Cursor::new(bytes), &context()).unwrap();
    assert!(decoded.metadata.embedded_icc && !decoded.metadata.assumed_srgb);
    assert_eq!(decoded.metadata.source_bit_depth, Some(16));
    let expected = (32768.0_f32 / 65535.0).powi(2);
    for channel in &decoded.frame.pixels[0][..3] {
        assert!((*channel - expected).abs() < 0.001);
    }
    assert!((decoded.frame.pixels[0][3] - 16384.0 / 65535.0).abs() < 1e-6);
    assert_eq!(decoded.frame.pixels[1][3], 1.0);
    assert!(decoded.frame.pixels[1][0] > decoded.frame.pixels[0][0]);
}

#[test]
fn gray_profile_on_rgb_pixels_is_rejected_without_guessing() {
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
    let gray = lcms2::Profile::new_gray(
        &lcms2::CIExyY {
            x: 0.3127,
            y: 0.3290,
            Y: 1.0,
        },
        &lcms2::ToneCurve::new(2.0),
    )
    .unwrap();
    let mut bytes = Vec::new();
    let mut encoder = PngEncoder::new(&mut bytes);
    encoder.set_icc_profile(gray.icc().unwrap()).unwrap();
    encoder
        .write_image(&[128, 64, 32], 1, 1, ExtendedColorType::Rgb8)
        .unwrap();
    assert!(source::decode(Cursor::new(bytes), &context()).is_err());
}

#[test]
fn tiny_tiff_reads_icc_larger_than_pixels_and_applies_orientation_once() {
    let profile = color::profile(OutputSpace::DisplayP3, false).unwrap();
    let icc = profile.icc().unwrap();
    let samples = [50000_u16, 8000, 4000, 65535, 10000, 45000, 30000, 32768];
    assert!(icc.len() > samples.len() * 2);
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut encoder = tiff::encoder::TiffEncoder::new(&mut bytes).unwrap();
        let mut image = encoder
            .new_image::<tiff::encoder::colortype::RGBA16>(2, 1)
            .unwrap();
        image
            .encoder()
            .write_tag(tiff::tags::Tag::IccProfile, icc.as_slice())
            .unwrap();
        image
            .encoder()
            .write_tag(tiff::tags::Tag::Orientation, 6_u16)
            .unwrap();
        image
            .encoder()
            .write_tag(tiff::tags::Tag::ExtraSamples, &[2_u16][..])
            .unwrap();
        image.write_data(&samples).unwrap();
    }
    let decoded = source::decode(Cursor::new(bytes.into_inner()), &context()).unwrap();
    assert_eq!((decoded.frame.width, decoded.frame.height), (1, 2));
    assert!(decoded.metadata.embedded_icc && !decoded.metadata.assumed_srgb);
    assert_eq!(decoded.metadata.source_bit_depth, Some(16));
    let encoded = Frame::new(
        1,
        2,
        samples
            .chunks_exact(4)
            .map(|p| {
                [
                    p[0] as f32 / 65535.0,
                    p[1] as f32 / 65535.0,
                    p[2] as f32 / 65535.0,
                    p[3] as f32 / 65535.0,
                ]
            })
            .collect(),
    )
    .unwrap();
    let working = color::profile(OutputSpace::LinearRec2020, true).unwrap();
    let expected = color::transform(&encoded, &profile, &working, &context()).unwrap();
    let wrong = color::transform(
        &encoded,
        &color::profile(OutputSpace::Srgb, false).unwrap(),
        &working,
        &context(),
    )
    .unwrap();
    assert!((expected.pixels[0][0] - wrong.pixels[0][0]).abs() > 0.01);
    for (actual, expected) in decoded.frame.pixels.iter().zip(expected.pixels) {
        for c in 0..4 {
            assert!((actual[c] - expected[c]).abs() < 1e-6);
        }
    }
}
