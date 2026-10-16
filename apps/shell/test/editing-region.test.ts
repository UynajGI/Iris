import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  nativeViewport,
  clampNativeCenter,
  decodeNativeRegion,
  nativeImageOrigin,
} from '../src/editing-region.js';
import { EditingClient, type EditDocument, type EditRecord } from '../src/editing-client.js';
import { IrisClient } from '../src/client.js';
import { Transport } from '../src/transport.js';

function responseHeaders() {
  return new Headers({
    'x-iris-revision': '3',
    'x-iris-sequence': '5',
    'x-iris-approximate': 'false',
    'x-iris-region-x': '200',
    'x-iris-region-y': '150',
    'x-iris-region-width': '800',
    'x-iris-region-height': '600',
    'x-iris-full-width': '3000',
    'x-iris-full-height': '2000',
  });
}

test('native placement follows authoritative pixel origin across floating-point rounding ties', () => {
  const region = decodeNativeRegion({ headers: responseHeaders(), blob: new Blob() }, 3, 5);
  for (const ratio of [1, 1.25, 2]) {
    const origin = nativeImageOrigin(
      { width: 3000, height: 2000 },
      { width: 800, height: 600 },
      [0.2001, 0.2251],
      ratio,
      region
    );
    assert.equal(origin.left + region.x / ratio, 0);
    assert.equal(origin.top + region.y / ratio, 0);
  }
});

test('native viewport uses physical pixels and caps memory instead of resizing requested pixels', () => {
  assert.deepEqual(nativeViewport({ width: 500, height: 300 }, 2, [0.5, 0.5]), {
    width: 1000,
    height: 600,
    center: [0.5, 0.5],
  });
  for (const ratio of [1, 1.25, 1.5, 2, 3]) {
    const view = nativeViewport({ width: 3000, height: 2000 }, ratio, [0, 1]);
    assert.ok(view.width <= 2048 && view.height <= 2048 && view.width * view.height <= 4_000_000);
    assert.ok(view.width / ratio <= 3000);
  }
});

test('native panning clamps at final output edges and centers images smaller than viewport', () => {
  assert.deepEqual(
    clampNativeCenter([-2, 3], { width: 3000, height: 2000 }, { width: 600, height: 400 }),
    [0.1, 0.9]
  );
  assert.deepEqual(
    clampNativeCenter([0.1, 0.9], { width: 100, height: 80 }, { width: 600, height: 400 }),
    [0.5, 0.5]
  );
});

test('native response rejects missing metadata, stale results, proxy output and invalid bounds', () => {
  const blob = new Blob(['fixture']);
  const headers = responseHeaders();
  const valid = decodeNativeRegion({ blob, headers }, 3, 5);
  assert.equal(valid.fullWidth, 3000);
  assert.equal(valid.width, 800);
  for (const [key, value] of [
    ['x-iris-approximate', 'true'],
    ['x-iris-revision', '4'],
    ['x-iris-sequence', '6'],
    ['x-iris-region-x', '-1'],
    ['x-iris-region-width', '0'],
    ['x-iris-region-width', '2049'],
    ['x-iris-full-width', '500'],
    ['x-iris-region-y', 'NaN'],
  ]) {
    const invalid = new Headers(headers);
    invalid.set(key!, value!);
    assert.throws(() => decodeNativeRegion({ blob, headers: invalid }, 3, 5));
  }
  headers.delete('x-iris-region-x');
  assert.throws(() => decodeNativeRegion({ blob, headers }, 3, 5));
});

test('native request carries unsaved document, revision and abort signal without saving', async () => {
  const calls: { url: string; init: RequestInit }[] = [];
  const api = new EditingClient(
    new IrisClient(
      new Transport(
        { base_url: 'http://127.0.0.1:9', token: 'fixture', version: 'test' },
        async (url, init) => {
          calls.push({ url: String(url), init: init! });
          return new Response(new Blob(['pixels']), { headers: responseHeaders() });
        }
      )
    )
  );
  const record = { id: 'edit/1', revision: 3 } as EditRecord;
  const document = { source: { photo_id: '1' }, nodes: [] } as unknown as EditDocument;
  const view = { center: [0.5, 0.5], width: 800, height: 600 };
  const signal = new AbortController().signal;
  const region = await api.nativeRegion(record, document, view, 'region-session', 5, signal);
  assert.equal(calls.length, 1);
  assert.match(calls[0]!.url, /edit%2F1\/region$/);
  assert.equal(new Headers(calls[0]!.init.headers).get('authorization'), 'Bearer fixture');
  assert.equal(calls[0]!.init.signal, signal);
  assert.deepEqual(JSON.parse(calls[0]!.init.body as string), {
    render: {
      revision: 3,
      sequence: 5,
      session: 'region-session',
      quality: 'full',
      max_edge: 1600,
    },
    document,
    view,
  });
  assert.equal(await region.blob.text(), 'pixels');
});
