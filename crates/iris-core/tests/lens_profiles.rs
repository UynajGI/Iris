// SPDX-License-Identifier: GPL-3.0-or-later
use iris_core::{services::EditResourceKind, Services, Store};
use iris_render::{
    color::ReferenceKind,
    document::{hash_bytes, SourceBinding},
    RenderContext,
};
use serde_json::json;

// Synthetic EXIF-only TIFF: primary Make/Model and an Exif IFD containing
// LensModel, FocalLength and FNumber. No personal file or camera fixture needed.
fn source_exif() -> Vec<u8> {
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    bytes.extend(3_u16.to_le_bytes());
    entry(&mut bytes, 0x010f, 2, 10, 50);
    entry(&mut bytes, 0x0110, 2, 12, 60);
    entry(&mut bytes, 0x8769, 4, 1, 72);
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(b"FixtureCo\0FixtureBody\0");
    bytes.extend(3_u16.to_le_bytes());
    entry(&mut bytes, 0x829d, 5, 1, 114);
    entry(&mut bytes, 0x920a, 5, 1, 122);
    entry(&mut bytes, 0xa434, 2, 12, 130);
    bytes.extend(0_u32.to_le_bytes());
    for value in [4_u32, 1, 50, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(b"FixtureLens\0");
    bytes
}
fn entry(bytes: &mut Vec<u8>, tag: u16, kind: u16, count: u32, value: u32) {
    bytes.extend(tag.to_le_bytes());
    bytes.extend(kind.to_le_bytes());
    bytes.extend(count.to_le_bytes());
    bytes.extend(value.to_le_bytes());
}
fn calibration() -> Vec<u8> {
    serde_json::to_vec(&json!({"schema_version":1,"name":"Synthetic", "model":"iris_inverse_radial_v1", "coordinates":"oriented_uncropped_image_edges_half_short_edge_v1", "camera_make":"FixtureCo", "camera_model":"FixtureBody", "lens_model":"FixtureLens", "calibrated_dimensions":[32,24], "focal_mm":[40,60], "f_number":[2.8,8], "correction":{"radial":[-0.02,0.001],"chromatic_aberration":[0.002,-0.001],"vignette_stops":0.3}})).unwrap()
}
#[test]
fn stored_profile_uses_original_exif_without_mutating_document() {
    let temp = tempfile::tempdir().unwrap();
    let photos = temp.path().join("photos");
    std::fs::create_dir(&photos).unwrap();
    let source = source_exif();
    std::fs::write(photos.join("synthetic.tiff"), &source).unwrap();
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let project = services.create_project(&photos).unwrap();
    services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'synthetic.tiff','synthetic.tiff','tiff',1,1,32,24)",[project.id]).unwrap();
    let photo = services.store.conn.last_insert_rowid();
    let edit = services
        .create_edit_document(
            photo,
            "Original",
            SourceBinding {
                photo_id: photo.to_string(),
                fingerprint: hash_bytes(&source),
                width: 32,
                height: 24,
                reference_kind: ReferenceKind::Display,
                raw_white_balance: None,
            },
        )
        .unwrap();
    let resource = services
        .import_edit_resource(EditResourceKind::LensProfile, &calibration())
        .unwrap();
    let job = services
        .prepare_lens_profile_suggestion(&edit.id, 0, &resource.hash)
        .unwrap();
    let suggestion = job.suggest(&RenderContext::default()).unwrap();
    assert!(suggestion.matching.applicable);
    assert_eq!(suggestion.capture.focal_mm, Some(50.0));
    assert_eq!(suggestion.capture.f_number, Some(4.0));
    assert_eq!(
        services.edit_document(&edit.id).unwrap().document,
        edit.document
    );
    assert!(services
        .prepare_lens_profile_suggestion(&edit.id, 1, &resource.hash)
        .is_err());
    let cancelled = RenderContext::default();
    cancelled.cancel();
    assert!(job.suggest(&cancelled).is_err());
    std::fs::write(photos.join("synthetic.tiff"), b"externally modified").unwrap();
    assert!(job.suggest(&RenderContext::default()).is_err());
    std::fs::write(
        temp.path().join("edit-resources-v1").join(&resource.hash),
        b"tampered",
    )
    .unwrap();
    assert!(services.read_edit_resource(&resource.hash).is_err());
}
