import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { EditDocument } from '../src/editing-client.js';
import { resetDocument } from '../src/editing-document.js';
import { parameters, updatePanel } from '../src/editing-document.js';
import {
  addCorrector,
  correctors,
  correctorDocument,
  updateCorrector,
  removeCorrector,
  addMixerBranch,
  addAlphaOutput,
  completeCorrector,
  disconnectKey,
} from '../src/editing-correctors.js';
import { nodeView } from '../src/editing-node-view.js';
import { maskLayers, setMaskBlend } from '../src/editing-masks.js';
import {
  connectVisiblePort,
  selectedCorrectorNode,
  setCorrectorEnabled,
  editCorrectorMask,
  updateCorrectorKey,
  nodePanelDocument,
} from '../src/editing-node-actions.js';
import {
  addSerialNode,
  connectNode,
  removeSerialNode,
  validateGraph,
} from '../src/editing-graph.js';
function fixture(): EditDocument {
  return resetDocument({
    schema_version: 1,
    source: {
      photo_id: '1',
      fingerprint: 'a'.repeat(64),
      width: 32,
      height: 24,
      reference_kind: 'display',
    },
    nodes: [
      {
        id: 'source',
        name: '原片',
        version: 1,
        enabled: true,
        inputs: [],
        operation: { type: 'source' },
      },
    ],
    output_node: 'source',
    panel_bindings: [],
    node_positions: {},
    output: {
      space: 'srgb',
      gamut_compression: true,
      tone_map: { kind: 'none' },
      background: [0, 0, 0],
    },
  } as EditDocument);
}
test('serial insertion and removal preserve managed parameters and source', () => {
  const source = fixture(),
    copy = structuredClone(source),
    next = addSerialNode(source, 'basic', 'adjustments', 'extra');
  assert.equal(next.nodes.find((n) => n.id === 'detail')!.inputs[0], 'extra');
  assert.equal(next.nodes.find((n) => n.id === 'extra')!.inputs[0], 'basic');
  assert.deepEqual(removeSerialNode(next, 'extra'), source);
  assert.deepEqual(source, copy);
  assert.throws(() => removeSerialNode(source, 'basic'), /基础节点/);
});
test('graph rejects cycles, missing and mistyped ports before saving', () => {
  const source = fixture();
  assert.throws(() => connectNode(source, 'geometry', 'basic', 0), /循环/);
  assert.throws(() => connectNode(source, 'missing', 'basic', 0), /混接/);
  const bad = structuredClone(source);
  bad.nodes[0]!.inputs = ['basic'];
  assert.throws(() => validateGraph(bad), /输入/);
});

test('a user node owns all adjustment categories without rewriting existing work', () => {
  const base = fixture(),
    original = structuredClone(base),
    doc = addCorrector(base, base.output_node, 'grade1');
  const group = correctors(doc).at(-1)!;
  let projected = correctorDocument(doc, group);
  projected = updatePanel(projected, {
    type: 'adjustments',
    parameters: { ...parameters(projected, 'adjustments'), exposure: 1 },
  });
  projected = updatePanel(projected, {
    type: 'detail',
    parameters: { ...parameters(projected, 'detail'), sharpening: 0.5 },
  });
  projected = updatePanel(projected, {
    type: 'geometry',
    parameters: { ...parameters(projected, 'geometry'), rotation_degrees: 12 },
  });
  const edited = updateCorrector(doc, group, projected);
  assert.equal(parameters(correctorDocument(edited, group), 'adjustments').exposure, 1);
  assert.equal(parameters(correctorDocument(edited, group), 'detail').sharpening, 0.5);
  assert.equal(parameters(correctorDocument(edited, group), 'geometry').rotation_degrees, 12);
  assert.deepEqual(edited.source, base.source);
  assert.deepEqual(removeCorrector(doc, group), base);
  assert.deepEqual(base, original);
  const view = nodeView(doc).find((n) => n.id === group.id)!;
  assert.equal(view.kind, 'corrector');
  assert.deepEqual(
    view.outputs.map((p) => p.key),
    [false, true]
  );
});
test('parallel and layer branches use unmasked correction once with separate keys', () => {
  const doc = addCorrector(fixture(), 'geometry', 'grade1'),
    group = correctors(doc).at(-1)!;
  for (const layer of [false, true]) {
    const branched = addMixerBranch(doc, group, layer),
      branch = correctors(branched).at(-1)!,
      mix = branched.nodes.at(-1)!;
    assert.equal(branch.input, group.input);
    assert.equal(mix.operation.type, layer ? 'layer_mix' : 'parallel_mix');
    assert.ok(mix.inputs.includes(branch.detail));
    assert.ok(mix.inputs.includes(branch.key!));
    assert.ok(!mix.inputs.includes(branch.output)); // Avoid applying the same mask twice.
    assert.doesNotThrow(() => validateGraph(branched));
  }
});
test('key routing and alpha output reject RGB/Key confusion and cycles', () => {
  const first = addCorrector(fixture(), 'geometry', 'grade1'),
    g1 = correctors(first).at(-1)!;
  const second = addCorrector(first, g1.output, 'grade2'),
    g2 = correctors(second).at(-1)!;
  const connected = connectNode(second, g1.key!, g2.key!, 2);
  assert.equal(connected.nodes.find((n) => n.id === g2.key)!.inputs[2], g1.key);
  assert.throws(() => connectNode(second, g1.output, g2.key!, 2), /混接/);
  assert.throws(() => connectNode(second, g2.output, g1.geometry, 0), /循环/);
  const output = addAlphaOutput(connected, g2.key!);
  assert.equal(output.nodes.at(-1)!.operation.type, 'set_alpha');
  assert.throws(() => removeCorrector(connected, g1), /Key/);
});

test('legacy nodes expose real working Key ports without changing the document on view', () => {
  const base = fixture(),
    copy = structuredClone(base),
    withSecond = addCorrector(base, base.output_node, 'second');
  const groups = correctors(withSecond),
    firstView = nodeView(withSecond).find((n) => n.id === groups[0]!.id)!,
    secondView = nodeView(withSecond).find((n) => n.id === groups[1]!.id)!;
  assert.equal(firstView.outputs.length, 2);
  assert.deepEqual(base, copy);
  const connected = connectVisiblePort(withSecond, firstView.outputs[1]!, secondView.inputs[1]!);
  const [first, second] = correctors(connected);
  const upgraded = first!.adjustment === 'basic' ? first! : second!;
  const target = first!.adjustment === 'basic' ? second! : first!;
  assert.equal(connected.nodes.find((n) => n.id === target.key)!.inputs[2], upgraded.key);
  assert.deepEqual(
    connected.nodes.find((n) => n.id === 'basic')!.operation,
    base.nodes.find((n) => n.id === 'basic')!.operation
  );
  assert.equal(selectedCorrectorNode(connected, 'geometry')!.id, upgraded.id);
  assert.throws(
    () => connectVisiblePort(withSecond, firstView.outputs[0]!, secondView.inputs[1]!),
    /混接/
  );
});

test('legacy geometry stays in sync through both interfaces and RGB reconnection', () => {
  const base = fixture();
  const ready = completeCorrector(base, correctors(base)[0]!);
  const edited = updatePanel(ready.document, {
    type: 'geometry',
    parameters: {
      ...parameters(ready.document, 'geometry'),
      rotation_degrees: 15,
    },
  });
  assert.doesNotThrow(() => validateGraph(edited));
  assert.deepEqual(
    edited.nodes.find((n) => n.id === ready.group.anchor)!.operation,
    edited.nodes.find((n) => n.id === ready.group.geometry)!.operation
  );
  const key = updateCorrectorKey(edited, ready.group, 0.4, true);
  const disabled = setCorrectorEnabled(key, ready.group, false);
  assert.equal(disabled.nodes.find((n) => n.id === ready.group.output)!.enabled, false);
  assert.deepEqual(
    disabled.nodes.find((n) => n.id === ready.group.geometry),
    key.nodes.find((n) => n.id === ready.group.geometry)
  );
  assert.deepEqual(setCorrectorEnabled(disabled, ready.group, true), key);
  const mask = editCorrectorMask(ready.document, ready.group);
  assert.equal(mask.mask, ready.group.mask);
  assert.deepEqual(editCorrectorMask(mask.document, ready.group), mask);
});

test('legacy branch creation, Key disconnect and Alpha selection preserve visible node identity', () => {
  const base = fixture();
  for (const layer of [false, true])
    assert.doesNotThrow(() => addMixerBranch(base, correctors(base)[0]!, layer));
  const ready = completeCorrector(base, correctors(base)[0]!);
  const alpha = addAlphaOutput(ready.document, ready.group.key!);
  assert.equal(selectedCorrectorNode(alpha, null)!.id, ready.group.id);
  assert.equal(disconnectKey(alpha, '__graph_output', 1).output_node, ready.group.output);
  const view = nodeView(ready.document).find((n) => n.id === ready.group.id)!;
  assert.throws(
    () => connectVisiblePort(ready.document, view.outputs[1]!, view.inputs[1]!),
    /循环/
  );
});

test('deleting a generated branch removes its mixer and reconnects the surviving grade', () => {
  const base = addCorrector(fixture(), 'geometry', 'left'),
    left = correctors(base).at(-1)!;
  for (const layer of [false, true]) {
    const branched = addMixerBranch(base, left, layer),
      right = correctors(branched).at(-1)!;
    const restored = removeCorrector(branched, right);
    assert.deepEqual(restored, base);
  }
  const parallel = addMixerBranch(base, left, false),
    right = correctors(parallel).at(-1)!;
  const remaining = removeCorrector(parallel, left);
  assert.equal(remaining.output_node, right.output);
  assert.doesNotThrow(() => validateGraph(remaining));
});

test('node crop projection edits the selected geometry and keeps traditional bindings unchanged', () => {
  const base = addCorrector(fixture(), 'geometry', 'crop-target'),
    target = correctors(base).at(-1)!;
  const scoped = nodePanelDocument(base, target.id);
  const edited = updatePanel(scoped, {
    type: 'geometry',
    parameters: { ...parameters(scoped, 'geometry'), crop: [0.1, 0.2, 0.7, 0.6] },
  });
  const saved = validateGraph({ ...edited, panel_bindings: base.panel_bindings });
  assert.deepEqual(parameters(saved, 'geometry').crop, [0, 0, 1, 1]);
  assert.deepEqual(
    parameters(nodePanelDocument(saved, target.id), 'geometry').crop,
    [0.1, 0.2, 0.7, 0.6]
  );
  assert.deepEqual(
    saved.nodes.find((n) => n.id === 'geometry'),
    base.nodes.find((n) => n.id === 'geometry')
  );
});

test('bypass reaches unmasked mixer inputs and restores preexisting disabled operations', () => {
  const doc = addCorrector(fixture(), 'geometry', 'branch-bypass'),
    group = correctors(doc).at(-1)!;
  const branch = addMixerBranch(doc, group, false);
  const original = {
    ...branch,
    nodes: branch.nodes.map((n) => (n.id === group.detail ? { ...n, enabled: false } : n)),
  };
  const bypassed = setCorrectorEnabled(original, group, false);
  assert.equal(bypassed.nodes.find((n) => n.id === group.adjustment)!.enabled, false);
  assert.equal(bypassed.nodes.find((n) => n.id === group.detail)!.enabled, false);
  assert.equal(bypassed.nodes.find((n) => n.id === group.geometry)!.enabled, true);
  assert.deepEqual(
    setCorrectorEnabled(
      bypassed,
      correctors(bypassed).find((g) => g.id === group.id)!,
      true
    ),
    original
  );
  assert.deepEqual(setCorrectorEnabled(bypassed, group, false), bypassed);
});

test('node mask opacity and visibility reach both serial output and parallel Key inputs', () => {
  const doc = addCorrector(fixture(), 'geometry', 'mask-mix'),
    group = correctors(doc).at(-1)!;
  const masked = editCorrectorMask(doc, group).document;
  const layer = maskLayers(masked)[0]!;
  const faded = setMaskBlend(masked, layer, true, 0.25);
  const key = faded.nodes.find((n) => n.id === group.key)!;
  assert.equal(key.operation.type === 'key_mix' && key.operation.parameters.gain, 0.25);
  assert.equal(maskLayers(faded)[0]!.opacity, 0.25);
  const disabled = setCorrectorEnabled(faded, group, false);
  assert.deepEqual(setMaskBlend(disabled, maskLayers(disabled)[0]!, true, 0.25), faded);
});
