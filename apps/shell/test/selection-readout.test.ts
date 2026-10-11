import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  faceRegions,
  originalDetailAvailable,
  originalDetailPhotoId,
  previewSource,
  scaleFaceRegion,
} from '../src/selection-readout.js';
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

test('original crops use decoded oriented dimensions, not preview dimensions', () => {
  const region = faceRegions(photo)[0]!;
  const original = scaleFaceRegion(region, 4000, 6000);
  assert.equal(original.width, 4000);
  assert.equal(original.height, 6000);
  assert.equal(original.x / 4000, region.x / 640);
  assert.equal(original.y / 6000, region.y / 960);
  assert.equal(original.w / 4000, region.w / 640);
  assert.ok(original.x + original.w <= 4000);
  assert.ok(original.y + original.h <= 6000);
});

test('a comparison requests an original only for its current focused photo', () => {
  const photos = [1, 2, 3].map((id) => ({ ...photo, id }));
  const requests = (focus: number) => photos.map((p) => originalDetailPhotoId(p, p.id === focus));
  assert.deepEqual(requests(1), [1, null, null]);
  assert.deepEqual(requests(2), [null, 2, null]);
  assert.deepEqual(requests(0), [null, null, null]);
  assert.equal(originalDetailPhotoId({ ...photo, format: 'dng' }, true), null);
});
