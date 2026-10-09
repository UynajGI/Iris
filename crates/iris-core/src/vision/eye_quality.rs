//! These gates measure local observability, not semantic occlusion.
//! The bundled models supply no per-eye or per-landmark visibility confidence.
use image::RgbImage;

pub(super) fn rejection_reason(
    image: &RgbImage,
    points: &[[f32; 3]],
    indices: [usize; 6],
    ear: f32,
) -> Option<String> {
    let mut eye = [[0f32; 3]; 6];
    for (i, index) in indices.into_iter().enumerate() {
        let Some(point) = points.get(index) else {
            return Some("eye landmarks missing".into());
        };
        if point.iter().any(|v| !v.is_finite()) {
            return Some("eye landmark geometry is non-finite".into());
        }
        eye[i] = *point;
    }
    let dx = eye[3][0] - eye[0][0];
    let dy = eye[3][1] - eye[0][1];
    let span = dx.hypot(dy);
    if span < 8. {
        return Some(format!(
            "eye span too small ({span:.1} preview pixels; minimum 8)"
        ));
    }
    if !ear.is_finite() || !(0.01..=0.65).contains(&ear) {
        return Some("eye landmark geometry outside plausible range".into());
    }
    let midpoint = [(eye[0][0] + eye[3][0]) / 2., (eye[0][1] + eye[3][1]) / 2.];
    let padding = span * 0.15;
    let min_x = eye.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) - padding;
    let max_x = eye.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max) + padding;
    let min_y = eye
        .iter()
        .map(|p| p[1])
        .fold(midpoint[1] - span * 0.2, f32::min)
        - padding;
    let max_y = eye
        .iter()
        .map(|p| p[1])
        .fold(midpoint[1] + span * 0.2, f32::max)
        + padding;
    if min_x < 0.
        || min_y < 0.
        || max_x.ceil() >= image.width() as f32
        || max_y.ceil() >= image.height() as f32
    {
        return Some("eye observation region crosses image boundary".into());
    }
    let (mut count, mut sum, mut square, mut dark, mut bright) =
        (0usize, 0f64, 0f64, 0usize, 0usize);
    for y in min_y.floor() as u32..=max_y.ceil() as u32 {
        for x in min_x.floor() as u32..=max_x.ceil() as u32 {
            let p = image.get_pixel(x, y);
            let v = 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]);
            count += 1;
            sum += v;
            square += v * v;
            if v <= 8. {
                dark += 1;
            }
            if v >= 247. {
                bright += 1;
            }
        }
    }
    if count == 0 {
        return Some("eye observation region is empty".into());
    }
    let mean = sum / count as f64;
    let contrast = (square / count as f64 - mean * mean).max(0.).sqrt();
    if mean < 20. || dark as f64 / count as f64 > 0.75 {
        return Some("eye region is too dark or shadow-clipped".into());
    }
    if mean > 240. || bright as f64 / count as f64 > 0.75 {
        return Some("eye region is too bright or highlight-clipped".into());
    }
    if contrast < 2.5 {
        return Some(format!(
            "eye region has insufficient local contrast ({contrast:.2})"
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn points(x: f32) -> Vec<[f32; 3]> {
        vec![
            [x, 30., 0.],
            [x + 5., 27., 0.],
            [x + 15., 27., 0.],
            [x + 20., 30., 0.],
            [x + 15., 33., 0.],
            [x + 5., 33., 0.],
        ]
    }
    #[test]
    fn quality_rejects_measured_failures() {
        let dark = RgbImage::from_pixel(100, 60, image::Rgb([0, 0, 0]));
        let bright = RgbImage::from_pixel(100, 60, image::Rgb([255, 255, 255]));
        assert!(
            rejection_reason(&dark, &points(10.), [0, 1, 2, 3, 4, 5], 0.3)
                .unwrap()
                .contains("dark")
        );
        assert!(
            rejection_reason(&bright, &points(10.), [0, 1, 2, 3, 4, 5], 0.3)
                .unwrap()
                .contains("bright")
        );
        assert!(
            rejection_reason(&dark, &points(0.), [0, 1, 2, 3, 4, 5], 0.3)
                .unwrap()
                .contains("boundary")
        );
        let flat = RgbImage::from_pixel(100, 60, image::Rgb([128, 128, 128]));
        assert!(
            rejection_reason(&flat, &points(10.), [0, 1, 2, 3, 4, 5], 0.3)
                .unwrap()
                .contains("contrast")
        );
        assert!(
            rejection_reason(&flat, &points(10.), [0, 1, 2, 3, 4, 5], 0.9)
                .unwrap()
                .contains("geometry")
        );
        let small: Vec<_> = points(10.)
            .iter()
            .map(|p| [p[0] * 0.25, p[1], p[2]])
            .collect();
        assert!(rejection_reason(&flat, &small, [0, 1, 2, 3, 4, 5], 0.3)
            .unwrap()
            .contains("span"));
    }
    #[test]
    fn separate_eyes_are_checked_independently() {
        let mut image = RgbImage::from_pixel(100, 60, image::Rgb([0, 0, 0]));
        for (x, y, p) in image.enumerate_pixels_mut() {
            if x > 50 {
                let v = if (x + y) % 2 == 0 { 80 } else { 180 };
                *p = image::Rgb([v, v, v]);
            }
        }
        assert!(rejection_reason(&image, &points(10.), [0, 1, 2, 3, 4, 5], 0.3).is_some());
        assert!(rejection_reason(&image, &points(60.), [0, 1, 2, 3, 4, 5], 0.3).is_none());
    }
}
