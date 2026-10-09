use iris_core::{
    vision::{decode_preview_with_media, image_dimensions_with_media},
    Action, PhotoFilter, Services, Store,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn native() -> PathBuf {
    std::env::var_os("IRIS_PUBLIC_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/public-corpus")
        })
        .join("native")
}
fn media() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/media")
}
fn hash(path: &Path) -> Vec<u8> {
    Sha256::digest(fs::read(path).unwrap()).to_vec()
}

#[test]
#[ignore = "requires manifest-verified CC0 RAW corpus and standalone ExifTool"]
fn native_raw_previews_and_rawler_fallback_preserve_sources() {
    for name in [
        "canon-5d3.cr2",
        "nikon-d7000.nef",
        "sony-a7r.arw",
        "iphone-6s.dng",
    ] {
        let path = native().join(name);
        let original = hash(&path);
        let dimensions = image_dimensions_with_media(&path, Some(&media())).unwrap();
        let preview = decode_preview_with_media(&path, 1280, Some(&media())).unwrap();
        assert_eq!(preview.source, "raw_embedded_jpeg", "{name}");
        assert_eq!(
            (preview.original_width, preview.original_height),
            dimensions
        );
        assert!(preview.image.width().max(preview.image.height()) <= 1280);
        assert!(preview.image.width().min(preview.image.height()) >= 320);
        assert_eq!(hash(&path), original);
    }
    // An explicit model root without ExifTool exercises compiled local development.
    let temp = tempfile::tempdir().unwrap();
    let path = native().join("iphone-6s.dng");
    let original = hash(&path);
    let preview = decode_preview_with_media(&path, 1280, Some(&temp.path().join("media"))).unwrap();
    assert_eq!(preview.source, "raw_developed");
    assert!(preview.image.width().max(preview.image.height()) <= 1280);
    assert!(preview.image.pixels().any(|p| p[0] != p[1]));
    assert_eq!(hash(&path), original);
}

#[test]
#[ignore = "requires manifest-verified CC0 RAW corpus and standalone ExifTool"]
fn raw_jpeg_variants_scan_cache_link_decisions_and_undo() {
    let temp = tempfile::tempdir().unwrap();
    let photos = temp.path().join("照片");
    fs::create_dir(&photos).unwrap();
    let raw = photos.join("同拍摄.CR2");
    fs::copy(native().join("canon-5d3.cr2"), &raw).unwrap();
    let decoded = decode_preview_with_media(&raw, 1280, Some(&media())).unwrap();
    decoded.image.save(photos.join("同拍摄.JPG")).unwrap();
    fs::write(photos.join("broken.nef"), b"broken RAW").unwrap();
    let mut services = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap())
        .with_media_runtime_dir(media());
    let project = services.create_project(&photos).unwrap().id;
    let scan = services.scan(project).unwrap();
    assert_eq!(scan.added, 2);
    assert_eq!(scan.errors.len(), 1);
    let rows = services.photos(project, PhotoFilter::default()).unwrap();
    assert_eq!(rows[0].capture_variant_id, rows[1].capture_variant_id);
    assert!(rows.iter().any(|p| p.format == "raw"));
    for photo in &rows {
        assert!(!services.cached_image(photo.id, 320).unwrap().is_empty());
    }
    services
        .cache_migrate(project, &temp.path().join("moved-cache"))
        .unwrap();
    for photo in &rows {
        assert!(!services.cached_image(photo.id, 2560).unwrap().is_empty());
    }
    services
        .decisions(project, &[rows[0].id], Action::Keep, "human", true)
        .unwrap();
    assert!(services
        .photos(project, PhotoFilter::default())
        .unwrap()
        .iter()
        .all(|p| p.decision == Action::Keep));
    assert_eq!(
        services
            .export_copy(project, &temp.path().join("export"), "keep")
            .unwrap()
            .written,
        2
    );
    assert_eq!(
        services.export_xmp(project, "keep", false).unwrap().written,
        1
    );
    assert_eq!(hash(&raw), hash(&native().join("canon-5d3.cr2")));
    services.undo(project).unwrap();
    assert!(services
        .photos(project, PhotoFilter::default())
        .unwrap()
        .iter()
        .all(|p| p.decision == Action::Pending));
    services
        .decisions(project, &[rows[0].id], Action::Reject, "human", true)
        .unwrap();
    let plan = services.quarantine_preview(project).unwrap();
    services.quarantine_commit(&plan.id).unwrap();
    assert!(!raw.exists());
    services.quarantine_restore(&plan.id).unwrap();
    assert_eq!(hash(&raw), hash(&native().join("canon-5d3.cr2")));
}

#[test]
#[ignore = "requires native CC0 RAW and standalone ExifTool; changes only a disposable copy"]
#[cfg(windows)]
fn raw_orientation_rotates_embedded_preview_once() {
    use std::{os::windows::process::CommandExt, process::Command};
    let original = native().join("canon-5d3.cr2");
    let source_hash = hash(&original);
    let temporary = tempfile::tempdir().unwrap();
    let copy = temporary.path().join("orientation.cr2");
    fs::copy(&original, &copy).unwrap();
    let exiftool = media().parent().unwrap().join("raw/bin/exiftool.exe");
    for orientation in [1, 6] {
        let output = Command::new(&exiftool)
            .creation_flags(0x08000000)
            .args([
                "-config",
                "",
                "-overwrite_original",
                &format!("-Orientation#={orientation}"),
            ])
            .arg(&copy)
            .output()
            .unwrap();
        assert!(output.status.success());
        let preview = decode_preview_with_media(&copy, 1280, Some(&media())).unwrap();
        assert_eq!(preview.source, "raw_embedded_jpeg");
        if orientation == 1 {
            preview
                .image
                .save(temporary.path().join("upright.png"))
                .unwrap();
        } else {
            let upright = image::open(temporary.path().join("upright.png"))
                .unwrap()
                .to_rgb8();
            assert_eq!(preview.image, image::imageops::rotate90(&upright));
        }
    }
    assert_eq!(hash(&original), source_hash);
}
