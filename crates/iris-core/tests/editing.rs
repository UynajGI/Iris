// SPDX-License-Identifier: GPL-3.0-or-later
use iris_core::services::{BatchEditTarget, EditCommand, EditResourceKind, SaveEdit};
use iris_core::{Services, Store};
use iris_render::{
    color::ReferenceKind,
    document::{hash_bytes, Node, Operation, SourceBinding},
    mask::MaskDefinition,
};
use rusqlite::params;

fn fixture() -> (tempfile::TempDir, Services, Vec<i64>) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let project = services.create_project(&root).unwrap();
    let mut ids = Vec::new();
    for name in ["a.png", "b.png", "c.png"] {
        image::RgbImage::from_pixel(2, 2, image::Rgb([64, 128, 192]))
            .save(root.join(name))
            .unwrap();
        services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,?2,?2,'png',1,1,2,2)",params![project.id,name]).unwrap();
        ids.push(services.store.conn.last_insert_rowid());
    }
    (temp, services, ids)
}
fn create(services: &mut Services, id: i64) -> iris_core::services::EditRecord {
    let bytes = std::fs::read(services.photo_path(id).unwrap()).unwrap();
    services
        .create_edit_document(
            id,
            "Original",
            SourceBinding {
                photo_id: id.to_string(),
                fingerprint: hash_bytes(&bytes),
                width: 2,
                height: 2,
                reference_kind: ReferenceKind::Display,
                raw_white_balance: None,
            },
        )
        .unwrap()
}
fn command(revision: u64, id: &str) -> EditCommand {
    EditCommand {
        expected_revision: revision,
        request_id: id.into(),
    }
}

#[test]
fn save_receipts_conflicts_undo_redo_and_restart() {
    let (temp, mut services, ids) = fixture();
    let initial = create(&mut services, ids[0]);
    let mut changed = initial.document.clone();
    changed.nodes[1].name = "Edited".into();
    let request = SaveEdit {
        expected_revision: 0,
        request_id: "save-one".into(),
        document: changed.clone(),
    };
    let saved = services
        .save_edit_document(&initial.id, request.clone())
        .unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(
        services
            .save_edit_document(&initial.id, request.clone())
            .unwrap()
            .revision,
        1
    );
    let mut reused = request;
    reused.document.nodes[1].name = "Different payload".into();
    assert!(services.save_edit_document(&initial.id, reused).is_err());
    assert!(services
        .save_edit_document(
            &initial.id,
            SaveEdit {
                expected_revision: 0,
                request_id: "stale".into(),
                document: changed.clone()
            }
        )
        .is_err());
    let undo = services
        .undo_edit_document(&initial.id, command(1, "undo-one"))
        .unwrap();
    assert_eq!(undo.revision, 2);
    assert_eq!(undo.document, initial.document);
    drop(services);
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let redo = services
        .redo_edit_document(&initial.id, command(2, "redo-one"))
        .unwrap();
    assert_eq!(redo.revision, 3);
    assert_eq!(redo.document, changed);
    let undo = services
        .undo_edit_document(&initial.id, command(3, "undo-two"))
        .unwrap();
    services
        .save_edit_document(
            &initial.id,
            SaveEdit {
                expected_revision: undo.revision,
                request_id: "branch".into(),
                document: initial.document,
            },
        )
        .unwrap();
    assert!(services
        .redo_edit_document(&initial.id, command(5, "invalid-redo"))
        .is_err());
    assert_eq!(services.edit_history(&initial.id, 100, 0).unwrap().len(), 6);
}

#[test]
fn immutable_source_unavailable_photos_and_atomic_batch() {
    let (_temp, mut services, ids) = fixture();
    let source = create(&mut services, ids[0]);
    let a = create(&mut services, ids[1]);
    let b = create(&mut services, ids[2]);
    let mut invalid = a.document.clone();
    invalid.source.fingerprint = "0".repeat(64);
    assert!(services
        .save_edit_document(
            &a.id,
            SaveEdit {
                expected_revision: 0,
                request_id: "rebind".into(),
                document: invalid
            }
        )
        .is_err());
    let targets = vec![
        BatchEditTarget {
            id: a.id.clone(),
            expected_revision: 0,
        },
        BatchEditTarget {
            id: b.id.clone(),
            expected_revision: 1,
        },
    ];
    assert!(services
        .batch_copy_edit_settings(&source.id, &targets, "batch-conflict", false)
        .is_err());
    assert_eq!(services.edit_document(&a.id).unwrap().revision, 0);
    assert_eq!(services.edit_history(&a.id, 100, 0).unwrap().len(), 1);
    let targets = vec![
        BatchEditTarget {
            id: a.id.clone(),
            expected_revision: 0,
        },
        BatchEditTarget {
            id: b.id.clone(),
            expected_revision: 0,
        },
    ];
    let results = services
        .batch_copy_edit_settings(&source.id, &targets, "batch-ok", false)
        .unwrap();
    assert_eq!(results[0].document.source, a.document.source);
    assert_eq!(results[1].document.source, b.document.source);
    services
        .store
        .conn
        .execute("UPDATE photos SET quarantined=1 WHERE id=?1", [ids[1]])
        .unwrap();
    assert!(services.edit_document(&a.id).is_err());
    assert!(services
        .save_edit_document(
            &a.id,
            SaveEdit {
                expected_revision: 1,
                request_id: "unavailable".into(),
                document: a.document
            }
        )
        .is_err());
}

#[test]
fn resources_are_validated_pinned_and_tampering_is_detected() {
    let (temp, mut services, ids) = fixture();
    let edit = create(&mut services, ids[0]);
    assert!(services
        .import_edit_resource(EditResourceKind::Lut, b"not a LUT")
        .is_err());
    let bytes = b"LUT_1D_SIZE 2\n0 0 0\n1 1 1\n";
    let resource = services
        .import_edit_resource(EditResourceKind::Lut, bytes)
        .unwrap();
    let mut document = edit.document.clone();
    document.nodes.push(Node {
        id: "lut".into(),
        name: "LUT".into(),
        version: 1,
        enabled: true,
        operation: Operation::Lut {
            resource: resource.hash.clone(),
            strength: 1.0,
            domain: iris_render::lut::LutDomain::LinearRec2020,
        },
        inputs: vec![document.output_node.clone()],
    });
    document.output_node = "lut".into();
    services
        .save_edit_document(
            &edit.id,
            SaveEdit {
                expected_revision: 0,
                request_id: "lut".into(),
                document,
            },
        )
        .unwrap();
    let pins: i64 = services
        .store
        .conn
        .query_row(
            "SELECT COUNT(*) FROM edit_resource_pins WHERE hash=?1",
            [&resource.hash],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pins, 1);
    let recipe = services.export_edit_recipe(&edit.id, false).unwrap();
    let preset = services.save_edit_preset(None, "Test LUT", recipe).unwrap();
    let pins: i64 = services
        .store
        .conn
        .query_row(
            "SELECT COUNT(*) FROM edit_preset_resources WHERE preset_id=?1",
            [preset.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pins, 1);
    std::fs::write(
        temp.path().join("edit-resources-v1").join(&resource.hash),
        b"corrupt",
    )
    .unwrap();
    assert!(services.read_edit_resource(&resource.hash).is_err());
}

#[test]
fn portable_recipe_omits_source_and_masks_unless_opted_in() {
    let (_temp, mut services, ids) = fixture();
    let edit = create(&mut services, ids[0]);
    let mut document = edit.document.clone();
    document.nodes.push(Node {
        id: "mask".into(),
        name: "Mask".into(),
        version: 1,
        enabled: true,
        operation: Operation::Mask(MaskDefinition::LinearGradient {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
        }),
        inputs: vec!["geometry".into()],
    });
    document.nodes.push(Node {
        id: "blend".into(),
        name: "Blend".into(),
        version: 1,
        enabled: true,
        operation: Operation::Blend { opacity: 1.0 },
        inputs: vec!["geometry".into(), "geometry".into(), "mask".into()],
    });
    document.output_node = "blend".into();
    services
        .save_edit_document(
            &edit.id,
            SaveEdit {
                expected_revision: 0,
                request_id: "mask".into(),
                document,
            },
        )
        .unwrap();
    let recipe = services.export_edit_recipe(&edit.id, false).unwrap();
    assert!(!recipe
        .nodes
        .iter()
        .any(|n| matches!(n.operation, Operation::Mask(_) | Operation::Blend { .. })));
    assert_eq!(recipe.output_node, "geometry");
    let json = serde_json::to_value(&recipe).unwrap();
    assert!(json.get("source").is_none());
    assert!(!serde_json::to_string(&json)
        .unwrap()
        .contains(&edit.document.source.fingerprint));
    assert!(services
        .export_edit_recipe(&edit.id, true)
        .unwrap()
        .nodes
        .iter()
        .any(|n| matches!(n.operation, Operation::Mask(_))));
}

#[test]
fn version_eight_upgrade_is_backed_up_and_preserves_catalog() {
    let (temp, services, ids) = fixture();
    services.store.conn.execute_batch("DROP TABLE edit_resource_pins; DROP TABLE edit_commands; DROP TABLE edit_states; DROP TABLE edit_revisions; DROP TABLE edit_documents; DROP TABLE edit_preset_resources; DROP TABLE edit_presets; DROP TABLE edit_resources; PRAGMA user_version=8;").unwrap();
    drop(services);
    let store = Store::open(temp.path().join("library.sqlite")).unwrap();
    assert!(store
        .recovery_notice
        .as_deref()
        .unwrap()
        .contains("recovery backup"));
    assert_eq!(
        store
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        9
    );
    let mut services = Services::new(store);
    assert_eq!(services.photo(ids[0]).unwrap().id, ids[0]);
    create(&mut services, ids[0]);
}

#[test]
fn copied_recipe_cannot_attach_an_unknown_resource() {
    let (_temp, mut services, ids) = fixture();
    let edit = create(&mut services, ids[0]);
    let mut document = edit.document.clone();
    document.nodes.push(Node {
        id: "missing".into(),
        name: "Missing resource".into(),
        version: 1,
        enabled: true,
        operation: Operation::Lut {
            resource: "a".repeat(64),
            strength: 1.0,
            domain: iris_render::lut::LutDomain::LinearRec2020,
        },
        inputs: vec![document.output_node.clone()],
    });
    document.output_node = "missing".into();
    assert!(services
        .save_edit_document(
            &edit.id,
            SaveEdit {
                expected_revision: 0,
                request_id: "missing".into(),
                document
            }
        )
        .is_err());
    assert_eq!(services.edit_document(&edit.id).unwrap().revision, 0);
    assert!(services.edit_documents(ids[0], 101, 0).is_err());
}

#[test]
fn edit_history_rejects_negative_sqlite_revision() {
    let (_temp, mut services, ids) = fixture();
    let edit = create(&mut services, ids[0]);
    services
        .store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints=ON")
        .unwrap();
    services.store.conn.execute("INSERT INTO edit_revisions(document_id,revision,document_json,action,created_at) SELECT document_id,-1,document_json,'corrupt',created_at FROM edit_revisions WHERE document_id=?1 AND revision=0",[&edit.id]).unwrap();
    services
        .store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints=OFF")
        .unwrap();
    assert!(services.edit_history(&edit.id, 100, 0).is_err());
    assert!(services.delete_edit_document(&edit.id, u64::MAX).is_err());
    assert_eq!(services.edit_document(&edit.id).unwrap().revision, 0);
}

#[test]
fn selective_sync_preserves_target_graph_and_replays_original_snapshot() {
    use iris_core::services::{EditSyncGroup, SelectiveEditSync};
    let (_temp, mut services, ids) = fixture();
    let source = create(&mut services, ids[0]);
    let target = create(&mut services, ids[1]);
    let mut document = source.document.clone();
    if let Operation::Adjustments(p) = &mut document.nodes[1].operation {
        p.exposure = 1.25;
        p.saturation = 0.4;
    }
    let source = services
        .save_edit_document(
            &source.id,
            SaveEdit {
                expected_revision: 0,
                request_id: "sync-source".into(),
                document,
            },
        )
        .unwrap();
    let mut document = target.document.clone();
    if let Operation::Geometry(p) = &mut document.nodes[3].operation {
        p.crop = [0.1, 0.2, 0.7, 0.6];
        p.radial = [0.03, 0.0];
    }
    document.nodes.push(Node {
        id: "mask".into(),
        name: "Local mask".into(),
        version: 1,
        enabled: true,
        operation: Operation::Mask(MaskDefinition::LinearGradient {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
        }),
        inputs: vec!["geometry".into()],
    });
    document.nodes.push(Node {
        id: "blend".into(),
        name: "Local blend".into(),
        version: 1,
        enabled: true,
        operation: Operation::Blend { opacity: 0.6 },
        inputs: vec!["geometry".into(), "geometry".into(), "mask".into()],
    });
    document.output_node = "blend".into();
    let target = services
        .save_edit_document(
            &target.id,
            SaveEdit {
                expected_revision: 0,
                request_id: "sync-target".into(),
                document,
            },
        )
        .unwrap();
    let request = SelectiveEditSync {
        expected_revision: source.revision,
        request_id: "selective-sync".into(),
        targets: vec![BatchEditTarget {
            id: target.id.clone(),
            expected_revision: target.revision,
        }],
        groups: vec![EditSyncGroup::Tone],
    };
    let result = services
        .sync_edit_settings(&source.id, request.clone())
        .unwrap()
        .remove(0);
    let mut expected = target.document.clone();
    if let Operation::Adjustments(p) = &mut expected.nodes[1].operation {
        p.exposure = 1.25;
    }
    assert_eq!(result.document, expected);
    assert_eq!(
        services.edit_document(&source.id).unwrap().revision,
        source.revision
    );
    let undone = services
        .undo_edit_document(&target.id, command(result.revision, "undo-sync"))
        .unwrap();
    assert_eq!(undone.document, target.document);
    // A lost response replay returns its historical result without re-applying
    // the sync over a later user edit or undo.
    let replay = services
        .sync_edit_settings(&source.id, request.clone())
        .unwrap()
        .remove(0);
    assert_eq!(replay.revision, result.revision);
    assert_eq!(
        services.edit_document(&target.id).unwrap().revision,
        undone.revision
    );
    let mut reused = request;
    reused.groups.push(EditSyncGroup::Color);
    assert!(services.sync_edit_settings(&source.id, reused).is_err());
    assert_eq!(
        services.edit_document(&target.id).unwrap().document,
        target.document
    );
}

#[test]
fn selective_sync_conflicts_are_atomic_and_source_revision_is_required() {
    use iris_core::services::{EditSyncGroup, SelectiveEditSync};
    let (_temp, mut services, ids) = fixture();
    let source = create(&mut services, ids[0]);
    let a = create(&mut services, ids[1]);
    let b = create(&mut services, ids[2]);
    services
        .save_edit_document(
            &b.id,
            SaveEdit {
                expected_revision: 0,
                request_id: "target-newer".into(),
                document: b.document.clone(),
            },
        )
        .unwrap();
    let mut request = SelectiveEditSync {
        expected_revision: 0,
        request_id: "atomic-sync".into(),
        targets: vec![
            BatchEditTarget {
                id: a.id.clone(),
                expected_revision: 0,
            },
            BatchEditTarget {
                id: b.id.clone(),
                expected_revision: 0,
            },
        ],
        groups: vec![EditSyncGroup::Tone],
    };
    assert!(services
        .sync_edit_settings(&source.id, request.clone())
        .is_err());
    assert_eq!(services.edit_document(&a.id).unwrap().revision, 0);
    request.targets[1].expected_revision = 1;
    request.expected_revision = 1;
    assert!(services
        .sync_edit_settings(&source.id, request.clone())
        .is_err());
    request.expected_revision = 0;
    request.groups.clear();
    assert!(services
        .sync_edit_settings(&source.id, request.clone())
        .is_err());
    request.groups = vec![EditSyncGroup::Tone, EditSyncGroup::Tone];
    assert!(services
        .sync_edit_settings(&source.id, request.clone())
        .is_err());
    request.groups = vec![EditSyncGroup::Tone];
    let result = services
        .sync_edit_settings(&source.id, request.clone())
        .unwrap();
    assert_eq!(result[0].revision, 1);
    assert_eq!(result[1].revision, 2);
    assert_eq!(
        services.sync_edit_settings(&source.id, request).unwrap()[0].revision,
        1
    );
}
