import { test } from 'node:test';
import assert from 'node:assert/strict';
import { defaultAdjustments, parameters, updatePanel } from '../src/editing-document.js';
import {
  defaultSeparated,
  observationDocument,
  separatedParameters,
  withSeparated,
} from '../src/editing-observation.js';
import type { EditDocument } from '../src/editing-client.js';
import { AdjustmentClipboard } from '../src/editing-clipboard.js';

function fixture(): EditDocument {
  return {
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
      tone_map: { kind: 'none' },
      gamut_compression: true,
      background: [1, 1, 1],
    },
  };
}
test('observation is a disposable final node, preserving persistent controls and output', () => {
  let doc = fixture();
  const a = defaultAdjustments();
  a.exposure = 0.5;
  a.separated.tone.exposure = 1;
  a.separated.color.hue = 30;
  doc = updatePanel(doc, { type: 'adjustments', parameters: a });
  const snapshot = structuredClone(doc);
  for (const mode of ['tone', 'color'] as const) {
    const view = observationDocument(doc, mode);
    assert.equal(view.nodes.length, doc.nodes.length + 1);
    assert.equal(view.nodes.at(-1)?.inputs[0], doc.output_node);
    const op = view.nodes.at(-1)!.operation;
    assert.equal(op.type, 'adjustments');
    if (op.type === 'adjustments') {
      assert.equal(op.parameters.separated.observation, mode);
      assert.equal(op.parameters.separated.tone.exposure, 0);
    }
    assert.deepEqual(parameters(view, 'adjustments'), a);
    assert.deepEqual(doc, snapshot);
  }
  assert.equal(observationDocument(doc, 'full'), doc);
});
test('independent groups coexist, reset separately, and survive copying', () => {
  const base = defaultAdjustments();
  base.exposure = 0.7;
  const s = defaultSeparated();
  s.tone.exposure = 1;
  s.color.temperature = 0.5;
  const a = withSeparated(base, s);
  const doc = updatePanel(fixture(), { type: 'adjustments', parameters: a });
  const clipboard = new AdjustmentClipboard();
  clipboard.copy(doc);
  const copied = parameters(
    clipboard.paste(
      updatePanel(fixture(), { type: 'adjustments', parameters: defaultAdjustments() })
    ),
    'adjustments'
  );
  assert.deepEqual(copied.separated, s);
  assert.equal(copied.exposure, 0.7);
  const reset = withSeparated(a, { ...s, tone: defaultSeparated().tone });
  assert.equal(reset.separated.tone.exposure, 0);
  assert.equal(reset.separated.color.temperature, 0.5);
  assert.equal(reset.exposure, 0.7);
});
test('legacy edits default to neutral groups and observation ids do not collide', () => {
  const a = defaultAdjustments();
  const legacy = JSON.parse(JSON.stringify(a));
  delete legacy.separated;
  assert.deepEqual(separatedParameters(legacy), defaultSeparated());
  const doc = fixture();
  doc.nodes[0]!.id = 'preview-observation';
  doc.output_node = 'preview-observation';
  const view = observationDocument(doc, 'tone');
  assert.notEqual(view.output_node, doc.output_node);
  assert.equal(new Set(view.nodes.map((n) => n.id)).size, view.nodes.length);
});
