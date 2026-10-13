// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    region::PixelRect,
    white_balance::{self as wb, RawCalibration, WhiteBalanceLocus as Locus},
    Frame, RenderContext,
};

fn calibration() -> RawCalibration {
    RawCalibration {
        version: 1,
        xyz_to_camera_d65: [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.5]],
        camera_to_working: [[1.0, -1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        as_shot_gains: Some([2.0, 1.0, 4.0]),
        applied_gains: [2.0, 1.0, 4.0],
    }
}

#[test]
fn published_loci_and_d65_camera_response() {
    let d65 = wb::illuminant_xy(6504.0, 0.0, Locus::Daylight).unwrap();
    assert!((d65[0] - 0.3127).abs() < 0.0001 && (d65[1] - 0.3290).abs() < 0.0002);
    let planck = wb::illuminant_xy(6504.38938305, 0.0, Locus::Planckian).unwrap();
    assert!((planck[0] - 0.31342600).abs() < 1e-7 && (planck[1] - 0.32359597).abs() < 1e-7);
    let gains = wb::gains_for_xy(&calibration(), [0.3127, 0.3290]).unwrap();
    assert!((gains[0] - 0.5260633).abs() < 1e-6);
    assert_eq!(gains[1], 1.0);
    assert!((gains[2] - 1.8364499).abs() < 1e-5);
    for kelvin in [1667.0, 2222.0, 2800.0, 3200.0, 4000.0, 25000.0] {
        assert!(wb::temperature_gains(&calibration(), kelvin, 0.0, Locus::Planckian).is_ok());
    }
    assert!(wb::illuminant_xy(3200.0, 0.0, Locus::Daylight).is_err());
}

#[test]
fn tint_has_explicit_uv_units_and_invalid_inputs_fail() {
    fn uv([x, y]: [f64; 2]) -> [f64; 2] {
        let d = -2.0 * x + 12.0 * y + 3.0;
        [4.0 * x / d, 6.0 * y / d]
    }
    let a = uv(wb::illuminant_xy(5500.0, 0.0, Locus::Daylight).unwrap());
    let b = uv(wb::illuminant_xy(5500.0, 0.001, Locus::Daylight).unwrap());
    assert!((a[0] - b[0]).abs() < 1e-12);
    assert!((b[1] - a[1] - 0.001).abs() < 1e-12);
    for t in [f64::NAN, 1666.0, 25001.0] {
        assert!(wb::illuminant_xy(t, 0.0, Locus::Planckian).is_err());
    }
    assert!(wb::illuminant_xy(5000.0, 0.021, Locus::Daylight).is_err());
    let mut bad = calibration();
    bad.camera_to_working = [[0.0; 3]; 3];
    assert!(bad.validate().is_err());
    bad = calibration();
    bad.applied_gains = [1.0, 2.0, 1.0];
    assert!(bad.validate().is_err());
}

#[test]
fn neutral_patch_reverses_camera_matrix_and_applied_sensor_gains() {
    // Camera samples [0.1,0.4,0.1], WB [2,1,4], then camera matrix.
    // Working red is negative; sampling must not clamp it to zero.
    let frame = Frame::new(2, 1, vec![[-0.2, 0.4, 0.4, 1.0]; 2]).unwrap();
    let rect = PixelRect {
        x: 0,
        y: 0,
        width: 2,
        height: 1,
    };
    let gains =
        wb::neutral_patch_gains(&frame, &calibration(), rect, &RenderContext::default()).unwrap();
    assert_eq!(gains, [4.0, 1.0, 4.0]);
    for p in [
        [0.0, 0.0, 0.0, 1.0],
        [2.0, 2.0, 2.0, 1.0],
        [-0.2, 0.4, 0.4, 0.5],
    ] {
        let bad = Frame::new(2, 1, vec![p; 2]).unwrap();
        assert!(
            wb::neutral_patch_gains(&bad, &calibration(), rect, &RenderContext::default()).is_err()
        );
    }
    let cancelled = RenderContext::default();
    cancelled.cancel();
    assert!(wb::neutral_patch_gains(&frame, &calibration(), rect, &cancelled).is_err());
}
