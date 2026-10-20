// SPDX-License-Identifier: GPL-3.0-or-later
use super::{Curve, CurveInterpolation};

struct Segment {
    x: f64,
    width: f64,
    coefficients: [f64; 4],
}

/// Cubic coefficients are prepared once, then shared by render worker threads.
/// Interior tangents use the Fritsch-Butland weighted harmonic mean:
/// https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.PchipInterpolator.html
/// Outside the control domain we extend the endpoint tangent, not the cubic.
pub(super) struct PreparedCurve<'a> {
    curve: &'a Curve,
    segments: Vec<Segment>,
    endpoints: [f64; 2],
}

impl<'a> PreparedCurve<'a> {
    pub(super) fn new(curve: &'a Curve) -> Self {
        let mut prepared = Self {
            curve,
            segments: Vec::new(),
            endpoints: [0.0; 2],
        };
        if curve.interpolation == CurveInterpolation::Linear || curve.points.is_empty() {
            return prepared;
        }
        let points = &curve.points;
        let widths: Vec<_> = points
            .windows(2)
            .map(|p| p[1].x as f64 - p[0].x as f64)
            .collect();
        let slopes: Vec<_> = points
            .windows(2)
            .zip(&widths)
            .map(|(p, h)| (p[1].y as f64 - p[0].y as f64) / h)
            .collect();
        let tangents = if curve.interpolation == CurveInterpolation::NaturalCubic {
            natural_tangents(&widths, &slopes)
        } else {
            tangents(&widths, &slopes)
        };
        prepared.endpoints = [tangents[0], *tangents.last().unwrap()];
        prepared.segments = points
            .windows(2)
            .enumerate()
            .map(|(i, p)| {
                let delta = p[1].y as f64 - p[0].y as f64;
                let a = widths[i] * tangents[i];
                let b = widths[i] * tangents[i + 1];
                Segment {
                    x: p[0].x as f64,
                    width: widths[i],
                    coefficients: [
                        p[0].y as f64,
                        a,
                        3.0 * delta - 2.0 * a - b,
                        a + b - 2.0 * delta,
                    ],
                }
            })
            .collect();
        prepared
    }

    pub(super) fn map(&self, x: f64) -> f64 {
        let points = &self.curve.points;
        if points.is_empty() {
            return x;
        }
        let i = points[1..]
            .partition_point(|p| (p.x as f64) < x)
            .min(points.len() - 2);
        let a = points[i];
        let b = points[i + 1];
        if self.curve.interpolation == CurveInterpolation::Linear {
            // Preserve the original f32 subtraction before widening, including HDR.
            return a.y as f64 + (x - a.x as f64) * (b.y - a.y) as f64 / (b.x - a.x) as f64;
        }
        let first = points[0];
        let last = points[points.len() - 1];
        if x < first.x as f64 {
            return first.y as f64 + (x - first.x as f64) * self.endpoints[0];
        }
        if x > last.x as f64 {
            return last.y as f64 + (x - last.x as f64) * self.endpoints[1];
        }
        let segment = &self.segments[i];
        let t = (x - segment.x) / segment.width;
        let [d, c, b, a] = segment.coefficients;
        ((a * t + b) * t + c) * t + d
    }
}

fn tangents(widths: &[f64], slopes: &[f64]) -> Vec<f64> {
    if slopes.len() == 1 {
        return vec![slopes[0]; 2];
    }
    let n = slopes.len();
    let mut values = vec![0.0; n + 1];
    values[0] = endpoint(widths[0], widths[1], slopes[0], slopes[1]);
    values[n] = endpoint(widths[n - 1], widths[n - 2], slopes[n - 1], slopes[n - 2]);
    for i in 1..n {
        if slopes[i - 1] > 0.0 && slopes[i] > 0.0 {
            let w1 = 2.0 * widths[i] + widths[i - 1];
            let w2 = widths[i] + 2.0 * widths[i - 1];
            values[i] = (w1 + w2) / (w1 / slopes[i - 1] + w2 / slopes[i]);
        }
    }
    values
}

fn endpoint(h0: f64, h1: f64, d0: f64, d1: f64) -> f64 {
    (((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1)).clamp(0.0, 3.0 * d0)
}

/// Solve the natural spline's tridiagonal system for knot second derivatives.
/// The end curvatures are zero; internal first AND second derivatives agree.
/// This is the standard natural cubic construction (also used by RawTherapee's
/// spline mode), not a change to the legacy shape-preserving PCHIP mode.
fn natural_tangents(widths: &[f64], slopes: &[f64]) -> Vec<f64> {
    let n = slopes.len();
    let mut upper = vec![0.0; n + 1];
    let mut second = vec![0.0; n + 1];
    for i in 1..n {
        let diagonal = 2.0 * (widths[i - 1] + widths[i]) - widths[i - 1] * upper[i - 1];
        upper[i] = widths[i] / diagonal;
        second[i] = (6.0 * (slopes[i] - slopes[i - 1]) - widths[i - 1] * second[i - 1]) / diagonal;
    }
    for i in (1..n).rev() {
        second[i] -= upper[i] * second[i + 1];
    }
    let mut result: Vec<_> = (0..n)
        .map(|i| slopes[i] - widths[i] * (2.0 * second[i] + second[i + 1]) / 6.0)
        .collect();
    result.push(slopes[n - 1] + widths[n - 1] * (second[n - 1] + 2.0 * second[n]) / 6.0);
    result
}

#[cfg(test)]
mod smooth_tests {
    use super::*;
    use crate::adjustments::CurvePoint;

    #[test]
    fn natural_spline_has_continuous_slope_and_curvature_for_uneven_knots() {
        let curve = Curve {
            points: vec![
                CurvePoint { x: 0.0, y: 0.0 },
                CurvePoint { x: 0.2, y: 0.35 },
                CurvePoint { x: 0.6, y: 0.7 },
                CurvePoint { x: 1.0, y: 1.0 },
            ],
            interpolation: CurveInterpolation::NaturalCubic,
        };
        let prepared = PreparedCurve::new(&curve);
        for pair in prepared.segments.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            let [_, c, d, e] = a.coefficients;
            let slope_left = (c + 2.0 * d + 3.0 * e) / a.width;
            let slope_right = b.coefficients[1] / b.width;
            let curvature_left = (2.0 * d + 6.0 * e) / a.width.powi(2);
            let curvature_right = 2.0 * b.coefficients[2] / b.width.powi(2);
            assert!((slope_left - slope_right).abs() < 1e-10);
            assert!((curvature_left - curvature_right).abs() < 1e-10);
        }
        let first = &prepared.segments[0];
        let last = prepared.segments.last().unwrap();
        assert!(first.coefficients[2].abs() < 1e-10);
        assert!((2.0 * last.coefficients[2] + 6.0 * last.coefficients[3]).abs() < 1e-10);
        for p in &curve.points {
            assert!((prepared.map(p.x as f64) - p.y as f64).abs() < 1e-10);
        }
    }
}
