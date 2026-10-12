//! On-demand native regions. Compressed sensor decoding is still whole-frame;
//! demosaicing/calibration is restricted to a padded ROI, never the whole RGB frame.
use anyhow::{ensure, Context, Result};
use rawler::{
    imgop::{develop::RawDevelop, Dim2, Point, Rect},
    rawimage::{RawImage, RawPhotometricInterpretation},
};
use std::io::Write;

fn request(value: &str) -> Result<[f64; 4]> {
    let values: Vec<f64> = value
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    ensure!(values.len() == 4, "region requires four coordinates");
    let r: [f64; 4] = values.try_into().unwrap();
    ensure!(
        r.iter().all(|v| v.is_finite())
            && r[0] >= 0.
            && r[1] >= 0.
            && r[2] > 0.
            && r[3] > 0.
            && r[0] + r[2] <= 1.000001
            && r[1] + r[3] <= 1.000001,
        "region outside oriented image"
    );
    Ok(r)
}

/// Invert EXIF orientation in normalized coordinates, preserving rectangle edges.
fn sensor_region(r: [f64; 4], orientation: u16) -> [f64; 4] {
    let [x, y, w, h] = r;
    match orientation {
        2 => [1. - x - w, y, w, h],
        3 => [1. - x - w, 1. - y - h, w, h],
        4 => [x, 1. - y - h, w, h],
        5 => [y, x, h, w],
        6 => [y, 1. - x - w, h, w],
        7 => [1. - y - h, 1. - x - w, h, w],
        8 => [1. - y - h, x, h, w],
        _ => r,
    }
}

pub fn develop(mut raw: RawImage, value: &str) -> Result<()> {
    let r = request(value)?;
    ensure!(
        raw.width as u64 * raw.height as u64 <= 60_000_000,
        "Native regional RAW detail exceeds the 60MP sensor limit"
    );
    ensure!(
        raw.cpp == 1
            && matches!(&raw.photometric, RawPhotometricInterpretation::Cfa(_))
            && raw.fuji_rotation_width.is_none(),
        "Native regional detail requires a conventional CFA sensor; this RAW layout is unsupported"
    );
    let orientation = raw.orientation.to_u16();
    ensure!((1..=8).contains(&orientation), "invalid RAW orientation");
    let full = raw.crop_area.or(raw.active_area).unwrap_or(Rect::new(
        Point::new(0, 0),
        Dim2::new(raw.width, raw.height),
    ));
    ensure!(
        full.d.w > 0
            && full.d.h > 0
            && full.p.x + full.d.w <= raw.width
            && full.p.y + full.d.h <= raw.height,
        "invalid RAW crop geometry"
    );
    let [x, y, w, h] = sensor_region(r, orientation);
    let left = (x.max(0.) * full.d.w as f64).floor() as usize;
    let top = (y.max(0.) * full.d.h as f64).floor() as usize;
    let right = ((x + w) * full.d.w as f64).ceil().min(full.d.w as f64) as usize;
    let bottom = ((y + h) * full.d.h as f64).ceil().min(full.d.h as f64) as usize;
    ensure!(right > left && bottom > top, "empty RAW region");
    let width = right - left;
    let height = bottom - top;
    ensure!(
        width >= 16 && height >= 16,
        "RAW region has fewer than 16 native pixels on one side"
    );
    ensure!(
        width as u64 * height as u64 <= 4_000_000,
        "RAW region exceeds 4MP; select a smaller region or a face"
    );
    // A 24-pixel halo gives both Bayer and X-Trans demosaicing neighboring samples.
    let px = (full.p.x + left).saturating_sub(24);
    let py = (full.p.y + top).saturating_sub(24);
    let pr = (full.p.x + right + 24).min(raw.width);
    let pb = (full.p.y + bottom + 24).min(raw.height);
    let roi = Rect::new(Point::new(px, py), Dim2::new(pr - px, pb - py));
    raw.active_area = Some(roi);
    raw.crop_area = Some(roi);
    let developed = RawDevelop::default()
        .develop_intermediate(&raw)?
        .to_dynamic_image()
        .context("RAW region development produced no image")?;
    ensure!(
        developed.width() as usize == roi.d.w && developed.height() as usize == roi.d.h,
        "RAW region geometry changed during development"
    );
    let image = developed
        .crop_imm(
            (full.p.x + left - px) as u32,
            (full.p.y + top - py) as u32,
            width as u32,
            height as u32,
        )
        .to_rgb8();
    let (source_width, source_height) = if orientation >= 5 {
        (full.d.h, full.d.w)
    } else {
        (full.d.w, full.d.h)
    };
    // Versioned framing separates metadata from binary PPM bytes.
    writeln!(
        std::io::stdout().lock(),
        "{}",
        serde_json::json!({
            "protocol": "iris-raw-region-v1", "source_width": source_width,
            "source_height": source_height, "orientation": orientation
        })
    )?;
    super::write_ppm(image.width(), image.height(), image.as_raw())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orientation_rectangles_round_trip() {
        let r = [0.1, 0.2, 0.3, 0.4];
        for o in 1..=8 {
            let inverse = match o {
                6 => 8,
                8 => 6,
                _ => o,
            };
            let restored = sensor_region(sensor_region(r, o), inverse);
            for (a, b) in restored.into_iter().zip(r) {
                assert!((a - b).abs() < 1e-9);
            }
        }
        assert_eq!(sensor_region(r, 5), [0.2, 0.1, 0.4, 0.3]);
        assert!(request("0,0,NaN,1").is_err());
        assert!(request("0.9,0,0.2,1").is_err());
        assert!(request("0,0,1").is_err());
        assert!(request("0,0,1,1").is_ok());
    }
}
