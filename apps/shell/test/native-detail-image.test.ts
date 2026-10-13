import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nativeDetailBlob } from '../src/native-detail-image.js';

test('native detail uses a bounded local JPEG blob without relaxing desktop CSP', async () => {
  const bytes = Buffer.from([255, 216, 255, 217]);
  const blob = nativeDetailBlob(`data:image/jpeg;base64,${bytes.toString('base64')}`);
  assert.equal(blob.type, 'image/jpeg');
  assert.deepEqual(Buffer.from(await blob.arrayBuffer()), bytes);
});

test('native detail rejects remote images, other formats, malformed and oversized evidence', () => {
  for (const value of [
    'https://example.com/image.jpg',
    'data:image/png;base64,/9j/2Q==',
    'data:image/jpeg;base64,not-base64!',
    'data:image/jpeg;base64,YWJj',
    'data:image/jpeg;base64,' + 'A'.repeat(16 * 1024 * 1024),
  ])
    assert.throws(() => nativeDetailBlob(value));
});
