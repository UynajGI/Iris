import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EditingClient, type EditDocument, type EditRecord } from '../src/editing-client.js';
import {
  defaultAdjustments,
  defaultDetail,
  parameters,
  updatePanel,
} from '../src/editing-document.js';
import { IrisClient } from '../src/client.js';
import { Transport, ApiError } from '../src/transport.js';
import { sourceDocument } from '../src/editing-document.js';
import { blackWhiteMix } from '../src/editing-document.js';
import { resetDocument } from '../src/editing-document.js';
import { focusEditingPhoto } from '../src/editing-navigation.js';
import {
  addMask,
  maskLayers,
  removeMask,
  setMaskBlend,
  swatchToWorking,
  workingToSwatch,
  validateMaskBudget,
  type MaskDefinition,
} from '../src/editing-masks.js';

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
        name: '原片',
        version: 1,
        enabled: true,
        operation: { type: 'source' },
        inputs: [],
      },
      {
        id: 'bound-basic',
        name: '影调',
        version: 1,
        enabled: true,
        operation: { type: 'adjustments', parameters: defaultAdjustments() },
        inputs: ['source'],
      },
      {
        id: 'custom-local',
        name: '局部',
        version: 1,
        enabled: true,
        operation: { type: 'adjustments', parameters: { ...defaultAdjustments(), exposure: 2 } },
        inputs: ['bound-basic'],
      },
    ],
    output_node: 'custom-local',
    output: {
      space: 'srgb',
      tone_map: { kind: 'none' },
      gamut_compression: true,
      background: [1, 1, 1],
    },
    panel_bindings: [{ role: 'basic', node_id: 'bound-basic' }],
    node_positions: { 'custom-local': [42, 64] },
  };
}
function record(): EditRecord {
  return {
    id: 'edit/1',
    photo_id: 1,
    name: '原始调整',
    revision: 7,
    document: document(),
    created_at: '',
    updated_at: '',
  };
}
test('a traditional panel changes only its bound node without flattening local branches', () => {
  const original = document();
  const edited = updatePanel(original, {
    type: 'adjustments',
    parameters: { ...parameters(original, 'adjustments'), exposure: -1.25 },
  });
  assert.equal(parameters(edited, 'adjustments').exposure, -1.25);
  assert.equal(parameters(original, 'adjustments').exposure, 0);
  assert.deepEqual(edited.nodes[2], original.nodes[2]);
  assert.equal(edited.output_node, original.output_node);
  assert.deepEqual(edited.node_positions, original.node_positions);
  assert.deepEqual(edited.source, original.source);
});
test('a missing panel appends a typed node while preserving the existing output chain', () => {
  const original = document();
  const edited = updatePanel(original, {
    type: 'detail',
    parameters: { ...defaultDetail(), texture: 0.3 },
  });
  const node = edited.nodes.find((item) => item.id === edited.output_node)!;
  assert.deepEqual(node.inputs, [original.output_node]);
  assert.equal(node.operation.type, 'detail');
  assert.equal(parameters(edited, 'detail').texture, 0.3);
  assert.deepEqual(edited.nodes.slice(0, original.nodes.length), original.nodes);
});
test('a conflicting node identifier cannot silently overwrite a custom node', () => {
  const value = document();
  value.nodes.push({ ...value.nodes[0]!, id: 'traditional-detail' });
  assert.throws(() => updatePanel(value, { type: 'detail', parameters: defaultDetail() }), /冲突/);
});
test('before comparison uses the high precision source binding and the same output transform', () => {
  const original = document();
  const before = sourceDocument(original);
  assert.equal(before.nodes.length, 1);
  assert.equal(before.nodes[0]!.operation.type, 'source');
  assert.equal(before.output_node, 'source');
  assert.deepEqual(before.source, original.source);
  assert.deepEqual(before.output, original.output);
  assert.equal(original.nodes.length, 3);
  assert.deepEqual(before.panel_bindings, []);
});
test('black and white channel edits preserve neutral gain and legal channel bounds', () => {
  let mix = [0.2627, 0.678, 0.0593];
  for (const channel of [0, 1, 2])
    for (const value of [-2, -0.4, 0, 0.8, 2]) {
      mix = blackWhiteMix(mix, channel, value);
      assert.ok(Math.abs(mix.reduce((sum, item) => sum + item, 0) - 1) < 1e-12);
      assert.ok(mix.every((item) => item >= -2 && item <= 2));
      assert.equal(mix[channel], value);
    }
});
test('resetting a photo removes local edits while retaining its immutable original identity', () => {
  const original = document();
  const local = addMask(original, 'radial_gradient', 'reset-example').document;
  const reset = resetDocument(local);
  assert.equal(maskLayers(reset).length, 0);
  assert.equal(parameters(reset, 'adjustments').exposure, 0);
  assert.equal(reset.nodes.length, 4);
  assert.equal(reset.source.photo_id, original.source.photo_id);
  assert.equal(reset.source.fingerprint, original.source.fingerprint);
  assert.equal(reset.source.width, original.source.width);
  assert.equal(local.nodes.length, original.nodes.length + 4);
});
test('editing photo navigation stays inside an active group and rejects members removed during save', () => {
  const actions: string[] = [];
  let members: number[] | null = [10, 11];
  const store = {
    getSnapshot: () => ({ groupSession: members ? { memberIds: members } : null }),
    focus: (id: number) => actions.push(`library:${id}`),
    focusGroupPhoto: (id: number) => actions.push(`group:${id}`),
  };
  assert.equal(focusEditingPhoto(store, 11), true);
  members = [10];
  assert.equal(focusEditingPhoto(store, 11), false);
  members = null;
  assert.equal(focusEditingPhoto(store, 11), true);
  assert.deepEqual(actions, ['group:11', 'library:11']);
});
test('draft image requests carry credentials in headers, exact base revision and cancellation', async () => {
  let request: { url: string; init: RequestInit } | undefined;
  const controller = new AbortController();
  const transport = new Transport(
    { base_url: 'http://127.0.0.1:9', token: 'synthetic-secret', version: 'test' },
    async (url, init) => {
      request = { url: String(url), init: init! };
      return new Response(new Uint8Array([1, 2, 3]), { headers: { 'Content-Type': 'image/jpeg' } });
    }
  );
  const api = new EditingClient(new IrisClient(transport));
  const result = await api.preview(record(), document(), 'session/1', 23, controller.signal);
  assert.equal(result.size, 3);
  assert.equal(request!.url, 'http://127.0.0.1:9/api/v1/edits/edit%2F1/preview-draft');
  assert.equal(request!.init.signal, controller.signal);
  assert.equal(new Headers(request!.init.headers).get('Authorization'), 'Bearer synthetic-secret');
  assert.equal(request!.init.redirect, 'error');
  assert.equal(request!.url.includes('synthetic-secret'), false);
  const body = JSON.parse(String(request!.init.body));
  assert.deepEqual(body.render, {
    revision: 7,
    session: 'session/1',
    sequence: 23,
    quality: 'interactive',
    max_edge: 1600,
  });
  assert.deepEqual(body.document, document());
});
test('saving and duplicating use their different server contracts', async () => {
  const requests: { url: string; body: unknown }[] = [];
  const transport = new Transport(
    { base_url: 'http://127.0.0.1:9', token: 'fixture', version: 'test' },
    async (url, init) => {
      requests.push({ url: String(url), body: JSON.parse(String(init?.body)) });
      return Response.json(record());
    }
  );
  const api = new EditingClient(new IrisClient(transport));
  await api.save(record(), document(), 'same-request-id');
  await api.duplicate(record(), '虚拟副本');
  assert.deepEqual(requests[0]!.body, {
    expected_revision: 7,
    request_id: 'same-request-id',
    document: document(),
  });
  assert.deepEqual(requests[1]!.body, { name: '虚拟副本' });
  assert.match(requests[1]!.url, /\/duplicate$/);
});
test('binary transport rejects authentication failures and paths outside the API', async () => {
  let calls = 0;
  const transport = new Transport(
    { base_url: 'http://127.0.0.1:9', token: 'fixture', version: 'test' },
    async () => {
      calls++;
      return new Response('denied', { status: 401 });
    }
  );
  await assert.rejects(
    transport.postBlob('/api/v1/edits/1/preview-draft', {}),
    (error) => error instanceof ApiError && error.status === 401
  );
  await assert.rejects(transport.postBlob('https://untrusted.invalid/', {}), /Invalid API path/);
  assert.equal(calls, 1);
});
test('local masks keep base and adjusted branches on identical geometry and can be removed without severing later layers', () => {
  const original = document();
  const first = addMask(original, 'linear_gradient', 'first');
  const second = addMask(first.document, 'radial_gradient', 'second');
  const layers = maskLayers(second.document);
  assert.equal(layers.length, 2);
  const blend = second.document.nodes.find((node) => node.id === layers[0]!.blendId)!;
  const adjustment = second.document.nodes.find((node) => node.id === layers[0]!.adjustmentId)!;
  const mask = second.document.nodes.find((node) => node.id === layers[0]!.id)!;
  assert.equal(blend.inputs[0], original.output_node);
  assert.equal(adjustment.inputs[0], original.output_node);
  assert.equal(mask.inputs[0], original.output_node);
  const removed = removeMask(second.document, layers[0]!);
  assert.equal(maskLayers(removed).length, 1);
  assert.equal(removed.output_node, second.document.output_node);
  assert.ok(
    removed.nodes.every((node) =>
      node.inputs.every((id) => removed.nodes.some((input) => input.id === id))
    )
  );
  assert.equal(
    removed.nodes.find((node) => node.id === layers[1]!.adjustmentId)!.inputs[0],
    original.output_node
  );
});
test('hiding a local mask bypasses its blend while keeping its recoverable settings', () => {
  const added = addMask(document(), 'brush', 'brush');
  const layer = maskLayers(added.document)[0]!;
  const hidden = setMaskBlend(added.document, layer, false, 0.37);
  const blend = hidden.nodes.find((node) => node.id === layer.blendId)!;
  assert.equal(blend.enabled, false);
  assert.equal(blend.operation.type === 'blend' && blend.operation.parameters.opacity, 0.37);
  assert.deepEqual(
    hidden.nodes.find((node) => node.id === layer.id),
    added.document.nodes.find((node) => node.id === layer.id)
  );
});
test('color swatches round-trip through the actual working primaries without shifting neutral colors', () => {
  for (const hex of ['#000000', '#ffffff', '#808080', '#ff0000', '#15b18c', '#425dc7'])
    assert.equal(workingToSwatch(swatchToWorking(hex)), hex);
  const white = swatchToWorking('#ffffff');
  assert.ok(white.every((value) => Math.abs(value - 1) < 1e-6));
});
test('paint recipes reject nesting and point counts beyond the backend safety contract', () => {
  let mask: MaskDefinition = { kind: 'brush', strokes: [] };
  for (let i = 0; i < 16; i++) mask = { kind: 'invert', mask };
  validateMaskBudget(mask);
  assert.throws(() => validateMaskBudget({ kind: 'invert', mask }), /上限/);
  assert.throws(
    () =>
      validateMaskBudget({
        kind: 'brush',
        strokes: [
          {
            points: Array.from({ length: 4097 }, () => [0.5, 0.5]),
            radius: 0.05,
            hardness: 0.5,
            flow: 1,
          },
        ],
      }),
    /控制点/
  );
});
