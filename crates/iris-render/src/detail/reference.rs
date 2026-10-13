// Test-only pre-tiling reference, frozen Iris detail v1 at full resolution.
use super::*;
fn checked_rgb(out: &mut [f32; 4], rgb: [f64; 3]) -> Result<()> {
    for c in 0..3 {
        ensure!(
            rgb[c].is_finite() && rgb[c].abs() <= f32::MAX as f64,
            "detail exceeds finite f32 range"
        );
        out[c] = rgb[c] as f32;
    }
    Ok(())
}
fn rgb(p: [f32; 4]) -> [f64; 3] {
    [p[0] as f64, p[1] as f64, p[2] as f64]
}

/// Separable truncated box mean. Alpha support is filtered independently so
/// hidden straight-alpha RGB cannot bleed into visible pixels.
fn horizontal_sums(
    values: &[f64],
    alpha: &[f64],
    w: usize,
    h: usize,
    r: usize,
    ctx: &RenderContext,
) -> Result<Vec<[f64; 2]>> {
    let mut horizontal = vec![[0.0; 2]; values.len()];
    for y in 0..h {
        ctx.check_cancelled()?;
        let mut sum = 0.0;
        let mut weight = 0.0;
        for x in 0..=r.min(w - 1) {
            let i = y * w + x;
            sum += values[i] * alpha[i];
            weight += alpha[i];
        }
        for x in 0..w {
            if x % 256 == 0 {
                ctx.check_cancelled()?;
            }
            horizontal[y * w + x] = [sum, weight];
            if x >= r {
                let i = y * w + x - r;
                sum -= values[i] * alpha[i];
                weight -= alpha[i];
            }
            if x + r + 1 < w {
                let i = y * w + x + r + 1;
                sum += values[i] * alpha[i];
                weight += alpha[i];
            }
        }
    }
    Ok(horizontal)
}
fn box_mean(
    values: &[f64],
    alpha: &[f64],
    w: usize,
    h: usize,
    r: usize,
    ctx: &RenderContext,
) -> Result<Vec<f64>> {
    let horizontal = horizontal_sums(values, alpha, w, h, r, ctx)?;
    let mut out = vec![0.0; values.len()];
    for x in 0..w {
        if x % 64 == 0 {
            ctx.check_cancelled()?;
        }
        let mut sum = 0.0;
        let mut weight = 0.0;
        for y in 0..=r.min(h - 1) {
            let p = horizontal[y * w + x];
            sum += p[0];
            weight += p[1];
        }
        for y in 0..h {
            if y % 256 == 0 {
                ctx.check_cancelled()?;
            }
            let i = y * w + x;
            out[i] = if weight > 1e-12 {
                sum / weight
            } else {
                values[i]
            };
            if y >= r {
                let p = horizontal[(y - r) * w + x];
                sum -= p[0];
                weight -= p[1];
            }
            if y + r + 1 < h {
                let p = horizontal[(y + r + 1) * w + x];
                sum += p[0];
                weight += p[1];
            }
        }
    }
    Ok(out)
}
fn denoise(input: &Frame, p: &DetailAdjustments, ctx: &RenderContext) -> Result<Frame> {
    let w = input.width as usize;
    let h = input.height as usize;
    let mut out = input.clone();
    let sigma = p.denoise_sigma as f64;
    let denom = 2.0 * sigma * sigma;
    for y in 0..h {
        ctx.check_cancelled()?;
        for x in 0..w {
            if x % 256 == 0 {
                ctx.check_cancelled()?;
            }
            let i = y * w + x;
            if input.pixels[i][3] == 0.0 {
                continue;
            }
            let center = rgb(input.pixels[i]);
            let cy = luma(center);
            let mut sum = [0.0; 3];
            let mut total = 0.0;
            for yy in y.saturating_sub(3)..=(y + 3).min(h - 1) {
                for xx in x.saturating_sub(3)..=(x + 3).min(w - 1) {
                    let q = input.pixels[yy * w + xx];
                    let v = rgb(q);
                    let lum = luma(v);
                    let dy = lum - cy;
                    let chroma_dist = (0..3)
                        .map(|c| ((v[c] - lum) - (center[c] - cy)).powi(2))
                        .sum::<f64>()
                        / 3.0;
                    let spatial =
                        ((xx as f64 - x as f64).powi(2) + (yy as f64 - y as f64).powi(2)) / 8.0;
                    let weight = (-spatial - (dy * dy + chroma_dist) / denom).exp() * q[3] as f64;
                    for c in 0..3 {
                        sum[c] += weight * v[c];
                    }
                    total += weight;
                }
            }
            if total > 1e-15 {
                for c in &mut sum {
                    *c /= total;
                }
                let sy = luma(sum);
                let new_y = cy + (sy - cy) * p.luminance_denoise as f64;
                let result = std::array::from_fn(|c| {
                    new_y
                        + (center[c] - cy)
                        + ((sum[c] - sy) - (center[c] - cy)) * p.chroma_denoise as f64
                });
                checked_rgb(&mut out.pixels[i], result)?;
            }
        }
    }
    Ok(out)
}
fn dark_channel(input: &Frame, air: f64, ctx: &RenderContext) -> Result<Vec<f64>> {
    let w = input.width as usize;
    let h = input.height as usize;
    let values: Vec<f64> = input
        .pixels
        .iter()
        .map(|p| {
            if p[3] == 0.0 {
                1.0
            } else {
                (p[0].min(p[1]).min(p[2]) as f64 / air).clamp(0.0, 1.0)
            }
        })
        .collect();
    let mut row = vec![0.0; values.len()];
    let mut out = row.clone();
    // Radius is bounded, so two 15-sample passes have linear cost in pixel count.
    for y in 0..h {
        ctx.check_cancelled()?;
        for x in 0..w {
            if x % 256 == 0 {
                ctx.check_cancelled()?;
            }
            row[y * w + x] = (x.saturating_sub(7)..=(x + 7).min(w - 1))
                .map(|xx| values[y * w + xx])
                .fold(1.0, f64::min);
        }
    }
    for y in 0..h {
        ctx.check_cancelled()?;
        for x in 0..w {
            if x % 256 == 0 {
                ctx.check_cancelled()?;
            }
            out[y * w + x] = (y.saturating_sub(7)..=(y + 7).min(h - 1))
                .map(|yy| row[yy * w + x])
                .fold(1.0, f64::min);
        }
    }
    Ok(out)
}

/// Denoise -> scale-separated local contrast -> atmospheric operation.
/// Alpha is copied exactly, including for hidden pixels. Identity returns exact bytes.
pub(super) fn apply(
    input: &Frame,
    params: &DetailAdjustments,
    ctx: &RenderContext,
) -> Result<Frame> {
    input.validate()?;
    params.validate()?;
    ctx.check_cancelled()?;
    // Input is owned/accounted by the caller. Peak extra buffers: output RGBA
    // (16 B/pixel), Y/alpha/delta (24), box horizontal pairs (16), box result (8).
    // Dark-channel-only peak is output + three scalar f64 buffers (40 B/pixel).
    let bytes_per_pixel = reference_bytes_per_pixel(params);
    let required = (input.pixels.len() as u64)
        .checked_mul(bytes_per_pixel)
        .ok_or_else(|| anyhow::anyhow!("detail allocation overflow"))?;
    let _memory = ctx.reserve(required)?;
    if params.identity() {
        return Ok(input.clone());
    }
    let mut out = if params.luminance_denoise > 0.0 || params.chroma_denoise > 0.0 {
        denoise(input, params, ctx)?
    } else {
        input.clone()
    };
    reference_local(&mut out, params, ctx)?;
    reference_dehaze(&mut out, params, ctx)?;
    ctx.check_cancelled()?;
    out.validate()?;
    Ok(out)
}

fn reference_local(out: &mut Frame, params: &DetailAdjustments, ctx: &RenderContext) -> Result<()> {
    let w = out.width as usize;
    let h = out.height as usize;
    if params.sharpening != 0.0 || params.texture != 0.0 || params.clarity != 0.0 {
        let ys: Vec<f64> = out.pixels.iter().map(|p| luma(rgb(*p))).collect();
        let alpha: Vec<f64> = out.pixels.iter().map(|p| p[3] as f64).collect();
        let mut delta = vec![0.0; ys.len()];
        for (radius, amount) in [
            (1, params.sharpening),
            (2, params.texture),
            (12, params.clarity),
        ] {
            if amount == 0.0 {
                continue;
            }
            let blurred = box_mean(&ys, &alpha, w, h, radius, ctx)?;
            reference_accumulate(
                &mut delta,
                &ys,
                &blurred,
                (w, h),
                (radius, amount, params.sharpening_threshold),
                ctx,
            )?;
        }
        for y in 0..h {
            ctx.check_cancelled()?;
            for x in 0..w {
                if x % 256 == 0 {
                    ctx.check_cancelled()?;
                }
                let i = y * w + x;
                if out.pixels[i][3] == 0.0 {
                    continue;
                }
                let old = rgb(out.pixels[i]);
                checked_rgb(&mut out.pixels[i], old.map(|v| v + delta[i]))?;
            }
        }
    }
    Ok(())
}

fn reference_dehaze(
    out: &mut Frame,
    params: &DetailAdjustments,
    ctx: &RenderContext,
) -> Result<()> {
    let w = out.width as usize;
    let h = out.height as usize;
    if params.dehaze != 0.0 {
        let air = params.atmospheric_light as f64;
        let dark = if params.dehaze > 0.0 {
            Some(dark_channel(&out, air, ctx)?)
        } else {
            None
        };
        for y in 0..h {
            ctx.check_cancelled()?;
            for x in 0..w {
                if x % 256 == 0 {
                    ctx.check_cancelled()?;
                }
                let i = y * w + x;
                if out.pixels[i][3] == 0.0 {
                    continue;
                }
                let old = rgb(out.pixels[i]);
                let result = if let Some(dark) = &dark {
                    let t = (1.0 - 0.95 * params.dehaze as f64 * dark[i]).max(0.1);
                    old.map(|v| (v - air) / t + air)
                } else {
                    let t = 1.0 + 0.75 * params.dehaze as f64;
                    old.map(|v| v * t + air * (1.0 - t))
                };
                checked_rgb(&mut out.pixels[i], result)?;
            }
        }
    }
    Ok(())
}

fn reference_accumulate(
    delta: &mut [f64],
    ys: &[f64],
    blurred: &[f64],
    dimensions: (usize, usize),
    effect: (usize, f32, f32),
    ctx: &RenderContext,
) -> Result<()> {
    let (w, h) = dimensions;
    let (radius, amount, threshold) = effect;
    for y in 0..h {
        ctx.check_cancelled()?;
        for x in 0..w {
            if x % 256 == 0 {
                ctx.check_cancelled()?;
            }
            let i = y * w + x;
            let difference = ys[i] - blurred[i];
            let residual = if radius == 1 {
                difference.signum() * (difference.abs() - threshold as f64).max(0.0)
            } else {
                difference
            };
            // Clarity fades in deep shadows and HDR extremes; texture does not.
            let mask = if radius == 12 {
                let t = ys[i].clamp(0.0, 1.0);
                4.0 * t * (1.0 - t)
            } else {
                1.0
            };
            delta[i] += amount as f64 * residual * mask;
        }
    }
    Ok(())
}

fn reference_bytes_per_pixel(params: &DetailAdjustments) -> u64 {
    let bytes_per_pixel =
        if params.sharpening != 0.0 || params.texture != 0.0 || params.clarity != 0.0 {
            64u64
        } else if params.dehaze > 0.0 {
            40u64
        } else {
            16u64
        };
    bytes_per_pixel
}
