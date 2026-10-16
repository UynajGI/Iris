import { test } from 'node:test';
import assert from 'node:assert/strict';
import { needsWorkingMask, maskGridKey, decodeMaskPreview } from '../src/editing-mask-preview.js';
import { EditingClient, type EditDocument, type EditRecord } from '../src/editing-client.js';
import { defaultGeometry } from '../src/editing-document.js';
import { IrisClient } from '../src/client.js';
import { Transport } from '../src/transport.js';

function headers() {
  return new Headers({
    'content-type': 'image/png',
    'x-iris-revision': '3',
    'x-iris-sequence': '5',
    'x-iris-mask-domain': 'linear_rec2020_d65',
    'x-iris-approximate': 'true',
  });
}

test('working coverage is required anywhere inside a spatial combination', () => {
  const spatial = { kind: 'linear_gradient', start: [0, 0], end: [1, 0] } as const;
  const range = { kind: 'luminance_range', low: 0, high: 0.3, feather: 0 } as const;
  // Mutable tuples match the generated API model.
  const gradient = { ...spatial, start: [0, 0], end: [1, 0] };
  assert.equal(needsWorkingMask(gradient), false);
  assert.equal(needsWorkingMask({ kind: 'invert', mask: gradient }), false);
  assert.equal(
    needsWorkingMask({
      kind: 'combine',
      operation: 'subtract',
      left: gradient,
      right: { kind: 'opacity', opacity: 0.5, mask: { kind: 'invert', mask: range } },
    }),
    true
  );
  assert.equal(
    needsWorkingMask({ kind: 'color_range', color: [0, 0, 0], radius: 0.2, feather: 0.1 }),
    true
  );
});

test('grid identity rejects same-sized but different crops, sources and topology', () => {
  const document = {
    source: { photo_id: '1' },
    output_node: 'geometry',
    nodes: [
      {
        id: 'geometry',
        name: 'Geometry',
        enabled: true,
        version: 1,
        inputs: ['source'],
        operation: {
          type: 'geometry',
          parameters: { ...defaultGeometry(), crop: [0, 0, 0.5, 0.5] },
        },
      },
    ],
  } as unknown as EditDocument;
  const key = maskGridKey(document);
  for (const change of [
    (d: EditDocument) => {
      d.source.photo_id = '2';
    },
    (d: EditDocument) => {
      d.output_node = 'other';
    },
    (d: EditDocument) => {
      d.nodes[0]!.inputs = ['different-base'];
    },
    (d: EditDocument) => {
      d.nodes[0]!.operation = {
        type: 'geometry',
        parameters: { ...defaultGeometry(), crop: [0.5, 0.5, 0.5, 0.5] },
      };
    },
  ]) {
    const changed = structuredClone(document);
    change(changed);
    assert.notEqual(maskGridKey(changed), key);
  }
});

test('mask response rejects stale revisions, sequences, display-domain masks and JPEG', () => {
  const blob = new Blob(['fixture']);
  assert.equal(decodeMaskPreview({ blob, headers: headers() }, 3, 5), blob);
  for (const [field, value] of [
    ['x-iris-revision', '4'],
    ['x-iris-sequence', '6'],
    ['x-iris-mask-domain', 'srgb'],
    ['x-iris-approximate', 'false'],
    ['content-type', 'image/jpeg'],
  ]) {
    const invalid = headers();
    invalid.set(field!, value!);
    assert.throws(() => decodeMaskPreview({ blob, headers: invalid }, 3, 5));
  }
  const missing = headers();
  missing.delete('x-iris-mask-domain');
  assert.throws(() => decodeMaskPreview({ blob, headers: missing }, 3, 5));
});

test('coverage request uses the draft and isolated render session without a save', async () => {
  const calls: { url: string; init: RequestInit }[] = [];
  const api = new EditingClient(
    new IrisClient(
      new Transport(
        { base_url: 'http://127.0.0.1:9', token: 'fixture', version: 'test' },
        async (url, init) => {
          calls.push({ url: String(url), init: init! });
          return new Response(new Blob(['coverage']), { headers: headers() });
        }
      )
    )
  );
  const record = { id: 'edit/1', revision: 3 } as EditRecord;
  const document = { nodes: [], source: { photo_id: '1' } } as unknown as EditDocument;
  const signal = new AbortController().signal;
  const blob = await api.maskPreview(record, document, 'mask-2', 'overlay-session', 5, signal);
  assert.equal(calls.length, 1);
  assert.match(calls[0]!.url, /edit%2F1\/mask-preview$/);
  assert.equal(new Headers(calls[0]!.init.headers).get('authorization'), 'Bearer fixture');
  assert.equal(calls[0]!.init.signal, signal);
  assert.deepEqual(JSON.parse(calls[0]!.init.body as string), {
    render: {
      revision: 3,
      session: 'overlay-session',
      sequence: 5,
      quality: 'interactive',
      max_edge: 512,
    },
    document,
    mask_id: 'mask-2',
  });
  assert.equal(await blob.text(), 'coverage');
});
