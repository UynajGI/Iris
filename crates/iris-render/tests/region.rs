// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    geometry::{self, Geometry},
    region::{extract, PixelRect},
    Frame, RenderContext,
};

fn fixture() -> Frame {
    Frame::new(
        8,
        6,
        (0..48)
            .map(|i| [i as f32 / 7.0 - 1.0, i as f32 / 49.0, 2.0, 1.0])
            .collect(),
    )
    .unwrap()
}

#[test]
fn extraction_is_one_to_one_and_preserves_float_range() {
    let frame = fixture();
    let rect = PixelRect {
        x: 2,
        y: 1,
        width: 3,
        height: 2,
    };
    let result = extract(&frame, rect, &RenderContext::default()).unwrap();
    assert_eq!((result.width, result.height), (3, 2));
    assert_eq!(
        result.pixels,
        [frame.pixels[10..13].to_vec(), frame.pixels[18..21].to_vec()].concat()
    );
    assert_eq!(result.pixels[0][2], 2.0);
}

#[test]
fn coordinates_are_after_crop_and_rotation() {
    let context = RenderContext::default();
    let frame = fixture();
    let geometry = Geometry {
        crop: [0.25, 0.0, 0.5, 1.0],
        rotation_degrees: 180.0,
        ..Default::default()
    };
    let edited = geometry::apply(&frame, &geometry, &context).unwrap();
    assert_eq!((edited.width, edited.height), (4, 6));
    let rect = PixelRect {
        x: 1,
        y: 2,
        width: 2,
        height: 2,
    };
    let result = extract(&edited, rect, &context).unwrap();
    assert_eq!(
        result.pixels,
        [
            edited.pixels[9..11].to_vec(),
            edited.pixels[13..15].to_vec()
        ]
        .concat()
    );
    // The crop center at final (1,2) maps to source (4,3) after 180 degrees.
    for c in 0..4 {
        assert!((result.pixels[0][c] - frame.pixels[28][c]).abs() < 1e-5);
    }
}

#[test]
fn invalid_bounds_and_cancelled_or_insufficient_budget_fail() {
    let frame = fixture();
    for rect in [
        PixelRect {
            x: 8,
            y: 0,
            width: 1,
            height: 1,
        },
        PixelRect {
            x: u32::MAX,
            y: 0,
            width: 2,
            height: 1,
        },
        PixelRect {
            x: 0,
            y: 0,
            width: 0,
            height: 1,
        },
        PixelRect {
            x: 0,
            y: 0,
            width: 2049,
            height: 1,
        },
        PixelRect {
            x: 0,
            y: 0,
            width: 2048,
            height: 2048,
        },
    ] {
        assert!(extract(&frame, rect, &RenderContext::default()).is_err());
    }
    let rect = PixelRect {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };
    assert!(extract(&frame, rect, &RenderContext::new(63)).is_err());
    let context = RenderContext::default();
    context.cancel();
    assert!(extract(&frame, rect, &context).is_err());
}
