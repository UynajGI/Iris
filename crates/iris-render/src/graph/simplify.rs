// SPDX-License-Identifier: GPL-3.0-or-later
//! Execution-only aliases. Persisted user nodes and connections remain untouched.
use super::*;
use crate::mask::MaskDefinition;

fn constant_mask(mask: &MaskDefinition) -> Option<f32> {
    match mask {
        MaskDefinition::Brush { strokes } if strokes.is_empty() => Some(0.0),
        MaskDefinition::Invert { mask } => constant_mask(mask).map(|v| 1.0 - v),
        MaskDefinition::Opacity { mask, opacity } => constant_mask(mask).map(|v| v * opacity),
        _ => None,
    }
}

pub(super) fn executable(document: &EditDocument, plan: &Plan) -> EditDocument {
    let mut constants: BTreeMap<String, f32> = BTreeMap::new();
    let mut result = document.clone();
    for &index in &plan.order {
        let node = &document.nodes[index];
        let value = match &node.operation {
            Operation::Mask(mask) => constant_mask(mask),
            Operation::KeyMix { gain, invert } => constants
                .get(&node.inputs[1])
                .zip(constants.get(&node.inputs[2]))
                .map(|(a, b)| {
                    let key = a * b;
                    (if *invert { 1.0 - key } else { key }) * gain
                }),
            _ => None,
        };
        if let Some(value) = value {
            constants.insert(node.id.clone(), value);
        }
        if !node.enabled {
            continue;
        }
        if let Operation::Blend { opacity } = node.operation {
            let weight = constants.get(&node.inputs[2]).map(|key| key * opacity);
            let alias = if opacity == 0.0 || weight == Some(0.0) {
                Some(0)
            } else if weight == Some(1.0) {
                Some(1)
            } else {
                None
            };
            if let Some(input) = alias {
                // The validated topological order remains valid after removing dependencies.
                result.nodes[index].enabled = false;
                result.nodes[index].inputs = vec![node.inputs[input].clone()];
            }
        }
    }
    result
}
