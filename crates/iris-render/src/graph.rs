// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    document::{EditDocument, Operation, MAX_NODES},
    mask::Mask,
    Frame, MemoryReservation, RenderContext,
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use utoipa::ToSchema;

#[derive(Default)]
pub struct Executor {
    #[cfg(feature = "gpu")]
    gpu: std::sync::OnceLock<std::result::Result<crate::gpu::GpuRenderer, String>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionTrace {
    /// Nodes which performed CPU pixel work; source, bypass and identity aliases are excluded.
    pub cpu_nodes: u32,
    /// Nodes which performed GPU pixel work.
    pub gpu_nodes: u32,
    pub fallback_reasons: Vec<String>,
}

impl Executor {
    fn adjustments(
        &self,
        input: &Frame,
        params: &crate::adjustments::Adjustments,
        context: &RenderContext,
        trace: &mut ExecutionTrace,
    ) -> Result<Frame> {
        context.check_cancelled()?;
        #[cfg(feature = "gpu")]
        if params.exposure != 0.0
            && input.pixels.len() >= 65536
            && crate::gpu::GpuRenderer::supports(params)
        {
            let device = self
                .gpu
                .get_or_init(|| crate::gpu::GpuRenderer::new().map_err(|error| error.to_string()));
            match device {
                Ok(device) => match device.apply(input, params, context) {
                    Ok(frame) => {
                        trace.gpu_nodes += 1;
                        return Ok(frame);
                    }
                    Err(error) => {
                        context.check_cancelled()?;
                        if trace.fallback_reasons.len() < 8 {
                            trace.fallback_reasons.push(error.to_string());
                        }
                    }
                },
                Err(error) => {
                    if trace.fallback_reasons.len() < 8 {
                        trace.fallback_reasons.push(error.clone());
                    }
                }
            }
        }
        trace.cpu_nodes += 1;
        crate::adjustments::apply(input, params, context)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortType {
    Image,
    Mask,
}

pub struct Plan {
    pub order: Vec<usize>,
    pub outputs: BTreeMap<String, PortType>,
}

mod compile;
pub use compile::compile;

/// Resource implementations must check their own storage permissions. The engine
/// additionally verifies content hashes before interpreting data.
pub trait Resources {
    fn load(&self, fingerprint: &str) -> Result<Vec<u8>>;
}
impl Resources for BTreeMap<String, Vec<u8>> {
    fn load(&self, fingerprint: &str) -> Result<Vec<u8>> {
        self.get(fingerprint)
            .cloned()
            .context("missing render resource")
    }
}

enum Value {
    Image(Arc<Frame>),
    Mask(Arc<Mask>),
}
struct HeldValue {
    value: Value,
    _memory: Option<MemoryReservation>,
}

pub fn render(
    document: &EditDocument,
    source: Arc<Frame>,
    resources: &impl Resources,
    context: &RenderContext,
) -> Result<Arc<Frame>> {
    render_internal(document, source, resources, context, None).map(|(frame, _)| frame)
}

pub fn render_with_executor(
    document: &EditDocument,
    source: Arc<Frame>,
    resources: &impl Resources,
    context: &RenderContext,
    executor: &Executor,
) -> Result<(Arc<Frame>, ExecutionTrace)> {
    render_internal(document, source, resources, context, Some(executor))
}

mod evaluate;

fn render_internal(
    document: &EditDocument,
    source: Arc<Frame>,
    resources: &impl Resources,
    context: &RenderContext,
    executor: Option<&Executor>,
) -> Result<(Arc<Frame>, ExecutionTrace)> {
    document.validate()?;
    source.validate()?;
    ensure!(
        source.width == document.source.width && source.height == document.source.height,
        "source dimensions changed"
    );
    context.check_cancelled()?;
    context.track_frame(&source)?;
    let plan = compile(document)?;
    let (needed, mut uses) = evaluate::live_inputs(document, &plan);
    let mut evaluator = evaluate::Evaluation {
        source: Some(source),
        resources,
        context,
        executor,
        trace: ExecutionTrace::default(),
    };
    let mut values = BTreeMap::new();
    for index in plan.order {
        let node = &document.nodes[index];
        if !needed.contains(&node.id) {
            continue;
        }
        context.check_cancelled()?;
        let value = evaluator.node(node, &values)?;
        values.insert(node.id.clone(), value);
        evaluate::release_inputs(node, &mut uses, &mut values);
    }
    let output = values
        .remove(&document.output_node)
        .context("missing render output")?;
    match output.value {
        Value::Image(frame) => Ok((frame, evaluator.trace)),
        _ => bail!("invalid graph output"),
    }
}

pub fn blend(
    base: &Frame,
    adjusted: &Frame,
    mask: &Mask,
    opacity: f32,
    context: &RenderContext,
) -> Result<Frame> {
    base.validate()?;
    adjusted.validate()?;
    validate_blend(base, adjusted, mask, opacity)?;
    let _memory = context.reserve(base.byte_len())?;
    let mut output = base.clone();
    for (row, pixels) in output.pixels.chunks_mut(base.width as usize).enumerate() {
        context.check_cancelled()?;
        for (x, pixel) in pixels.iter_mut().enumerate() {
            let i = row * base.width as usize + x;
            *pixel = blend_pixel(base.pixels[i], adjusted.pixels[i], mask.values[i] * opacity);
        }
    }
    output.validate()?;
    Ok(output)
}

fn validate_blend(base: &Frame, adjusted: &Frame, mask: &Mask, opacity: f32) -> Result<()> {
    ensure!(
        base.width == adjusted.width
            && base.height == adjusted.height
            && base.width == mask.width
            && base.height == mask.height
            && mask.values.len() == base.pixels.len(),
        "blend inputs use different coordinate grids"
    );
    ensure!(
        opacity.is_finite()
            && (0.0..=1.0).contains(&opacity)
            && mask
                .values
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
        "invalid blend weights"
    );
    Ok(())
}

fn blend_pixel(a: [f32; 4], b: [f32; 4], weight: f32) -> [f32; 4] {
    // Endpoint blends are byte-preserving identities, including hidden RGB.
    if weight == 0.0 {
        return a;
    }
    if weight == 1.0 {
        return b;
    }
    // Evaluate premultiplied weights in f64: FP32 products can underflow for
    // valid subnormal alpha, or lose finite extended-range RGB on division.
    let weight = f64::from(weight);
    let left = f64::from(a[3]) * (1.0 - weight);
    let right = f64::from(b[3]) * weight;
    let alpha = left + right;
    let mut pixel = [0.0, 0.0, 0.0, alpha as f32];
    if alpha > 0.0 {
        for c in 0..3 {
            pixel[c] = ((f64::from(a[c]) * left + f64::from(b[c]) * right) / alpha) as f32;
        }
    }
    pixel
}
