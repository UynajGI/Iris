// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    color::{self, OutputSpace, ReferenceKind},
    document::{EditDocument, Operation, SourceBinding},
    graph, Frame, RenderContext,
};
use std::{collections::BTreeMap, sync::Arc};

fn document() -> EditDocument {
    EditDocument::new(SourceBinding {
        photo_id: "1".into(),
        fingerprint: "a".repeat(64),
        width: 2,
        height: 1,
        reference_kind: ReferenceKind::Scene,
        raw_white_balance: None,
    })
}
fn source() -> Arc<Frame> {
    Arc::new(Frame::new(2, 1, vec![[0.18, 0.18, 0.18, 1.0], [-0.1, 2.0, 0.5, 0.5]]).unwrap())
}

fn local_branch() -> EditDocument {
    use iris_render::{document::Node, mask::MaskDefinition};
    let mut doc = document();
    doc.nodes.push(Node {
        id: "mask".into(),
        name: "mask".into(),
        version: 1,
        enabled: true,
        operation: Operation::Mask(MaskDefinition::LinearGradient {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
        }),
        inputs: vec!["basic".into()],
    });
    doc.nodes.push(Node {
        id: "blend".into(),
        name: "blend".into(),
        version: 1,
        enabled: true,
        operation: Operation::Blend { opacity: 1.0 },
        inputs: vec!["basic".into(), "geometry".into(), "mask".into()],
    });
    doc.output_node = "blend".into();
    doc
}

#[test]
fn matching_dimensions_do_not_allow_mismatched_mask_geometry() {
    let mut doc = local_branch();
    if let Operation::Geometry(params) = &mut doc.nodes[3].operation {
        params.rotation_degrees = 180.0;
    }
    assert!(doc
        .validate()
        .unwrap_err()
        .to_string()
        .contains("geometry lineage"));
    doc.nodes
        .iter_mut()
        .find(|n| n.id == "mask")
        .unwrap()
        .inputs = vec!["geometry".into()];
    doc.nodes.last_mut().unwrap().inputs[0] = "geometry".into();
    doc.validate().unwrap();
}

#[test]
fn bypassed_blend_does_not_load_unused_lut_branch() {
    let mut doc = local_branch();
    doc.nodes[3].operation = Operation::Lut {
        resource: "b".repeat(64),
        strength: 1.0,
        domain: iris_render::lut::LutDomain::LinearRec2020,
    };
    doc.nodes.last_mut().unwrap().enabled = false;
    let result =
        graph::render(&doc, source(), &BTreeMap::new(), &RenderContext::default()).unwrap();
    assert_eq!(*result, *source());
}

#[test]
fn default_graph_is_identity_and_keeps_extended_range() {
    let image = source();
    let output = graph::render(
        &document(),
        image.clone(),
        &BTreeMap::new(),
        &RenderContext::default(),
    )
    .unwrap();
    assert_eq!(*image, *output);
}

#[test]
fn graph_parameters_survive_roundtrip_and_do_not_depend_on_panel_layout() {
    let mut doc = document();
    if let Operation::Adjustments(p) = &mut doc.nodes[1].operation {
        p.exposure = 1.0;
    }
    let hash = doc.fingerprint().unwrap();
    doc.node_positions.insert("basic".into(), [400.0, 12.0]);
    doc.nodes[1].name = "任意显示名称".into();
    doc.panel_bindings.clear();
    assert_eq!(hash, doc.fingerprint().unwrap());
    let restored: EditDocument =
        serde_json::from_slice(&serde_json::to_vec(&doc).unwrap()).unwrap();
    let output = graph::render(
        &restored,
        source(),
        &BTreeMap::new(),
        &RenderContext::default(),
    )
    .unwrap();
    assert_eq!(output.pixels[1], [-0.2, 4.0, 1.0, 0.5]);
}

#[test]
fn graph_rejects_cycles_missing_versions_and_illegal_source_changes() {
    let mut doc = document();
    doc.nodes[1].inputs = vec!["geometry".into()];
    assert!(doc.validate().is_err());
    let mut doc = document();
    doc.nodes[1].version = 999;
    assert!(doc.validate().is_err());
    let mut doc = document();
    doc.source.fingerprint = "not a hash".into();
    assert!(doc.validate().is_err());
    let mut doc = document();
    doc.nodes[1].inputs = vec!["deleted".into()];
    assert!(doc.validate().is_err());
}

#[test]
fn cancelled_graph_and_memory_exhaustion_fail_before_returning_pixels() {
    let context = RenderContext::default();
    context.cancel();
    assert!(graph::render(&document(), source(), &BTreeMap::new(), &context).is_err());
    let context = RenderContext::new(1);
    assert!(graph::render(&document(), source(), &BTreeMap::new(), &context).is_err());
    assert_eq!(context.memory_used(), 0);
}

#[test]
fn profile_conversion_retains_float_detail_and_neutral_extended_range() {
    let ctx = RenderContext::default();
    let working = color::profile(OutputSpace::LinearRec2020, true).unwrap();
    let srgb_linear = color::profile(OutputSpace::Srgb, true).unwrap();
    let image = Frame::new(2, 1, vec![[-0.1, -0.1, -0.1, 1.0], [2.0, 2.0, 2.0, 0.4]]).unwrap();
    let converted = color::transform(&image, &working, &srgb_linear, &ctx).unwrap();
    for (expected, actual) in image.pixels.iter().zip(&converted.pixels) {
        for c in 0..3 {
            assert!((expected[c] - actual[c]).abs() < 0.001);
        }
        assert_eq!(expected[3], actual[3]);
    }
    assert!((color::srgb_decode(color::srgb_encode(0.000_321)) - 0.000_321).abs() < 1e-8);
}

#[test]
fn proxy_averages_linear_premultiplied_values() {
    let image = Frame::new(
        2,
        1,
        vec![[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.333_333_34, 1.0]],
    )
    .unwrap();
    let result = image.proxy(1, &RenderContext::default()).unwrap();
    assert_eq!(result.pixels, vec![[0.0, 0.0, 0.333_333_34, 0.5]]);
}

#[test]
fn lut_red_fastest_order_and_outside_domain_policy() {
    let lut = iris_render::lut::CubeLut::parse(
        "LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n",
    )
    .unwrap();
    let p = lut.sample([0.2, 0.4, 0.7]);
    for (a, b) in p.into_iter().zip([0.2, 0.4, 0.7]) {
        assert!((a - b).abs() < 1e-6);
    }
    assert_eq!(lut.sample([-0.5, 4.0, 0.2]), [-0.5, 4.0, 0.2]);
    assert!(iris_render::lut::CubeLut::parse("LUT_3D_SIZE 2\n0 0 NaN").is_err());
}

#[test]
fn blend_endpoints_preserve_hidden_rgb_and_signed_zero() {
    let a = Frame::new(1, 1, vec![[-0.0, -0.25, 4.0, 0.0]]).unwrap();
    let b = Frame::new(1, 1, vec![[2.0, 0.125, -0.0, 0.0]]).unwrap();
    for (weight, expected) in [(0.0, a.pixels[0]), (1.0, b.pixels[0])] {
        let mask = iris_render::mask::Mask {
            width: 1,
            height: 1,
            values: vec![weight],
        };
        let out = graph::blend(&a, &b, &mask, 1.0, &RenderContext::default()).unwrap();
        assert_eq!(out.pixels[0].map(f32::to_bits), expected.map(f32::to_bits));
    }
}

#[test]
fn blend_preserves_subnormal_alpha_and_finite_hdr_color() {
    let alpha = f32::from_bits(1);
    let p = [f32::MAX, -f32::MAX, 0.12345, alpha];
    let a = Frame::new(1, 1, vec![p]).unwrap();
    let mask = iris_render::mask::Mask {
        width: 1,
        height: 1,
        values: vec![0.5],
    };
    let out = graph::blend(&a, &a, &mask, 1.0, &RenderContext::default()).unwrap();
    assert_eq!(out.pixels[0].map(f32::to_bits), p.map(f32::to_bits));
}

#[test]
fn source_only_and_all_default_graphs_share_the_original_allocation() {
    let image =
        Arc::new(Frame::new(2, 1, vec![[-0.0, -0.2, 4.0, 0.0], [0.1, 2.0, 0.3, 0.5]]).unwrap());
    for output in ["source", "geometry"] {
        let mut doc = document();
        doc.output_node = output.into();
        // Exactly one frame fits. Even one extra full-frame clone must fail.
        let ctx = RenderContext::new(image.byte_len());
        let (result, trace) = graph::render_with_executor(
            &doc,
            image.clone(),
            &BTreeMap::new(),
            &ctx,
            &graph::Executor::default(),
        )
        .unwrap();
        assert!(Arc::ptr_eq(&result, &image));
        assert_eq!(
            result.pixels[0].map(f32::to_bits),
            image.pixels[0].map(f32::to_bits)
        );
        assert_eq!((trace.cpu_nodes, trace.gpu_nodes), (0, 0));
        assert_eq!(ctx.memory_used(), image.byte_len());
    }
}

#[test]
fn linear_graph_releases_consumed_source_before_the_next_allocation() {
    let mut doc = document();
    doc.nodes[1].operation = Operation::Adjustments(iris_render::adjustments::Adjustments {
        exposure: 1.0,
        ..Default::default()
    });
    doc.nodes[2].operation = Operation::Adjustments(iris_render::adjustments::Adjustments {
        exposure: -1.0,
        ..Default::default()
    });
    let image = source();
    let old_source = Arc::downgrade(&image);
    let ctx = RenderContext::new(image.byte_len() * 2);
    // Move the only strong source reference. The second operator can fit only
    // when the consumed original is freed, leaving current input plus output.
    let (result, trace) = graph::render_with_executor(
        &doc,
        image,
        &BTreeMap::new(),
        &ctx,
        &graph::Executor::default(),
    )
    .unwrap();
    assert!(old_source.upgrade().is_none());
    assert_eq!(*result, *source());
    assert_eq!((trace.cpu_nodes, trace.gpu_nodes), (2, 0));
    assert_eq!(ctx.memory_used(), result.byte_len());
    drop(result);
    assert_eq!(ctx.memory_used(), 0);
}
