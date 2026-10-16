import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyLut, lutNode, outputLut, removeLut, updateLut } from '../src/editing-lut.js';
import { addMask, maskLayers, removeMask } from '../src/editing-masks.js';
import { defaultAdjustments } from '../src/editing-document.js';
import { EditingClient, type EditDocument } from '../src/editing-client.js';
import { IrisClient } from '../src/client.js';
import { Transport } from '../src/transport.js';

function document(): EditDocument {
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
        name: 'Source',
        enabled: true,
        version: 1,
        inputs: [],
        operation: { type: 'source' },
      },
      {
        id: 'basic',
        name: 'Basic',
        enabled: true,
        version: 1,
        inputs: ['source'],
        operation: { type: 'adjustments', parameters: defaultAdjustments() },
      },
    ],
    output_node: 'basic',
    panel_bindings: [{ role: 'basic', node_id: 'basic' }],
    node_positions: { basic: [10, 20] },
    output: {
      space: 'display_p3',
      tone_map: { kind: 'none' },
      gamut_compression: true,
      background: [1, 1, 1],
    },
  };
}
test('LUT import and replacement preserve existing graph, source, output space and strength', () => {
  const original = document();
  let next = applyLut(original, 'b'.repeat(64), 'Warm.cube', 'encoded_srgb');
  assert.deepEqual(next.source, original.source);
  assert.deepEqual(next.output, original.output);
  assert.deepEqual(next.nodes.slice(0, 2), original.nodes);
  assert.deepEqual(outputLut(next)?.inputs, ['basic']);
  next = updateLut(next, { strength: 0.42 }, false);
  assert.equal(lutNode(next)?.enabled, false);
  next = applyLut(next, 'c'.repeat(64), 'Cool.cube', 'linear_rec2020');
  assert.equal(next.nodes.length, 3);
  assert.equal(lutNode(next)?.enabled, true);
  assert.equal(lutNode(next)?.operation.parameters.strength, 0.42);
  assert.equal(lutNode(next)?.operation.parameters.resource, 'c'.repeat(64));
  assert.deepEqual(removeLut(next), original);
  assert.throws(() => applyLut(original, '../unsafe', 'bad', 'encoded_srgb'));
});
test('adding and removing local masks keeps the terminal LUT after all local adjustments', () => {
  const original = document();
  const masked = addMask(original, 'linear_gradient', 'one').document;
  const graded = applyLut(masked, 'b'.repeat(64), 'Look.cube', 'encoded_srgb');
  const next = addMask(graded, 'radial_gradient', 'two').document;
  const lut = outputLut(next)!;
  assert.equal(next.output_node, lut.id);
  assert.deepEqual(lut.inputs, ['mask-two-blend']);
  assert.deepEqual(next.nodes.find((n) => n.id === 'mask-two')?.inputs, ['mask-one-blend']);
  assert.deepEqual(next.nodes.find((n) => n.id === 'mask-two-blend')?.inputs, [
    'mask-one-blend',
    'mask-two-detail',
    'mask-two',
  ]);
  assert.deepEqual(
    removeMask(
      next,
      maskLayers(next).find((m) => m.id === 'mask-two')!
    ),
    graded
  );
  assert.deepEqual(removeLut(graded), masked);
});
test('panel LUT controls never take over custom LUT nodes or move nodes added downstream', () => {
  const original = document();
  original.nodes.push({
    id: 'color-lut',
    name: 'Custom',
    version: 1,
    enabled: true,
    inputs: ['basic'],
    operation: {
      type: 'lut',
      parameters: { resource: 'd'.repeat(64), strength: 0.2, domain: 'encoded_srgb' },
    },
  });
  original.output_node = 'color-lut';
  assert.equal(lutNode(original), undefined);
  const next = applyLut(original, 'b'.repeat(64), 'Panel.cube', 'encoded_srgb');
  assert.equal(lutNode(next)?.id, 'color-lut-1');
  assert.deepEqual(next.nodes[2], original.nodes[2]);
  assert.deepEqual(removeLut(next), original);
  next.nodes.push({
    id: 'after',
    name: 'After',
    version: 1,
    enabled: true,
    inputs: [next.output_node],
    operation: { type: 'adjustments', parameters: defaultAdjustments() },
  });
  next.output_node = 'after';
  assert.equal(outputLut(next), undefined);
  const replaced = applyLut(next, 'c'.repeat(64), 'Replace.cube', 'encoded_srgb');
  assert.equal(replaced.output_node, 'after');
  assert.deepEqual(lutNode(replaced)?.inputs, ['color-lut']);
  assert.deepEqual(removeLut(replaced).nodes.find((n) => n.id === 'after')?.inputs, ['color-lut']);
});
test('LUT upload sends binary contents with authentication to the existing resource API', async () => {
  const bytes = new Blob(['LUT_1D_SIZE 2\n0 0 0\n1 1 1\n']);
  const client = new EditingClient(
    new IrisClient(
      new Transport({ base_url: 'http://127.0.0.1:8900', token: 'test', version: 'test' }, (async (
        url,
        init
      ) => {
        assert.equal(String(url), 'http://127.0.0.1:8900/api/v1/edit-resources/import/lut');
        assert.equal(init?.method, 'POST');
        assert.equal(init?.body, bytes);
        assert.equal(new Headers(init?.headers).get('Authorization'), 'Bearer test');
        return Response.json({ hash: 'b'.repeat(64), kind: 'lut', size_bytes: bytes.size });
      }) as typeof fetch)
    )
  );
  assert.equal((await client.importLut(bytes)).kind, 'lut');
});
