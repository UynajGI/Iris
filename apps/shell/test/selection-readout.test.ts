import { test } from 'node:test';
import assert from 'node:assert/strict';
import { faceRegions, originalDetailAvailable, previewSource } from '../src/selection-readout.js';
import type { Photo } from '../src/types.js';

const photo = {
  id: 1,
  format: 'jpeg',
  width: 4000,
  height: 6000,
  size_bytes: 10_000_000,
  analysis_status: 'current',
  analysis: {
    width: 640,
    height: 960,
    preview_source: 'raw_embedded_jpeg',
    faces: [
      { bbox: [550, 100, 150, 200] },
      { bbox: [-20, 400, 100, 100] },
      { bbox: [700, 0, 20, 20] },
      { bbox: [0, 0, Number.NaN, 5] },
    ],
  },
} as unknown as Photo;

test('face regions use oriented preview pixels, clip bounds and keep square crops', () => {
  const regions = faceRegions(photo);
  assert.equal(regions.length, 2);
  assert.equal(regions[0]!.index, 1, 'left-to-right ordering is spatial, not identity');
  for (const region of regions) {
    assert.ok(region.x >= 0 && region.y >= 0);
    assert.ok(region.x + region.w <= 640 && region.y + region.h <= 960);
    assert.equal(region.w, region.h);
  }
  assert.deepEqual(faceRegions({ ...photo, analysis_status: 'stale' }), []);
});

test('original detail is bounded and unsupported formats explicitly retain preview', () => {
  assert.equal(originalDetailAvailable(photo), true);
  assert.equal(originalDetailAvailable({ ...photo, format: 'raw' }), false);
  assert.equal(originalDetailAvailable({ ...photo, width: 20_000 }), false);
  assert.equal(originalDetailAvailable({ ...photo, size_bytes: 100_000_000 }), false);
  assert.equal(previewSource(photo), '相机内嵌预览');
  assert.equal(previewSource({ ...photo, analysis_status: 'stale' }), '缩小预览');
});
