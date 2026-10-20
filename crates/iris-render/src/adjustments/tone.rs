// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordered lightness anchors with shape-preserving cubic Hermite interpolation.
//! Fritsch–Butland harmonic tangents keep ordered anchors monotone. The anchor
//! positions/strengths are Iris policy, not Adobe's unpublished processing model.
//! No spatial detail recovery or reconstruction of clipped RAW data is implied.

const X: [f64; 5] = [0.0, 0.25, 0.56, 0.82, 1.0];

pub(super) struct ToneMap {
    y: [f64; 5],
    slopes: [f64; 5],
    identity: bool,
}

impl ToneMap {
    pub fn new(blacks: f32, shadows: f32, highlights: f32, whites: f32) -> Self {
        let [b, s, h, w] = [blacks, shadows, highlights, whites].map(|v| (v as f64).tanh());
        // These bounded displacements keep every pair strictly ordered for ALL
        // valid slider combinations, without sorting or flattening user's values.
        let y = [
            0.04 * b,
            0.25 + 0.12 * s + 0.03 * b,
            0.56,
            0.82 + 0.10 * h,
            1.0 + 0.07 * w,
        ];
        let d: [f64; 4] = std::array::from_fn(|i| (y[i + 1] - y[i]) / (X[i + 1] - X[i]));
        // Match the identity extension when possible: compressing SDR highlights
        // must not accidentally amplify HDR values through a steeper extrapolation.
        // The 3*secant bound is the monotone Hermite endpoint constraint.
        let mut slopes = [
            1.0_f64.min(3.0 * d[0]),
            0.0,
            0.0,
            0.0,
            1.0_f64.min(3.0 * d[3]),
        ];
        for i in 1..4 {
            let left = X[i] - X[i - 1];
            let right = X[i + 1] - X[i];
            let a = 2.0 * right + left;
            let b = right + 2.0 * left;
            slopes[i] = (a + b) / (a / d[i - 1] + b / d[i]);
        }
        Self {
            y,
            slopes,
            identity: [blacks, shadows, highlights, whites] == [0.0; 4],
        }
    }

    pub fn map(&self, x: f64) -> f64 {
        if self.identity {
            return x;
        }
        // Positive endpoint tangents preserve signed/HDR ordering outside [0,1].
        if x <= X[0] {
            return self.y[0] + x * self.slopes[0];
        }
        if x >= X[4] {
            return self.y[4] + (x - X[4]) * self.slopes[4];
        }
        let i = X[1..].partition_point(|v| *v < x).min(3);
        let width = X[i + 1] - X[i];
        let t = (x - X[i]) / width;
        let delta = self.y[i + 1] - self.y[i];
        let a = width * self.slopes[i];
        let b = width * self.slopes[i + 1];
        ((a + b - 2.0 * delta) * t + 3.0 * delta - 2.0 * a - b) * t * t + a * t + self.y[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_extreme_combination_preserves_signed_hdr_order() {
        for b in [-2.0, 0.0, 2.0] {
            for s in [-2.0, 0.0, 2.0] {
                for h in [-2.0, 0.0, 2.0] {
                    for w in [-2.0, 0.0, 2.0] {
                        let map = ToneMap::new(b, s, h, w);
                        let mut previous = f64::NEG_INFINITY;
                        for i in -1024..=8192 {
                            let value = map.map(i as f64 / 4096.0);
                            assert!(
                                value.is_finite() && value > previous,
                                "{b} {s} {h} {w} at {i}"
                            );
                            previous = value;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn broad_controls_keep_endpoints_and_mid_gray_while_endpoints_move() {
        for amount in [-2.0, 2.0] {
            let broad = ToneMap::new(0.0, amount, amount, 0.0);
            for x in [0.0, 0.56, 1.0] {
                assert!((broad.map(x) - x).abs() < 1e-12);
            }
            let endpoints = ToneMap::new(amount, 0.0, 0.0, amount);
            assert!((endpoints.map(0.56) - 0.56).abs() < 1e-12);
            assert!((endpoints.map(0.0)).signum() == (amount as f64).signum());
            assert!((endpoints.map(1.0) - 1.0).signum() == (amount as f64).signum());
        }
    }

    #[test]
    fn slope_is_continuous_at_every_join() {
        let map = ToneMap::new(1.3, -1.7, 1.9, -1.4);
        for x in X {
            let e = 1e-7;
            let left = (map.map(x) - map.map(x - e)) / e;
            let right = (map.map(x + e) - map.map(x)) / e;
            assert!((left - right).abs() < 1e-4);
        }
    }

    #[test]
    fn compressing_highlights_does_not_amplify_hdr() {
        let map = ToneMap::new(0.0, 0.0, -2.0, 0.0);
        for x in [1.0, 1.5, 4.0] {
            assert!((map.map(x) - x).abs() < 1e-12);
        }
    }
}
