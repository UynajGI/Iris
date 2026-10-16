import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  cameraName,
  captureDate,
  decimal,
  ev,
  shutter,
  tagLabel,
  flashLabel,
} from '../src/metadata-format.js';
import { IrisClient } from '../src/client.js';
import { Transport } from '../src/transport.js';

test('metadata request is read-only, authenticated and forwards cancellation', async () => {
  const controller = new AbortController();
  const client = new IrisClient(
    new Transport(
      { base_url: 'http://127.0.0.1:8900', token: 'fixture-token', version: 'test' },
      (async (url, init) => {
        assert.equal(String(url), 'http://127.0.0.1:8900/api/v1/photos/7/metadata');
        assert.equal(init?.method, 'GET');
        assert.equal(init?.body, undefined);
        assert.equal(init?.signal, controller.signal);
        assert.equal(new Headers(init?.headers).get('Authorization'), 'Bearer fixture-token');
        assert.equal(init?.cache, 'no-store');
        return Response.json({ status: 'not_found' });
      }) as typeof fetch
    )
  );
  assert.equal((await client.photoMetadata(7, controller.signal)).status, 'not_found');
});

test('exposure readouts preserve long exposures, fractions, signed EV and absent fields', () => {
  assert.equal(shutter(1 / 250), '1/250 s');
  assert.equal(shutter(1 / 1.3), '0.7692 s');
  assert.equal(shutter(30), '30 s');
  assert.equal(shutter(2.5), '2.5 s');
  for (const value of [null, undefined, NaN, Infinity, 0, -1]) assert.equal(shutter(value), '—');
  assert.equal(decimal(2.8), '2.8');
  assert.equal(ev(-2 / 3), '-0.67 EV');
  assert.equal(ev(0), '0 EV');
  assert.equal(ev(1), '+1 EV');
  assert.equal(ev(null), '—');
});
test('capture date and codes do not manufacture timezone or unknown camera settings', () => {
  assert.equal(cameraName('Canon', 'Canon EOS 5D Mark III'), 'Canon EOS 5D Mark III');
  assert.equal(cameraName('Sony', 'ILCE-7M3'), 'Sony ILCE-7M3');
  assert.equal(cameraName(null, null), '—');
  assert.equal(captureDate('2026:10:16 12:34:56'), '2026-10-16 12:34:56');
  assert.equal(captureDate('2026:10:16 12:34:56', '+08:00'), '2026-10-16 12:34:56 +08:00');
  assert.equal(captureDate(null), '—');
  assert.equal(tagLabel(null, { 0: '自动' }), '—');
  assert.equal(tagLabel(0, { 0: '自动' }), '自动');
  assert.equal(tagLabel(99, {}), '未知（99）');
  assert.equal(flashLabel(0), '未闪光');
  assert.equal(flashLabel(0x19), '已闪光');
  assert.equal(flashLabel(0x20), '无闪光灯功能');
  assert.equal(flashLabel(null), '—');
});
