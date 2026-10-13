//! Iris detail v1: traditional CPU filters in linear Rec.2020, straight alpha.
//! Luminance/chroma denoise uses a joint bilateral kernel with luminance AND chroma
//! range weights, alpha-weighted neighbours and a radius-three spatial Gaussian.
//! The bilateral principle follows Tomasi & Manduchi (ICCV 1998),
//! https://users.soe.ucsc.edu/~manduchi/Papers/ICCV98.pdf .
//! Sharpening is luminance unsharp masking (radius 1); texture uses radius 2 and
//! clarity radius 12, with separable box kernels and distinct gain/tonal masks.
//! Dehaze uses the dark-channel atmospheric scattering equation of He et al.,
//! https://people.csail.mit.edu/kaiming/publications/cvpr09.pdf , but a configurable
//! scalar atmospheric light and box minimum (radius 7), WITHOUT guided refinement
//! or automatic sky detection. It can create halos and is not a commercial match.
//! Radii are full-resolution pixels. Proxy previews use floor(radius*pixel_scale)
//! with minimum one pixel and scaled bilateral spatial sigma: an approximation,
//! not identical to reducing a fully rendered image. No RGB gamut clipping.
//! Frames use 256x128 cores plus dependency halos on a bounded shared CPU pool.
//! Only image edges truncate kernels; internal tile boundaries are never sampled.
use crate::{
    adjustments::{luma, range},
    Frame, RenderContext,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DetailAdjustments {
    /// Luminance unsharp gain [0,3].
    pub sharpening: f32,
    /// Suppress differences smaller than this linear-light value [0,0.25].
    pub sharpening_threshold: f32,
    pub texture: f32,
    pub clarity: f32,
    /// [-1,1]: negative adds veiling light; positive removes it.
    pub dehaze: f32,
    /// Scalar atmospheric light in linear RGB [0.01,16].
    pub atmospheric_light: f32,
    /// Blend with a joint bilateral estimate [0,1].
    pub luminance_denoise: f32,
    pub chroma_denoise: f32,
    /// Range sigma in linear-light units [0.001,1].
    pub denoise_sigma: f32,
}
impl Default for DetailAdjustments {
    fn default() -> Self {
        Self {
            sharpening: 0.0,
            sharpening_threshold: 0.0,
            texture: 0.0,
            clarity: 0.0,
            dehaze: 0.0,
            atmospheric_light: 1.0,
            luminance_denoise: 0.0,
            chroma_denoise: 0.0,
            denoise_sigma: 0.04,
        }
    }
}
impl DetailAdjustments {
    pub fn validate(&self) -> Result<()> {
        range(self.sharpening, 0.0, 3.0, "sharpening")?;
        range(self.sharpening_threshold, 0.0, 0.25, "sharpening threshold")?;
        range(self.texture, -1.0, 1.0, "texture")?;
        range(self.clarity, -1.0, 1.0, "clarity")?;
        range(self.dehaze, -1.0, 1.0, "dehaze")?;
        range(self.atmospheric_light, 0.01, 16.0, "atmospheric light")?;
        range(self.luminance_denoise, 0.0, 1.0, "luminance denoise")?;
        range(self.chroma_denoise, 0.0, 1.0, "chroma denoise")?;
        range(self.denoise_sigma, 0.001, 1.0, "denoise sigma")?;
        Ok(())
    }
    fn identity(&self) -> bool {
        self.sharpening == 0.0
            && self.texture == 0.0
            && self.clarity == 0.0
            && self.dehaze == 0.0
            && self.luminance_denoise == 0.0
            && self.chroma_denoise == 0.0
    }
}

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
fn spatial_weights(radius: usize, scale: f32) -> [f64; 49] {
    let spatial_denominator = 2.0 * (2.0 * scale as f64).max(0.5).powi(2);
    let mut spatial = [0.0; 49];
    for dy in -(radius as isize)..=radius as isize {
        for dx in -(radius as isize)..=radius as isize {
            spatial[((dy + 3) * 7 + dx + 3) as usize] =
                (dx * dx + dy * dy) as f64 / spatial_denominator;
        }
    }
    spatial
}
fn denoise(
    input: &Frame,
    p: &DetailAdjustments,
    ctx: &RenderContext,
    radius: usize,
    edges: TileEdges,
) -> Result<Frame> {
    let w = input.width as usize;
    let h = input.height as usize;
    let mut out = input.clone();
    let sigma = p.denoise_sigma as f64;
    let denom = 2.0 * sigma * sigma;
    let luminance: Vec<f64> = input.pixels.iter().map(|p| luma(rgb(*p))).collect();
    let spatial = spatial_weights(radius, ctx.pixel_scale());
    for y in 0..h {
        ctx.check_cancelled()?;
        for x in 0..w {
            if x % 256 == 0 {
                ctx.check_cancelled()?;
            }
            let i = y * w + x;
            if input.pixels[i][3] == 0.0 || !edges.valid(x, y, w, h, radius) {
                continue;
            }
            let center = rgb(input.pixels[i]);
            let cy = luminance[i];
            let mut sum = [0.0; 3];
            let mut total = 0.0;
            for yy in y.saturating_sub(radius)..=(y + radius).min(h - 1) {
                for xx in x.saturating_sub(radius)..=(x + radius).min(w - 1) {
                    let q = input.pixels[yy * w + xx];
                    let v = rgb(q);
                    let lum = luminance[yy * w + xx];
                    let dy = lum - cy;
                    let chroma_dist = (0..3)
                        .map(|c| ((v[c] - lum) - (center[c] - cy)).powi(2))
                        .sum::<f64>()
                        / 3.0;
                    let spatial_index = ((yy as isize - y as isize + 3) * 7 + xx as isize
                        - x as isize
                        + 3) as usize;
                    let weight = (-spatial[spatial_index] - (dy * dy + chroma_dist) / denom).exp()
                        * q[3] as f64;
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
fn dark_channel(input: &Frame, air: f64, ctx: &RenderContext, radius: usize) -> Result<Vec<f64>> {
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
            row[y * w + x] = (x.saturating_sub(radius)..=(x + radius).min(w - 1))
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
            out[y * w + x] = (y.saturating_sub(radius)..=(y + radius).min(h - 1))
                .map(|yy| row[yy * w + x])
                .fold(1.0, f64::min);
        }
    }
    Ok(out)
}

/// Denoise -> scale-separated local contrast -> atmospheric operation.
/// Alpha is copied exactly, including for hidden pixels. Identity returns exact bytes.
pub fn apply(input: &Frame, params: &DetailAdjustments, ctx: &RenderContext) -> Result<Frame> {
    use rayon::prelude::*;
    input.validate()?;
    params.validate()?;
    ctx.check_cancelled()?;
    if params.identity() {
        let _memory = ctx.reserve(input.byte_len())?;
        return Ok(input.clone());
    }
    let radii = Radii::new(params, ctx.pixel_scale());
    let w = input.width as usize;
    let pool = crate::parallel::pool()?;
    let _memory = ctx.reserve(required_bytes(input, radii, pool.current_num_threads())?)?;
    let mut out = input.clone();
    let band_length = w
        .checked_mul(TILE_HEIGHT)
        .ok_or_else(|| anyhow::anyhow!("detail band overflow"))?;
    pool.install(|| {
        out.pixels
            .par_chunks_mut(band_length)
            .enumerate()
            .try_for_each(|(band, rows)| -> Result<()> {
                render_band(input, params, ctx, radii, band, rows)
            })
    })?;
    ctx.check_cancelled()?;
    Ok(out)
}

fn render_band(
    input: &Frame,
    params: &DetailAdjustments,
    ctx: &RenderContext,
    radii: Radii,
    band: usize,
    rows: &mut [[f32; 4]],
) -> Result<()> {
    let w = input.width as usize;
    let h = input.height as usize;
    ctx.check_cancelled()?;
    let y = band * TILE_HEIGHT;
    let core_height = rows.len() / w;
    for x in (0..w).step_by(TILE_WIDTH) {
        ctx.check_cancelled()?;
        let core_width = TILE_WIDTH.min(w - x);
        let halo = radii.halo();
        let x0 = x.saturating_sub(halo);
        let y0 = y.saturating_sub(halo);
        let x1 = (x + core_width).saturating_add(halo).min(w);
        let y1 = (y + core_height).saturating_add(halo).min(h);
        let tile_width = x1 - x0;
        let tile_height = y1 - y0;
        let mut pixels = Vec::with_capacity(tile_width * tile_height);
        for yy in y0..y1 {
            ctx.check_cancelled()?;
            pixels.extend_from_slice(&input.pixels[yy * w + x0..yy * w + x1]);
        }
        let tile = Frame {
            width: tile_width as u32,
            height: tile_height as u32,
            pixels,
        };
        let edges = TileEdges {
            left: x0 > 0,
            right: x1 < w,
            top: y0 > 0,
            bottom: y1 < h,
        };
        let processed = process_tile(&tile, params, ctx, radii, edges)?;
        for yy in 0..core_height {
            let source = (yy + y - y0) * tile_width + x - x0;
            let target = yy * w + x;
            rows[target..target + core_width]
                .copy_from_slice(&processed.pixels[source..source + core_width]);
        }
    }
    Ok(())
}

const TILE_WIDTH: usize = 256;
const TILE_HEIGHT: usize = 128;
#[derive(Clone, Copy)]
struct Radii {
    denoise: usize,
    local: [usize; 3],
    dehaze: usize,
}
impl Radii {
    fn new(p: &DetailAdjustments, scale: f32) -> Self {
        let radius = |base: usize| ((base as f32 * scale).floor() as usize).max(1);
        Self {
            denoise: if p.luminance_denoise != 0.0 || p.chroma_denoise != 0.0 {
                radius(3)
            } else {
                0
            },
            local: [
                if p.sharpening != 0.0 { radius(1) } else { 0 },
                if p.texture != 0.0 { radius(2) } else { 0 },
                if p.clarity != 0.0 { radius(12) } else { 0 },
            ],
            dehaze: if p.dehaze > 0.0 { radius(7) } else { 0 },
        }
    }
    fn local_radius(self) -> usize {
        *self.local.iter().max().unwrap()
    }
    fn halo(self) -> usize {
        self.denoise + self.local_radius() + self.dehaze
    }
}
/// True flags mark artificial crop edges. Only the dependency-valid interior is
/// modified at each stage, so discarded halo results cannot trigger spurious
/// overflow errors or pollute the later stage's actual dependencies.
#[derive(Clone, Copy, Default)]
struct TileEdges {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}
impl TileEdges {
    fn valid(self, x: usize, y: usize, w: usize, h: usize, margin: usize) -> bool {
        (!self.left || x >= margin)
            && (!self.right || x + margin < w)
            && (!self.top || y >= margin)
            && (!self.bottom || y + margin < h)
    }
}
fn required_bytes(input: &Frame, radii: Radii, threads: usize) -> Result<u64> {
    input
        .byte_len()
        .checked_add(scratch_bytes(input.width, input.height, radii, threads)?)
        .ok_or_else(|| anyhow::anyhow!("detail allocation overflow"))
}
fn scratch_bytes(width: u32, height: u32, radii: Radii, threads: usize) -> Result<u64> {
    let halo = radii.halo();
    let tw = (width as usize).min(TILE_WIDTH + 2 * halo) as u64;
    let th = (height as usize).min(TILE_HEIGHT + 2 * halo) as u64;
    let active = threads.min((height as usize).div_ceil(TILE_HEIGHT)) as u64;
    // Per worker: cropped RGBA input16 + RGBA result16 + Y/alpha/delta24
    // + horizontal box pairs16 + box result8 =80 bytes per expanded tile pixel.
    tw.checked_mul(th)
        .and_then(|n| n.checked_mul(80))
        .and_then(|n| n.checked_mul(active))
        .ok_or_else(|| anyhow::anyhow!("detail scratch overflow"))
}

fn process_tile(
    input: &Frame,
    params: &DetailAdjustments,
    ctx: &RenderContext,
    radii: Radii,
    edges: TileEdges,
) -> Result<Frame> {
    let mut out = if radii.denoise > 0 {
        denoise(input, params, ctx, radii.denoise, edges)?
    } else {
        input.clone()
    };
    local_contrast(&mut out, params, ctx, radii, edges)?;
    dehaze_tile(&mut out, params, ctx, radii, edges)?;
    ctx.check_cancelled()?;
    Ok(out)
}

#[cfg(test)]
mod reference;
#[cfg(test)]
mod tiled_tests {
    use super::*;
    fn fixture(w: u32, h: u32) -> Frame {
        Frame::new(
            w,
            h,
            (0..h)
                .flat_map(|y| {
                    (0..w).map(move |x| {
                        let t = ((x * 17 + y * 13) % 101) as f32 / 100.0;
                        [
                            t * 1.2 - 0.05,
                            0.1 + t * 0.7,
                            0.8 - t * 0.3,
                            if (x + y) % 23 == 0 { 0.0 } else { 0.7 },
                        ]
                    })
                })
                .collect(),
        )
        .unwrap()
    }
    fn compare(a: &Frame, b: &Frame) {
        assert_eq!((a.width, a.height), (b.width, b.height));
        for (a, b) in a.pixels.iter().zip(&b.pixels) {
            assert_eq!(a[3].to_bits(), b[3].to_bits());
            for c in 0..3 {
                assert!(
                    (a[c] - b[c]).abs() <= b[c].abs().max(1.0) * 8.0 * f32::EPSILON,
                    "tiled {} reference {}",
                    a[c],
                    b[c]
                );
            }
        }
    }
    #[test]
    fn halo_tiles_match_frozen_whole_frame_on_edges_and_tile_seams() {
        let input = fixture(279, 141);
        let p = DetailAdjustments {
            sharpening: 0.3,
            texture: 0.2,
            clarity: 0.1,
            luminance_denoise: 0.4,
            chroma_denoise: 0.3,
            dehaze: 0.2,
            ..Default::default()
        };
        let tiled = apply(&input, &p, &RenderContext::default()).unwrap();
        let whole = reference::apply(&input, &p, &RenderContext::default()).unwrap();
        compare(&tiled, &whole);
        let negative = DetailAdjustments {
            dehaze: -0.4,
            ..Default::default()
        };
        compare(
            &apply(&input, &negative, &RenderContext::default()).unwrap(),
            &reference::apply(&input, &negative, &RenderContext::default()).unwrap(),
        );
    }
    #[test]
    fn scaled_preview_tiles_match_scaled_whole_frame_but_are_approximate() {
        let input = fixture(269, 133);
        let ctx = RenderContext::default().with_pixel_scale(0.35).unwrap();
        let p = DetailAdjustments {
            sharpening: 0.2,
            sharpening_threshold: 0.03,
            texture: 0.3,
            clarity: 0.2,
            luminance_denoise: 0.2,
            dehaze: 0.2,
            ..Default::default()
        };
        let radii = Radii::new(&p, ctx.pixel_scale());
        assert_eq!(radii.local, [1, 1, 4]);
        assert_eq!(radii.denoise, 1);
        assert_eq!(radii.dehaze, 2);
        let whole = process_tile(&input, &p, &ctx, radii, TileEdges::default()).unwrap();
        let tiled = apply(&input, &p, &ctx).unwrap();
        compare(&tiled, &whole);
        assert_ne!(
            tiled.pixels,
            apply(&input, &p, &RenderContext::default()).unwrap().pixels
        );
    }
    #[test]
    fn temporary_memory_is_bounded_by_tiles_not_image_area() {
        let p = DetailAdjustments {
            clarity: 1.0,
            ..Default::default()
        };
        let radii = Radii::new(&p, 1.0);
        // Check scratch planning without allocating giant synthetic images.
        assert_eq!(
            scratch_bytes(4000, 3000, radii, 8).unwrap(),
            scratch_bytes(8000, 6000, radii, 8).unwrap()
        );
        assert!(scratch_bytes(4000, 3000, radii, 8).unwrap() < 40 * 1024 * 1024);
    }
}

fn local_contrast(
    out: &mut Frame,
    params: &DetailAdjustments,
    ctx: &RenderContext,
    radii: Radii,
    edges: TileEdges,
) -> Result<()> {
    let w = out.width as usize;
    let h = out.height as usize;
    let local_margin = radii.denoise + radii.local_radius();
    if radii.local_radius() > 0 {
        let ys: Vec<f64> = out.pixels.iter().map(|p| luma(rgb(*p))).collect();
        let alpha: Vec<f64> = out.pixels.iter().map(|p| p[3] as f64).collect();
        let mut delta = vec![0.0; ys.len()];
        for (kind, amount) in [params.sharpening, params.texture, params.clarity]
            .into_iter()
            .enumerate()
        {
            if amount == 0.0 {
                continue;
            }
            let blurred = box_mean(&ys, &alpha, w, h, radii.local[kind], ctx)?;
            accumulate_local(
                &mut delta,
                &ys,
                &blurred,
                (w, h),
                (kind, amount, params.sharpening_threshold),
                (edges, local_margin),
                ctx,
            )?;
        }
        for y in 0..h {
            ctx.check_cancelled()?;
            for x in 0..w {
                let i = y * w + x;
                if out.pixels[i][3] == 0.0 || !edges.valid(x, y, w, h, local_margin) {
                    continue;
                }
                let old = rgb(out.pixels[i]);
                checked_rgb(&mut out.pixels[i], old.map(|v| v + delta[i]))?;
            }
        }
    }
    Ok(())
}

fn dehaze_tile(
    out: &mut Frame,
    params: &DetailAdjustments,
    ctx: &RenderContext,
    radii: Radii,
    edges: TileEdges,
) -> Result<()> {
    let w = out.width as usize;
    let h = out.height as usize;
    if params.dehaze != 0.0 {
        let air = params.atmospheric_light as f64;
        let dark = if radii.dehaze > 0 {
            Some(dark_channel(&out, air, ctx, radii.dehaze)?)
        } else {
            None
        };
        for y in 0..h {
            ctx.check_cancelled()?;
            for x in 0..w {
                let i = y * w + x;
                if out.pixels[i][3] == 0.0 || !edges.valid(x, y, w, h, radii.halo()) {
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

fn accumulate_local(
    delta: &mut [f64],
    ys: &[f64],
    blurred: &[f64],
    dimensions: (usize, usize),
    effect: (usize, f32, f32),
    boundary: (TileEdges, usize),
    ctx: &RenderContext,
) -> Result<()> {
    let (w, h) = dimensions;
    let (kind, amount, threshold) = effect;
    let (edges, local_margin) = boundary;
    for y in 0..h {
        ctx.check_cancelled()?;
        for x in 0..w {
            if !edges.valid(x, y, w, h, local_margin) {
                continue;
            }
            let i = y * w + x;
            let difference = ys[i] - blurred[i];
            let residual = if kind == 0 {
                difference.signum() * (difference.abs() - threshold as f64).max(0.0)
            } else {
                difference
            };
            let mask = if kind == 2 {
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
