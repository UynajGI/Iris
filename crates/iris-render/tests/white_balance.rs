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

#[test]
fn recovered_temperature_recreates_saved_sensor_gains() {
    let mut camera = calibration();
    // Non-diagonal real-camera-like matrix exercises XYZ inversion, not just
    // channel-wise reciprocals. Values are synthetic, not a camera profile.
    camera.xyz_to_camera_d65 = [[0.7, -0.2, -0.1], [-0.3, 1.3, 0.1], [0.02, 0.1, 0.6]];
    for (locus, temperatures) in [
        (
            Locus::Planckian,
            vec![
                1667.0, 2222.0, 2223.0, 3200.0, 4000.0, 4001.0, 6500.0, 25000.0,
            ],
        ),
        (
            Locus::Daylight,
            vec![4000.0, 5500.0, 7000.0, 7001.0, 10000.0, 25000.0],
        ),
    ] {
        for kelvin in temperatures {
            for tint in [-0.02, -0.005, 0.0, 0.005, 0.02] {
                // At these warm temperatures +0.02 v is outside physical xy.
                // It must remain rejected rather than being clamped to a white.
                if locus == Locus::Planckian && kelvin <= 2223.0 && tint == 0.02 {
                    assert!(wb::temperature_gains(&camera, kelvin, tint, locus).is_err());
                    continue;
                }
                let gains = wb::temperature_gains(&camera, kelvin, tint, locus).unwrap();
                let reading = wb::temperature_from_gains(&camera, gains, locus)
                    .unwrap()
                    .unwrap_or_else(|| panic!("supported {locus:?} {kelvin} K tint {tint}: {gains:?} must be readable"));
                let rebuilt = wb::temperature_gains(
                    &camera,
                    reading.kelvin,
                    reading.tint_delta_v,
                    reading.locus,
                )
                .unwrap();
                for (expected, actual) in gains.into_iter().zip(rebuilt) {
                    assert!(
                        (actual - expected).abs() / expected < 2e-6,
                        "{locus:?} {kelvin} K tint {tint}: {gains:?} -> {reading:?} -> {rebuilt:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn readings_do_not_invent_temperature_for_unrepresentable_gains() {
    let camera = calibration();
    let tungsten = wb::temperature_gains(&camera, 2800.0, 0.0, Locus::Planckian).unwrap();
    assert!(
        wb::temperature_from_gains(&camera, tungsten, Locus::Daylight)
            .unwrap()
            .is_none()
    );
    let green = wb::gains_for_xy(&camera, [0.3, 0.55]).unwrap();
    assert!(wb::temperature_from_gains(&camera, green, Locus::Planckian)
        .unwrap()
        .is_none());
    for gains in [[1.0, 2.0, 1.0], [f32::NAN, 1.0, 1.0], [0.0, 1.0, 1.0]] {
        assert!(wb::temperature_from_gains(&camera, gains, Locus::Planckian).is_err());
    }
    let mut negative = camera;
    negative.xyz_to_camera_d65 = [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    assert!(
        wb::temperature_from_gains(&negative, [1.0; 3], Locus::Planckian)
            .unwrap()
            .is_none()
    );
}
