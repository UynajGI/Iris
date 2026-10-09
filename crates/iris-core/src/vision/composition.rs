use super::{Composition, Face};

/// Position geometry only: a useful explanation of framing, never aesthetic truth.
pub(super) fn measure(faces: &[Face], width: u32, height: u32) -> Option<Composition> {
    if width == 0 || height == 0 {
        return None;
    }
    let (mut x, mut y, mut total) = (0f64, 0f64, 0f64);
    let mut landmark_count = 0usize;
    let mut box_count = 0usize;
    for face in faces {
        let [bx, by, bw, bh] = face.bbox;
        if face.bbox.iter().any(|v| !v.is_finite())
            || bw <= 0.
            || bh <= 0.
            || !face.confidence.is_finite()
            || face.confidence < 0.55
        {
            continue;
        }
        let landmark_point = if face
            .landmark_confidence
            .is_some_and(|c| c >= 0.8 && c.is_finite())
        {
            face.landmarks
                .get(33)
                .zip(face.landmarks.get(263))
                .map(|(a, b)| [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.])
                .filter(|p| {
                    p.iter().all(|v| v.is_finite())
                        && p[0] >= 0.
                        && p[0] <= width as f32
                        && p[1] >= 0.
                        && p[1] <= height as f32
                })
        } else {
            None
        };
        let point = landmark_point.unwrap_or([bx + bw / 2., by + bh / 2.]);
        let normalized = [point[0] / width as f32, point[1] / height as f32];
        if normalized.iter().any(|v| !(0.0..=1.0).contains(v)) {
            continue;
        }
        if landmark_point.is_some() {
            landmark_count += 1;
        } else {
            box_count += 1;
        }
        let weight = f64::from(bw) * f64::from(bh) * f64::from(face.confidence);
        x += f64::from(normalized[0]) * weight;
        y += f64::from(normalized[1]) * weight;
        total += weight;
    }
    if total <= 0. {
        return None;
    }
    x /= total;
    y /= total;
    let thirds_distance = [1. / 3., 2. / 3.]
        .into_iter()
        .flat_map(|tx| {
            [1. / 3., 2. / 3.]
                .into_iter()
                .map(move |ty| (x - tx).hypot(y - ty))
        })
        .fold(f64::INFINITY, f64::min);
    let center_distance = (x - 0.5).hypot(y - 0.5);
    // Both centered portraits and rule-of-thirds placement receive full credit.
    let score =
        (100. * (1. - thirds_distance.min(center_distance) / (2f64.sqrt() / 3.))).clamp(0., 100.);
    let method = match (landmark_count > 0, box_count > 0) {
        (true, false) => "face_landmark_centroid",
        (true, true) => "face_landmark_bbox_centroid",
        _ => "face_bbox_centroid",
    };
    Some(Composition {
        subject: [x as f32, y as f32],
        thirds_distance,
        center_distance,
        score,
        method: method.into(),
    })
}
