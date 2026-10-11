import { test } from 'node:test';
import assert from 'node:assert/strict';
import { containRect, GALLERY_MIN, GALLERY_LOUPE } from '../src/ui/gallery-motion.js';

test('gallery shared image rectangles preserve landscape and portrait aspect ratios', () => {
  const frame = { left: 10, top: 20, width: 300, height: 200 };
  assert.deepEqual(containRect(frame, 1.5), frame);
  assert.deepEqual(containRect(frame, 0.5), { left: 110, top: 20, width: 100, height: 200 });
  assert.deepEqual(containRect(frame, 3), { left: 10, top: 70, width: 300, height: 100 });
  assert.ok(GALLERY_LOUPE > GALLERY_MIN * 3);
});
