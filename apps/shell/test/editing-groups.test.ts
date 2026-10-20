import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { EditDocument } from '../src/editing-client.js';
import {
  defaultAdjustments,
  defaultDetail,
  defaultGeometry,
  parameters,
  updatePanel,
} from '../src/editing-document.js';
import {
  adjustmentGroupModified,
  bypassAdjustmentGroup,
  resetAdjustmentGroup,
} from '../src/editing-groups.js';

function fixture(): EditDocument {
  const source: EditDocument = {
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
  let result = updatePanel(source, {
    type: 'adjustments',
    parameters: {
      ...defaultAdjustments(),
      exposure: 1,
      saturation: 0.3,
      grain: { amount: 0.1, seed: 7 },
    },
  });
  result = updatePanel(result, {
    type: 'detail',
    parameters: { ...defaultDetail(), texture: 0.4, sharpening: 1, luminance_denoise: 0.3 },
  });
  return updatePanel(result, {
    type: 'geometry',
    parameters: {
      ...defaultGeometry(),
      crop: [0.1, 0.2, 0.8, 0.7],
      rotation_degrees: 2,
      radial: [0.1, 0.0],
    },
  });
}
test('basic bypass is disposable and preserves detail, crop, effects and source', () => {
  const original = fixture();
  const saved = structuredClone(original);
  const preview = bypassAdjustmentGroup(original, 'basic');
  assert.equal(parameters(preview, 'adjustments').exposure, 0);
  assert.equal(parameters(preview, 'adjustments').saturation, 0);
  assert.equal(parameters(preview, 'detail').texture, 0);
  assert.equal(parameters(preview, 'detail').sharpening, 1);
  assert.deepEqual(parameters(preview, 'geometry'), parameters(original, 'geometry'));
  assert.deepEqual(
    parameters(preview, 'adjustments').grain,
    parameters(original, 'adjustments').grain
  );
  assert.deepEqual(original, saved);
  assert.equal(bypassAdjustmentGroup(original, null), original);
});
test('detail reset retains base texture; optical and transform reset never erase crop', () => {
  const original = fixture();
  const detail = resetAdjustmentGroup(original, 'detail');
  assert.equal(parameters(detail, 'detail').texture, 0.4);
  assert.equal(parameters(detail, 'detail').sharpening, 0);
  for (const group of ['optics', 'transform'] as const) {
    const result = resetAdjustmentGroup(original, group);
    assert.deepEqual(parameters(result, 'geometry').crop, parameters(original, 'geometry').crop);
    assert.equal(adjustmentGroupModified(result, group), false);
  }
  assert.equal(
    parameters(resetAdjustmentGroup(original, 'optics'), 'geometry').rotation_degrees,
    2
  );
});
test('modified markers track reset values without creating nodes on pristine sources', () => {
  const original = fixture();
  assert.equal(adjustmentGroupModified(original, 'basic'), true);
  assert.equal(adjustmentGroupModified(resetAdjustmentGroup(original, 'basic'), 'basic'), false);
  const source = {
    ...original,
    nodes: original.nodes.slice(0, 1),
    panel_bindings: [],
    output_node: 'source',
  };
  assert.equal(adjustmentGroupModified(source, 'basic'), false);
  assert.deepEqual(resetAdjustmentGroup(source, 'basic'), source);
});
test('mask reset touches only its own bindings and retains the global adjustment', () => {
  const original = fixture();
  const a = original.nodes.find((n) => n.operation.type === 'adjustments')!;
  const d = original.nodes.find((n) => n.operation.type === 'detail')!;
  const document = {
    ...original,
    nodes: [...original.nodes, { ...a, id: 'local-a' }, { ...d, id: 'local-d' }],
  };
  const result = resetAdjustmentGroup(document, 'basic', {
    adjustmentId: 'local-a',
    detailId: 'local-d',
  });
  assert.equal(parameters(result, 'adjustments').exposure, 1);
  const local = result.nodes.find((n) => n.id === 'local-a')!.operation;
  assert.equal(local.type, 'adjustments');
  if (local.type === 'adjustments') assert.equal(local.parameters.exposure, 0);
  assert.deepEqual(result.panel_bindings, document.panel_bindings);
});
