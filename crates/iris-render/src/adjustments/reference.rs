// Test-only pre-optimization reference, frozen Iris adjustments v1.
use super::*;
pub(super) fn apply(input: &Frame, params: &Adjustments, context: &RenderContext) -> Result<Frame> {
    params.validate()?;
    input.validate()?;
    context.check_cancelled()?;
    let _memory = context.reserve(input.byte_len())?;
    if *params == Adjustments::default() {
        return Ok(input.clone());
    }
    let mut pixels = Vec::with_capacity(input.pixels.len());
    for y in 0..input.height {
        context.check_cancelled()?;
        for x in 0..input.width {
            if x % 1024 == 0 {
                context.check_cancelled()?;
            }
            let src = input.pixels[(y as usize) * (input.width as usize) + x as usize];
            if src[3] == 0.0 {
                pixels.push(src);
                continue;
            }
            let p = reference_color(src, params);
            let nx = (2.0 * (x as f64 + 0.5) / input.width as f64 - 1.0).abs();
            let ny = (2.0 * (y as f64 + 0.5) / input.height as f64 - 1.0).abs();
            let r = (nx * nx + ny * ny).sqrt() / 2.0f64.sqrt();
            let v = smooth(
                (r - params.vignette.midpoint as f64) / (1.0 - params.vignette.midpoint as f64),
            )
            .powf(params.vignette.feather as f64);
            let gain = (params.vignette.amount as f64 * v).exp2();
            let grain = noise(x, y, params.grain.seed) * params.grain.amount as f64;
            let mut out = src;
            for c in 0..3 {
                let value = p[c] * gain + grain;
                ensure!(
                    value.is_finite() && value.abs() <= f32::MAX as f64,
                    "adjustment exceeds finite f32 range"
                );
                out[c] = value as f32;
            }
            pixels.push(out);
        }
    }
    context.check_cancelled()?;
    Frame::new(input.width, input.height, pixels)
}

fn reference_color(src: [f32; 4], params: &Adjustments) -> [f64; 3] {
    let curves = [&params.red_curve, &params.green_curve, &params.blue_curve];
    let mut p = [src[0] as f64, src[1] as f64, src[2] as f64];
    let gains = [
        params.temperature as f64,
        -params.tint as f64,
        -params.temperature as f64,
    ];
    for c in 0..3 {
        p[c] *= (params.exposure as f64 + gains[c]).exp2();
        if params.contrast != 0.0 {
            let pivot = params.contrast_pivot as f64;
            p[c] =
                p[c].signum() * pivot * (p[c].abs() / pivot).powf((params.contrast as f64).exp2());
        }
    }
    let t = luma(p).clamp(0.0, 1.0);
    let s = 1.0 - smooth(t * 2.0);
    let h = smooth((t - 0.5) * 2.0);
    let v = params.shadows as f64 * s
        + params.highlights as f64 * h
        + params.blacks as f64 * (1.0 - smooth(t * 5.0))
        + params.whites as f64 * smooth((t - 0.8) * 5.0);
    let q = t * 3.0;
    let j = (q.floor() as usize).min(2);
    let k = q - j as f64;
    let ts = [
        params.tone_curve.shadows,
        params.tone_curve.darks,
        params.tone_curve.lights,
        params.tone_curve.highlights,
    ];
    let gain = (v + ts[j] as f64 * (1.0 - k) + ts[j + 1] as f64 * k).exp2();
    for c in 0..3 {
        p[c] = curves[c].map(params.master_curve.map(p[c] * gain));
    }
    let lum = luma(p);
    let chroma = p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]);
    let sat = chroma / (lum.abs() + chroma + 1e-12);
    let boost = (1.0 + params.saturation as f64) * (1.0 + params.vibrance as f64 * (1.0 - sat));
    if boost != 1.0 {
        for c in &mut p {
            *c = lum + (*c - lum) * boost;
        }
    }
    p = reference_hue(p, params);
    let t = luma(p).clamp(0.0, 1.0);
    let weights = [
        (1.0 - 2.0 * t).max(0.0),
        1.0 - (2.0 * t - 1.0).abs(),
        (2.0 * t - 1.0).max(0.0),
    ];
    for (c, value) in p.iter_mut().enumerate() {
        *value *= (weights[0] * params.color_balance.shadows[c] as f64
            + weights[1] * params.color_balance.midtones[c] as f64
            + weights[2] * params.color_balance.highlights[c] as f64)
            .exp2();
    }
    if params.black_and_white.enabled {
        let gray = (0..3)
            .map(|c| p[c] * params.black_and_white.mix[c] as f64)
            .sum::<f64>();
        p = [gray; 3];
    }
    p
}

fn reference_hue(mut p: [f64; 3], params: &Adjustments) -> [f64; 3] {
    if !params.hsl.is_empty() {
        let lum = luma(p);
        let chroma = p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]);
        if chroma > 1e-15 {
            let original_hue = hue(p);
            let mut dh = 0.0;
            let mut ds = 0.0;
            let mut dl = 0.0;
            for b in &params.hsl {
                let distance =
                    ((original_hue - b.center as f64 + 180.0).rem_euclid(360.0) - 180.0).abs();
                let weight = (1.0 - distance / b.width as f64).max(0.0);
                dh += weight * b.hue_shift as f64;
                ds += weight * b.saturation as f64;
                dl += weight * b.luminance as f64;
            }
            if dh != 0.0 || ds != 0.0 || dl != 0.0 {
                let rgb = hue_rgb(original_hue + dh);
                let base = luma(rgb);
                for c in 0..3 {
                    p[c] = lum + dl * chroma + (rgb[c] - base) * chroma * (1.0 + ds).max(0.0);
                }
            }
        }
    }
    p
}
