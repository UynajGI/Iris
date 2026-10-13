// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use iris_render::{
    color::OutputConfig,
    document::{Node, Operation, PanelBinding},
};
use std::collections::{BTreeMap, BTreeSet};

/// Portable settings only: no photo identifier, filename or source fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EditRecipe {
    pub schema_version: u32,
    pub nodes: Vec<Node>,
    pub output_node: String,
    pub output: OutputConfig,
    pub panel_bindings: Vec<PanelBinding>,
    pub node_positions: BTreeMap<String, [f32; 2]>,
    pub raw_white_balance: Option<[f32; 3]>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EditPreset {
    pub id: String,
    pub name: String,
    pub recipe: EditRecipe,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchEditTarget {
    pub id: String,
    pub expected_revision: u64,
}

impl EditRecipe {
    pub fn from_document(document: &EditDocument, include_masks: bool) -> Result<Self> {
        document.validate()?;
        let mut recipe = Self {
            schema_version: document.schema_version,
            nodes: document.nodes.clone(),
            output_node: document.output_node.clone(),
            output: document.output.clone(),
            panel_bindings: document.panel_bindings.clone(),
            node_positions: document.node_positions.clone(),
            raw_white_balance: document.source.raw_white_balance,
        };
        if !include_masks {
            recipe.remove_masks()?;
        }
        Ok(recipe)
    }
    pub fn bind(&self, mut source: SourceBinding) -> Result<EditDocument> {
        source.raw_white_balance = self.raw_white_balance;
        let document = EditDocument {
            schema_version: self.schema_version,
            source,
            nodes: self.nodes.clone(),
            output_node: self.output_node.clone(),
            output: self.output.clone(),
            panel_bindings: self.panel_bindings.clone(),
            node_positions: self.node_positions.clone(),
        };
        document.validate()?;
        bounded_json(&document)?;
        Ok(document)
    }
    fn remove_masks(&mut self) -> Result<()> {
        let aliases: BTreeMap<String, String> = self
            .nodes
            .iter()
            .filter_map(|n| match n.operation {
                Operation::Blend { .. } => {
                    n.inputs.first().map(|input| (n.id.clone(), input.clone()))
                }
                _ => None,
            })
            .collect();
        let resolve = |id: &str| -> Result<String> {
            let mut id = id.to_string();
            for _ in 0..=iris_render::document::MAX_NODES {
                match aliases.get(&id) {
                    Some(next) => id = next.clone(),
                    None => return Ok(id),
                }
            }
            anyhow::bail!("cyclic mask alias")
        };
        self.nodes
            .retain(|n| !matches!(n.operation, Operation::Mask(_) | Operation::Blend { .. }));
        for node in &mut self.nodes {
            for input in &mut node.inputs {
                *input = resolve(input)?;
            }
        }
        self.output_node = resolve(&self.output_node)?;
        let ids: BTreeSet<_> = self.nodes.iter().map(|n| n.id.clone()).collect();
        self.panel_bindings.retain(|b| ids.contains(&b.node_id));
        self.node_positions.retain(|id, _| ids.contains(id));
        Ok(())
    }
    fn validate_portable(&self) -> Result<EditDocument> {
        self.bind(SourceBinding {
            photo_id: "preset-validation".into(),
            fingerprint: "0".repeat(64),
            width: 1,
            height: 1,
            reference_kind: iris_render::color::ReferenceKind::Scene,
            raw_white_balance: None,
        })
    }
}

impl Services {
    /// Masks are excluded unless explicitly requested. Blends are bypassed to
    /// their base branch; adjusted branches remain editable as detached nodes.
    pub fn export_edit_recipe(&self, id: &str, include_masks: bool) -> Result<EditRecipe> {
        EditRecipe::from_document(&self.edit_document(id)?.document, include_masks)
    }
    pub fn import_edit_recipe(
        &mut self,
        id: &str,
        command: EditCommand,
        json: &str,
    ) -> Result<EditRecord> {
        ensure!(json.len() <= MAX_EDIT_JSON, "recipe exceeds size limit");
        let recipe: EditRecipe = serde_json::from_str(json)?;
        let source = self.edit_document(id)?.document.source;
        let document = recipe.bind(source)?;
        self.save_edit_document(
            id,
            SaveEdit {
                expected_revision: command.expected_revision,
                request_id: command.request_id,
                document,
            },
        )
    }
    /// The complete batch commits or rolls back. Every target retains its own
    /// source binding; cross-project targets and duplicate IDs are rejected.
    pub fn batch_copy_edit_settings(
        &mut self,
        source_id: &str,
        targets: &[BatchEditTarget],
        request_id: &str,
        include_masks: bool,
    ) -> Result<Vec<EditRecord>> {
        ensure!(
            !targets.is_empty() && targets.len() <= 100,
            "invalid edit batch size"
        );
        ensure!(request_id.len() <= 100, "batch request id too long");
        validate_command(&EditCommand {
            expected_revision: 0,
            request_id: request_id.into(),
        })?;
        let source = self.edit_document(source_id)?;
        let project: i64 = self.store.conn.query_row(
            "SELECT project_id FROM photos WHERE id=?1",
            [source.photo_id],
            |r| r.get(0),
        )?;
        let recipe = EditRecipe::from_document(&source.document, include_masks)?;
        let mut seen = BTreeSet::new();
        let mut prepared = Vec::new();
        for (index, target) in targets.iter().enumerate() {
            ensure!(
                target.id != source_id && seen.insert(&target.id),
                "duplicate or source document in batch targets"
            );
            prepared.push(self.prepare_batch_target(target, &recipe, project, request_id, index)?);
        }
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let latest = record(&tx, source_id, None)?;
        ensure!(
            latest.revision == source.revision,
            "source edit changed while preparing batch"
        );
        state::require_available(&tx, source.photo_id)?;
        let mut results = Vec::new();
        for (id, request) in prepared {
            results.push(state::save_in_transaction(&tx, &id, &request)?);
        }
        tx.commit()?;
        Ok(results)
    }
    fn prepare_batch_target(
        &self,
        target: &BatchEditTarget,
        recipe: &EditRecipe,
        project: i64,
        request_id: &str,
        index: usize,
    ) -> Result<(String, SaveEdit)> {
        let current = self.edit_document(&target.id)?;
        let target_project: i64 = self.store.conn.query_row(
            "SELECT project_id FROM photos WHERE id=?1",
            [current.photo_id],
            |r| r.get(0),
        )?;
        ensure!(
            target_project == project,
            "edit batch target outside source project"
        );
        let document = recipe.bind(current.document.source)?;
        self.verify_edit_resources(&document)?;
        Ok((
            target.id.clone(),
            SaveEdit {
                expected_revision: target.expected_revision,
                request_id: format!("{request_id}:{index}"),
                document,
            },
        ))
    }
    pub fn save_edit_preset(
        &mut self,
        id: Option<&str>,
        name: &str,
        recipe: EditRecipe,
    ) -> Result<EditPreset> {
        validate_label(name)?;
        let document = recipe.validate_portable()?;
        self.verify_edit_resources(&document)?;
        let json = bounded_json(&recipe)?;
        let at = now();
        let preset_id = id
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        ensure!(
            !preset_id.is_empty() && preset_id.len() <= 128,
            "invalid preset id"
        );
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        write_preset_row(&tx, &preset_id, id.is_some(), name, &json, &at)?;
        for hash in document.resources() {
            tx.execute(
                "INSERT INTO edit_preset_resources(preset_id,hash) VALUES(?1,?2)",
                params![preset_id, hash],
            )?;
        }
        tx.commit()?;
        self.edit_preset(&preset_id)
    }
    pub fn edit_preset(&self, id: &str) -> Result<EditPreset> {
        let (id, name, json, created_at, updated_at): (String, String, String, String, String) =
            self.store.conn.query_row(
                "SELECT id,name,recipe_json,created_at,updated_at FROM edit_presets WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )?;
        ensure!(
            json.len() <= MAX_EDIT_JSON,
            "stored preset exceeds size limit"
        );
        let recipe: EditRecipe = serde_json::from_str(&json)?;
        recipe.validate_portable()?;
        Ok(EditPreset {
            id,
            name,
            recipe,
            created_at,
            updated_at,
        })
    }
    pub fn edit_presets(&self, limit: u32, offset: u32) -> Result<Vec<EditPreset>> {
        paginated(limit, offset)?;
        let mut query = self
            .store
            .conn
            .prepare("SELECT id FROM edit_presets ORDER BY name,id LIMIT ?1 OFFSET ?2")?;
        let ids = query
            .query_map(params![limit, offset], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.iter().map(|id| self.edit_preset(id)).collect()
    }
    pub fn delete_edit_preset(&mut self, id: &str) -> Result<()> {
        ensure!(
            self.store
                .conn
                .execute("DELETE FROM edit_presets WHERE id=?1", [id])?
                == 1,
            "preset does not exist"
        );
        Ok(())
    }
}

fn write_preset_row(
    tx: &rusqlite::Connection,
    preset_id: &str,
    update: bool,
    name: &str,
    json: &str,
    at: &str,
) -> Result<()> {
    if update {
        let changed = tx.execute(
            "UPDATE edit_presets SET name=?2,recipe_json=?3,updated_at=?4 WHERE id=?1",
            params![preset_id, name, json, at],
        )?;
        ensure!(changed == 1, "preset does not exist");
        tx.execute(
            "DELETE FROM edit_preset_resources WHERE preset_id=?1",
            [preset_id],
        )?;
    } else {
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM edit_presets", [], |r| r.get(0))?;
        ensure!(count < 1000, "preset limit reached");
        tx.execute("INSERT INTO edit_presets(id,name,recipe_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",params![preset_id,name,json,at])?;
    }
    Ok(())
}
