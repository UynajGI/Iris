// SPDX-License-Identifier: GPL-3.0-or-later
use super::{luma, ColorBalance};

/// Iris range weighting, not a reverse-engineering of a proprietary grading transform.
/// Neutral controls preserve the original linear-Y triangular weights exactly.
pub(super) struct PreparedBalance<'a> {
    params: &'a ColorBalance,
    ranges: bool,
    global: bool,
    global_gains: [f64; 3],
    hardening: f64,
    overlap: f64,
    shift: f64,
}

impl<'a> PreparedBalance<'a> {
    pub(super) fn new(params: &'a ColorBalance) -> Self {
        Self {
            params,
            ranges: [params.shadows, params.midtones, params.highlights]
                .into_iter()
                .flatten()
                .any(|v| v != 0.0),
            global: params.global != [0.0; 3],
            global_gains: params.global.map(|v| (v as f64).exp2()),
            hardening: (1.0 - 2.0 * params.blending as f64).max(0.0),
            overlap: (2.0 * params.blending as f64 - 1.0).max(0.0),
            shift: params.balance as f64 * 0.5,
        }
    }

    fn weights(&self, luminance: f64) -> [f64; 3] {
        let t = (luminance.clamp(0.0, 1.0) + self.shift).clamp(0.0, 1.0);
        let mut weights = [
            (1.0 - 2.0 * t).max(0.0),
            1.0 - (2.0 * t - 1.0).abs(),
            (2.0 * t - 1.0).max(0.0),
        ];
        if self.hardening > 0.0 {
            // Quartic concentration reduces overlap without a hard range boundary.
            let sharp = weights.map(|v| v * v * v * v);
            let total: f64 = sharp.iter().sum();
            for i in 0..3 {
                weights[i] += self.hardening * (sharp[i] / total - weights[i]);
            }
        } else if self.overlap > 0.0 {
            // Quadratic Bernstein weights overlap throughout the tonal interval
            // while retaining a distinct shadow-to-highlight direction at 100%.
            let broad = [(1.0 - t).powi(2), 2.0 * t * (1.0 - t), t * t];
            for i in 0..3 {
                weights[i] += self.overlap * (broad[i] - weights[i]);
            }
        }
        weights
    }

    pub(super) fn apply_ranges(&self, mut pixel: [f64; 3]) -> [f64; 3] {
        if !self.ranges {
            return pixel;
        }
        let weights = self.weights(luma(pixel));
        for (c, value) in pixel.iter_mut().enumerate() {
            *value *= (weights[0] * self.params.shadows[c] as f64
                + weights[1] * self.params.midtones[c] as f64
                + weights[2] * self.params.highlights[c] as f64)
                .exp2();
        }
        pixel
    }

    pub(super) fn apply_global(&self, mut pixel: [f64; 3]) -> [f64; 3] {
        if self.global {
            for (value, gain) in pixel.iter_mut().zip(self.global_gains) {
                *value *= gain;
            }
        }
        pixel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_are_a_continuous_partition_across_balance_and_blending() {
        for blending in [0.0, 0.2, 0.5, 0.8, 1.0] {
            for balance in [-1.0, -0.4, 0.0, 0.4, 1.0] {
                let params = ColorBalance {
                    blending,
                    balance,
                    ..Default::default()
                };
                let prepared = PreparedBalance::new(&params);
                for i in -100..=1100 {
                    let x = i as f64 / 1000.0;
                    let weights = prepared.weights(x);
                    assert!(weights
                        .iter()
                        .all(|w| w.is_finite() && *w >= 0.0 && *w <= 1.0));
                    assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-12);
                    let nearby = prepared.weights(x + 1e-7);
                    assert!(weights
                        .iter()
                        .zip(nearby)
                        .all(|(a, b)| (a - b).abs() < 1e-5));
                }
            }
        }
    }
}
