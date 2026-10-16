// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    adjustments::{self, Adjustments, Curve, CurveInterpolation, CurvePoint},
    Frame, RenderContext,
};
use serde::Deserialize;

fn render(curve: Curve, values: &[f32]) -> Vec<[f32; 4]> {
    let input = Frame::new(
        values.len() as u32,
        1,
        values.iter().map(|x| [*x, *x, *x, 1.0]).collect(),
    )
    .unwrap();
    let params = Adjustments {
        master_curve: curve,
        ..Default::default()
    };
    adjustments::apply(&input, &params, &RenderContext::default())
        .unwrap()
        .pixels
}

#[derive(Deserialize)]
struct Fixture {
    name: String,
    curve: Curve,
    samples: Vec<[f32; 2]>,
}

#[test]
fn renderer_matches_shared_analytic_curve_fixtures() {
    let cases: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/curve-interpolation.json")).unwrap();
    for case in cases {
        // Alpha must be nonzero: transparent pixels intentionally bypass adjustments.
        let values: Vec<_> = case.samples.iter().map(|s| s[0]).collect();
        let input = Frame::new(
            values.len() as u32,
            1,
            values.iter().map(|x| [*x, *x, *x, 0.7]).collect(),
        )
        .unwrap();
        let params = Adjustments {
            master_curve: case.curve,
            ..Default::default()
        };
        let output = adjustments::apply(&input, &params, &RenderContext::default()).unwrap();
        for (pixel, sample) in output.pixels.iter().zip(case.samples) {
            for value in &pixel[..3] {
                assert!(
                    (value - sample[1]).abs() < 1e-6,
                    "{}: {} => {value}, expected {}",
                    case.name,
                    sample[0],
                    sample[1]
                );
            }
            assert_eq!(pixel[3], 0.7);
        }
    }
}

#[test]
fn legacy_serialization_remains_unchanged_and_unknown_interpolation_is_rejected() {
    let legacy = serde_json::json!({"points":[{"x":0.0,"y":0.0},{"x":1.0,"y":1.0}]});
    let curve: Curve = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(curve.interpolation, CurveInterpolation::Linear);
    assert_eq!(serde_json::to_value(&curve).unwrap(), legacy);
    let cubic = Curve {
        interpolation: CurveInterpolation::MonotoneCubic,
        ..curve
    };
    assert_eq!(
        serde_json::from_value::<Curve>(serde_json::to_value(&cubic).unwrap()).unwrap(),
        cubic
    );
    assert!(serde_json::from_value::<Curve>(
        serde_json::json!({"points":[],"interpolation":"spline"})
    )
    .is_err());
}

#[test]
fn cubic_master_precedes_individual_channel_curves() {
    let cases: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/curve-interpolation.json")).unwrap();
    let params = Adjustments {
        master_curve: cases[0].curve.clone(),
        red_curve: Curve {
            points: vec![
                CurvePoint { x: 0.0, y: 0.0 },
                CurvePoint { x: 0.5, y: 0.25 },
                CurvePoint { x: 1.0, y: 1.0 },
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    let input = Frame::new(1, 1, vec![[0.5, 0.5, 0.5, 0.4]]).unwrap();
    let output = adjustments::apply(&input, &params, &RenderContext::default()).unwrap();
    assert_eq!(output.pixels[0], [0.1875, 0.375, 0.375, 0.4]);
}

#[test]
fn cubic_preserves_knots_plateaus_and_monotonicity_for_uneven_spacing() {
    let curve = Curve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.05 },
            CurvePoint { x: 0.0002, y: 0.2 },
            CurvePoint { x: 0.2, y: 0.2 },
            CurvePoint { x: 0.8, y: 0.9 },
            CurvePoint { x: 1.0, y: 0.95 },
        ],
        interpolation: CurveInterpolation::MonotoneCubic,
    };
    let values: Vec<_> = curve
        .points
        .windows(2)
        .flat_map(|p| (0..=256).map(move |i| p[0].x + (p[1].x - p[0].x) * i as f32 / 256.0))
        .collect();
    // Use opaque input so x=0 is also processed.
    let input = Frame::new(
        values.len() as u32,
        1,
        values.iter().map(|x| [*x, *x, *x, 1.0]).collect(),
    )
    .unwrap();
    let params = Adjustments {
        master_curve: curve.clone(),
        ..Default::default()
    };
    let output = adjustments::apply(&input, &params, &RenderContext::default()).unwrap();
    for (i, segment) in output.pixels.chunks(257).enumerate() {
        let low = curve.points[i].y;
        let high = curve.points[i + 1].y;
        assert!((segment[0][0] - low).abs() < 1e-6);
        assert!((segment[256][0] - high).abs() < 1e-6);
        for pixel in segment {
            assert!(pixel[0] >= low - 1e-6 && pixel[0] <= high + 1e-6);
        }
        for pair in segment.windows(2) {
            assert!(pair[0][0] <= pair[1][0] + 1e-6);
        }
    }
}

#[test]
fn cubic_first_derivative_is_continuous_at_internal_knots() {
    let curve = Curve {
        points: vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.5, y: 0.375 },
            CurvePoint { x: 1.0, y: 1.0 },
        ],
        interpolation: CurveInterpolation::MonotoneCubic,
    };
    let out = render(curve, &[0.499, 0.5, 0.501]);
    let left = (out[1][0] - out[0][0]) / 0.001;
    let right = (out[2][0] - out[1][0]) / 0.001;
    assert!((left - right).abs() < 0.01, "left={left}, right={right}");
    assert!((left - 0.9375).abs() < 0.01);
}
