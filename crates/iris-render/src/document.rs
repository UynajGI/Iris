// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    adjustments::Adjustments,
    color::{OutputConfig, ReferenceKind},
    detail::DetailAdjustments,
    geometry::Geometry,
    mask::MaskDefinition,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use utoipa::ToSchema;

pub const DOCUMENT_VERSION: u32 = 1;
pub const MAX_NODES: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {
    pub photo_id: String,
    pub fingerprint: String,
    pub width: u32,
    pub height: u32,
    pub reference_kind: ReferenceKind,
    /// None means camera as-shot WB. Explicit gains are sensor RGB, not Kelvin.
    pub raw_white_balance: Option<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(
    tag = "type",
    content = "parameters",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Operation {
    Source,
    Adjustments(Adjustments),
    Detail(DetailAdjustments),
    Geometry(Geometry),
    Mask(MaskDefinition),
    /// Inputs: base Image, adjusted Image, Mask. Linear-light interpolation.
    Blend {
        opacity: f32,
    },
    Lut {
        resource: String,
        strength: f32,
        domain: crate::lut::LutDomain,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub enabled: bool,
    pub operation: Operation,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PanelBinding {
    pub role: String,
    pub node_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EditDocument {
    pub schema_version: u32,
    pub source: SourceBinding,
    pub nodes: Vec<Node>,
    pub output_node: String,
    pub output: OutputConfig,
    pub panel_bindings: Vec<PanelBinding>,
    /// View-only data is excluded from the rendering fingerprint.
    pub node_positions: BTreeMap<String, [f32; 2]>,
}

impl EditDocument {
    pub fn new(source: SourceBinding) -> Self {
        let nodes = vec![
            Node {
                id: "source".into(),
                name: "原片".into(),
                version: 1,
                enabled: true,
                operation: Operation::Source,
                inputs: vec![],
            },
            Node {
                id: "basic".into(),
                name: "影调与颜色".into(),
                version: 1,
                enabled: true,
                operation: Operation::Adjustments(Adjustments::default()),
                inputs: vec!["source".into()],
            },
            Node {
                id: "detail".into(),
                name: "细节".into(),
                version: 1,
                enabled: true,
                operation: Operation::Detail(DetailAdjustments::default()),
                inputs: vec!["basic".into()],
            },
            Node {
                id: "geometry".into(),
                name: "几何与光学".into(),
                version: 1,
                enabled: true,
                operation: Operation::Geometry(Geometry::default()),
                inputs: vec!["detail".into()],
            },
        ];
        Self {
            schema_version: DOCUMENT_VERSION,
            source,
            nodes,
            output_node: "geometry".into(),
            output: OutputConfig::default(),
            panel_bindings: vec![
                PanelBinding {
                    role: "basic".into(),
                    node_id: "basic".into(),
                },
                PanelBinding {
                    role: "detail".into(),
                    node_id: "detail".into(),
                },
                PanelBinding {
                    role: "geometry".into(),
                    node_id: "geometry".into(),
                },
            ],
            node_positions: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == DOCUMENT_VERSION,
            "unsupported edit document version"
        );
        ensure!(
            !self.source.photo_id.is_empty() && self.source.photo_id.len() <= 128,
            "invalid source id"
        );
        validate_hash(&self.source.fingerprint)?;
        crate::frame::pixel_count(self.source.width, self.source.height)?;
        if let Some(gains) = self.source.raw_white_balance {
            ensure!(
                self.source.reference_kind == ReferenceKind::Scene,
                "sensor white balance requires a RAW scene input"
            );
            ensure!(
                gains
                    .iter()
                    .all(|g| g.is_finite() && (0.01..=100.0).contains(g)),
                "invalid sensor white balance"
            );
        }
        self.output.validate()?;
        let _ = crate::graph::compile(self)?;
        ensure!(
            self.panel_bindings.len() <= MAX_NODES && self.node_positions.len() <= MAX_NODES,
            "too much view metadata"
        );
        let mut roles = BTreeSet::new();
        for binding in &self.panel_bindings {
            ensure!(
                !binding.role.is_empty() && binding.role.len() <= 64 && roles.insert(&binding.role),
                "invalid panel role"
            );
            ensure!(
                self.nodes.iter().any(|node| node.id == binding.node_id),
                "panel references missing node"
            );
        }
        for (id, point) in &self.node_positions {
            ensure!(
                self.nodes.iter().any(|node| &node.id == id)
                    && point
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 1_000_000.0),
                "invalid graph layout"
            );
        }
        Ok(())
    }

    pub fn fingerprint(&self) -> Result<String> {
        self.validate()?;
        let mut nodes = self.nodes.clone();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        // Labels, layout and panel bindings never change rendered pixels.
        for node in &mut nodes {
            node.name.clear();
        }
        let content = serde_json::to_vec(&(
            crate::ENGINE_VERSION,
            &self.source,
            nodes,
            &self.output_node,
            &self.output,
        ))?;
        Ok(hash_bytes(&content))
    }

    pub fn resources(&self) -> BTreeSet<String> {
        self.nodes
            .iter()
            .filter_map(|node| match &node.operation {
                Operation::Lut { resource, .. } => Some(resource.clone()),
                _ => None,
            })
            .collect()
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn validate_hash(hash: &str) -> Result<()> {
    ensure!(
        hash.len() == 64
            && hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "invalid content fingerprint"
    );
    Ok(())
}
