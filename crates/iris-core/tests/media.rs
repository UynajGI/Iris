use image::{ImageEncoder, Rgba, RgbaImage};
use iris_core::{
    vision::{decode_preview, decode_preview_with_media, image_dimensions_with_media},
    PhotoFilter, Services, Store,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn media_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/media")
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/media")
        .join(name)
}

fn rgba(path: &Path, orientation: u16) {
    let mut image = RgbaImage::from_pixel(64, 40, Rgba([230, 20, 30, 255]));
    for y in 20..40 {
        for x in 0..64 {
            image.put_pixel(x, y, Rgba([0, 0, 200, 0]));
        }
    }
    // TIFF IFD with a single SHORT Orientation entry.
    let mut exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&[0; 6]);
    let file = fs::File::create(path).unwrap();
    if path.extension().unwrap() == "png" {
        let mut encoder = image::codecs::png::PngEncoder::new(file);
        encoder.set_exif_metadata(exif).unwrap();
        encoder
            .write_image(&image, 64, 40, image::ExtendedColorType::Rgba8)
            .unwrap();
    } else {
        let mut encoder = image::codecs::webp::WebPEncoder::new_lossless(file);
        encoder.set_exif_metadata(exif).unwrap();
        encoder
            .write_image(&image, 64, 40, image::ExtendedColorType::Rgba8)
            .unwrap();
    }
}

#[test]
fn png_webp_decode_bound_orientation_alpha_and_originals() {
    let temp = tempfile::tempdir().unwrap();
    for ext in ["png", "webp"] {
        for orientation in [1, 6] {
            let path = temp.path().join(format!("image-{orientation}.{ext}"));
            rgba(&path, orientation);
            let original = fs::read(&path).unwrap();
            let decoded = decode_preview(&path, 1280).unwrap();
            assert_eq!((decoded.original_width, decoded.original_height), (64, 40));
            assert_eq!(decoded.orientation, orientation as u32);
            assert_eq!(
                decoded.image.dimensions(),
                if orientation == 6 { (40, 64) } else { (64, 40) }
            );
            assert!(
                decoded.image.pixels().any(|p| p.0 == [255, 255, 255]),
                "transparent pixels composite to white"
            );
            let reduced = decode_preview(&path, 16).unwrap();
            assert_eq!(reduced.image.width().max(reduced.image.height()), 16);
            assert_eq!(fs::read(path).unwrap(), original);
        }
    }
}

#[test]
fn scanner_records_png_webp_formats_and_generates_jpeg_caches() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("photos");
    fs::create_dir(&root).unwrap();
    rgba(&root.join("one.png"), 1);
    rgba(&root.join("two.webp"), 1);
    fs::write(root.join("unsupported.txt"), b"unsupported").unwrap();
    let mut service = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let id = service.create_project(root).unwrap().id;
    let scan = service.scan(id).unwrap();
    assert_eq!((scan.added, scan.skipped), (2, 1));
    assert!(scan.errors.is_empty());
    for format in ["png", "webp"] {
        let photos = service
            .photos(
                id,
                PhotoFilter {
                    format: Some(format.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(photos.len(), 1);
        assert_eq!((photos[0].width, photos[0].height), (64, 40));
        let preview = service.cached_image(photos[0].id, 2560).unwrap();
        assert_eq!(image::load_from_memory(&preview).unwrap().width(), 64);
    }
    assert_eq!(service.scan(id).unwrap().unchanged, 2);
}

#[test]
fn missing_heic_runtime_is_explicit_and_corrupt_other_formats_do_not_scan() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing-runtime");
    let error = decode_preview_with_media(&fixture("quadrants.heic"), 1280, Some(&missing))
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("HEIC runtime missing"));
    let root = temp.path().join("photos");
    fs::create_dir(&root).unwrap();
    for ext in ["png", "webp"] {
        fs::write(root.join(format!("bad.{ext}")), b"corrupt").unwrap();
    }
    let mut service = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap());
    let id = service.create_project(root).unwrap().id;
    let scan = service.scan(id).unwrap();
    assert_eq!(scan.added, 0);
    assert_eq!(scan.errors.len(), 2);
}

#[test]
fn oversized_png_header_is_rejected_before_pixel_allocation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("huge.png");
    let mut bytes = fs::read(fixture("quadrants.png")).unwrap();
    bytes[16..20].copy_from_slice(&40_000u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&40_000u32.to_be_bytes());
    // Recompute the PNG IHDR CRC so rejection exercises size limits rather
    // than a damaged-header checksum. IDAT is never decoded for this header.
    let mut crc = !0u32;
    for byte in &bytes[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32.wrapping_mul(crc & 1));
        }
    }
    bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
    fs::write(&path, bytes).unwrap();
    let error = decode_preview(&path, 1280).err().unwrap();
    assert!(format!("{error:#}").to_lowercase().contains("limit"));
}

#[test]
#[ignore = "requires locally built offline HEIC runtime; run tools/setup-heif-runtime.py"]
fn real_heic_pixels_item_rotation_and_bounded_preview() {
    for (name, dimensions) in [
        ("quadrants.heic", (64, 40)),
        ("quadrants-rotated.heic", (40, 64)),
    ] {
        let path = fixture(name);
        let original = fs::read(&path).unwrap();
        assert_eq!(&original[4..8], b"ftyp");
        assert_eq!(
            image_dimensions_with_media(&path, Some(&media_dir())).unwrap(),
            dimensions
        );
        let decoded = decode_preview_with_media(&path, 1280, Some(&media_dir())).unwrap();
        assert_eq!(decoded.image.dimensions(), dimensions);
        assert_eq!(
            decoded.orientation, 1,
            "HEIF item rotation is already applied"
        );
        let (x, y) = if name.contains("rotated") {
            (30, 8)
        } else {
            (8, 8)
        };
        let red = decoded.image.get_pixel(x, y);
        assert!(
            red[0] > 190 && red[1] < 60 && red[2] < 70,
            "actual decoded red quadrant: {red:?}"
        );
        let reduced = decode_preview_with_media(&path, 16, Some(&media_dir())).unwrap();
        assert_eq!(reduced.image.width().max(reduced.image.height()), 16);
        assert_eq!(fs::read(path).unwrap(), original);
    }
}

#[test]
#[ignore = "requires locally built offline HEIC runtime; run tools/setup-heif-runtime.py"]
fn real_heic_scan_cache_migration_and_corrupt_input() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("照片");
    fs::create_dir(&root).unwrap();
    fs::copy(fixture("quadrants.heic"), root.join("色块.HEIC")).unwrap();
    fs::copy(fixture("quadrants-rotated.heic"), root.join("旋转.heif")).unwrap();
    let mut service = Services::new(Store::open(temp.path().join("library.sqlite")).unwrap())
        .with_media_runtime_dir(media_dir());
    let id = service.create_project(&root).unwrap().id;
    let scan = service.scan(id).unwrap();
    assert_eq!(scan.added, 2);
    assert!(scan.errors.is_empty(), "{:?}", scan.errors);
    let photos = service
        .photos(
            id,
            PhotoFilter {
                format: Some("heic".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(photos.len(), 2);
    for photo in &photos {
        let cache = service.cached_image(photo.id, 320).unwrap();
        assert_eq!(
            image::load_from_memory(&cache).unwrap().width(),
            photo.width
        );
    }
    service
        .cache_migrate(id, &temp.path().join("new-cache"))
        .unwrap();
    for photo in &photos {
        assert!(!service.cached_image(photo.id, 2560).unwrap().is_empty());
    }
    fs::write(root.join("broken.heic"), b"not HEIF").unwrap();
    let scan = service.scan(id).unwrap();
    assert_eq!(scan.unchanged, 2);
    assert_eq!(scan.errors.len(), 1);
    assert!(scan.errors[0].contains("HEIC decode error"));
}
