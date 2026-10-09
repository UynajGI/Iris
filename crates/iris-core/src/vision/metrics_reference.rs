//! Frozen pre-reuse FFT path, test-only oracle and paired timing baseline.
use super::*;

pub(super) fn metrics(image: &RgbImage) -> (f64, f64, Exposure) {
    let (lap, exposure) = spatial_metrics(image);
    let mean = exposure.mean;
    let small = imageops::resize(image, 128, 128, FilterType::Triangle);
    let mut spectrum: Vec<Complex<f64>> = small
        .pixels()
        .enumerate()
        .map(|(i, p)| {
            let x = (i % 128) as f64;
            let y = (i / 128) as f64;
            let window = (0.5 - 0.5 * (2. * std::f64::consts::PI * x / 127.).cos())
                * (0.5 - 0.5 * (2. * std::f64::consts::PI * y / 127.).cos());
            Complex::new(
                (0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2])
                    - mean)
                    * window,
                0.,
            )
        })
        .collect();
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(128);
    for row in spectrum.chunks_mut(128) {
        fft.process(row);
    }
    let mut col = vec![Complex::new(0., 0.); 128];
    for x in 0..128 {
        for y in 0..128 {
            col[y] = spectrum[y * 128 + x];
        }
        fft.process(&mut col);
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
