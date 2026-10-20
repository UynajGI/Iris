// SPDX-License-Identifier: GPL-3.0-or-later
//! Selective synchronization operates on traditional panel bindings, not whole recipes.
use super::*;
use iris_render::{adjustments::Adjustments, document::Operation};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EditSyncGroup {
    Tone,
    Color,
    Curves,
    ColorGrading,
    Detail,
    Effects,
    Crop,
    Optics,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectiveEditSync {
    pub expected_revision: u64,
    pub request_id: String,
    pub targets: Vec<BatchEditTarget>,
    pub groups: Vec<EditSyncGroup>,
}

impl SelectiveEditSync {
    fn validate(&self, source_id: &str) -> Result<()> {
        validate_command(&EditCommand {
            expected_revision: self.expected_revision,
            request_id: self.request_id.clone(),
        })?;
        ensure!(self.request_id.len() <= 100, "batch request id too long");
        ensure!(
            !self.targets.is_empty() && self.targets.len() <= 100,
            "invalid edit batch size"
        );
        let groups: BTreeSet<_> = self.groups.iter().collect();
        ensure!(
            !groups.is_empty() && groups.len() == self.groups.len(),
            "invalid sync groups"
        );
        let mut ids = BTreeSet::new();
        for target in &self.targets {
            ensure!(
                target.id != source_id && ids.insert(&target.id),
                "duplicate or source document in batch targets"
            );
            ensure!(
                target.expected_revision <= MAX_HISTORY as u64,
                "invalid target revision"
            );
        }
        Ok(())
    }
}

impl Services {
    pub fn sync_edit_settings(
        &mut self,
        source_id: &str,
        request: SelectiveEditSync,
    ) -> Result<Vec<EditRecord>> {
        request.validate(source_id)?;
        let source = self.edit_document(source_id)?;
        ensure!(
            source.revision == request.expected_revision,
            "source edit revision conflict"
        );
        let project = edit_project(&self.store.conn, source.photo_id)?;
        // Bind receipts to the whole batch, including fields and targets. Replays
        // derive from the original target revision even after subsequent edits.
        let context = hash_bytes(bounded_json(&(source_id, &request))?.as_bytes());
        let mut prepared = Vec::new();
        for (index, target) in request.targets.iter().enumerate() {
            let document =
                self.prepare_selected_target(target, project, &source.document, &request.groups)?;
            prepared.push((
                target.id.clone(),
                SaveEdit {
                    expected_revision: target.expected_revision,
                    request_id: format!("{}:{index}", request.request_id),
                    document,
                },
            ));
        }
        let tx = self
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure!(
            record(&tx, source_id, None)?.revision == source.revision,
            "source edit changed while preparing batch"
        );
        state::require_available(&tx, source.photo_id)?;
        let mut results = Vec::new();
        for (id, save) in prepared {
            results.push(state::save_with_context(&tx, &id, &save, Some(&context))?);
        }
        tx.commit()?;
        Ok(results)
    }

    fn prepare_selected_target(
        &self,
        target: &BatchEditTarget,
        project: i64,
        source: &EditDocument,
        groups: &[EditSyncGroup],
    ) -> Result<EditDocument> {
        let current = self.edit_document(&target.id)?;
        ensure!(
            edit_project(&self.store.conn, current.photo_id)? == project,
            "edit batch target outside source project"
        );
        let baseline = record(
            &self.store.conn,
            &target.id,
            Some(target.expected_revision as i64),
        )?;
        let document = merge_selected(source, baseline.document, groups)?;
        self.verify_edit_resources(&document)?;
        Ok(document)
    }
}

fn edit_project(conn: &Connection, photo: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT project_id FROM photos WHERE id=?1",
        [photo],
        |row| row.get(0),
    )?)
}

fn panel_index(document: &EditDocument, role: &str, kind: &str) -> Result<usize> {
    let bindings: Vec<_> = document
        .panel_bindings
        .iter()
        .filter(|binding| binding.role == role || binding.role == format!("traditional.{kind}"))
        .collect();
    ensure!(
        bindings.len() == 1,
        "selective sync requires one unambiguous {role} panel binding"
    );
    let index = document
        .nodes
        .iter()
        .position(|node| node.id == bindings[0].node_id)
        .ok_or_else(|| anyhow::anyhow!("missing {role} panel node"))?;
    // A disconnected node cannot affect the displayed result. Do not claim it
    // has been synchronized; arbitrary graphs remain available via recipes.
    let mut pending = vec![document.output_node.as_str()];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        if let Some(node) = document.nodes.iter().find(|node| node.id == id) {
            pending.extend(node.inputs.iter().map(String::as_str));
        }
    }
    ensure!(
        visited.contains(document.nodes[index].id.as_str()),
        "{role} panel is disconnected"
    );
    Ok(index)
}

fn merge_selected(
    source: &EditDocument,
    mut target: EditDocument,
    groups: &[EditSyncGroup],
) -> Result<EditDocument> {
    source.validate()?;
    target.validate()?;
    for group in groups {
        let (role, kind) = match group {
            EditSyncGroup::Detail => ("detail", "detail"),
            EditSyncGroup::Crop | EditSyncGroup::Optics => ("geometry", "geometry"),
            _ => ("basic", "adjustments"),
        };
        let from = &source.nodes[panel_index(source, role, kind)?];
        let index = panel_index(&target, role, kind)?;
        let to = &mut target.nodes[index];
        ensure!(
            to.enabled,
            "enable target {role} panel before selective sync"
        );
        copy_panel(from, to, role, *group)?;
    }
    target.validate()?;
    Ok(target)
}

fn copy_panel(
    from: &iris_render::document::Node,
    to: &mut iris_render::document::Node,
    role: &str,
    group: EditSyncGroup,
) -> Result<()> {
    match (&from.operation, &mut to.operation) {
        (Operation::Adjustments(a), Operation::Adjustments(b)) if role == "basic" => {
            let value = if from.enabled {
                a.clone()
            } else {
                Adjustments::default()
            };
            copy_adjustments(&value, b, group);
        }
        (Operation::Detail(a), Operation::Detail(b)) if role == "detail" => {
            *b = if from.enabled {
                a.clone()
            } else {
                Default::default()
            };
        }
        (Operation::Geometry(a), Operation::Geometry(b)) if role == "geometry" => {
            let value = if from.enabled {
                a.clone()
            } else {
                Default::default()
            };
            if group == EditSyncGroup::Crop {
                b.crop = value.crop;
                b.rotation_degrees = value.rotation_degrees;
                b.perspective = value.perspective;
            } else {
                b.radial = value.radial;
                b.chromatic_aberration = value.chromatic_aberration;
                b.vignette_stops = value.vignette_stops;
            }
        }
        _ => anyhow::bail!("incompatible {role} panel for selective sync"),
    }
    Ok(())
}

fn copy_adjustments(from: &Adjustments, to: &mut Adjustments, group: EditSyncGroup) {
    match group {
        EditSyncGroup::Tone => {
            to.separated.tone = from.separated.tone;
            to.exposure = from.exposure;
            to.contrast = from.contrast;
            to.contrast_pivot = from.contrast_pivot;
            to.highlights = from.highlights;
            to.shadows = from.shadows;
            to.whites = from.whites;
            to.blacks = from.blacks;
        }
        EditSyncGroup::Color => {
            to.temperature = from.temperature;
            to.tint = from.tint;
            to.saturation = from.saturation;
            to.vibrance = from.vibrance;
            to.hsl = from.hsl.clone();
            to.black_and_white = from.black_and_white.clone();
            to.separated.color = from.separated.color;
        }
        EditSyncGroup::Curves => {
            to.tone_curve = from.tone_curve.clone();
            to.master_curve = from.master_curve.clone();
            to.red_curve = from.red_curve.clone();
            to.green_curve = from.green_curve.clone();
            to.blue_curve = from.blue_curve.clone();
        }
        EditSyncGroup::ColorGrading => to.color_balance = from.color_balance.clone(),
        EditSyncGroup::Effects => {
            to.grain = from.grain.clone();
            to.vignette = from.vignette.clone();
        }
        _ => unreachable!("non-adjustment sync group"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_render::{color::ReferenceKind, geometry::Geometry};

    fn document(id: &str) -> EditDocument {
        EditDocument::new(SourceBinding {
            photo_id: id.into(),
            fingerprint: "a".repeat(64),
            width: 100,
            height: 80,
            reference_kind: ReferenceKind::Scene,
            raw_white_balance: Some([1.2, 1.0, 0.9]),
        })
    }

    #[test]
    fn groups_copy_only_the_selected_parameters() {
        let mut source = document("1");
        let target = document("2");
        if let Operation::Adjustments(p) = &mut source.nodes[1].operation {
            p.temperature = 0.2;
            p.saturation = 0.3;
            p.master_curve.points = vec![
                iris_render::adjustments::CurvePoint { x: 0.0, y: 0.1 },
                iris_render::adjustments::CurvePoint { x: 1.0, y: 0.9 },
            ];
            p.color_balance.midtones = [0.01, 0.02, 0.03];
            p.grain.amount = 0.1;
        }
        if let Operation::Detail(p) = &mut source.nodes[2].operation {
            p.sharpening = 0.2;
        }
        source.nodes[3].operation = Operation::Geometry(Geometry {
            crop: [0.1, 0.1, 0.8, 0.8],
            radial: [0.05, 0.01],
            ..Default::default()
        });
        source.source.raw_white_balance = Some([2.0, 1.0, 1.5]);
        for group in [
            EditSyncGroup::Color,
            EditSyncGroup::Curves,
            EditSyncGroup::ColorGrading,
            EditSyncGroup::Effects,
            EditSyncGroup::Detail,
            EditSyncGroup::Crop,
            EditSyncGroup::Optics,
        ] {
            let synced = merge_selected(&source, target.clone(), &[group]).unwrap();
            assert_eq!(synced.source, target.source);
            assert_eq!(synced.output, target.output);
            assert_ne!(synced.nodes, target.nodes);
            if group == EditSyncGroup::Crop {
                let Operation::Geometry(p) = &synced.nodes[3].operation else {
                    panic!()
                };
                assert_eq!(p.radial, [0.0, 0.0]);
            }
            if group == EditSyncGroup::Optics {
                let Operation::Geometry(p) = &synced.nodes[3].operation else {
                    panic!()
                };
                assert_eq!(p.crop, [0.0, 0.0, 1.0, 1.0]);
            }
        }
    }

    #[test]
    fn grading_sync_includes_global_blending_and_balance_without_copying_other_controls() {
        let mut source = document("1");
        let target = document("2");
        let Operation::Adjustments(from) = &mut source.nodes[1].operation else {
            panic!()
        };
        from.exposure = 1.0;
        from.color_balance.global = [0.1, -0.2, 0.3];
        from.color_balance.blending = 0.8;
        from.color_balance.balance = -0.4;
        let expected = from.color_balance.clone();
        let synced =
            merge_selected(&source, target.clone(), &[EditSyncGroup::ColorGrading]).unwrap();
        let Operation::Adjustments(actual) = &synced.nodes[1].operation else {
            panic!()
        };
        assert_eq!(actual.color_balance, expected);
        assert_eq!(actual.exposure, 0.0);
        assert_eq!(synced.source, target.source);
        assert_eq!(synced.output, target.output);
    }

    #[test]
    fn unsupported_panels_fail_instead_of_silently_modifying_other_nodes() {
        let mut source = document("1");
        let mut target = document("2");
        target.nodes[1].enabled = false;
        assert!(merge_selected(&source, target, &[EditSyncGroup::Tone]).is_err());
        source.panel_bindings.clear();
        assert!(merge_selected(&source, document("2"), &[EditSyncGroup::Tone]).is_err());
        let mut source = document("1");
        source.output_node = "source".into();
        assert!(merge_selected(&source, document("2"), &[EditSyncGroup::Tone]).is_err());
    }
}
