// SPDX-License-Identifier: GPL-3.0-or-later
use super::EditRenderJob;
use anyhow::{ensure, Context, Result};
use iris_render::{
    color::{OutputConfig, OutputSpace},
    document::{EditDocument, Operation},
    engine::{RenderEngine, RenderQuality, RenderRequest},
    mask::{self, MaskDefinition},
    output::{self, OutputFormat},
    Frame, RenderContext,
};
use std::collections::BTreeMap;

/// Selection overlay only: no display RGB is used to estimate a range mask.
/// The preview input is a bounded working-linear proxy; it is not a full-size
/// coverage measurement and never changes the edit graph or saved revision.
impl EditRenderJob {
    pub fn encode_mask_preview(
        &self,
        engine: &RenderEngine,
        request: &RenderRequest,
        mask_id: &str,
        context: &RenderContext,
    ) -> Result<Vec<u8>> {
        ensure!(
            request.quality == RenderQuality::Interactive && request.max_edge <= 1024,
            "mask preview requires interactive quality and max_edge <= 1024"
        );
        let (document, definition) = mask_input(&self.record.document, mask_id)?;
        let input = self.preview_document(engine, request, context, &document)?;
        let mask = mask::render(&input.frame, &definition, context)?;
        let _mask_memory = context.reserve(mask.values.len() as u64 * 4)?;
        let _overlay_memory = context.reserve(mask.values.len() as u64 * 16)?;
        let mut pixels = Vec::with_capacity(mask.values.len());
        for row in mask.values.chunks(mask.width as usize) {
            context.check_cancelled()?;
            pixels.extend(
                row.iter()
                    .map(|weight| [1.0, 35.0 / 255.0, 40.0 / 255.0, weight * 140.0 / 255.0]),
            );
        }
        let overlay = Frame::new(mask.width, mask.height, pixels)?;
        let config = OutputConfig {
            space: OutputSpace::Srgb,
            ..Default::default()
        };
        // Already encoded annotation colors; do not run a photo color transform.
        output::encode_output(&overlay, &config, OutputFormat::Png16, context)
    }
}

fn mask_input(document: &EditDocument, mask_id: &str) -> Result<(EditDocument, MaskDefinition)> {
    let plan = iris_render::graph::compile(document)?;
    let selected = document
        .nodes
        .iter()
        .find(|node| node.id == mask_id)
        .context("mask not found")?;
    let Operation::Mask(definition) = &selected.operation else {
        anyhow::bail!("selected node is not a mask");
    };
    ensure!(selected.enabled, "mask node is disabled");
    // Keep a coordinate lineage rather than comparing dimensions: two crops
    // can have the same size while referring to entirely different pixels.
    let mut grids: BTreeMap<String, Option<Vec<String>>> = BTreeMap::new();
    for index in plan.order {
        let node = &document.nodes[index];
        let mut grid = if matches!(node.operation, Operation::Source) {
            Some(Vec::new())
        } else {
            grids.get(&node.inputs[0]).cloned().flatten()
        };
        if node.enabled {
            match &node.operation {
                Operation::Geometry(parameters) if parameters != &Default::default() => {
                    if let Some(path) = &mut grid {
                        path.push(node.id.clone());
                    }
                }
                Operation::Blend { .. } => {
                    if node.inputs.iter().any(|id| grids.get(id) != Some(&grid)) {
                        grid = None;
                    }
                }
                _ => {}
            }
        }
        grids.insert(node.id.clone(), grid);
    }
    let grid = grids.get(mask_id).context("mask grid missing")?;
    ensure!(
        grid.is_some() && grids.get(&document.output_node) == Some(grid),
        "mask preview coordinates differ from final output"
    );
    let mut input = document.clone();
    input.output_node = selected.inputs[0].clone();
    Ok((input, definition.clone()))
}
