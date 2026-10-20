use iris_render::{
    adjustments::{self, Adjustments, Curve, CurvePoint, HueBand},
    Frame, RenderContext,
};

fn run(pixels: Vec<[f32; 4]>, p: Adjustments) -> Frame {
    let frame = Frame::new(pixels.len() as u32, 1, pixels).unwrap();
    adjustments::apply(&frame, &p, &RenderContext::default()).unwrap()
}
fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}

#[test]
fn identity_is_bit_exact_for_signed_hdr_and_alpha() {
    let pixels = vec![[-0.25, 0.0, 3.2, 0.4], [-0.0, 0.1234567, 1.0001, 0.0]];
    let out = run(pixels.clone(), Adjustments::default());
    for (a, b) in pixels.iter().zip(out.pixels) {
        for c in 0..4 {
            assert_eq!(a[c].to_bits(), b[c].to_bits());
        }
    }
}
#[test]
fn exposure_is_linear_and_preserves_alpha() {
    let p = Adjustments {
        exposure: 2.0,
        ..Default::default()
    };
    let out = run(vec![[-0.1, 0.18, 1.5, 0.25]], p);
    assert_eq!(out.pixels[0], [-0.4, 0.72, 6.0, 0.25]);
}
#[test]
fn contrast_has_fixed_pivot_and_retains_negative_values() {
    let out = run(
        vec![[0.18, 0.09, -0.18, 1.0]],
        Adjustments {
            contrast: 1.0,
            ..Default::default()
        },
    );
    near(out.pixels[0][0], 0.18);
    near(out.pixels[0][1], 0.045);
    near(out.pixels[0][2], -0.18);
}
#[test]
fn endpoint_and_broad_tone_masks_are_distinct() {
    let input = vec![
        [0.015625, 0.015625, 0.015625, 1.0],
        [0.551368, 0.551368, 0.551368, 1.0],
    ];
    let shadows = run(
        input.clone(),
        Adjustments {
            shadows: 1.0,
            ..Default::default()
        },
    );
    let blacks = run(
        input.clone(),
        Adjustments {
            blacks: 1.0,
            ..Default::default()
        },
    );
    let highlights = run(
        input.clone(),
        Adjustments {
            highlights: 1.0,
            ..Default::default()
        },
    );
    let whites = run(
        input.clone(),
        Adjustments {
            whites: 1.0,
            ..Default::default()
        },
    );
    assert!(shadows.pixels[0][0] > blacks.pixels[0][0]);
    assert!(highlights.pixels[1][0] > whites.pixels[1][0]);
}

#[test]
fn tone_extremes_do_not_reverse_a_grayscale_ramp_and_preserve_alpha() {
    let input: Vec<_> = (0..=4096)
        .map(|i| {
            let y = i as f32 / 2048.0;
            [y, y, y, 0.75]
        })
        .collect();
    for parameters in [
        Adjustments {
            highlights: -2.0,
            ..Default::default()
        },
        Adjustments {
            whites: -2.0,
            ..Default::default()
        },
        Adjustments {
            shadows: 2.0,
            ..Default::default()
        },
        Adjustments {
            blacks: 2.0,
            shadows: -2.0,
            highlights: 2.0,
            whites: -2.0,
            ..Default::default()
        },
    ] {
        let output = run(input.clone(), parameters);
        for pair in output.pixels.windows(2) {
            assert!(pair[1][0] > pair[0][0]);
            near(pair[0][0], pair[0][1]);
            near(pair[0][0], pair[0][2]);
            assert_eq!(pair[0][3], 0.75);
        }
    }
}

#[test]
fn ordinary_and_isolated_tone_agree_on_neutral_lightness() {
    use iris_render::adjustments::IsolatedTone;
    let input = vec![
        [0.0, 0.0, 0.0, 1.0],
        [0.01, 0.01, 0.01, 1.0],
        [0.5, 0.5, 0.5, 1.0],
        [3.0, 3.0, 3.0, 1.0],
    ];
    let ordinary = Adjustments {
        shadows: 1.0,
        highlights: -1.5,
        whites: -0.5,
        blacks: 0.3,
        ..Default::default()
    };
    let mut isolated = Adjustments::default();
    isolated.separated.tone = IsolatedTone {
        shadows: 1.0,
        highlights: -1.5,
        whites: -0.5,
        blacks: 0.3,
        ..Default::default()
    };
    let a = run(input.clone(), ordinary);
    let b = run(input, isolated);
    for (a, b) in a.pixels.iter().zip(b.pixels) {
        for c in 0..4 {
            near(a[c], b[c]);
        }
    }
}
#[test]
fn curves_are_monotone_and_extrapolate_hdr() {
    let curve = Curve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.5, y: 0.25 },
            CurvePoint { x: 1.0, y: 1.0 },
        ],
        ..Default::default()
    };
    let out = run(
        vec![[-0.2, 0.25, 1.5, 1.0]],
        Adjustments {
            red_curve: curve.clone(),
            green_curve: curve.clone(),
            blue_curve: curve,
            ..Default::default()
        },
    );
    near(out.pixels[0][0], -0.1);
    near(out.pixels[0][1], 0.125);
    near(out.pixels[0][2], 1.75);
    let invalid = Curve {
        points: vec![CurvePoint { x: 0.0, y: 1.0 }, CurvePoint { x: 1.0, y: 0.0 }],
        ..Default::default()
    };
    assert!(invalid.validate().is_err());
}
#[test]
fn desaturation_preserves_rec2020_luma() {
    let out = run(
        vec![[0.8, 0.2, 0.1, 0.6]],
        Adjustments {
            saturation: -1.0,
            ..Default::default()
        },
    );
    let y = 0.8 * 0.2627 + 0.2 * 0.678 + 0.1 * 0.0593;
    for v in &out.pixels[0][..3] {
        near(*v, y);
    }
    near(out.pixels[0][3], 0.6);
}
#[test]
fn hue_selection_wraps_and_does_not_touch_gray() {
    let mut p = Adjustments::default();
    p.hsl.push(HueBand {
        center: 360.0,
        hue_shift: 120.0,
        ..Default::default()
    });
    let out = run(vec![[0.8, 0.1, 0.1, 1.0], [0.3, 0.3, 0.3, 1.0]], p);
    assert!(out.pixels[0][1] > out.pixels[0][0]);
    assert_eq!(out.pixels[1], [0.3, 0.3, 0.3, 1.0]);
}
#[test]
fn creative_balance_bw_and_parametric_curve_are_real_operators() {
    let source = vec![[0.2, 0.2, 0.2, 1.0]];
    let out = run(
        source.clone(),
        Adjustments {
            temperature: 1.0,
            tint: 1.0,
            ..Default::default()
        },
    );
    assert!(out.pixels[0][0] > out.pixels[0][2]);
    assert!(out.pixels[0][1] < 0.2);
    let mut p = Adjustments::default();
    p.color_balance.shadows = [1.0, 0.0, 0.0];
    let out = run(source.clone(), p);
    assert!(out.pixels[0][0] > out.pixels[0][1]);
    let mut p = Adjustments::default();
    p.tone_curve.darks = 1.0;
    let out = run(source, p);
    assert!(out.pixels[0][0] > 0.2);
    let mut p = Adjustments::default();
    p.black_and_white.enabled = true;
    p.black_and_white.mix = [1.0, 0.0, 0.0];
    assert_eq!(
        run(vec![[0.4, 0.2, 0.1, 1.0]], p).pixels[0],
        [0.4, 0.4, 0.4, 1.0]
    );
}
#[test]
fn vignette_has_spatial_falloff_and_grain_is_repeatable() {
    let frame = Frame::new(9, 9, vec![[0.4, 0.4, 0.4, 0.7]; 81]).unwrap();
    let ctx = RenderContext::default();
    let mut p = Adjustments::default();
    p.vignette.amount = -2.0;
    let out = adjustments::apply(&frame, &p, &ctx).unwrap();
    assert!(out.pixels[0][0] < out.pixels[40][0]);
    near(out.pixels[40][0], 0.4);
    let mut p = Adjustments::default();
    p.grain.amount = 0.05;
    p.grain.seed = 123;
    let a = adjustments::apply(&frame, &p, &ctx).unwrap();
    let b = adjustments::apply(&frame, &p, &ctx).unwrap();
    assert_eq!(a.pixels, b.pixels);
    assert_ne!(a.pixels, frame.pixels);
    p.grain.seed = 124;
    assert_ne!(
        a.pixels,
        adjustments::apply(&frame, &p, &ctx).unwrap().pixels
    );
}
#[test]
fn validation_cancel_and_overflow_fail_explicitly() {
    let mut p = Adjustments {
        exposure: f32::NAN,
        ..Default::default()
    };
    assert!(p.validate().is_err());
    p.exposure = 17.0;
    assert!(p.validate().is_err());
    let frame = Frame::new(1, 1, vec![[f32::MAX, 1.0, 0.0, 1.0]]).unwrap();
    let ctx = RenderContext::default();
    assert!(adjustments::apply(
        &frame,
        &Adjustments {
            exposure: 16.0,
            ..Default::default()
        },
        &ctx
    )
    .is_err());
    ctx.cancel();
    assert!(adjustments::apply(&frame, &Adjustments::default(), &ctx).is_err());
}

#[test]
fn vibrance_favours_muted_colors_and_gray_remains_neutral() {
    let source = vec![
        [0.5, 0.4, 0.4, 1.0],
        [0.9, 0.1, 0.1, 1.0],
        [0.3, 0.3, 0.3, 1.0],
    ];
    let out = run(
        source.clone(),
        Adjustments {
            vibrance: 1.0,
            ..Default::default()
        },
    );
    let relative = |i: usize| (out.pixels[i][0] - out.pixels[i][1]) / (source[i][0] - source[i][1]);
    assert!(relative(0) > relative(1));
    assert_eq!(out.pixels[2], source[2]);
}

#[test]
fn point_operations_preserve_hidden_rgb_and_enforce_memory_limit() {
    let input = Frame::new(1, 1, vec![[f32::MAX, -1.0, 9.0, 0.0]]).unwrap();
    let p = Adjustments {
        exposure: 16.0,
        ..Default::default()
    };
    let out = adjustments::apply(&input, &p, &RenderContext::default()).unwrap();
    assert_eq!(out.pixels, input.pixels);
    assert!(adjustments::apply(&input, &p, &RenderContext::new(15)).is_err());
}

#[test]
fn separated_controls_persist_validate_and_keep_transparent_pixels() {
    let old: Adjustments = serde_json::from_str("{}").unwrap();
    assert!(old.separated.is_default());
    assert!(serde_json::to_value(&old)
        .unwrap()
        .get("separated")
        .is_none());
    let mut p = old;
    p.separated.tone.exposure = 1.0;
    p.separated.color.hue = 30.0;
    let json = serde_json::to_string(&p).unwrap();
    let loaded: Adjustments = serde_json::from_str(&json).unwrap();
    assert_eq!(p, loaded);
    let source = vec![[0.4, 0.2, 0.1, 0.7], [-0.3, 2.0, 4.0, 0.0]];
    let out = run(source.clone(), p.clone());
    assert_ne!(source[0], out.pixels[0]);
    assert_eq!(source[0][3], out.pixels[0][3]);
    assert_eq!(source[1], out.pixels[1]);
    p.separated.color.hue = f32::NAN;
    assert!(p.validate().is_err());
    p.separated.color.hue = 181.0;
    assert!(p.validate().is_err());
}
