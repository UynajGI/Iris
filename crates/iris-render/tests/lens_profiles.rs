// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{geometry::Geometry, lens_profile::*};

fn profile() -> LensProfile {
    LensProfile {
        schema_version: 1,
        name: "Synthetic calibration".into(),
        model: LensCalibrationModel::IrisInverseRadialV1,
        coordinates: LensCoordinates::OrientedUncroppedImageEdgesHalfShortEdgeV1,
        camera_make: "FixtureCo".into(),
        camera_model: "FixtureBody".into(),
        lens_model: "FixtureLens".into(),
        calibrated_dimensions: [32, 24],
        focal_mm: [40.0, 60.0],
        f_number: [2.8, 8.0],
        correction: LensCorrection {
            radial: [-0.02, 0.001],
            chromatic_aberration: [0.002, -0.001],
            vignette_stops: 0.3,
        },
    }
}
fn capture() -> LensCaptureMetadata {
    LensCaptureMetadata {
        camera_make: Some(" fixtureco ".into()),
        camera_model: Some("FixtureBody".into()),
        lens_model: Some("FixtureLens".into()),
        focal_mm: Some(50.0),
        f_number: Some(4.0),
        dimensions: [24, 32],
    }
}
#[test]
fn profile_matches_evidence_and_merges_only_optics() {
    let profile = profile();
    let matched = profile.match_capture(&capture()).unwrap();
    assert!(matched.applicable);
    let geometry = Geometry {
        crop: [0.1, 0.1, 0.8, 0.8],
        rotation_degrees: 12.0,
        perspective: [1.0, 0.01, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        ..Default::default()
    };
    let merged = matched.correction.unwrap().merge(&geometry).unwrap();
    assert_eq!(merged.crop, geometry.crop);
    assert_eq!(merged.rotation_degrees, geometry.rotation_degrees);
    assert_eq!(merged.perspective, geometry.perspective);
    assert_eq!(merged.radial, profile.correction.radial);
    assert_eq!(
        merged.chromatic_aberration,
        profile.correction.chromatic_aberration
    );
}
#[test]
fn profile_refuses_missing_mismatched_and_uncalibrated_capture() {
    let profile = profile();
    let mut capture = capture();
    capture.camera_make = None;
    capture.lens_model = Some("FixtureLens II".into());
    capture.focal_mm = Some(61.0);
    capture.f_number = Some(11.0);
    capture.dimensions = [16, 12];
    let result = profile.match_capture(&capture).unwrap();
    assert!(!result.applicable);
    assert!(result.correction.is_none());
    assert_eq!(
        result.issues,
        [
            "camera_make_missing",
            "lens_model_mismatch",
            "focal_mm_outside_calibration",
            "f_number_outside_calibration",
            "calibrated_dimensions_mismatch"
        ]
    );
}
#[test]
fn invalid_calibration_is_rejected_before_use() {
    let mut invalid = profile();
    invalid.correction.radial[0] = f32::NAN;
    assert!(invalid.validate().is_err());
    let mut json = serde_json::to_value(profile()).unwrap();
    json["schema_version"] = 99.into();
    assert!(LensProfile::parse(&serde_json::to_vec(&json).unwrap()).is_err());
    json["schema_version"] = 1.into();
    json["model"] = "lensfun".into();
    assert!(LensProfile::parse(&serde_json::to_vec(&json).unwrap()).is_err());
    assert!(LensProfile::parse(&vec![b' '; 65537]).is_err());
}
