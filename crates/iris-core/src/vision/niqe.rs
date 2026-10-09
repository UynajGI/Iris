//! Classical NIQE (Mittal et al., 2013), with BasicSR pristine statistics.
//! Clean Rust implementation of two-scale MSCN / AGGD statistics and Eq. 10.
//! Reference: BasicSR niqe.py, Copyright 2018-2022 BasicSR Authors, Apache-2.0.
use anyhow::{bail, Context, Result};
use image::RgbImage;
use nalgebra::{DMatrix, DVector};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{path::Path, sync::OnceLock};

#[derive(Deserialize)]
pub struct NiqeModel {
    mu_pris_param: Vec<f64>,
    cov_pris_param: Vec<f64>,
    gaussian_window: Vec<f64>,
}
impl NiqeModel {
    pub fn load(dir: &Path) -> Result<Self> {
        let data = std::fs::read(dir.join("niqe_params.json"))
            .context("NIQE pristine statistics missing")?;
        if format!("{:x}", Sha256::digest(&data))
            != "1bd2f8072a1f0cb3c3d4734aa049dbda2a4ce74729a4d682f58363960442e816"
        {
            bail!("NIQE statistics SHA-256 mismatch");
        }
        let model: Self = serde_json::from_slice(&data)?;
        if model.mu_pris_param.len() != 36
            || model.cov_pris_param.len() != 1296
            || model.gaussian_window.len() != 49
        {
            bail!("invalid NIQE statistics dimensions");
        }
        Ok(model)
    }
    pub fn score(&self, image: &RgbImage) -> Result<f64> {
        let w = image.width() as usize;
        let h = image.height() as usize;
        // MATLAB studio-range Y, then rounding, matches BasicSR calculate_niqe.
        let gray: Vec<_> = image
            .pixels()
            .map(|p| {
                (16. + (65.481 * f64::from(p[0])
                    + 128.553 * f64::from(p[1])
                    + 24.966 * f64::from(p[2]))
                    / 255.)
                    .round_ties_even()
            })
            .collect();
        self.score_luma(&gray, w, h)
    }
    pub fn score_luma(&self, input: &[f64], width: usize, height: usize) -> Result<f64> {
        if input.len() != width * height || input.iter().any(|v| !v.is_finite()) {
            bail!("invalid NIQE luminance input");
        }
        let (cols, rows) = (width / 96, height / 96);
        if cols * rows < 2 {
            bail!("NIQE needs at least two 96x96 blocks");
        }
        let (w, h) = (cols * 96, rows * 96);
        let mut gray = Vec::with_capacity(w * h);
        for y in 0..h {
            gray.extend_from_slice(&input[y * width..y * width + w]);
        }
        let mut features = vec![[f64::NAN; 36]; cols * rows];
        let kernel: Vec<f64> = (0..7)
            .map(|x| (0..7).map(|y| self.gaussian_window[y * 7 + x]).sum())
            .collect();
        for scale in 1..=2 {
            let (sw, sh) = (w / scale, h / scale);
            let mu = gaussian(&gray, sw, sh, &kernel);
            let square: Vec<_> = gray.iter().map(|v| v * v).collect();
            let variance = gaussian(&square, sw, sh, &kernel);
            let normalized: Vec<_> = gray
                .iter()
                .zip(mu.iter().zip(variance))
                .map(|(v, (m, s))| (v - m) / ((s - m * m).abs().sqrt() + 1.))
                .collect();
            let block_size = 96 / scale;
            for x in 0..cols {
                for y in 0..rows {
                    let mut block = Vec::with_capacity(block_size * block_size);
                    for r in 0..block_size {
                        let start = (y * block_size + r) * sw + x * block_size;
                        block.extend_from_slice(&normalized[start..start + block_size]);
                    }
                    let f = feature(&block, block_size);
                    features[x * rows + y][(scale - 1) * 18..scale * 18].copy_from_slice(&f);
                }
            }
            if scale == 1 {
                gray = downsample(&gray, w, h);
            }
        }
        let mut mean = [0.; 36];
        for j in 0..36 {
            let values: Vec<_> = features
                .iter()
                .map(|f| f[j])
                .filter(|v| v.is_finite())
                .collect();
            if values.is_empty() {
                bail!("NIQE undefined on textureless image");
            }
            mean[j] = values.iter().sum::<f64>() / values.len() as f64;
        }
        let valid: Vec<_> = features
            .iter()
            .filter(|f| f.iter().all(|v| v.is_finite()))
            .collect();
        if valid.len() < 2 {
            bail!("NIQE has fewer than two valid texture blocks");
        }
        let mut valid_mean = [0.; 36];
        for j in 0..36 {
            valid_mean[j] = valid.iter().map(|f| f[j]).sum::<f64>() / valid.len() as f64;
        }
        let mut covariance = DMatrix::zeros(36, 36);
        for j in 0..36 {
            for k in 0..36 {
                let sample = valid
                    .iter()
                    .map(|f| (f[j] - valid_mean[j]) * (f[k] - valid_mean[k]))
                    .sum::<f64>()
                    / (valid.len() - 1) as f64;
                covariance[(j, k)] = (self.cov_pris_param[j * 36 + k] + sample) / 2.;
            }
        }
        let svd = covariance.svd(true, true);
        let epsilon = svd.singular_values.max() * 1e-15;
        let inverse = svd
            .pseudo_inverse(epsilon)
            .map_err(|e| anyhow::anyhow!("NIQE covariance: {e}"))?;
        let difference =
            DVector::from_iterator(36, (0..36).map(|j| self.mu_pris_param[j] - mean[j]));
        let square = (difference.transpose() * inverse * difference)[(0, 0)];
        if !square.is_finite() || square < -1e-8 {
            bail!("NIQE covariance produced invalid distance");
        }
        Ok(square.max(0.).sqrt())
    }
}

fn gaussian(data: &[f64], w: usize, h: usize, kernel: &[f64]) -> Vec<f64> {
    let mut row = vec![0.; w * h];
    let mut out = vec![0.; w * h];
    for y in 0..h {
        for x in 0..w {
            row[y * w + x] = (0..7)
                .map(|k| {
                    data[y * w + (x as isize + k as isize - 3).clamp(0, w as isize - 1) as usize]
                        * kernel[k]
                })
                .sum();
        }
    }
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = (0..7)
                .map(|k| {
                    row[(y as isize + k as isize - 3).clamp(0, h as isize - 1) as usize * w + x]
                        * kernel[k]
                })
                .sum();
        }
    }
    out
}
fn reflect(i: isize, n: usize) -> usize {
    if i < 0 {
        (-i - 1) as usize
    } else if i >= n as isize {
        (2 * n as isize - i - 1) as usize
    } else {
        i as usize
    }
}
fn cubic(x: f64) -> f64 {
    let x = x.abs();
    if x <= 1. {
        1.5 * x * x * x - 2.5 * x * x + 1.
    } else if x <= 2. {
        -0.5 * x * x * x + 2.5 * x * x - 4. * x + 2.
    } else {
        0.
    }
}
/// MATLAB bicubic half-scale, widened antialias kernel and symmetric padding.
fn downsample(data: &[f64], w: usize, h: usize) -> Vec<f64> {
    let mut row = vec![0.; w * (h / 2)];
    let mut out = vec![0.; (w / 2) * (h / 2)];
    // Input centre for output y is 2*y+0.5; 8 nonzero taps, positions -3..4.
    let weights: Vec<_> = (-3..=4)
        .map(|k| 0.5 * cubic((0.5 - k as f64) * 0.5))
        .collect();
    for y in 0..h / 2 {
        for x in 0..w {
            row[y * w + x] = (-3..=4)
                .enumerate()
                .map(|(k, dy)| weights[k] * data[reflect(2 * y as isize + dy, h) * w + x])
                .sum();
        }
    }
    for y in 0..h / 2 {
        for x in 0..w / 2 {
            out[y * (w / 2) + x] = (-3..=4)
                .enumerate()
                .map(|(k, dx)| weights[k] * row[y * w + reflect(2 * x as isize + dx, w)])
                .sum();
        }
    }
    out
}
#[derive(Clone, Copy)]
struct AggdEntry {
    alpha: f64,
    ratio: f64,
    beta: f64,
    mean: f64,
}
fn gamma_log(z: f64) -> f64 {
    const C: [f64; 8] = [
        676.5203681218851,
        -1259.1392167224028,
        771.3234287776531,
        -176.6150291621406,
        12.507343278686905,
        -0.13857109526572012,
        9.984369578019572e-6,
        1.5056327351493116e-7,
    ];
    if z < 0.5 {
        return std::f64::consts::PI.ln()
            - (std::f64::consts::PI * z).sin().ln()
            - gamma_log(1. - z);
    }
    let z = z - 1.;
    let mut x = 0.9999999999998099;
    for (i, c) in C.iter().enumerate() {
        x += c / (z + i as f64 + 1.);
    }
    let t = z + 7.5;
    0.5 * (2. * std::f64::consts::PI).ln() + (z + 0.5) * t.ln() - t + x.ln()
}
fn table() -> &'static Vec<AggdEntry> {
    static TABLE: OnceLock<Vec<AggdEntry>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=9800)
            .map(|i| {
                let alpha = 0.2 + i as f64 * 0.001;
                let g1 = gamma_log(1. / alpha);
                let g2 = gamma_log(2. / alpha);
                let g3 = gamma_log(3. / alpha);
                AggdEntry {
                    alpha,
                    ratio: (2. * g2 - g1 - g3).exp(),
                    beta: ((g1 - g3) / 2.).exp(),
                    mean: (g2 - g1).exp(),
                }
            })
            .collect()
    })
}
fn aggd(data: &[f64]) -> (f64, f64, f64, f64) {
    let (mut left, mut right, mut nl, mut nr, mut sum_abs, mut sum_sq) =
        (0., 0., 0usize, 0usize, 0., 0.);
    for v in data {
        let square = v * v;
        if *v < 0. {
            left += square;
            nl += 1;
        } else if *v > 0. {
            right += square;
            nr += 1;
        }
        sum_abs += v.abs();
        sum_sq += square;
    }
    if nl == 0 || nr == 0 || sum_sq < 1e-20 {
        return (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    }
    let ls = (left / nl as f64).sqrt();
    let rs = (right / nr as f64).sqrt();
    let gamma = ls / rs;
    let rhat = sum_abs.powi(2) / (data.len() as f64 * sum_sq);
    let ratio = rhat * (gamma.powi(3) + 1.) * (gamma + 1.) / (gamma.powi(2) + 1.).powi(2);
    let entries = table();
    let upper = entries
        .partition_point(|e| e.ratio < ratio)
        .min(entries.len() - 1);
    let lower = upper.saturating_sub(1);
    let entry = if (entries[lower].ratio - ratio).abs() <= (entries[upper].ratio - ratio).abs() {
        entries[lower]
    } else {
        entries[upper]
    };
    (entry.alpha, ls * entry.beta, rs * entry.beta, entry.mean)
}
fn feature(block: &[f64], n: usize) -> [f64; 18] {
    let mut f = [0.; 18];
    let (alpha, left, right, _) = aggd(block);
    f[0] = alpha;
    f[1] = (left + right) / 2.;
    for (i, (dy, dx)) in [(0isize, 1isize), (1, 0), (1, 1), (1, -1)]
        .into_iter()
        .enumerate()
    {
        let mut product = vec![0.; n * n];
        for y in 0..n {
            for x in 0..n {
                let sy = (y as isize - dy).rem_euclid(n as isize) as usize;
                let sx = (x as isize - dx).rem_euclid(n as isize) as usize;
                product[y * n + x] = block[y * n + x] * block[sy * n + sx];
            }
        }
        let (a, l, r, m) = aggd(&product);
        f[2 + i * 4..6 + i * 4].copy_from_slice(&[a, (r - l) * m, l, r]);
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    // Candidate only: production continues to call the original gaussian().
    fn gaussian_neighbor_indices(w: usize, h: usize) -> (Vec<[usize; 7]>, Vec<[usize; 7]>) {
        let xs = (0..w)
            .map(|x| {
                std::array::from_fn(|k| {
                    (x as isize + k as isize - 3).clamp(0, w as isize - 1) as usize
                })
            })
            .collect();
        let ys = (0..h)
            .map(|y| {
                std::array::from_fn(|k| {
                    (y as isize + k as isize - 3).clamp(0, h as isize - 1) as usize * w
                })
            })
            .collect();
        (xs, ys)
    }
    fn gaussian_indexed(
        data: &[f64],
        w: usize,
        h: usize,
        kernel: &[f64],
        xs: &[[usize; 7]],
        ys: &[[usize; 7]],
    ) -> Vec<f64> {
        let mut row = vec![0.; w * h];
        let mut out = vec![0.; w * h];
        for y in 0..h {
            for x in 0..w {
                row[y * w + x] = (0..7).map(|k| data[y * w + xs[x][k]] * kernel[k]).sum();
            }
        }
        for y in 0..h {
            for x in 0..w {
                out[y * w + x] = (0..7).map(|k| row[ys[y][k] + x] * kernel[k]).sum();
            }
        }
        out
    }
    fn gaussian_probe_kernel() -> [f64; 7] {
        // Symmetric seven-tap kernel representative of the pinned NIQE window.
        // Both paths receive exactly these same bits; no kernel substitution in
        // production is proposed by this allocation/indexing experiment.
        [
            0.012560200468474614,
            0.07882796468173003,
            0.2372960771171706,
            0.34263151546524945,
            0.2372960771171706,
            0.07882796468173003,
            0.012560200468474614,
        ]
    }
    #[test]
    fn gaussian_indexed_matches_original_bitwise() {
        let kernel = gaussian_probe_kernel();
        for (w, h) in [
            (1, 1),
            (1, 9),
            (9, 1),
            (2, 3),
            (3, 2),
            (7, 11),
            (336, 240),
            (672, 480),
        ] {
            let (xs, ys) = gaussian_neighbor_indices(w, h);
            let mut seed = 123u32;
            let noise: Vec<_> = (0..w * h)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    f64::from(seed) / f64::from(u32::MAX) * 255.
                })
                .collect();
            for (name, data) in [
                ("dark", vec![0.; w * h]),
                ("constant", vec![128.; w * h]),
                ("noise", noise),
                (
                    "corners",
                    (0..w * h)
                        .map(|i| {
                            if i == 0 {
                                255.
                            } else if i + 1 == w * h {
                                17.
                            } else {
                                0.
                            }
                        })
                        .collect(),
                ),
            ] {
                let reference = gaussian(&data, w, h, &kernel);
                let candidate = gaussian_indexed(&data, w, h, &kernel, &xs, &ys);
                assert!(
                    reference
                        .iter()
                        .zip(&candidate)
                        .all(|(a, b)| a.to_bits() == b.to_bits()),
                    "{name}: {w}x{h}"
                );
                let square: Vec<_> = data.iter().map(|v| v * v).collect();
                let reference = gaussian(&square, w, h, &kernel);
                let candidate = gaussian_indexed(&square, w, h, &kernel, &xs, &ys);
                assert!(
                    reference
                        .iter()
                        .zip(&candidate)
                        .all(|(a, b)| a.to_bits() == b.to_bits()),
                    "squared {name}: {w}x{h}"
                );
            }
        }
    }
    #[test]
    #[ignore = "optimized Gaussian pair ABBA microbenchmark; coordinate exclusive CPU window"]
    fn profile_niqe_gaussian_abba() {
        use std::{hint::black_box, time::Instant};
        assert!(!cfg!(debug_assertions), "requires optimized compilation");
        fn batch(
            indexed: bool,
            data: &[f64],
            square: &[f64],
            w: usize,
            h: usize,
            kernel: &[f64],
            iterations: usize,
        ) -> f64 {
            let start = Instant::now();
            for _ in 0..iterations {
                let d = black_box(data);
                let s = black_box(square);
                let k = black_box(kernel);
                if indexed {
                    // Included in measured time, reused for mean and square.
                    let (xs, ys) = gaussian_neighbor_indices(black_box(w), black_box(h));
                    let mu = gaussian_indexed(d, w, h, k, &xs, &ys);
                    let variance = gaussian_indexed(s, w, h, k, &xs, &ys);
                    black_box((mu, variance));
                } else {
                    let mu = gaussian(d, w, h, k);
                    let variance = gaussian(s, w, h, k);
                    black_box((mu, variance));
                }
            }
            start.elapsed().as_secs_f64() * 1000.
        }
        gaussian_indexed_matches_original_bitwise();
        let kernel = gaussian_probe_kernel();
        let rounds = 8;
        let iterations = 64;
        for (w, h) in [(672, 480), (336, 240)] {
            let mut seed = 0x12345678u32;
            let data: Vec<_> = (0..w * h)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    f64::from(seed) / f64::from(u32::MAX) * 255.
                })
                .collect();
            let square: Vec<_> = data.iter().map(|v| v * v).collect();
            batch(false, &data, &square, w, h, &kernel, 2);
            batch(true, &data, &square, w, h, &kernel, 2);
            let mut a_total = 0.;
            let mut b_total = 0.;
            let mut faster_rounds = 0;
            for round in 0..rounds {
                let a1 = batch(false, &data, &square, w, h, &kernel, iterations);
                let b1 = batch(true, &data, &square, w, h, &kernel, iterations);
                let b2 = batch(true, &data, &square, w, h, &kernel, iterations);
                let a2 = batch(false, &data, &square, w, h, &kernel, iterations);
                a_total += a1 + a2;
                b_total += b1 + b2;
                faster_rounds += usize::from(b1 + b2 < a1 + a2);
                println!("NIQE_GAUSSIAN_ABBA width={w} height={h} round={round} iterations={iterations} original_first_ms={a1:.6} indexed_first_ms={b1:.6} indexed_second_ms={b2:.6} original_second_ms={a2:.6} indexed_over_original={:.6} index_creation_included=true bitwise_equal=true",(b1+b2)/(a1+a2));
            }
            println!("NIQE_GAUSSIAN_SUMMARY width={w} height={h} rounds={rounds} iterations_per_batch={iterations} original_total_ms={a_total:.6} indexed_total_ms={b_total:.6} indexed_over_original={:.6} indexed_faster_rounds={faster_rounds} index_creation_included=true bitwise_equal=true",b_total/a_total);
        }
    }
    // Rejected optimization: kept only for reproducible performance and bitwise
    // diagnostics. Same-process ABBA found this slower at both production sizes.
    fn streaming_aggd(data: &[f64]) -> (f64, f64, f64, f64) {
        streaming_aggd_values(data.iter().copied(), data.len())
    }
    // The iterator must retain row-major order: every sum is deliberately the same
    // left-to-right f64 accumulation as the former materialized-product path.
    fn streaming_aggd_values(
        data: impl Iterator<Item = f64>,
        count: usize,
    ) -> (f64, f64, f64, f64) {
        let (mut left, mut right, mut nl, mut nr, mut sum_abs, mut sum_sq) =
            (0., 0., 0usize, 0usize, 0., 0.);
        for v in data {
            let square = v * v;
            if v < 0. {
                left += square;
                nl += 1;
            } else if v > 0. {
                right += square;
                nr += 1;
            }
            sum_abs += v.abs();
            sum_sq += square;
        }
        if nl == 0 || nr == 0 || sum_sq < 1e-20 {
            return (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
        }
        let ls = (left / nl as f64).sqrt();
        let rs = (right / nr as f64).sqrt();
        let gamma = ls / rs;
        let rhat = sum_abs.powi(2) / (count as f64 * sum_sq);
        let ratio = rhat * (gamma.powi(3) + 1.) * (gamma + 1.) / (gamma.powi(2) + 1.).powi(2);
        let entries = table();
        let upper = entries
            .partition_point(|e| e.ratio < ratio)
            .min(entries.len() - 1);
        let lower = upper.saturating_sub(1);
        let entry = if (entries[lower].ratio - ratio).abs() <= (entries[upper].ratio - ratio).abs()
        {
            entries[lower]
        } else {
            entries[upper]
        };
        (entry.alpha, ls * entry.beta, rs * entry.beta, entry.mean)
    }
    fn streaming_feature(block: &[f64], n: usize) -> [f64; 18] {
        let mut f = [0.; 18];
        let (alpha, left, right, _) = streaming_aggd(block);
        f[0] = alpha;
        f[1] = (left + right) / 2.;
        for (i, (dy, dx)) in [(0isize, 1isize), (1, 0), (1, 1), (1, -1)]
            .into_iter()
            .enumerate()
        {
            // Generate exactly the previous y-then-x product sequence, consuming
            // each value once instead of allocating, filling and rescanning a Vec.
            let product = (0..n).flat_map(|y| {
                (0..n).map(move |x| {
                    let sy = (y as isize - dy).rem_euclid(n as isize) as usize;
                    let sx = (x as isize - dx).rem_euclid(n as isize) as usize;
                    block[y * n + x] * block[sy * n + sx]
                })
            });
            let (a, l, r, m) = streaming_aggd_values(product, n * n);
            f[2 + i * 4..6 + i * 4].copy_from_slice(&[a, (r - l) * m, l, r]);
        }
        f
    }

    /// Same-process diagnostic; excluded from normal correctness test runs.
    /// Can also be extracted with its exact source dependencies into a standalone
    /// rustc -O probe, avoiding any changes to normal release binaries.
    #[test]
    #[ignore = "optimized NIQE product ABBA microbenchmark; coordinate exclusive CPU window"]
    fn profile_niqe_product_abba() {
        use std::{hint::black_box, time::Instant};
        assert!(
            !cfg!(debug_assertions),
            "requires optimized release compilation"
        );
        fn batch(
            f: fn(&[f64], usize) -> [f64; 18],
            blocks: &[Vec<f64>],
            n: usize,
            iterations: usize,
        ) -> f64 {
            let f = black_box(f);
            let start = Instant::now();
            for _ in 0..iterations {
                for block in blocks {
                    black_box(f(black_box(block), black_box(n)));
                }
            }
            start.elapsed().as_secs_f64() * 1000.
        }
        let iterations = 256;
        let rounds = 6;
        black_box(table());
        for n in [48, 96] {
            let mut seed = 0x12345678u32;
            let noise: Vec<_> = (0..n * n)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    f64::from(seed) / f64::from(u32::MAX) * 6. - 3.
                })
                .collect();
            let gradient: Vec<_> = (0..n * n)
                .map(|i| ((i % n) as f64 / n as f64 - 0.5) * 3. + ((i / n) as f64 * 0.3).sin())
                .collect();
            let sparse: Vec<_> = (0..n * n)
                .map(|i| match (i / n + i % n) % 7 {
                    0 => -1.25,
                    1 => 0.75,
                    2 => 0.01,
                    _ => 0.,
                })
                .collect();
            let blocks = [noise, gradient, sparse];
            let verify = || {
                for block in &blocks {
                    assert_eq!(
                        streaming_feature(block, n).map(f64::to_bits),
                        materialized_reference_feature(block, n).map(f64::to_bits)
                    );
                }
            };
            verify();
            batch(materialized_reference_feature, &blocks, n, 8);
            batch(streaming_feature, &blocks, n, 8);
            let mut a_total = 0.;
            let mut b_total = 0.;
            let mut faster_rounds = 0;
            for round in 0..rounds {
                let a1 = batch(materialized_reference_feature, &blocks, n, iterations);
                let b1 = batch(streaming_feature, &blocks, n, iterations);
                let b2 = batch(streaming_feature, &blocks, n, iterations);
                let a2 = batch(materialized_reference_feature, &blocks, n, iterations);
                verify();
                let a = a1 + a2;
                let b = b1 + b2;
                a_total += a;
                b_total += b;
                faster_rounds += usize::from(b < a);
                println!("NIQE_PRODUCT_ABBA n={n} round={round} blocks={} iterations={iterations} materialized_first_ms={a1:.6} streaming_first_ms={b1:.6} streaming_second_ms={b2:.6} materialized_second_ms={a2:.6} streaming_over_materialized={:.6} bitwise_equal=true",blocks.len(),b/a);
            }
            println!("NIQE_PRODUCT_SUMMARY n={n} rounds={rounds} blocks={} iterations_per_batch={iterations} materialized_total_ms={a_total:.6} streaming_total_ms={b_total:.6} streaming_over_materialized={:.6} streaming_faster_rounds={faster_rounds} bitwise_equal=true",blocks.len(),b_total/a_total);
        }
    }
    #[test]
    fn streaming_features_match_materialized_reference_bit_for_bit() {
        let mut cases = 0;
        // 48/96 are production blocks; 0/1/2/3 and odd sizes stress circular
        // neighbor boundaries, empty input and degenerate statistics.
        for n in [0, 1, 2, 3, 7, 48, 96] {
            let mut seed = 0x12345678u32;
            let noise: Vec<_> = (0..n * n)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    f64::from(seed) / f64::from(u32::MAX) * 6. - 3.
                })
                .collect();
            let fixtures = [
                ("dark", vec![0.; n * n]),
                ("positive_constant", vec![128.; n * n]),
                ("negative_constant", vec![-1.; n * n]),
                ("noise", noise),
                (
                    "horizontal_gradient",
                    (0..n * n).map(|i| (i % n) as f64 - n as f64 / 2.).collect(),
                ),
                (
                    "vertical_gradient",
                    (0..n * n).map(|i| (i / n) as f64 - n as f64 / 2.).collect(),
                ),
                (
                    "checkerboard",
                    (0..n * n)
                        .map(|i| {
                            if (i / n + i % n) % 2 == 0 {
                                -0.75
                            } else {
                                1.25
                            }
                        })
                        .collect(),
                ),
                (
                    "signed_zero",
                    (0..n * n)
                        .map(|i| if i % 2 == 0 { -0. } else { 0. })
                        .collect(),
                ),
                (
                    "tiny_mixed",
                    (0..n * n)
                        .map(|i| if i % 2 == 0 { -1e-12 } else { 2e-12 })
                        .collect(),
                ),
                (
                    "corners",
                    (0..n * n)
                        .map(|i| {
                            if i == 0 {
                                -2.5
                            } else if i + 1 == n * n {
                                3.75
                            } else {
                                0.
                            }
                        })
                        .collect(),
                ),
            ];
            for (name, block) in fixtures {
                let expected = materialized_reference_feature(&block, n);
                let actual = streaming_feature(&block, n);
                assert_eq!(
                    actual.map(f64::to_bits),
                    expected.map(f64::to_bits),
                    "fixture={name}, size={n}"
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 70);
    }

    #[test]
    fn streaming_aggd_preserves_undefined_threshold_and_accumulation_order() {
        // Exercise both sides of the small-square cutoff and order-sensitive
        // magnitudes; neither signed-zero nor undefined outputs may be masked.
        for input in [
            vec![],
            vec![0., -0.],
            vec![1., 2.],
            vec![-1., -2.],
            vec![-1e-11, 1e-11],
            vec![-1e-10, 1e-10],
            vec![1e12, -1., 1e-12, -1e12, 3., -4., 0.],
            vec![-3., -0., 0., 0.125, 4.75, -7.25],
        ] {
            let expected = materialized_reference_aggd(&input);
            let actual = streaming_aggd_values(input.iter().copied(), input.len());
            let bits = |v: (f64, f64, f64, f64)| [v.0, v.1, v.2, v.3].map(f64::to_bits);
            assert_eq!(bits(actual), bits(expected), "input={input:?}");
        }
    }
    // Frozen pre-streaming implementation: independent accumulation and Vec
    // materialization make this a bitwise regression oracle, not a tolerance test.
    fn materialized_reference_aggd(data: &[f64]) -> (f64, f64, f64, f64) {
        let (mut left, mut right, mut nl, mut nr, mut sum_abs, mut sum_sq) =
            (0., 0., 0usize, 0usize, 0., 0.);
        for v in data {
            let square = v * v;
            if *v < 0. {
                left += square;
                nl += 1;
            } else if *v > 0. {
                right += square;
                nr += 1;
            }
            sum_abs += v.abs();
            sum_sq += square;
        }
        if nl == 0 || nr == 0 || sum_sq < 1e-20 {
            return (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
        }
        let ls = (left / nl as f64).sqrt();
        let rs = (right / nr as f64).sqrt();
        let gamma = ls / rs;
        let rhat = sum_abs.powi(2) / (data.len() as f64 * sum_sq);
        let ratio = rhat * (gamma.powi(3) + 1.) * (gamma + 1.) / (gamma.powi(2) + 1.).powi(2);
        let entries = table();
        let upper = entries
            .partition_point(|e| e.ratio < ratio)
            .min(entries.len() - 1);
        let lower = upper.saturating_sub(1);
        let entry = if (entries[lower].ratio - ratio).abs() <= (entries[upper].ratio - ratio).abs()
        {
            entries[lower]
        } else {
            entries[upper]
        };
        (entry.alpha, ls * entry.beta, rs * entry.beta, entry.mean)
    }
    fn materialized_reference_feature(block: &[f64], n: usize) -> [f64; 18] {
        let mut f = [0.; 18];
        let (alpha, left, right, _) = materialized_reference_aggd(block);
        f[0] = alpha;
        f[1] = (left + right) / 2.;
        for (i, (dy, dx)) in [(0isize, 1isize), (1, 0), (1, 1), (1, -1)]
            .into_iter()
            .enumerate()
        {
            let mut product = vec![0.; n * n];
            for y in 0..n {
                for x in 0..n {
                    let sy = (y as isize - dy).rem_euclid(n as isize) as usize;
                    let sx = (x as isize - dx).rem_euclid(n as isize) as usize;
                    product[y * n + x] = block[y * n + x] * block[sy * n + sx];
                }
            }
            let (a, l, r, m) = materialized_reference_aggd(&product);
            f[2 + i * 4..6 + i * 4].copy_from_slice(&[a, (r - l) * m, l, r]);
        }
        f
    }

    #[test]
    fn gamma_and_lookup_are_sound() {
        assert!((gamma_log(5.).exp() - 24.).abs() < 1e-10);
        assert!(table().windows(2).all(|w| w[0].ratio < w[1].ratio));
    }
    #[test]
    fn resampling_preserves_constant() {
        let output = downsample(&vec![7.; 96 * 96], 96, 96);
        assert!(output.iter().all(|v| (v - 7.).abs() < 1e-10));
    }
    #[test]
    fn reference_fixture() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
        let model = NiqeModel::load(&root).unwrap();
        let w = 384;
        let h = 288;
        let input: Vec<_> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                ((x * 17 + y * 31 + (x * y) % 71) % 256) as f64
            })
            .collect();
        let score = model.score_luma(&input, w, h).unwrap();
        println!("NIQE_REFERENCE_SCORE={score:.12}");
        // Upstream BasicSR NumPy/SciPy streaming_feature code with float64 bicubic resize.
        // AGGD quantization amplifies rounding near zero on this periodic fixture.
        assert!((score - 51.794572937334).abs() < 0.02);
        assert!(model.score_luma(&vec![128.; w * h], w, h).is_err());
    }
    #[test]
    #[ignore = "external reference luminance fixture generated by tools/verify-niqe.py"]
    fn external_reference() {
        let data = std::fs::read(std::env::var("IRIS_NIQE_LUMA_FILE").unwrap()).unwrap();
        let w = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let h = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let gray: Vec<_> = data[8..].iter().map(|v| f64::from(*v)).collect();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
        let model = NiqeModel::load(&root).unwrap();
        println!(
            "NIQE_REFERENCE_SCORE={:.12}",
            model.score_luma(&gray, w, h).unwrap()
        );
    }
}
