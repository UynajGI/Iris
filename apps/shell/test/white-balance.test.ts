import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import {
  EditingClient,
  type EditDocument,
  type EditRecord,
  type PreparedRawWhiteBalance,
} from '../src/editing-client.js';
import { IrisClient } from '../src/client.js';
import { Transport } from '../src/transport.js';
import { applyWhiteBalance, neutralPatch, whiteBalanceKey } from '../src/editing-white-balance.js';
import { sourceDocument } from '../src/editing-document.js';
import type { RawWhiteBalancePanel } from '../src/ui/color/RawWhiteBalancePanel.js';
import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';

function fixture() {
  const document: EditDocument = {
    schema_version: 1,
    source: {
      photo_id: '42',
      fingerprint: 'a'.repeat(64),
      width: 6000,
      height: 4000,
      reference_kind: 'scene',
      raw_white_balance: [2, 1, 1.5],
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
    ],
    output_node: 'source',
    panel_bindings: [],
    node_positions: {},
    output: {
      space: 'display_p3',
      tone_map: { kind: 'none' },
      gamut_compression: false,
      background: [0, 0, 0],
    },
  };
  const record = { id: 'edit/42', photo_id: 42, revision: 5, document } as EditRecord;
  const prepared: PreparedRawWhiteBalance = {
    gains_rgb: [1.8, 1, 1.9],
    revision: 5,
    source_fingerprint: document.source.fingerprint,
    sampled_pixels: null,
    temperature: { kelvin: 5500, tint_delta_v: 0, locus: 'planckian' },
    calibration: {
      version: 1,
      xyz_to_camera_d65: [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
      ],
      camera_to_working: [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
      ],
      applied_gains: [2, 1, 1.5],
      as_shot_gains: [2, 1, 1.5],
    },
  };
  return { document, record, prepared };
}

test('sensor white balance preserves graph, output and identity and rejects stale or invalid results', () => {
  const { document, record, prepared } = fixture();
  const original = structuredClone(document);
  const next = applyWhiteBalance(record, document, prepared);
  assert.deepEqual(next.source.raw_white_balance, [1.8, 1, 1.9]);
  assert.equal(next.nodes, document.nodes);
  assert.equal(next.output, document.output);
  assert.equal(next.source.fingerprint, document.source.fingerprint);
  assert.deepEqual(document, original);
  assert.notEqual(whiteBalanceKey(record, document), whiteBalanceKey(record, next));
  assert.equal(applyWhiteBalance(record, next, prepared, true).source.raw_white_balance, null);
  assert.throws(() => applyWhiteBalance(record, next, { ...prepared, revision: 4 }), /版本已变化/);
  assert.throws(
    () => applyWhiteBalance(record, next, { ...prepared, source_fingerprint: 'b'.repeat(64) }),
    /版本已变化/
  );
  for (const gains of [
    [NaN, 1, 1],
    [1, 2, 1],
    [101, 1, 1],
    [1, 1],
  ])
    assert.throws(
      () => applyWhiteBalance(record, next, { ...prepared, gains_rgb: gains }),
      /参数不可用/
    );
});

test('before comparison restores camera as-shot white balance without changing the draft', () => {
  const { document } = fixture();
  const before = sourceDocument(document);
  assert.equal(before.source.raw_white_balance, null);
  assert.deepEqual(document.source.raw_white_balance, [2, 1, 1.5]);
  assert.equal(before.source.fingerprint, document.source.fingerprint);
  assert.equal(before.output, document.output);
});

test('neutral picking maps uncropped source coordinates and bounds the full-resolution patch', () => {
  const { document } = fixture();
  assert.deepEqual(neutralPatch([0.5, 0.5], document.source), {
    x: 2996,
    y: 1996,
    width: 8,
    height: 8,
  });
  assert.deepEqual(neutralPatch([1, 1], document.source), {
    x: 5992,
    y: 3992,
    width: 8,
    height: 8,
  });
  assert.deepEqual(neutralPatch([-2, -2], document.source), { x: 0, y: 0, width: 8, height: 8 });
  assert.deepEqual(neutralPatch([1, 1], { ...document.source, width: 3, height: 2 }), {
    x: 0,
    y: 0,
    width: 3,
    height: 2,
  });
  assert.throws(() => neutralPatch([NaN, 0], document.source));
  assert.throws(() => neutralPatch([0, 0], { ...document.source, width: 0 }));
});

test('white balance preparation uses the authenticated revision-bound endpoint without saving', async () => {
  const { record, prepared } = fixture();
  const calls: { url: string; init: RequestInit }[] = [];
  const api = new EditingClient(
    new IrisClient(
      new Transport(
        { base_url: 'http://127.0.0.1:9', token: 'fixture', version: 'test' },
        async (url, init) => {
          calls.push({ url: String(url), init: init! });
          return Response.json(prepared);
        }
      )
    )
  );
  const signal = new AbortController().signal;
  await api.rawWhiteBalance(
    record,
    { kind: 'temperature', kelvin: 5500, tint_delta_v: 0.001, locus: 'planckian' },
    signal
  );
  assert.equal(calls.length, 1);
  assert.match(calls[0]!.url, /\/edit%2F42\/raw-white-balance$/);
  assert.equal(calls[0]!.init.method, 'POST');
  assert.equal(new Headers(calls[0]!.init.headers).get('authorization'), 'Bearer fixture');
  assert.equal(calls[0]!.init.signal, signal);
  assert.deepEqual(JSON.parse(calls[0]!.init.body as string), {
    expected_revision: 5,
    request: { kind: 'temperature', kelvin: 5500, tint_delta_v: 0.001, locus: 'planckian' },
  });
});

test('RAW controls use Kelvin and never display a fabricated temperature when conversion is unavailable', async () => {
  const bundle = await build({
    entryPoints: [
      fileURLToPath(new URL('../src/ui/color/RawWhiteBalancePanel.tsx', import.meta.url)),
    ],
    bundle: true,
    write: false,
    platform: 'node',
    format: 'cjs',
    packages: 'external',
    loader: { '.svg': 'text' },
  });
  const module = { exports: {} as { RawWhiteBalancePanel: typeof RawWhiteBalancePanel } };
  runInNewContext(bundle.outputFiles[0]!.text, {
    module,
    exports: module.exports,
    require: createRequire(import.meta.url),
  });
  const Component = module.exports.RawWhiteBalancePanel;
  const model = {
    reading: null,
    loading: false,
    pending: false,
    error: '',
    request: () => {},
    retry: () => {},
  };
  const props = { model, disabled: false, asShot: true, picking: false, onPick: () => {} };
  const unavailable = renderToStaticMarkup(createElement(Component, props));
  assert.doesNotMatch(unavailable, /aria-label="色温数值"/);
  assert.match(unavailable, /无法换算为色温/);
  const known = renderToStaticMarkup(
    createElement(Component, {
      ...props,
      model: { ...model, reading: { kelvin: 4789, tint_delta_v: 0.001, locus: 'planckian' } },
    })
  );
  assert.match(known, /value="4789"/);
  assert.match(known, /白平衡吸管/);
});
