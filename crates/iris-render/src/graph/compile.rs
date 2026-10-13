// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::document::Node;

fn ports(node: &Node) -> Result<(usize, PortType)> {
    let image = (1, PortType::Image);
    match &node.operation {
        Operation::Source => {
            ensure!(node.enabled, "source cannot be disabled");
            Ok((0, PortType::Image))
        }
        Operation::Adjustments(p) => {
            p.validate()?;
            Ok(image)
        }
        Operation::Detail(p) => {
            p.validate()?;
            Ok(image)
        }
        Operation::Geometry(p) => {
            p.validate()?;
            Ok(image)
        }
        Operation::Mask(p) => {
            p.validate()?;
            ensure!(node.enabled, "disable the blend to bypass a mask");
            Ok((1, PortType::Mask))
        }
        Operation::Blend { opacity } => {
            ensure!(
                opacity.is_finite() && (0.0..=1.0).contains(opacity),
                "invalid blend opacity"
            );
            Ok((3, PortType::Image))
        }
        Operation::Lut {
            resource, strength, ..
        } => {
            crate::document::validate_hash(resource)?;
            ensure!(
                strength.is_finite() && (0.0..=1.0).contains(strength),
                "invalid LUT strength"
            );
            Ok(image)
        }
    }
}

fn validate_node(node: &Node) -> Result<()> {
    ensure!(
        !node.id.is_empty()
            && node.id.len() <= 64
            && node
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "invalid node id"
    );
    ensure!(
        node.name.len() <= 256 && node.version == 1,
        "unsupported node version or label"
    );
    Ok(())
}

fn typed_inputs(document: &EditDocument, outputs: &BTreeMap<String, PortType>) -> Result<()> {
    for node in &document.nodes {
        for (index, input) in node.inputs.iter().enumerate() {
            let expected = if matches!(node.operation, Operation::Blend { .. }) && index == 2 {
                PortType::Mask
            } else {
                PortType::Image
            };
            ensure!(
                outputs.get(input) == Some(&expected),
                "missing or mistyped input {input} on {}",
                node.id
            );
        }
    }
    Ok(())
}

fn topological(document: &EditDocument) -> Result<Vec<usize>> {
    let mut done = BTreeSet::new();
    let mut order = Vec::with_capacity(document.nodes.len());
    while order.len() < document.nodes.len() {
        let previous = order.len();
        for (index, node) in document.nodes.iter().enumerate() {
            if !done.contains(&node.id) && node.inputs.iter().all(|id| done.contains(id)) {
                done.insert(node.id.clone());
                order.push(index);
            }
        }
        ensure!(order.len() > previous, "edit graph contains a cycle");
    }
    Ok(order)
}

/// Matching dimensions alone is insufficient: a rotated branch and an unrotated
/// mask can have identical bounds. Track the geometry lineage independently.
fn coordinate_grids(document: &EditDocument, order: &[usize]) -> Result<()> {
    let mut grids: BTreeMap<String, String> = BTreeMap::new();
    for &index in order {
        let node = &document.nodes[index];
        let parent = node
            .inputs
            .first()
            .and_then(|id| grids.get(id))
            .cloned()
            .unwrap_or_default();
        let grid = match &node.operation {
            Operation::Geometry(params) if node.enabled => geometry_grid(parent, params)?,
            Operation::Blend { .. } if node.enabled => {
                ensure!(
                    node.inputs.iter().all(|id| grids.get(id) == Some(&parent)),
                    "blend branches and mask require the same geometry lineage"
                );
                parent
            }
            _ => parent,
        };
        grids.insert(node.id.clone(), grid);
    }
    Ok(())
}

fn geometry_grid(parent: String, params: &crate::geometry::Geometry) -> Result<String> {
    let mut spatial = params.clone();
    // Optical light falloff changes samples but not their coordinates.
    spatial.vignette_stops = 0.0;
    if spatial == crate::geometry::Geometry::default() {
        return Ok(parent);
    }
    Ok(crate::document::hash_bytes(&serde_json::to_vec(&(
        parent, spatial,
    ))?))
}

/// Validate every node, including disconnected nodes, before execution.
pub fn compile(document: &EditDocument) -> Result<Plan> {
    ensure!(
        !document.nodes.is_empty() && document.nodes.len() <= MAX_NODES,
        "invalid node count"
    );
    let mut outputs = BTreeMap::new();
    for node in &document.nodes {
        validate_node(node)?;
        let (inputs, output) = ports(node)?;
        ensure!(
            node.inputs.len() == inputs,
            "wrong input count for {}",
            node.id
        );
        ensure!(
            outputs.insert(node.id.clone(), output).is_none(),
            "duplicate node id"
        );
    }
    ensure!(
        document
            .nodes
            .iter()
            .filter(|n| matches!(n.operation, Operation::Source))
            .count()
            == 1,
        "a document requires exactly one source"
    );
    ensure!(
        outputs.get(&document.output_node) == Some(&PortType::Image),
        "output must reference an image node"
    );
    typed_inputs(document, &outputs)?;
    let order = topological(document)?;
    coordinate_grids(document, &order)?;
    Ok(Plan { order, outputs })
}
