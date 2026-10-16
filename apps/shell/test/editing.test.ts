import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EditingClient, type EditDocument, type EditRecord } from '../src/editing-client.js';
import {
  defaultAdjustments,
  defaultGeometry,
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
import { applyLensProfile, lensProfileNode } from '../src/editing-lens.js';
import { applyColorPreset, colorPresetRecipe } from '../src/editing-presets.js';
import type { LensProfileSuggestion } from '../src/editing-client.js';
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

test('color presets contain portable global controls without source identity or local branches', () => {
  const original = document();
  original.source.raw_white_balance = [1.8, 1, 1.3];
  const recipe = colorPresetRecipe(original);
  assert.equal(recipe.raw_white_balance, null);
  assert.equal(recipe.nodes.length, 3);
  assert.equal(JSON.stringify(recipe).includes(original.source.fingerprint), false);
  assert.equal(
    recipe.nodes.some((node) => node.id === 'custom-local'),
    false
  );
  const basic = recipe.nodes.find((node) => node.operation.type === 'adjustments')!;
  assert.equal(basic.operation.type === 'adjustments' && basic.operation.parameters.exposure, 0);
  assert.equal('source' in recipe, false);
});

test('applying a color preset preserves target masks, optical geometry, output, and RAW balance', () => {
  let donor = updatePanel(document(), {
    type: 'adjustments',
    parameters: { ...defaultAdjustments(), exposure: 1.25 },
  });
  const recipe = colorPresetRecipe(donor);
  let target = resetDocument(document());
  target.source = {
    ...target.source,
    photo_id: '2',
    fingerprint: 'b'.repeat(64),
    raw_white_balance: [2, 1, 1.4],
  };
  target = updatePanel(target, {
    type: 'geometry',
    parameters: { ...defaultGeometry(), crop: [0.1, 0.2, 0.6, 0.5], radial: [0.02, -0.01] },
  });
  target = addMask(target, 'linear_gradient').document;
  target.output.space = 'display_p3';
  const before = structuredClone(target);
  const next = applyColorPreset(target, recipe);
  assert.deepEqual(target, before);
  assert.deepEqual(next.source, before.source);
  assert.deepEqual(next.output, before.output);
  assert.equal(next.output_node, before.output_node);
  assert.deepEqual(next.panel_bindings, before.panel_bindings);
  assert.deepEqual(next.node_positions, before.node_positions);
  assert.deepEqual(
    next.nodes.filter((node) => !['basic', 'detail'].includes(node.id)),
    before.nodes.filter((node) => !['basic', 'detail'].includes(node.id))
  );
  assert.equal(parameters(next, 'adjustments').exposure, 1.25);
});

test('disabled donor controls are neutral and applying a preset enables the target controls', () => {
  const donor = document();
  donor.nodes.find((node) => node.id === 'bound-basic')!.enabled = false;
  const recipe = colorPresetRecipe(donor);
  const target = document();
  const basic = target.nodes.find((node) => node.id === 'bound-basic')!;
  basic.enabled = false;
  const next = applyColorPreset(target, recipe);
  assert.equal(next.nodes.find((node) => node.id === 'bound-basic')!.enabled, true);
  assert.equal(parameters(next, 'adjustments').exposure, 0);
});

test('unsupported preset graphs fail before changing the target', () => {
  const target = document(),
    before = structuredClone(target);
  const recipe = colorPresetRecipe(target);
  recipe.nodes.push({ ...recipe.nodes[1]!, id: 'extra' });
  assert.throws(() => applyColorPreset(target, recipe), /其他节点/);
  assert.deepEqual(target, before);
});

test('preset transport paginates and preserves recipe data during rename', async () => {
  const requests: { url: string; init: RequestInit }[] = [];
  const fetcher: typeof fetch = async (url, init) => {
    requests.push({ url: String(url), init: init! });
    return new Response(JSON.stringify([]), { status: 200 });
  };
  const api = new EditingClient(
    new IrisClient(
      new Transport({ base_url: 'http://127.0.0.1:5412', token: 'test', version: '' }, fetcher)
    )
  );
  const recipe = colorPresetRecipe(document());
  await api.presets(100);
  await api.savePreset('人像', recipe);
  await api.renamePreset(
    { id: 'preset/a', name: '人像', recipe, created_at: '', updated_at: '' },
    '自然人像'
  );
  await api.deletePreset('preset/a');
  assert.ok(requests[0]!.url.endsWith('?limit=100&offset=100'));
  assert.deepEqual(JSON.parse(requests[2]!.init.body as string), { name: '自然人像', recipe });
  assert.ok(requests[2]!.url.endsWith('/preset%2Fa'));
  assert.equal(requests[3]!.init.method, 'DELETE');
  for (const request of requests)
    assert.equal((request.init.headers as Record<string, string>).Authorization, 'Bearer test');
});
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
function lensSuggestion(): LensProfileSuggestion {
  return {
    document_id: 'edit/1',
    expected_revision: 7,
    source_fingerprint: 'a'.repeat(64),
    resource: 'b'.repeat(64),
    placement: 'optical_coefficients_only_on_uncropped_source_extent',
    capture: { dimensions: [32, 24] },
    matching: {
      applicable: true,
      issues: [],
      correction: { radial: [0.01, 0], chromatic_aberration: [0, 0.001], vignette_stops: 0.2 },
    },
  };
}
test('lens calibration precedes user crop and preserves existing local branches', () => {
  const original = updatePanel(document(), {
    type: 'geometry',
    parameters: {
      crop: [0.1, 0.2, 0.8, 0.9],
      rotation_degrees: 12,
      perspective: [1, 0, 0, 0, 1, 0, 0, 0, 1],
      radial: [0, 0],
      chromatic_aberration: [0, 0],
      vignette_stops: 0,
    },
  });
  const result = applyLensProfile(record(), original, lensSuggestion());
  const lens = lensProfileNode(result)!;
  assert.deepEqual(lens.inputs, ['source']);
  assert.deepEqual(result.nodes.find((n) => n.id === 'bound-basic')!.inputs, [lens.id]);
  assert.deepEqual(parameters(result, 'geometry'), parameters(original, 'geometry'));
  assert.deepEqual(
    result.nodes.find((n) => n.id === 'custom-local'),
    original.nodes.find((n) => n.id === 'custom-local')
  );
  assert.deepEqual(result.source, original.source);
  assert.equal(result.output_node, original.output_node);
  assert.deepEqual(
    lens.operation.type === 'geometry' && lens.operation.parameters.crop,
    [0, 0, 1, 1]
  );
});
test('reapplying a lens profile updates its existing source calibration node', () => {
  const first = applyLensProfile(record(), document(), lensSuggestion());
  const result = applyLensProfile(record(), first, lensSuggestion());
  assert.equal(result.nodes.length, first.nodes.length);
  assert.deepEqual(result.panel_bindings, first.panel_bindings);
  assert.deepEqual(lensProfileNode(result), lensProfileNode(first));
});
test('stale or inapplicable lens suggestions cannot alter an edit', () => {
  assert.throws(
    () => applyLensProfile(record(), document(), { ...lensSuggestion(), expected_revision: 8 }),
    /已变更/
  );
  assert.throws(
    () =>
      applyLensProfile(record(), document(), {
        ...lensSuggestion(),
        source_fingerprint: 'c'.repeat(64),
      }),
    /已变更/
  );
  assert.throws(
    () =>
      applyLensProfile(record(), document(), {
        ...lensSuggestion(),
        matching: { applicable: false, issues: ['missing_metadata'] },
      }),
    /不匹配/
  );
});
test('lens resource import sends authenticated binary bytes and matching binds the saved revision', async () => {
  const requests: { url: string; init: RequestInit }[] = [];
  const fetcher: typeof fetch = async (url, init) => {
    requests.push({ url: String(url), init: init! });
    return new Response(
      JSON.stringify(
        requests.length === 1
          ? { hash: 'b'.repeat(64), kind: 'lens_profile', size_bytes: 2 }
          : lensSuggestion()
      ),
      { status: 200 }
    );
  };
  const api = new EditingClient(
    new IrisClient(
      new Transport({ base_url: 'http://127.0.0.1:5412', token: 'test', version: '' }, fetcher)
    )
  );
  const bytes = new Blob(['{}']);
  await api.importLensProfile(bytes);
  await api.lensProfile(record(), 'b'.repeat(64));
  assert.equal(requests[0]!.init.body, bytes);
  assert.equal(
    (requests[0]!.init.headers as Record<string, string>)['Content-Type'],
    'application/octet-stream'
  );
  assert.equal((requests[0]!.init.headers as Record<string, string>).Authorization, 'Bearer test');
  assert.equal(requests[0]!.init.redirect, 'error');
  assert.deepEqual(JSON.parse(requests[1]!.init.body as string), {
    expected_revision: 7,
    resource: 'b'.repeat(64),
  });
});
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

test('soft proof sends the unsaved document and can switch back without changing output or saving', async () => {
  const requests: { url: string; init: RequestInit }[] = [];
  const api = new EditingClient(
    new IrisClient(
      new Transport(
        { base_url: 'http://127.0.0.1:5412', token: 'test', version: '' },
        async (url, init) => {
          requests.push({ url: String(url), init: init! });
          return new Response(new Uint8Array([255, 216, 255, 217]), {
            headers: { 'content-type': 'image/jpeg' },
          });
        }
      )
    )
  );
  const saved = record();
  const draft = updatePanel(saved.document, {
    type: 'adjustments',
    parameters: { ...defaultAdjustments(), exposure: 1.5 },
  });
  const original = structuredClone(saved);
  const signal = new AbortController().signal;
  const hash = 'b'.repeat(64);
  await api.preview(saved, draft, 'proof-session', 1, signal, 384, hash);
  await api.preview(saved, draft, 'proof-session', 2, signal, 1600);
  assert.match(requests[0]!.url, /\/edit%2F1\/proof$/);
  assert.match(requests[1]!.url, /\/edit%2F1\/preview-draft$/);
  const proof = JSON.parse(requests[0]!.init.body as string);
  assert.deepEqual(proof, {
    resource: hash,
    document: draft,
    render: {
      revision: 7,
      session: 'proof-session',
      sequence: 1,
      quality: 'interactive',
      max_edge: 384,
    },
  });
  const normal = JSON.parse(requests[1]!.init.body as string);
  assert.equal('resource' in normal, false);
  assert.deepEqual(normal.document.output, saved.document.output);
  assert.deepEqual(saved, original);
  for (const request of requests) {
    assert.equal(new Headers(request.init.headers).get('Authorization'), 'Bearer test');
    assert.equal(request.init.signal, signal);
  }
});

test('ICC upload sends binary bytes to the authenticated resource endpoint', async () => {
  const bytes = new Blob([new Uint8Array(128)]);
  const api = new EditingClient(
    new IrisClient(
      new Transport(
        { base_url: 'http://127.0.0.1:5412', token: 'test', version: '' },
        async (url, init) => {
          assert.match(String(url), /\/edit-resources\/import\/icc$/);
          assert.equal(init?.body, bytes);
          assert.equal(new Headers(init?.headers).get('Authorization'), 'Bearer test');
          return Response.json({ kind: 'icc', hash: 'c'.repeat(64), size_bytes: bytes.size });
        }
      )
    )
  );
  assert.equal((await api.importIcc(bytes)).kind, 'icc');
});

test('export-only color settings travel with the export without saving the photo version', async () => {
  const calls: string[] = [];
  const saved = record();
  const output = { ...saved.document.output, space: 'adobe_rgb' as const };
  const api = new EditingClient(
    new IrisClient(
      new Transport(
        { base_url: 'http://127.0.0.1:5412', token: 'test', version: '' },
        async (url, init) => {
          calls.push(String(url));
          assert.match(String(url), /\/rendered-exports$/);
          const request = JSON.parse(init?.body as string).exports[0];
          assert.equal(request.expected_revision, saved.revision);
          assert.deepEqual(request.document.output, output);
          assert.deepEqual(request.document.source, saved.document.source);
          assert.equal('proof' in request, false);
          return Response.json([]);
        }
      )
    )
  );
  await api.exports([
    {
      document_id: saved.id,
      expected_revision: saved.revision,
      destination_directory: 'fixture-output',
      filename: 'image.tif',
      format: { format: 'tiff16' },
      ...{ document: { ...saved.document, output } },
    },
  ]);
  assert.equal(calls.length, 1);
  assert.equal(saved.document.output.space, 'srgb');
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
