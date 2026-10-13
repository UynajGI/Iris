// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    mask::{self, BrushStroke, MaskDefinition, MaskOperation},
    Frame, RenderContext,
};

fn fixture() -> Frame {
    Frame::new(5, 1, vec![[0.5, 0.5, 0.5, 1.0]; 5]).unwrap()
}
fn gradient() -> MaskDefinition {
    MaskDefinition::LinearGradient {
        start: [0.0, 0.0],
        end: [1.0, 0.0],
    }
}

#[test]
fn gradients_use_pixel_centers_and_algebra_is_bounded() {
    let frame = fixture();
    let ctx = RenderContext::default();
    let original = mask::render(&frame, &gradient(), &ctx).unwrap();
    assert_eq!(original.values, vec![0.1, 0.3, 0.5, 0.7, 0.9]);
    let inverse = MaskDefinition::Invert {
        mask: Box::new(gradient()),
    };
    let sum = MaskDefinition::Combine {
        operation: MaskOperation::Add,
        left: Box::new(gradient()),
        right: Box::new(inverse),
    };
    assert!(mask::render(&frame, &sum, &ctx)
        .unwrap()
        .values
        .iter()
        .all(|v| (*v - 1.0).abs() < 1e-6));
    let difference = MaskDefinition::Combine {
        operation: MaskOperation::Subtract,
        left: Box::new(gradient()),
        right: Box::new(gradient()),
    };
    assert_eq!(
        mask::render(&frame, &difference, &ctx).unwrap().values,
        vec![0.0; 5]
    );
}

#[test]
fn brush_polyline_has_continuous_coverage_and_soft_edges() {
    let frame = Frame::new(5, 5, vec![[0.0, 0.0, 0.0, 1.0]; 25]).unwrap();
    let recipe = MaskDefinition::Brush {
        strokes: vec![BrushStroke {
            points: vec![[0.1, 0.5], [0.9, 0.5]],
            radius: 0.4,
            hardness: 0.0,
            flow: 0.5,
        }],
    };
    let mask = mask::render(&frame, &recipe, &RenderContext::default()).unwrap();
    assert!((mask.values[12] - 0.5).abs() < 1e-6);
    assert!((mask.values[7] - 0.25).abs() < 1e-6);
    assert!(mask.values[2] < 1e-6);
}

#[test]
fn range_feather_and_invalid_recipes() {
    let frame = Frame::new(
        3,
        1,
        vec![
            [0.5, 0.5, 0.5, 1.0],
            [0.75, 0.75, 0.75, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        ],
    )
    .unwrap();
    let recipe = MaskDefinition::LuminanceRange {
        low: 0.0,
        high: 0.5,
        feather: 0.5,
    };
    let result = mask::render(&frame, &recipe, &RenderContext::default()).unwrap();
    for (a, b) in result.values.iter().zip([1.0, 0.5, 0.0]) {
        assert!((*a - b).abs() < 1e-6);
    }
    assert!(MaskDefinition::LinearGradient {
        start: [0.0, 0.0],
        end: [0.0, 0.0]
    }
    .validate()
    .is_err());
    let mut nested = gradient();
    for _ in 0..18 {
        nested = MaskDefinition::Invert {
            mask: Box::new(nested),
        };
    }
    assert!(nested.validate().is_err());
    let ctx = RenderContext::default();
    ctx.cancel();
    assert!(mask::render(&frame, &recipe, &ctx).is_err());
}

#[test]
fn color_range_radial_and_memory_admission() {
    let frame = fixture();
    let color = MaskDefinition::ColorRange {
        color: [0.5; 3],
        radius: 0.0,
        feather: 0.1,
    };
    assert_eq!(
        mask::render(&frame, &color, &RenderContext::default())
            .unwrap()
            .values,
        vec![1.0; 5]
    );
    let radial = MaskDefinition::RadialGradient {
        center: [0.5, 0.5],
        radii: [0.4, 0.4],
        hardness: 0.0,
    };
    let out = mask::render(&frame, &radial, &RenderContext::default()).unwrap();
    assert_eq!(out.values[2], 1.0);
    assert!((out.values[1] - 0.5).abs() < 1e-6);
    assert!(out.values[0] < 1e-6);
    assert!(mask::render(&frame, &color, &RenderContext::new(1)).is_err());
}
