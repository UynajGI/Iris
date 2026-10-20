// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    color::ReferenceKind,
    document::{EditDocument, Node, Operation, SourceBinding},
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
fn node(id: &str, operation: Operation, inputs: &[&str]) -> Node {
    Node {
        id: id.into(),
        name: id.into(),
        version: 1,
        enabled: true,
        operation,
        inputs: inputs.iter().map(|v| (*v).into()).collect(),
    }
}
#[test]
fn key_is_separate_from_rgb_and_final_alpha_is_applied() {
    let mut doc = document();
    doc.nodes
        .push(node("key", Operation::AlphaMask, &["source"]));
    doc.nodes
        .push(node("output", Operation::SetAlpha, &["geometry", "key"]));
    doc.output_node = "output".into();
    let input =
        Arc::new(Frame::new(2, 1, vec![[2.0, -0.1, 0.4, 0.5], [0.2, 0.3, 0.4, 1.0]]).unwrap());
    let result = graph::render(
        &doc,
        input.clone(),
        &BTreeMap::new(),
        &RenderContext::new(1_000_000),
    )
    .unwrap();
    assert_eq!(result.pixels[0], [2.0, -0.1, 0.4, 0.25]);
    assert_eq!(result.pixels[1], input.pixels[1]);
    // An image cannot be silently accepted at a Key input.
    doc.nodes.last_mut().unwrap().inputs[1] = "source".into();
    assert!(doc.validate().is_err());
}
#[test]
fn key_mix_preserves_coverage_and_rejects_cycles() {
    let mut doc = document();
    doc.nodes
        .push(node("alpha", Operation::AlphaMask, &["source"]));
    doc.nodes.push(node(
        "key",
        Operation::KeyMix {
            gain: 0.5,
            invert: false,
        },
        &["source", "alpha", "alpha"],
    ));
    doc.nodes
        .push(node("output", Operation::SetAlpha, &["source", "key"]));
    doc.output_node = "output".into();
    let input = Arc::new(Frame::new(2, 1, vec![[0.2, 0.3, 0.4, 0.5]; 2]).unwrap());
    let result = graph::render(
        &doc,
        input,
        &BTreeMap::new(),
        &RenderContext::new(1_000_000),
    )
    .unwrap();
    assert_eq!(result.pixels[0][3], 0.0625);
    doc.nodes.iter_mut().find(|n| n.id == "key").unwrap().inputs[1] = "key".into();
    assert!(doc.validate().is_err());
}
