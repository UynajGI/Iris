// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

type Entry = (u16, u16, u32, Vec<u8>);
fn ifd(bytes: &mut Vec<u8>, entries: Vec<Entry>) -> u32 {
    let offset = bytes.len();
    bytes.extend((entries.len() as u16).to_le_bytes());
    bytes.resize(bytes.len() + entries.len() * 12 + 4, 0);
    for (i, (tag, kind, count, value)) in entries.into_iter().enumerate() {
        let start = offset + 2 + i * 12;
        bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[start + 2..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
        if value.len() <= 4 {
            bytes[start + 8..start + 8 + value.len()].copy_from_slice(&value);
        } else {
            let pointer = bytes.len() as u32;
            bytes[start + 8..start + 12].copy_from_slice(&pointer.to_le_bytes());
            bytes.extend(value);
        }
    }
    offset as u32
}
fn ascii(tag: u16, value: &str) -> Entry {
    (
        tag,
        2,
        value.len() as u32 + 1,
        [value.as_bytes(), &[0]].concat(),
    )
}
fn rational(tag: u16, n: i32, d: i32, signed: bool) -> Entry {
    (
        tag,
        if signed { 10 } else { 5 },
        1,
        [n.to_le_bytes(), d.to_le_bytes()].concat(),
    )
}
fn fixture(exposure_denominator: i32) -> Vec<u8> {
    let mut bytes = b"II\x2a\0\0\0\0\0".to_vec();
    let exif = ifd(
        &mut bytes,
        vec![
            rational(0x829a, 1, exposure_denominator, false),
            rational(0x829d, 28, 10, false),
            (0x8827, 3, 1, 400u16.to_le_bytes().to_vec()),
            ascii(0x9003, "2026:10:16 12:34:56"),
            ascii(0x9011, "+08:00"),
            rational(0x9204, -2, 3, true),
            rational(0x920a, 85, 1, false),
            ascii(0xa434, "Fixture 85mm"),
        ],
    );
    let primary = ifd(
        &mut bytes,
        vec![
            ascii(0x010f, "FixtureCo"),
            ascii(0x0110, "Synthetic camera"),
            (0x0112, 3, 1, 6u16.to_le_bytes().to_vec()),
            ascii(0x013b, "invalid\nauthor"),
            (0x8769, 4, 1, exif.to_le_bytes().to_vec()),
        ],
    );
    bytes[4..8].copy_from_slice(&primary.to_le_bytes());
    bytes
}

#[test]
fn extracts_camera_exposure_lens_and_signed_values_without_guessing_missing_tags() {
    let exif = exif::Reader::new().read_raw(fixture(250)).unwrap();
    let metadata = from_exif(&exif);
    assert_eq!(metadata.status, PhotoMetadataStatus::Available);
    assert_eq!(metadata.camera_make.as_deref(), Some("FixtureCo"));
    assert_eq!(metadata.camera_model.as_deref(), Some("Synthetic camera"));
    assert_eq!(metadata.lens_model.as_deref(), Some("Fixture 85mm"));
    assert_eq!(metadata.captured_at.as_deref(), Some("2026:10:16 12:34:56"));
    assert_eq!(metadata.captured_offset.as_deref(), Some("+08:00"));
    assert_eq!(metadata.exposure_seconds, Some(1.0 / 250.0));
    assert_eq!(metadata.f_number, Some(2.8));
    assert_eq!(metadata.iso, Some(400));
    assert_eq!(metadata.focal_mm, Some(85.0));
    assert_eq!(metadata.exposure_bias_ev, Some(-2.0 / 3.0));
    assert_eq!(metadata.orientation, Some(6));
    assert!(metadata.artist.is_none());
    assert!(metadata.flash.is_none());
    assert!(metadata.lens_make.is_none());
    assert!(metadata.focal_35mm.is_none());
    let invalid = exif::Reader::new().read_raw(fixture(0)).unwrap();
    assert!(from_exif(&invalid).exposure_seconds.is_none());
}

#[test]
fn read_is_bounded_cancellable_and_reports_unreadable_sources() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("synthetic.tiff");
    std::fs::write(&path, fixture(250)).unwrap();
    assert_eq!(read(&path, &AtomicBool::new(false)).unwrap().iso, Some(400));
    assert!(read(&path, &AtomicBool::new(true)).is_err());
    std::fs::write(&path, b"broken container").unwrap();
    assert_eq!(
        read(&path, &AtomicBool::new(false)).unwrap().status,
        PhotoMetadataStatus::Unreadable
    );
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(17 * 1024 * 1024)
        .unwrap();
    assert_eq!(
        read(&path, &AtomicBool::new(false)).unwrap().status,
        PhotoMetadataStatus::ReadLimit
    );
    std::fs::write(&path, [0xff, 0xd8, 0xff, 0xd9]).unwrap();
    assert_eq!(
        read(&path, &AtomicBool::new(false)).unwrap().status,
        PhotoMetadataStatus::NotFound
    );
}
