// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit RGB/Key composition. Key is selection coverage, separate from image alpha.
use super::*;
use crate::document::Node;

fn mask<'a>(
    node: &Node,
    values: &'a BTreeMap<String, HeldValue>,
    index: usize,
) -> Result<&'a Mask> {
    match &values
        .get(&node.inputs[index])
        .context("missing Key input")?
        .value
    {
        Value::Mask(mask) => Ok(mask),
        _ => bail!("Key input required"),
    }
}
fn image<'a>(
    node: &Node,
    values: &'a BTreeMap<String, HeldValue>,
    index: usize,
) -> Result<&'a Frame> {
    match &values
        .get(&node.inputs[index])
        .context("missing image input")?
        .value
    {
        Value::Image(frame) => Ok(frame),
        _ => bail!("image input required"),
    }
}

pub(super) fn evaluate(
    node: &Node,
    values: &BTreeMap<String, HeldValue>,
    input: &Frame,
    context: &RenderContext,
) -> Result<Value> {
    match node.operation {
        Operation::AlphaMask => {
            let _memory = context.reserve(input.pixels.len() as u64 * 4)?;
            let mut coverage = Vec::with_capacity(input.pixels.len());
            for row in input.pixels.chunks(input.width as usize) {
                context.check_cancelled()?;
                coverage.extend(row.iter().map(|pixel| pixel[3]));
            }
            Ok(Value::Mask(Arc::new(Mask {
                width: input.width,
                height: input.height,
                values: coverage,
            })))
        }
        Operation::KeyMix { gain, invert } => key(node, values, input, gain, invert, context),
        _ => composite_image(node, values, input, context),
    }
}

fn key(
    node: &Node,
    values: &BTreeMap<String, HeldValue>,
    input: &Frame,
    gain: f32,
    invert: bool,
    context: &RenderContext,
) -> Result<Value> {
    let local = mask(node, values, 1)?;
    let external = mask(node, values, 2)?;
    let _memory = context.reserve(input.pixels.len() as u64 * 4)?;
    let mut coverage = Vec::with_capacity(input.pixels.len());
    for (row, pixels) in input.pixels.chunks(input.width as usize).enumerate() {
        context.check_cancelled()?;
        for x in 0..pixels.len() {
            let index = row * input.width as usize + x;
            let selected = local.values[index] * external.values[index];
            coverage.push((if invert { 1.0 - selected } else { selected }) * gain);
        }
    }
    Ok(Value::Mask(Arc::new(Mask {
        width: input.width,
        height: input.height,
        values: coverage,
    })))
}

fn composite_image(
    node: &Node,
    values: &BTreeMap<String, HeldValue>,
    input: &Frame,
    context: &RenderContext,
) -> Result<Value> {
    let _memory = context.reserve(input.byte_len())?;
    let mut result = input.clone();
    let pixel = pixel_function(node, values)?;
    for (row, pixels) in result.pixels.chunks_mut(input.width as usize).enumerate() {
        context.check_cancelled()?;
        for (x, p) in pixels.iter_mut().enumerate() {
            *p = pixel(row * input.width as usize + x, *p);
        }
    }
    result.validate()?;
    Ok(Value::Image(Arc::new(result)))
}

type PixelFunction<'a> = Box<dyn Fn(usize, [f32; 4]) -> [f32; 4] + 'a>;
fn pixel_function<'a>(
    node: &Node,
    values: &'a BTreeMap<String, HeldValue>,
) -> Result<PixelFunction<'a>> {
    Ok(match node.operation {
        Operation::SetAlpha => {
            let key = mask(node, values, 1)?;
            Box::new(move |i, mut p| {
                p[3] *= key.values[i];
                p
            })
        }
        Operation::LayerMix => {
            let top = image(node, values, 1)?;
            let key = mask(node, values, 2)?;
            Box::new(move |i, p| over(p, top.pixels[i], key.values[i]))
        }
        Operation::ParallelMix => {
            let left = image(node, values, 1)?;
            let right = image(node, values, 2)?;
            let lk = mask(node, values, 3)?;
            let rk = mask(node, values, 4)?;
            Box::new(move |i, p| {
                parallel(
                    p,
                    left.pixels[i],
                    right.pixels[i],
                    lk.values[i],
                    rk.values[i],
                )
            })
        }
        _ => bail!("unsupported composition"),
    })
}

fn over(base: [f32; 4], top: [f32; 4], coverage: f32) -> [f32; 4] {
    let alpha = top[3] * coverage;
    let output_alpha = alpha + base[3] * (1.0 - alpha);
    if output_alpha <= f32::EPSILON {
        return [0.0; 4];
    }
    let mut result = [0.0; 4];
    for i in 0..3 {
        result[i] = (top[i] * alpha + base[i] * base[3] * (1.0 - alpha)) / output_alpha;
    }
    result[3] = output_alpha;
    result
}

// Normalize overlapping coverage; uncovered areas keep the common input.
// This is an explicit Iris rule, not a claim about proprietary Resolve math.
fn parallel(base: [f32; 4], left: [f32; 4], right: [f32; 4], lk: f32, rk: f32) -> [f32; 4] {
    let denominator = (lk + rk).max(1.0);
    let mut result = base;
    for i in 0..3 {
        result[i] += ((left[i] - base[i]) * lk + (right[i] - base[i]) * rk) / denominator;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_over_and_parallel_are_distinct() {
        assert_eq!(
            over([0.0, 0.0, 1.0, 1.0], [1.0, 0.0, 0.0, 0.5], 1.0),
            [0.5, 0.0, 0.5, 1.0]
        );
        assert_eq!(
            parallel(
                [0.2, 0.2, 0.2, 1.0],
                [0.8, 0.2, 0.2, 1.0],
                [0.2, 0.8, 0.2, 1.0],
                1.0,
                1.0
            ),
            [0.5, 0.5, 0.2, 1.0]
        );
        assert_eq!(
            parallel(
                [0.2, 0.2, 0.2, 1.0],
                [0.8, 0.2, 0.2, 1.0],
                [0.2, 0.8, 0.2, 1.0],
                0.0,
                0.0
            ),
            [0.2, 0.2, 0.2, 1.0]
        );
    }
}
