// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::document::Node;

pub(super) fn live_inputs(
    document: &EditDocument,
    plan: &Plan,
) -> (BTreeSet<String>, BTreeMap<String, usize>) {
    let mut needed = BTreeSet::from([document.output_node.clone()]);
    for &index in plan.order.iter().rev() {
        let node = &document.nodes[index];
        if needed.contains(&node.id) {
            needed.extend(active_inputs(node).iter().cloned());
        }
    }
    let mut uses = BTreeMap::new();
    for node in document.nodes.iter().filter(|n| needed.contains(&n.id)) {
        for input in active_inputs(node) {
            *uses.entry(input.clone()).or_default() += 1;
        }
    }
    (needed, uses)
}

pub(super) fn release_inputs(
    node: &Node,
    uses: &mut BTreeMap<String, usize>,
    values: &mut BTreeMap<String, HeldValue>,
) {
    for input in active_inputs(node) {
        if let Some(count) = uses.get_mut(input) {
            *count -= 1;
            if *count == 0 {
                values.remove(input);
            }
        }
    }
}

fn active_inputs(node: &Node) -> &[String] {
    if node.enabled {
        &node.inputs
    } else {
        &node.inputs[..1]
    }
}

pub(super) struct Evaluation<'a, R> {
    // Transfer the sole evaluator-owned source into the graph, so last-use
    // release can reclaim it instead of pinning it for the entire evaluation.
    pub source: Option<Arc<Frame>>,
    pub resources: &'a R,
    pub context: &'a RenderContext,
    pub executor: Option<&'a Executor>,
    pub trace: ExecutionTrace,
}
impl<R: Resources> Evaluation<'_, R> {
    pub fn node(&mut self, node: &Node, values: &BTreeMap<String, HeldValue>) -> Result<HeldValue> {
        let value = self.evaluate(node, values)?;
        if node.enabled
            && !identity(&node.operation)
            && !matches!(
                node.operation,
                Operation::Source | Operation::Adjustments(_)
            )
        {
            self.trace.cpu_nodes += 1;
        }
        let memory = match &value {
            Value::Image(f) => {
                self.context.track_frame(f)?;
                None
            }
            Value::Mask(m) => Some(self.context.reserve(m.values.len() as u64 * 4)?),
        };
        Ok(HeldValue {
            value,
            _memory: memory,
        })
    }

    fn evaluate(&mut self, node: &Node, values: &BTreeMap<String, HeldValue>) -> Result<Value> {
        if matches!(node.operation, Operation::Source) {
            return Ok(Value::Image(
                self.source.take().context("source already consumed")?,
            ));
        }
        let input = image(node, values, 0)?;
        if !node.enabled || identity(&node.operation) {
            return Ok(Value::Image(input));
        }
        match &node.operation {
            Operation::Mask(params) => Ok(Value::Mask(Arc::new(crate::mask::render(
                &input,
                params,
                self.context,
            )?))),
            Operation::Blend { opacity } => {
                blend_node(node, values, &input, *opacity, self.context)
            }
            _ => Ok(Value::Image(Arc::new(
                self.image_operation(&input, &node.operation)?,
            ))),
        }
    }

    fn image_operation(&mut self, input: &Frame, operation: &Operation) -> Result<Frame> {
        match operation {
            Operation::Adjustments(params) => {
                if let Some(executor) = self.executor {
                    executor.adjustments(input, params, self.context, &mut self.trace)
                } else {
                    self.trace.cpu_nodes += 1;
                    crate::adjustments::apply(input, params, self.context)
                }
            }
            Operation::Detail(params) => crate::detail::apply(input, params, self.context),
            Operation::Geometry(params) => crate::geometry::apply(input, params, self.context),
            Operation::Lut {
                resource,
                strength,
                domain,
            } => self.lut(input, resource, *strength, *domain),
            _ => bail!("operation does not produce an image"),
        }
    }

    fn lut(
        &self,
        input: &Frame,
        resource: &str,
        strength: f32,
        domain: crate::lut::LutDomain,
    ) -> Result<Frame> {
        let bytes = self.resources.load(resource)?;
        let _memory = self.context.reserve(bytes.len() as u64)?;
        ensure!(
            crate::document::hash_bytes(&bytes) == resource,
            "LUT content changed"
        );
        let table = crate::lut::CubeLut::parse(std::str::from_utf8(&bytes)?)?;
        table.apply(input, strength, domain, self.context)
    }
}

// Restrict this fast path to exact defaults; nonidentity kernels retain their
// existing behavior and validation still covers every node before evaluation.
fn identity(operation: &Operation) -> bool {
    match operation {
        Operation::Adjustments(params) => params == &Default::default(),
        Operation::Detail(params) => params == &Default::default(),
        Operation::Geometry(params) => params == &Default::default(),
        _ => false,
    }
}

fn image(node: &Node, values: &BTreeMap<String, HeldValue>, index: usize) -> Result<Arc<Frame>> {
    match &values
        .get(&node.inputs[index])
        .context("missing graph image")?
        .value
    {
        Value::Image(frame) => Ok(frame.clone()),
        _ => bail!("image input required"),
    }
}

fn blend_node(
    node: &Node,
    values: &BTreeMap<String, HeldValue>,
    input: &Frame,
    opacity: f32,
    context: &RenderContext,
) -> Result<Value> {
    let adjusted = image(node, values, 1)?;
    let Value::Mask(mask) = &values
        .get(&node.inputs[2])
        .context("missing blend mask")?
        .value
    else {
        bail!("mask input required");
    };
    Ok(Value::Image(Arc::new(blend(
        input, &adjusted, mask, opacity, context,
    )?)))
}
