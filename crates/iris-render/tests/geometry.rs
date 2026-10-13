// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    geometry::{self, Geometry},
    Frame, RenderContext,
};

#[test]
fn identity_is_exact_and_crop_preserves_pixel_anchors() {
    let frame = Frame::new(4, 2, (0..8).map(|i| [i as f32, -0.2, 1.2, 0.5]).collect()).unwrap();
    let context = RenderContext::default();
    assert_eq!(
        geometry::apply(&frame, &Geometry::default(), &context)
            .unwrap()
            .pixels,
        frame.pixels
    );
    let params = Geometry {
        crop: [0.25, 0.0, 0.5, 1.0],
        ..Geometry::default()
    };
    let output = geometry::apply(&frame, &params, &context).unwrap();
    assert_eq!((output.width, output.height), (2, 2));
    assert_eq!(
        output.pixels,
        vec![
            frame.pixels[1],
            frame.pixels[2],
            frame.pixels[5],
            frame.pixels[6]
        ]
    );
}

#[test]
fn resampling_does_not_leak_hidden_transparent_rgb() {
    let frame = Frame::new(2, 1, vec![[1.0, 0.0, 0.0, 1.0], [0.0, 100.0, 0.0, 0.0]]).unwrap();
    let mut params = Geometry::default();
    params.perspective[2] = 0.25;
    let output = geometry::apply(&frame, &params, &RenderContext::default()).unwrap();
    assert_eq!(output.pixels[0], [1.0, 0.0, 0.0, 0.5]);
    assert_eq!(output.pixels[1], [0.0; 4]);
}

#[test]
fn rotation_mapping_and_malformed_geometry() {
    let params = Geometry {
        rotation_degrees: 90.0,
        ..Geometry::default()
    };
    let p = params.map_to_input([0.75, 0.5], 2, 2).unwrap();
    assert!((p[0] - 0.5).abs() < 1e-12 && (p[1] - 0.25).abs() < 1e-12);
    assert!(Geometry {
        crop: [0.9, 0.0, 0.5, 1.0],
        ..Geometry::default()
    }
    .validate()
    .is_err());
    assert!(Geometry {
        perspective: [0.0; 9],
        ..Geometry::default()
    }
    .validate()
    .is_err());
    assert!(Geometry {
        radial: [f32::NAN, 0.0],
        ..Geometry::default()
    }
    .validate()
    .is_err());
}

#[test]
fn budget_and_cancellation_apply_even_to_identity() {
    let frame = Frame::new(2, 1, vec![[0.5, 0.5, 0.5, 1.0]; 2]).unwrap();
    assert!(geometry::apply(&frame, &Geometry::default(), &RenderContext::new(16)).is_err());
    let context = RenderContext::default();
    context.cancel();
    assert!(geometry::apply(&frame, &Geometry::default(), &context).is_err());
    let empty = Frame {
        width: 0,
        height: 0,
        pixels: vec![],
    };
    assert!(geometry::apply(&empty, &Geometry::default(), &RenderContext::default()).is_err());
}

#[test]
fn optical_vignette_preserves_center_and_lifts_edge_without_clipping_hdr() {
    let frame = Frame::new(3, 3, vec![[2.0, 1.0, 0.5, 0.8]; 9]).unwrap();
    let params = Geometry {
        vignette_stops: 1.0,
        ..Geometry::default()
    };
    let out = geometry::apply(&frame, &params, &RenderContext::default()).unwrap();
    assert_eq!(out.pixels[4], frame.pixels[4]);
    assert!(out.pixels[0][0] > 3.0);
    assert_eq!(out.pixels[0][3], 0.8);
}
