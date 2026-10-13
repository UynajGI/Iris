// SPDX-License-Identifier: GPL-3.0-or-later
use iris_core::{
    services::{
        EditRecord, EditResourceKind, RenderedExportRequest, RenderedExportState, SaveEdit,
    },
    Services, Store,
};
use iris_render::{
    color::ReferenceKind,
    document::{hash_bytes, Node, Operation, SourceBinding},
    lut::LutDomain,
    output::OutputFormat,
    RenderContext,
};
use rusqlite::params;

fn fixture() -> (tempfile::TempDir, Services, EditRecord) {
    fixture_size(4)
}

fn fixture_size(edge: u32) -> (tempfile::TempDir, Services, EditRecord) {
    let temp = tempfile::tempdir().unwrap();
    let photos = temp.path().join("photos");
    std::fs::create_dir(&photos).unwrap();
    std::fs::create_dir(temp.path().join("exports")).unwrap();
    let path = photos.join("source.png");
    image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_fn(edge, edge, |x, y| {
        image::Rgba([
            10000 + ((x + y * edge) % 1024) as u16 * 17,
            20000,
            30000,
            65535,
        ])
    })
    .save(&path)
    .unwrap();
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let project = services.create_project(&photos).unwrap();
    services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'source.png','source.png','png',1,1,?2,?2)",params![project.id,edge]).unwrap();
    let photo = services.store.conn.last_insert_rowid();
    let record = services
        .create_edit_document(
            photo,
            "Original",
            SourceBinding {
                photo_id: photo.to_string(),
                fingerprint: hash_bytes(&std::fs::read(path).unwrap()),
                width: edge,
                height: edge,
                reference_kind: ReferenceKind::Display,
                raw_white_balance: None,
            },
        )
        .unwrap();
    (temp, services, record)
}
fn request(temp: &tempfile::TempDir, record: &EditRecord, name: &str) -> RenderedExportRequest {
    RenderedExportRequest {
        document_id: record.id.clone(),
        expected_revision: record.revision,
        destination_directory: temp.path().join("exports").to_string_lossy().into_owned(),
        filename: name.into(),
        format: OutputFormat::Png16,
    }
}
fn complete(services: &mut Services, id: &str) -> iris_core::services::RenderedExportResult {
    let context = RenderContext::new(128 * 1024 * 1024);
    let job = services.prepare_rendered_export(id).unwrap();
    let mut prepared = job.render(&context).unwrap();
    services
        .authorize_rendered_export_publication(&mut prepared)
        .unwrap();
    let result = prepared.publish(&context).unwrap();
    services.finish_rendered_export(&result).unwrap();
    result
}

#[test]
fn export_roundtrip_preserves_source_and_16bit_differences() {
    let (temp, mut services, record) = fixture();
    let source = services.photo_path(record.photo_id).unwrap();
    let original = std::fs::read(&source).unwrap();
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "out.png")])
        .unwrap();
    let result = complete(&mut services, &queued[0].id);
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let bytes = std::fs::read(&result.destination).unwrap();
    assert_eq!(result.hash, hash_bytes(&bytes));
    assert_eq!(result.bytes, bytes.len() as u64);
    let image = image::load_from_memory(&bytes).unwrap();
    assert_eq!(image.color(), image::ColorType::Rgba16);
    let pixels = image.to_rgba16();
    assert!(pixels.get_pixel(1, 0).0[0] > pixels.get_pixel(0, 0).0[0]);
    assert!(pixels.get_pixel(1, 0).0[0] - pixels.get_pixel(0, 0).0[0] < 257);
    assert!(matches!(
        services.rendered_export(&queued[0].id).unwrap().state,
        RenderedExportState::Completed
    ));
}

#[test]
fn existing_files_originals_hardlinks_and_late_collisions_are_never_overwritten() {
    let (temp, mut services, record) = fixture();
    let original = services.photo_path(record.photo_id).unwrap();
    let mut original_request = request(&temp, &record, "source.png");
    original_request.destination_directory =
        original.parent().unwrap().to_string_lossy().into_owned();
    assert!(services
        .queue_rendered_exports(&[original_request])
        .is_err());
    let linked = temp.path().join("exports/hardlink.png");
    std::fs::hard_link(&original, &linked).unwrap();
    assert!(services
        .queue_rendered_exports(&[request(&temp, &record, "hardlink.png")])
        .is_err());
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "late.png")])
        .unwrap();
    let context = RenderContext::new(128 * 1024 * 1024);
    let job = services.prepare_rendered_export(&queued[0].id).unwrap();
    let mut prepared = job.render(&context).unwrap();
    std::fs::write(temp.path().join("exports/late.png"), b"existing").unwrap();
    services
        .authorize_rendered_export_publication(&mut prepared)
        .unwrap();
    assert!(prepared.publish(&context).is_err());
    assert_eq!(
        std::fs::read(temp.path().join("exports/late.png")).unwrap(),
        b"existing"
    );
}

#[test]
fn cancellation_blocks_publication_even_after_render_finishes() {
    let (temp, mut services, record) = fixture();
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "cancel.png")])
        .unwrap();
    let context = RenderContext::new(128 * 1024 * 1024);
    let job = services.prepare_rendered_export(&queued[0].id).unwrap();
    let mut prepared = job.render(&context).unwrap();
    services.cancel_rendered_export(&queued[0].id).unwrap();
    assert!(services
        .authorize_rendered_export_publication(&mut prepared)
        .is_err());
    assert!(prepared.publish(&context).is_err());
    assert!(!temp.path().join("exports/cancel.png").exists());
    assert_eq!(
        std::fs::read_dir(temp.path().join("exports"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn queued_snapshot_survives_new_revision_document_deletion_and_restart() {
    let (temp, mut services, record) = fixture();
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "snapshot.png")])
        .unwrap();
    let mut changed = record.document.clone();
    changed.output.space = iris_render::color::OutputSpace::LinearRec2020;
    let revised = services
        .save_edit_document(
            &record.id,
            SaveEdit {
                expected_revision: record.revision,
                request_id: "later-edit".into(),
                document: changed,
            },
        )
        .unwrap();
    services
        .delete_edit_document(&record.id, revised.revision)
        .unwrap();
    drop(services);
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    assert_eq!(services.recover_rendered_exports().unwrap(), 0);
    assert_eq!(
        services.rendered_export(&queued[0].id).unwrap().revision,
        record.revision
    );
    let result = complete(&mut services, &queued[0].id);
    let image = image::open(result.destination).unwrap().to_rgba16();
    assert!(
        image.get_pixel(0, 0).0[0] > 9000,
        "queued sRGB output must not adopt later linear output configuration"
    );
}

#[test]
fn pinned_lut_bytes_survive_resource_file_deletion() {
    let (temp, mut services, record) = fixture();
    let resource = services
        .import_edit_resource(EditResourceKind::Lut, b"LUT_1D_SIZE 2\n0 0 0\n1 1 1\n")
        .unwrap();
    let mut document = record.document.clone();
    document.nodes.push(Node {
        id: "lut".into(),
        name: "Identity".into(),
        version: 1,
        enabled: true,
        operation: Operation::Lut {
            resource: resource.hash.clone(),
            strength: 1.0,
            domain: LutDomain::LinearRec2020,
        },
        inputs: vec![document.output_node.clone()],
    });
    document.output_node = "lut".into();
    let revised = services
        .save_edit_document(
            &record.id,
            SaveEdit {
                expected_revision: record.revision,
                request_id: "lut".into(),
                document,
            },
        )
        .unwrap();
    let queued = services
        .queue_rendered_exports(&[request(&temp, &revised, "lut.png")])
        .unwrap();
    services
        .delete_edit_document(&record.id, revised.revision)
        .unwrap();
    std::fs::remove_file(temp.path().join("edit-resources-v1").join(&resource.hash)).unwrap();
    complete(&mut services, &queued[0].id);
}

#[test]
fn interrupted_publication_is_reconciled_without_rewriting_output() {
    let (temp, mut services, record) = fixture();
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "recovered.png")])
        .unwrap();
    let context = RenderContext::new(128 * 1024 * 1024);
    let mut prepared = services
        .prepare_rendered_export(&queued[0].id)
        .unwrap()
        .render(&context)
        .unwrap();
    services
        .authorize_rendered_export_publication(&mut prepared)
        .unwrap();
    let result = prepared.publish(&context).unwrap(); // Simulate crash before SQLite finish.
    drop(services);
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    assert_eq!(services.recover_rendered_exports().unwrap(), 1);
    let recovered = services
        .prepare_rendered_export_recovery(&result.id)
        .unwrap()
        .inspect(&context)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.hash, result.hash);
    services.finish_rendered_export(&recovered).unwrap();
    assert_eq!(
        hash_bytes(&std::fs::read(result.destination).unwrap()),
        result.hash
    );
}

#[test]
fn invalid_batch_is_atomic_and_changed_source_fails_closed() {
    let (temp, mut services, record) = fixture();
    assert!(services
        .queue_rendered_exports(&[
            request(&temp, &record, "good.png"),
            request(&temp, &record, "../bad.png")
        ])
        .is_err());
    assert!(services.rendered_exports(100, 0).unwrap().is_empty());
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "changed.png")])
        .unwrap();
    std::fs::write(
        services.photo_path(record.photo_id).unwrap(),
        b"source changed",
    )
    .unwrap();
    let job = services.prepare_rendered_export(&queued[0].id).unwrap();
    assert!(job.render(&RenderContext::default()).is_err());
    assert!(!temp.path().join("exports/changed.png").exists());
}

#[cfg(unix)]
#[test]
fn destination_symlinks_and_replaced_parent_are_rejected() {
    let (temp, mut services, record) = fixture();
    let root = temp.path().join("exports");
    std::os::unix::fs::symlink(temp.path().join("absent"), root.join("dangling.png")).unwrap();
    assert!(services
        .queue_rendered_exports(&[request(&temp, &record, "dangling.png")])
        .is_err());
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "parent.png")])
        .unwrap();
    let context = RenderContext::new(128 * 1024 * 1024);
    let mut prepared = services
        .prepare_rendered_export(&queued[0].id)
        .unwrap()
        .render(&context)
        .unwrap();
    services
        .authorize_rendered_export_publication(&mut prepared)
        .unwrap();
    std::fs::rename(&root, temp.path().join("moved")).unwrap();
    let elsewhere = temp.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &root).unwrap();
    assert!(prepared.publish(&context).is_err());
    assert!(!elsewhere.join("parent.png").exists());
}

#[test]
fn export_storage_rejects_negative_bytes_and_out_of_range_result() {
    let (temp, mut services, record) = fixture();
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "integer.png")])
        .unwrap();
    let id = &queued[0].id;
    services
        .store
        .conn
        .execute(
            "UPDATE rendered_export_jobs SET result_bytes=-1 WHERE id=?1",
            [id],
        )
        .unwrap();
    assert!(services.rendered_export(id).is_err());
    services
        .store
        .conn
        .execute(
            "UPDATE rendered_export_jobs SET result_bytes=NULL WHERE id=?1",
            [id],
        )
        .unwrap();
    let result = iris_core::services::RenderedExportResult {
        id: id.clone(),
        destination: "unused".into(),
        hash: "0".repeat(64),
        bytes: u64::MAX,
    };
    assert!(services.finish_rendered_export(&result).is_err());
    assert!(matches!(
        services.rendered_export(id).unwrap().state,
        RenderedExportState::Queued
    ));
    services
        .store
        .conn
        .execute(
            "UPDATE rendered_export_jobs SET result_bytes=?2 WHERE id=?1",
            params![id, i64::MAX],
        )
        .unwrap();
    assert_eq!(
        services.rendered_export(id).unwrap().result_bytes,
        Some(i64::MAX as u64)
    );
}

#[test]
fn export_snapshot_revision_must_fit_and_match_sqlite_revision() {
    let (temp, mut services, record) = fixture();
    let queued = services
        .queue_rendered_exports(&[request(&temp, &record, "revision.png")])
        .unwrap();
    let id = &queued[0].id;
    let original: String = services
        .store
        .conn
        .query_row(
            "SELECT snapshot_json FROM rendered_export_jobs WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    let mut snapshot: serde_json::Value = serde_json::from_str(&original).unwrap();
    snapshot["record"]["revision"] = serde_json::json!(u64::MAX);
    services
        .store
        .conn
        .execute(
            "UPDATE rendered_export_jobs SET snapshot_json=?2 WHERE id=?1",
            params![id, snapshot.to_string()],
        )
        .unwrap();
    assert!(services.rendered_export(id).is_err());
    services
        .store
        .conn
        .execute(
            "UPDATE rendered_export_jobs SET snapshot_json=?2,revision=1 WHERE id=?1",
            params![id, original],
        )
        .unwrap();
    assert!(services.rendered_export(id).is_err());
    services
        .store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints=ON")
        .unwrap();
    services
        .store
        .conn
        .execute(
            "UPDATE rendered_export_jobs SET revision=-1 WHERE id=?1",
            [id],
        )
        .unwrap();
    services
        .store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints=OFF")
        .unwrap();
    assert!(services.prepare_rendered_export(id).is_err());
}

#[test]
fn export_counts_working_and_output_frames_once_but_keeps_codec_budget() {
    let (temp, mut services, record) = fixture_size(512);
    let queued = services
        .queue_rendered_exports(&[
            request(&temp, &record, "tight.png"),
            request(&temp, &record, "adequate.png"),
        ])
        .unwrap();
    // Each 512x512 FP32 frame is 4 MiB. Streaming output legitimately needs
    // working + encoded frames (8 MiB), codec admission (8 MiB) and a small ICC.
    // Color conversion peaks at three real frames plus 192 KiB, below this.
    // An extra admission for the already tracked working Arc would need 20 MiB.
    let tight = RenderContext::new(15 * 1024 * 1024);
    let job = services.prepare_rendered_export(&queued[0].id).unwrap();
    assert!(
        job.render(&tight).is_err(),
        "must retain real codec/output admission"
    );
    let adequate = RenderContext::new(17 * 1024 * 1024);
    let job = services.prepare_rendered_export(&queued[1].id).unwrap();
    let prepared = job
        .render(&adequate)
        .expect("two frames plus codec scratch fit without double charging");
    drop(prepared);
    assert!(!temp.path().join("exports/adequate.png").exists());
}
