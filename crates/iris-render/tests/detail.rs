use iris_render::{
    detail::{self, DetailAdjustments},
    Frame, RenderContext,
};
fn run(frame: &Frame, p: DetailAdjustments) -> Frame {
    detail::apply(frame, &p, &RenderContext::default()).unwrap()
}
fn frame(w: u32, h: u32, f: impl Fn(u32, u32) -> [f32; 4]) -> Frame {
    Frame::new(
        w,
        h,
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect(),
    )
    .unwrap()
}

#[test]
fn identity_preserves_hdr_negatives_and_alpha_bits() {
    let input = frame(3, 1, |x, _| [-0.0, -0.2, x as f32 * 2.0, 0.3]);
    let out = run(&input, DetailAdjustments::default());
    for (a, b) in input.pixels.iter().zip(out.pixels) {
        for c in 0..4 {
            assert_eq!(a[c].to_bits(), b[c].to_bits());
        }
    }
}
#[test]
fn flat_fields_are_not_sharpened_and_threshold_suppresses_noise() {
    let flat = frame(7, 7, |_, _| [0.4, 0.2, -0.1, 0.5]);
    let out = run(
        &flat,
        DetailAdjustments {
            sharpening: 2.0,
            texture: 1.0,
            clarity: 1.0,
            ..Default::default()
        },
    );
    for (a, b) in flat.pixels.iter().zip(out.pixels) {
        for c in 0..4 {
            assert!((a[c] - b[c]).abs() < 1e-6);
        }
    }
    let noise = frame(5, 5, |x, y| {
        let v = if (x + y) % 2 == 0 { 0.4 } else { 0.41 };
        [v, v, v, 1.0]
    });
    let out = run(
        &noise,
        DetailAdjustments {
            sharpening: 3.0,
            sharpening_threshold: 0.05,
            ..Default::default()
        },
    );
    assert_eq!(noise.pixels, out.pixels);
}
#[test]
fn sharpening_texture_and_clarity_have_distinct_scales() {
    let input = frame(41, 1, |x, _| {
        let v = if x == 20 { 0.7 } else { 0.3 };
        [v, v, v, 1.0]
    });
    let sharp = run(
        &input,
        DetailAdjustments {
            sharpening: 1.0,
            ..Default::default()
        },
    );
    let texture = run(
        &input,
        DetailAdjustments {
            texture: 1.0,
            ..Default::default()
        },
    );
    let clarity = run(
        &input,
        DetailAdjustments {
            clarity: 1.0,
            ..Default::default()
        },
    );
    assert!(sharp.pixels[20][0] > 0.7);
    assert_eq!(sharp.pixels[18][0], input.pixels[18][0]);
    assert!(texture.pixels[18][0] < 0.3);
    assert!(clarity.pixels[10][0] < 0.3);
    assert_eq!(texture.pixels[10][0], input.pixels[10][0]);
}
#[test]
fn bilateral_denoise_reduces_noise_without_blurring_large_edge() {
    let input = frame(21, 9, |x, y| {
        let base = if x < 10 { 0.2 } else { 0.8 };
        let noise = if (x + y) % 2 == 0 { 0.01 } else { -0.01 };
        [base + noise, base + noise, base + noise, 1.0]
    });
    let out = run(
        &input,
        DetailAdjustments {
            luminance_denoise: 1.0,
            denoise_sigma: 0.05,
            ..Default::default()
        },
    );
    assert!((out.pixels[4 * 21 + 5][0] - 0.2).abs() < 0.008);
    assert!(out.pixels[4 * 21 + 9][0] < 0.25);
    assert!(out.pixels[4 * 21 + 10][0] > 0.75);
}
#[test]
fn chroma_denoise_preserves_luminance() {
    let input = frame(9, 9, |x, y| {
        let d = if (x + y) % 2 == 0 { 0.01 } else { -0.01 };
        [0.4 + d, 0.4 - d * 0.2627 / 0.678, 0.4, 0.7]
    });
    let out = run(
        &input,
        DetailAdjustments {
            chroma_denoise: 1.0,
            ..Default::default()
        },
    );
    let i = 40;
    let lum = |p: [f32; 4]| p[0] * 0.2627 + p[1] * 0.678 + p[2] * 0.0593;
    assert!((lum(input.pixels[i]) - lum(out.pixels[i])).abs() < 1e-6);
    assert!((out.pixels[i][0] - 0.4).abs() < 0.008);
    assert_eq!(out.pixels[i][3], 0.7);
}
#[test]
fn transparent_hidden_rgb_cannot_contaminate_neighbours() {
    let input = frame(9, 9, |x, y| {
        if x == 4 && y == 4 {
            [1000.0, -1000.0, 900.0, 0.0]
        } else {
            [0.4, 0.4, 0.4, 1.0]
        }
    });
    let out = run(
        &input,
        DetailAdjustments {
            sharpening: 2.0,
            texture: 1.0,
            clarity: 1.0,
            luminance_denoise: 1.0,
            chroma_denoise: 1.0,
            ..Default::default()
        },
    );
    for (a, b) in input.pixels.iter().zip(out.pixels) {
        for c in 0..4 {
            assert!((a[c] - b[c]).abs() < 1e-5);
        }
    }
}
#[test]
fn atmospheric_model_removes_and_adds_veiling_light() {
    let input = frame(5, 5, |_, _| [0.6, 0.7, 0.8, 0.5]);
    let removed = run(
        &input,
        DetailAdjustments {
            dehaze: 0.8,
            ..Default::default()
        },
    );
    let added = run(
        &input,
        DetailAdjustments {
            dehaze: -0.8,
            ..Default::default()
        },
    );
    assert!(removed.pixels[0][0] < 0.6);
    assert!(removed.pixels[0][2] - removed.pixels[0][0] > 0.2);
    assert!(added.pixels[0][0] > 0.6);
    assert_eq!(removed.pixels[0][3], 0.5);
}
#[test]
fn invalid_parameters_and_cancellation_are_errors() {
    assert!(DetailAdjustments {
        denoise_sigma: 0.0,
        ..Default::default()
    }
    .validate()
    .is_err());
    assert!(DetailAdjustments {
        clarity: f32::INFINITY,
        ..Default::default()
    }
    .validate()
    .is_err());
    let input = frame(1, 1, |_, _| [0.5; 4]);
    let ctx = RenderContext::default();
    ctx.cancel();
    assert!(detail::apply(&input, &DetailAdjustments::default(), &ctx).is_err());
}

#[test]
fn detail_accounts_for_scratch_memory_before_allocating() {
    let input = frame(1, 1, |_, _| [0.5; 4]);
    let p = DetailAdjustments {
        sharpening: 1.0,
        ..Default::default()
    };
    // One output pixel16 + one expanded tile pixel80 (input/result/scratch).
    assert!(detail::apply(&input, &p, &RenderContext::new(95)).is_err());
    assert!(detail::apply(&input, &p, &RenderContext::new(96)).is_ok());
}

#[test]
fn tiled_detail_fits_a_budget_that_rejects_full_frame_scratch() {
    // The caller owns the input. Old64 B/pixel scratch admission required64MiB;
    // tiled admission needs output16MiB plus at most two expanded tile buffers.
    let input = frame(4096, 256, |_, _| [0.4, 0.3, 0.2, 0.7]);
    let ctx = RenderContext::new(32 * 1024 * 1024);
    let p = DetailAdjustments {
        clarity: 0.5,
        ..Default::default()
    };
    let output = detail::apply(&input, &p, &ctx).unwrap();
    for (a, b) in input.pixels.iter().zip(output.pixels) {
        for c in 0..4 {
            assert!((a[c] - b[c]).abs() < 1e-6);
        }
    }
    assert_eq!(ctx.memory_used(), 0);
}
