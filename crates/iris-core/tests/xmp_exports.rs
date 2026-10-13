use iris_core::{Action, ColorLabel, MarkRequest, PhotoFilter, Services, Store};
use std::{fs, path::PathBuf};

fn setup() -> (tempfile::TempDir, Services, i64, i64, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    fs::create_dir(&root).unwrap();
    image::RgbImage::from_pixel(32, 32, image::Rgb([120, 130, 140]))
        .save(root.join("a.jpg"))
        .unwrap();
    let mut svc = Services::new(Store::open(temp.path().join("db.sqlite")).unwrap());
    let project = svc.create_project(&root).unwrap();
    svc.scan(project.id).unwrap();
    let photo = svc
        .photos(project.id, PhotoFilter::default())
        .unwrap()
        .remove(0);
    (temp, svc, project.id, photo.id, root)
}

fn mark(svc: &mut Services, project: i64, photo: i64, color: ColorLabel, rating: u8) {
    svc.mark_photos(
        project,
        MarkRequest {
            photo_ids: vec![photo],
            decision: Some(Action::Keep),
            rating: Some(rating),
            color_label: Some(color),
        },
    )
    .unwrap();
}

#[test]
fn exports_actual_marks_and_retains_original_adjustments_and_backup() {
    let (_temp, mut svc, project, photo, root) = setup();
    mark(&mut svc, project, photo, ColorLabel::Blue, 2);
    let original_photo = fs::read(root.join("a.jpg")).unwrap();
    let original = "<x:xmpmeta xmlns:x='adobe:ns:meta/'><r:RDF xmlns:r='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><r:Description xmlns:crs='http://ns.adobe.com/camera-raw-settings/1.0/' crs:Exposure2012='1.50'/></r:RDF></x:xmpmeta>";
    let path = root.join("a.xmp");
    fs::write(&path, original).unwrap();
    assert_eq!(svc.export_xmp(project, "all", false).unwrap().skipped, 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    assert_eq!(svc.export_xmp(project, "all", true).unwrap().written, 1);
    let output = fs::read_to_string(&path).unwrap();
    for text in [
        "xmp:Rating='2'",
        "xmp:Label='Blue'",
        "iris:Decision='keep'",
        "crs:Exposure2012='1.50'",
    ] {
        assert!(output.contains(text));
    }
    let backup = fs::read_dir(&root)
        .unwrap()
        .map(|p| p.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "bak"))
        .unwrap();
    assert_eq!(fs::read_to_string(backup).unwrap(), original);
    assert_eq!(fs::read(root.join("a.jpg")).unwrap(), original_photo);
    svc.export_xmp(project, "all", true).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), output);
}

#[test]
fn malformed_existing_sidecar_is_never_replaced_and_no_temporary_files_leak() {
    let (_temp, svc, project, _photo, root) = setup();
    let path = root.join("a.xmp");
    fs::write(&path, b"private existing metadata without XML").unwrap();
    assert!(svc
        .export_xmp(project, "all", true)
        .unwrap_err()
        .to_string()
        .contains("invalid XMP"));
    assert_eq!(
        fs::read(&path).unwrap(),
        b"private existing metadata without XML"
    );
    assert_eq!(fs::read_dir(root).unwrap().count(), 2);
}

#[test]
fn oversized_sidecars_are_rejected_before_replacement() {
    let (_temp, svc, project, _photo, root) = setup();
    let path = root.join("a.xmp");
    let bytes = vec![b' '; 8 * 1024 * 1024 + 1];
    fs::write(&path, &bytes).unwrap();
    assert!(svc
        .export_xmp(project, "all", true)
        .unwrap_err()
        .to_string()
        .contains("size limit"));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn raw_jpeg_shared_sidecar_checks_stars_and_colors_before_writing() {
    let (_temp, mut svc, project, photo, root) = setup();
    // Synthetic source bytes suffice: this checks sidecar planning, not RAW decoding.
    fs::write(root.join("a.dng"), b"synthetic source").unwrap();
    svc.store
        .conn
        .execute(
            "UPDATE photos SET capture_variant_id='capture' WHERE id=?1",
            [photo],
        )
        .unwrap();
    svc.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height,capture_variant_id) VALUES(?1,'a.dng','a.dng','raw',1,16,32,32,'capture')", [project]).unwrap();
    let raw = svc.store.conn.last_insert_rowid();
    mark(&mut svc, project, photo, ColorLabel::Green, 3);
    for (rating, color) in [(4, ColorLabel::Green), (3, ColorLabel::Red)] {
        mark(&mut svc, project, raw, color, rating);
        assert!(svc
            .export_xmp(project, "all", true)
            .unwrap_err()
            .to_string()
            .contains("multiple selected photos"));
        assert!(!root.join("a.xmp").exists());
    }
    mark(&mut svc, project, raw, ColorLabel::Green, 3);
    assert_eq!(svc.export_xmp(project, "all", false).unwrap().written, 1);
}

#[cfg(unix)]
#[test]
fn sidecar_links_cannot_modify_their_targets_in_either_mode() {
    let (temp, svc, project, _photo, root) = setup();
    let target = temp.path().join("target.xmp");
    fs::write(&target, b"original").unwrap();
    std::os::unix::fs::symlink(&target, root.join("a.xmp")).unwrap();
    for overwrite in [false, true] {
        assert!(svc.export_xmp(project, "all", overwrite).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }
}
