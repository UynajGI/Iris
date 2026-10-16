// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    adjustments::{self, Adjustments, ColorBalance},
    Frame, RenderContext,
};

fn render(balance: ColorBalance, pixels: Vec<[f32; 4]>) -> Vec<[f32; 4]> {
    let frame = Frame::new(pixels.len() as u32, 1, pixels).unwrap();
    adjustments::apply(
        &frame,
        &Adjustments {
            color_balance: balance,
            ..Default::default()
        },
        &RenderContext::default(),
    )
    .unwrap()
    .pixels
}

#[test]
fn legacy_color_balance_json_retains_its_canonical_shape() {
    let json = serde_json::json!({
        "shadows": [0.25, -0.5, 0.0], "midtones": [0.0, 0.0, 0.0],
        "highlights": [0.0, 0.0, 0.75]
    });
    let decoded: ColorBalance = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(decoded.global, [0.0; 3]);
    assert_eq!(decoded.blending, 0.5);
    assert_eq!(decoded.balance, 0.0);
    assert_eq!(serde_json::to_value(&decoded).unwrap(), json);
    let changed = ColorBalance {
        global: [0.1, -0.2, 0.3],
        blending: 0.8,
        balance: -0.4,
        ..decoded
    };
    assert_eq!(
        serde_json::from_value::<ColorBalance>(serde_json::to_value(&changed).unwrap()).unwrap(),
        changed
    );
}

#[test]
fn global_gains_apply_to_negative_and_hdr_values_without_changing_alpha() {
    let pixels = vec![
        [0.125, 0.25, 0.5, 0.7],
        [2.0, -0.5, 4.0, 1.0],
        [-0.2, 0.3, 0.4, 0.0],
    ];
    let output = render(
        ColorBalance {
            global: [1.0, -1.0, 0.0],
            ..Default::default()
        },
        pixels.clone(),
    );
    for (input, actual) in pixels.iter().zip(output) {
        let expected = if input[3] == 0.0 {
            *input
        } else {
            [input[0] * 2.0, input[1] * 0.5, input[2], input[3]]
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn range_weights_match_independently_derived_stops_and_balance_direction() {
    for (blending, balance, weights) in [
        (0.5, 0.0, [0.75, 0.25, 0.0]),
        (0.0, 0.0, [81.0 / 82.0, 1.0 / 82.0, 0.0]),
        (1.0, 0.0, [49.0 / 64.0, 7.0 / 32.0, 1.0 / 64.0]),
        (0.5, 0.5, [0.25, 0.75, 0.0]),
        (0.5, -0.5, [1.0, 0.0, 0.0]),
    ] {
        let output = render(
            ColorBalance {
                shadows: [1.0, 0.0, 0.0],
                midtones: [0.0, 1.0, 0.0],
                highlights: [0.0, 0.0, 1.0],
                global: [0.5, -0.5, 1.0],
                blending,
                balance,
            },
            vec![[0.125; 4]],
        );
        for c in 0..3 {
            let stops: f64 = weights[c] + [0.5, -0.5, 1.0][c];
            assert!((output[0][c] as f64 - 0.125 * stops.exp2()).abs() < 1e-6);
        }
        assert_eq!(output[0][3], 0.125);
    }
}

#[test]
fn blending_and_balance_without_any_wheel_adjustment_are_exact_identity() {
    let pixels = vec![[0.3, -0.5, 2.0, 0.8], [0.01, 0.02, 0.03, 1.0]];
    let actual = render(
        ColorBalance {
            blending: 1.0,
            balance: -1.0,
            ..Default::default()
        },
        pixels.clone(),
    );
    assert_eq!(actual, pixels);
}

#[test]
fn full_blending_retains_distinct_shadow_and_highlight_influence() {
    let output = render(
        ColorBalance {
            shadows: [1.0, 0.0, 0.0],
            highlights: [0.0, 0.0, 1.0],
            blending: 1.0,
            ..Default::default()
        },
        vec![[0.125, 0.125, 0.125, 1.0], [0.875, 0.875, 0.875, 1.0]],
    );
    assert!(output[0][0] > output[0][2]);
    assert!(output[1][2] > output[1][0]);
}

#[test]
fn global_color_tints_monochrome_after_the_existing_black_and_white_mix() {
    let input = Frame::new(1, 1, vec![[0.2, 0.4, 0.6, 0.7]]).unwrap();
    let mut params = Adjustments::default();
    params.black_and_white.enabled = true;
    params.color_balance.global = [1.0, -1.0, 0.0];
    let output = adjustments::apply(&input, &params, &RenderContext::default()).unwrap();
    let gray = 0.2627 * input.pixels[0][0] as f64
        + 0.6780 * input.pixels[0][1] as f64
        + 0.0593 * input.pixels[0][2] as f64;
    for (c, gain) in [2.0, 0.5, 1.0].into_iter().enumerate() {
        assert!((output.pixels[0][c] as f64 - gray * gain).abs() < 1e-6);
    }
    assert_eq!(output.pixels[0][3], 0.7);
}

#[test]
fn invalid_grading_controls_are_rejected_before_rendering() {
    for balance in [
        ColorBalance {
            global: [2.01, 0.0, 0.0],
            ..Default::default()
        },
        ColorBalance {
            global: [f32::NAN, 0.0, 0.0],
            ..Default::default()
        },
        ColorBalance {
            blending: -0.01,
            ..Default::default()
        },
        ColorBalance {
            blending: 1.01,
            ..Default::default()
        },
        ColorBalance {
            balance: -1.01,
            ..Default::default()
        },
        ColorBalance {
            balance: f32::INFINITY,
            ..Default::default()
        },
    ] {
        assert!(Adjustments {
            color_balance: balance,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}
