import assert from 'node:assert/strict';
import test from 'node:test';
import { clippingPixels } from '../src/ui/scope-pixels.js';
import { sampleScopes } from '../src/ui/scopes.js';
import { wheelPoint } from '../src/editing-color-wheel.js';

test('clipping overlays use the same boundary samples as the percentage readout', () => {
  const pixels = new Uint8ClampedArray([
    2, 2, 2, 255, 3, 2, 2, 255, 253, 253, 253, 255, 252, 252, 252, 255,
  ]);
  const mask = clippingPixels(pixels, { shadows: true, highlights: true });
  assert.deepEqual([...mask], [35, 100, 255, 230, 0, 0, 0, 0, 255, 40, 40, 230, 0, 0, 0, 0]);
  const data = sampleScopes(pixels, 4, 1);
  assert.equal(data.clipLow, 0.25);
  assert.equal(data.clipHigh, 0.25);
  assert.equal(clippingPixels(pixels, { shadows: false, highlights: false }).some(Boolean), false);
  assert.equal(
    clippingPixels(new Uint8ClampedArray([255, 255, 255, 0]), {
      shadows: true,
      highlights: true,
    }).some(Boolean),
    false
  );
});

test('warm skin and saturated colors are channel warnings, not white clipping', () => {
  const pixels = new Uint8ClampedArray([
    255, 180, 145, 255, 255, 0, 0, 255, 0, 255, 0, 255, 255, 255, 0, 255, 255, 254, 253, 255,
  ]);
  const overlay = clippingPixels(pixels, { shadows: true, highlights: true });
  assert.equal(overlay.slice(0, 16).some(Boolean), false);
  assert.deepEqual([...overlay.slice(16)], [255, 40, 40, 230]);
  const scopes = sampleScopes(pixels, 5, 1);
  assert.equal(scopes.clipHigh, 1 / 5);
  assert.equal(scopes.clipLow, 0);
  assert.equal(scopes.channelClipping.r.high, 4 / 5);
});

test('transparent pixels do not contribute to overlay percentages or channel counts', () => {
  const data = sampleScopes(
    new Uint8ClampedArray([0, 0, 0, 0, 255, 255, 255, 0, 255, 254, 253, 255]),
    3,
    1
  );
  assert.equal(data.clipLow, 0);
  assert.equal(data.clipHigh, 1);
  assert.equal(data.channelClipping.r.high, 1);
  const empty = sampleScopes(new Uint8ClampedArray(4), 1, 1);
  assert.equal(empty.clipHigh, 0);
  assert.equal(empty.clipLow, 0);
});

test('wheel disk maps angle and radial distance, retaining hue at neutral and clamping outside', () => {
  assert.deepEqual(wheelPoint(0.5, 0.5, 217), { hue: 217, saturation: 0 });
  assert.equal(wheelPoint(0.5, 0.73, 0).hue, 270);
  assert.ok(Math.abs(wheelPoint(0.5, 0.73, 0).saturation - 1) < 1e-12);
  assert.deepEqual(wheelPoint(2, 0.5, 0), { hue: 0, saturation: 2 });
});
