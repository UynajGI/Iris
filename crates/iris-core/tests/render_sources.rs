// SPDX-License-Identifier: GPL-3.0-or-later
use iris_core::{render::RenderContext, PhotoFilter, Services, Store};

#[test]
fn tiff_is_importable_previewable_and_edits_start_from_sixteen_bits() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let path = root.join("gradient.tiff");
    image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::from_fn(1024, 1, |x, _| {
        image::Rgb([x as u16 * 64; 3])
    })
    .save(&path)
    .unwrap();
    let original = std::fs::read(&path).unwrap();
    let mut services = Services::new(Store::open(temp.path().join("catalog.sqlite")).unwrap());
    let project = services.create_project(&root).unwrap();
    let report = services.scan(project.id).unwrap();
    assert_eq!(report.added, 1);
    assert!(report.errors.is_empty());
    let photos = services.photos(project.id, PhotoFilter::default()).unwrap();
    assert_eq!(photos[0].format, "tiff");
    let preview = iris_core::vision::decode_preview(&path, 256).unwrap();
    assert_eq!(preview.image.width(), 256);
    let context = RenderContext::default();
    let job = services.prepare_edit_source(photos[0].id).unwrap();
    let source = job.decode(&context).unwrap();
    assert_eq!(source.metadata.source_bit_depth, Some(16));
    let distinct = source
        .frame
        .pixels
        .iter()
        .map(|p| p[0].to_bits())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct.len(), 1024);
    let edit = services
        .create_edit_document(photos[0].id, "High precision", source.binding)
        .unwrap();
    let prepared = services
        .prepare_edit_render(&edit.id, edit.revision)
        .unwrap();
    let rendered = prepared.full_resolution(&context).unwrap();
    assert_eq!(rendered.pixels, source.frame.pixels);
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn changing_source_after_edit_creation_rejects_render() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    std::fs::create_dir(&root).unwrap();
    let path = root.join("original.png");
    image::RgbImage::from_pixel(4, 4, image::Rgb([100, 110, 120]))
        .save(&path)
        .unwrap();
    let mut services = Services::new(Store::open(temp.path().join("catalog.sqlite")).unwrap());
    let project = services.create_project(&root).unwrap();
    services.scan(project.id).unwrap();
    let photo = services
        .photos(project.id, PhotoFilter::default())
        .unwrap()
        .remove(0);
    let ctx = RenderContext::default();
    let source = services
        .prepare_edit_source(photo.id)
        .unwrap()
        .decode(&ctx)
        .unwrap();
    let edit = services
        .create_edit_document(photo.id, "Version", source.binding)
        .unwrap();
    let job = services
        .prepare_edit_render(&edit.id, edit.revision)
        .unwrap();
    image::RgbImage::from_pixel(4, 4, image::Rgb([120, 110, 100]))
        .save(&path)
        .unwrap();
    assert!(job.full_resolution(&ctx).is_err());
}

fn validation_fixture() -> (tempfile::TempDir, iris_core::services::EditRenderJob) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("synthetic.png");
    image::RgbImage::from_pixel(32, 24, image::Rgb([80, 120, 160]))
        .save(&path)
        .unwrap();
    let source = iris_core::services::EditSourceJob {
        photo_id: 1,
        source: path.clone(),
    }
    .decode(&RenderContext::default())
    .unwrap();
    let record = iris_core::services::EditRecord {
        id: "fixture".into(),
        photo_id: 1,
        name: "Synthetic".into(),
        revision: 0,
        document: iris_core::render::document::EditDocument::new(source.binding),
        created_at: "fixture".into(),
        updated_at: "fixture".into(),
    };
    (
        temp,
        iris_core::services::EditRenderJob {
            record,
            source: path,
            resources: Default::default(),
        },
    )
}

#[test]
fn warm_source_validation_uses_metadata_and_refresh_forces_hash() {
    use iris_core::services::SourceValidation;
    let (_temp, job) = validation_fixture();
    let context = RenderContext::default();
    assert_eq!(
        job.validate_preview_source(&context, false).unwrap(),
        SourceValidation::StrongHash
    );
    assert_eq!(
        job.validate_preview_source(&context, false).unwrap(),
        SourceValidation::MetadataCache
    );
    assert_eq!(
        job.validate_preview_source(&context, true).unwrap(),
        SourceValidation::StrongHash
    );
    job.clear_source_validation().unwrap();
    assert_eq!(
        job.validate_preview_source(&context, false).unwrap(),
        SourceValidation::StrongHash
    );
    let cancelled = RenderContext::default();
    cancelled.cancel();
    assert!(job.validate_preview_source(&cancelled, false).is_err());
}

#[test]
fn warm_source_mutation_and_replacement_are_rejected() {
    let (_temp, job) = validation_fixture();
    let context = RenderContext::default();
    job.validate_preview_source(&context, false).unwrap();
    let original = std::fs::read(&job.source).unwrap();
    let mut changed = original.clone();
    changed.push(0);
    std::fs::write(&job.source, &changed).unwrap();
    assert!(job.validate_preview_source(&context, false).is_err());
    std::fs::write(&job.source, &original).unwrap();
    job.verify_source(&context).unwrap();
    let replacement = job.source.with_extension("replacement");
    std::fs::write(&replacement, b"replaced").unwrap();
    std::fs::remove_file(&job.source).unwrap();
    std::fs::rename(replacement, &job.source).unwrap();
    assert!(job.validate_preview_source(&context, false).is_err());
}

#[test]
fn metadata_preserved_mutation_is_detected_by_force_or_cold_decode() {
    use iris_core::render::engine::{RenderEngine, RenderQuality, RenderRequest};
    let (_temp, job) = validation_fixture();
    let context = RenderContext::default();
    job.validate_preview_source(&context, false).unwrap();
    let metadata = std::fs::metadata(&job.source).unwrap();
    let mut bytes = std::fs::read(&job.source).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&job.source, bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&job.source)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
        .unwrap();
    // Unix change-time may reject this even in metadata mode. On filesystems
    // where all observed fields survive the write, forced verification is the
    // explicit integrity path. The test does not assume a timestamp resolution.
    assert!(job.validate_preview_source(&context, true).is_err());
    let engine = RenderEngine::new(256 * 1024 * 1024, 64 * 1024 * 1024).unwrap();
    let request = RenderRequest {
        revision: 0,
        session: "cold".into(),
        sequence: 1,
        quality: RenderQuality::Interactive,
        max_edge: 16,
    };
    assert!(job.preview(&engine, &request, &context).is_err());
    let full = RenderRequest {
        revision: 0,
        session: "full".into(),
        sequence: 1,
        quality: RenderQuality::Full,
        max_edge: 16,
    };
    assert!(job.preview(&engine, &full, &context).is_err());
}

#[test]
fn repeated_interactive_previews_preserve_pixels_and_allow_full_validation() {
    use iris_core::render::engine::{RenderEngine, RenderQuality, RenderRequest};
    let (_temp, job) = validation_fixture();
    let context = RenderContext::default();
    let engine = RenderEngine::new(256 * 1024 * 1024, 64 * 1024 * 1024).unwrap();
    let mut request = RenderRequest {
        revision: 0,
        session: "sliders".into(),
        sequence: 1,
        quality: RenderQuality::Interactive,
        max_edge: 16,
    };
    let first = job.preview(&engine, &request, &context).unwrap();
    request.sequence = 2;
    let second = job.preview(&engine, &request, &context).unwrap();
    assert_eq!(first.frame.pixels, second.frame.pixels);
    assert_eq!(
        job.validate_preview_source(&context, false).unwrap(),
        iris_core::services::SourceValidation::MetadataCache
    );
    request.sequence = 3;
    request.quality = RenderQuality::Full;
    let full = job.preview(&engine, &request, &context).unwrap();
    assert!(!full.approximate);
}
