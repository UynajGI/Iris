use super::Exposure;
use image::{
    imageops::{self, FilterType},
    RgbImage,
};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::{Arc, OnceLock};

// Only immutable, input-independent data is shared; scratch stays call-local.
fn fft_resources() -> &'static (Arc<dyn rustfft::Fft<f64>>, Vec<f64>) {
    static RESOURCES: OnceLock<(Arc<dyn rustfft::Fft<f64>>, Vec<f64>)> = OnceLock::new();
    RESOURCES.get_or_init(|| {
        let fft = FftPlanner::new().plan_fft_forward(128);
        let window = (0..128 * 128)
            .map(|i| {
                let x = (i % 128) as f64;
                let y = (i / 128) as f64;
                (0.5 - 0.5 * (2. * std::f64::consts::PI * x / 127.).cos())
                    * (0.5 - 0.5 * (2. * std::f64::consts::PI * y / 127.).cos())
            })
            .collect();
        (fft, window)
    })
}

pub fn spatial_metrics(image: &RgbImage) -> (f64, Exposure) {
    let (w, h) = image.dimensions();
    let gray: Vec<f64> = image
        .pixels()
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect();
    let n = gray.len() as f64;
    let mean = gray.iter().sum::<f64>() / n;
    let shadow_clip = gray.iter().filter(|x| **x < 8.).count() as f64 / n;
    let highlight_clip = gray.iter().filter(|x| **x > 247.).count() as f64 / n;
    let mut sum = 0.;
    let mut square = 0.;
    let mut count = 0.;
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = (y * w + x) as usize;
            let v = gray[i - 1] + gray[i + 1] + gray[i - w as usize] + gray[i + w as usize]
                - 4. * gray[i];
            sum += v;
            square += v * v;
            count += 1.;
        }
    }
    let lap = if count > 0. {
        square / count - (sum / count).powi(2)
    } else {
        0.
    };
    (
        lap,
        Exposure {
            mean,
            shadow_clip,
            highlight_clip,
            verdict: if mean < 70. || shadow_clip > 0.35 {
                "underexposed"
            } else if mean > 188. || highlight_clip > 0.25 {
                "overexposed"
            } else {
                "normal"
            }
            .into(),
        },
    )
}

pub fn metrics(image: &RgbImage) -> (f64, f64, Exposure) {
    let (lap, exposure) = spatial_metrics(image);
    let mean = exposure.mean;
    let small = imageops::resize(image, 128, 128, FilterType::Triangle);
    let (fft, window) = fft_resources();
    let mut spectrum: Vec<Complex<f64>> = small
        .pixels()
        .enumerate()
        .map(|(i, p)| {
            Complex::new(
                (0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2])
                    - mean)
                    * window[i],
                0.,
            )
        })
        .collect();
    let mut scratch = vec![Complex::new(0., 0.); fft.get_inplace_scratch_len()];
    for row in spectrum.chunks_mut(128) {
        fft.process_with_scratch(row, &mut scratch);
    }
    let mut col = vec![Complex::new(0., 0.); 128];
    for x in 0..128 {
        for y in 0..128 {
            col[y] = spectrum[y * 128 + x];
        }
        fft.process_with_scratch(&mut col, &mut scratch);
        for y in 0..128 {
            spectrum[y * 128 + x] = col[y];
        }
    }
    let mut total = 0.;
    let mut high = 0.;
    for (y, row) in spectrum.chunks(128).enumerate() {
        for (x, v) in row.iter().enumerate() {
            let r = x.min(128 - x).pow(2) + y.min(128 - y).pow(2);
            if r > 0 {
                total += v.norm_sqr();
                if r > 24 * 24 {
                    high += v.norm_sqr();
                }
            }
        }
    }
    (lap, if total > 1e-9 { high / total } else { 0. }, exposure)
}

pub fn phash(image: &RgbImage) -> String {
    let small = imageops::resize(image, 32, 32, FilterType::Triangle);
    let gray: Vec<f64> = small
        .pixels()
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect();
    let mut coeffs = [0.; 64];
    let cos: Vec<Vec<f64>> = (0..8)
        .map(|u| {
            (0..32)
                .map(|x| ((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI / 64.).cos())
                .collect()
        })
        .collect();
    for v in 0..8 {
        for u in 0..8 {
            let mut c = 0.;
            for y in 0..32 {
                for x in 0..32 {
                    c += gray[y * 32 + x] * cos[u][x] * cos[v][y];
                }
            }
            coeffs[v * 8 + u] = c;
        }
    }
    let mut sorted = coeffs[1..].to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[31];
    let mut hash = 0u64;
    for (i, c) in coeffs.iter().enumerate().skip(1) {
        if *c > median && c.abs() > 1e-7 {
            hash |= 1 << i;
        }
    }
    format!("{hash:016x}")
}

pub fn perceptual_distance(a: &str, b: &str) -> Option<u32> {
    Some((u64::from_str_radix(a, 16).ok()? ^ u64::from_str_radix(b, 16).ok()?).count_ones())
}

#[cfg(test)]
#[path = "metrics_reference.rs"]
mod reference;

#[cfg(test)]
mod tests {
    use super::*;
    fn pattern(w: u32, h: u32, kind: u8) -> RgbImage {
        let mut seed = 0x12345678u32;
        RgbImage::from_fn(w, h, |x, y| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            image::Rgb(match kind {
                0 => [0; 3],
                1 => [255; 3],
                2 => [(x.wrapping_mul(7)) as u8, (y.wrapping_mul(13)) as u8, 123],
                3 => [if (x / 3 + y / 3) % 2 == 0 { 0 } else { 255 }; 3],
                _ => [(seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8],
            })
        })
    }

    #[test]
    fn reused_fft_matches_original_bit_for_bit_across_threads() {
        // Concurrent first use exercises shared initialization and local scratch.
        std::thread::scope(|scope| {
            for kind in 0..5 {
                scope.spawn(move || {
                    for (w, h) in [(1, 1), (3, 7), (128, 128), (750, 500), (31, 139)] {
                        let image = pattern(w, h, kind);
                        let expected = reference::metrics(&image);
                        let actual = metrics(&image);
                        assert_eq!(actual.0.to_bits(), expected.0.to_bits());
                        assert_eq!(
                            actual.1.to_bits(),
                            expected.1.to_bits(),
                            "{w}x{h} kind={kind}"
                        );
                        assert_eq!(
                            serde_json::to_value(actual.2).unwrap(),
                            serde_json::to_value(expected.2).unwrap()
                        );
                    }
                });
            }
        });
    }

    #[test]
    #[ignore = "explicit release ABBA diagnostic, not a CI timing assertion"]
    fn profile_fft_reuse_abba() {
        use std::{hint::black_box, time::Instant};
        let image = pattern(750, 500, 4);
        let expected = reference::metrics(&image);
        assert_eq!(metrics(&image).1.to_bits(), expected.1.to_bits());
        let mut totals = [0.; 2];
        for round in 0..8 {
            let order = if round % 2 == 0 {
                [0, 1, 1, 0]
            } else {
                [1, 0, 0, 1]
            };
            for variant in order {
                let start = Instant::now();
                for _ in 0..32 {
                    black_box(if variant == 0 {
                        reference::metrics(black_box(&image))
                    } else {
                        metrics(black_box(&image))
                    });
                }
                let ms = start.elapsed().as_secs_f64() * 1000.;
                totals[variant] += ms;
                println!("round={round} variant={variant} ms={ms:.6}");
            }
        }
        println!(
            "original_ms={:.6} reused_ms={:.6} reused_over_original={:.6}",
            totals[0],
            totals[1],
            totals[1] / totals[0]
        );
    }
    #[test]
    fn constant_image_has_no_texture() {
        let i = RgbImage::from_pixel(64, 64, image::Rgb([128, 128, 128]));
        let (l, f, e) = metrics(&i);
        assert!(l.abs() < 1e-6);
        assert!(f < 0.001);
        assert_eq!(e.verdict, "normal");
        assert_eq!(phash(&i), "0000000000000000");
    }
    #[test]
    fn blur_reduces_laplacian() {
        let mut i = RgbImage::new(64, 64);
        for (x, y, p) in i.enumerate_pixels_mut() {
            *p = image::Rgb([if (x / 4 + y / 4) % 2 == 0 { 0 } else { 255 }; 3]);
        }
        assert!(metrics(&i).0 > metrics(&imageops::blur(&i, 2.)).0);
    }
    #[test]
    fn hamming_distance() {
        assert_eq!(
            perceptual_distance("0000000000000000", "000000000000000f"),
            Some(4)
        );
        assert_eq!(perceptual_distance("bad!", "1"), None);
    }
}
