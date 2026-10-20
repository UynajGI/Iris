import test from 'node:test';
import assert from 'node:assert/strict';
import { histogramOutline, histogramPath } from '../src/ui/histogram-path.js';

test('display smoothing suppresses bin chatter without changing measurements or clipped endpoints', () => {
  const bins = Uint32Array.from({ length: 256 }, (_, i) => (i % 2 ? 40 : 60));
  bins[0] = 100;
  bins[255] = 90;
  const original = bins.slice();
  const outline = histogramOutline(bins, 100);
  assert.deepEqual(bins, original);
  assert.equal(outline[0], 0);
  assert.equal(outline[255], 10);
  for (const height of outline.slice(5, 251)) assert.equal(height, 50);
  assert.ok(outline.every((height) => Number.isFinite(height) && height >= 0 && height <= 100));
  const path = histogramPath(bins, 100);
  assert.match(path, /^M0 100 L0 0 Q/);
  assert.match(path, /255 10 L255 100Z$/);
});

test('narrow peaks remain visible and empty histograms stay on the baseline', () => {
  const bins = new Uint32Array(256);
  assert.ok(histogramOutline(bins, 0).every((height) => height === 100));
  bins[128] = 100;
  const outline = histogramOutline(bins, 100);
  assert.equal(outline.indexOf(Math.min(...outline)), 128);
  assert.ok(outline[128]! < 75);
  assert.equal(outline[127], outline[129]);
  assert.equal(outline[123], 100);
  assert.equal(outline[133], 100);
});

test('suppresses several-bin ripples while retaining broad tonal distribution', () => {
  const bins = Uint32Array.from({ length: 256 }, (_, i) => 100 + 20 * Math.cos((i * Math.PI) / 2));
  const outline = histogramOutline(bins, 200).slice(5, 251);
  assert.ok(Math.max(...outline) - Math.min(...outline) < 2);
  const broad = Uint32Array.from({ length: 256 }, (_, i) =>
    Math.round(1000 * Math.exp(-(((i - 128) / 20) ** 2)))
  );
  const smooth = histogramOutline(broad, 1000);
  assert.equal(smooth.indexOf(Math.min(...smooth)), 128);
  assert.ok(smooth[128]! < 1);
});
