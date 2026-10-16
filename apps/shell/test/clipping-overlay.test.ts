import assert from 'node:assert/strict';
import test from 'node:test';
import { clippingPixels } from '../src/ui/scope-pixels.js';
import { sampleScopes } from '../src/ui/scopes.js';
import { wheelPoint } from '../src/editing-color-wheel.js';

test('clipping overlays use the same boundary samples as the percentage readout', () => {
  const pixels = new Uint8ClampedArray([
    2, 2, 2, 255, 3, 2, 2, 255, 253, 20, 20, 255, 252, 252, 252, 255,
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

test('wheel disk maps angle and radial distance, retaining hue at neutral and clamping outside', () => {
  assert.deepEqual(wheelPoint(0.5, 0.5, 217), { hue: 217, saturation: 0 });
  assert.equal(wheelPoint(0.5, 0.73, 0).hue, 90);
  assert.ok(Math.abs(wheelPoint(0.5, 0.73, 0).saturation - 1) < 1e-12);
  assert.deepEqual(wheelPoint(2, 0.5, 0), { hue: 0, saturation: 2 });
});
