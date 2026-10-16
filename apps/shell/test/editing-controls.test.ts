import { test } from 'node:test';
import assert from 'node:assert/strict';
import { gainsToWheel, wheelToGains } from '../src/editing-color-wheel.js';
import { spatialMaskCoverage } from '../src/editing-mask-overlay.js';
import { dragCrop } from '../src/editing-crop.js';
import { fitPreview } from '../src/editing-canvas.js';

test('interactive proxy and refined image occupy the same canvas rectangle', () => {
  const viewport = { width: 560, height: 640 };
  assert.deepEqual(fitPreview({ width: 320, height: 480 }, viewport), { width: 400, height: 600 });
  assert.deepEqual(fitPreview({ width: 640, height: 960 }, viewport), { width: 400, height: 600 });
});

test('color wheels preserve saved channel gains including neutral luminance and negative colors', () => {
  for (const values of [
    [0, 0, 0],
    [1, 1, 1],
    [1, -0.5, -0.5],
    [-2, 1, 2],
    [0.37, -0.18, 1.1],
  ]) {
    const restored = wheelToGains(gainsToWheel(values));
    values.forEach((value, i) => assert.ok(Math.abs(value - restored[i]!) < 1e-9));
  }
  assert.equal(gainsToWheel([1, -0.5, -0.5]).hue, 0);
  assert.equal(gainsToWheel([-0.5, 1, -0.5]).hue, 120);
  assert.equal(gainsToWheel([-0.5, -0.5, 1]).hue, 240);
});
test('linear overlay covers the same physical gradient on a non-square photograph', () => {
  const sample = spatialMaskCoverage(
    { kind: 'linear_gradient', start: [0.25, 0.25], end: [0.75, 0.75] },
    200,
    100
  )!;
  assert.equal(sample(0.25, 0.25), 0);
  assert.equal(sample(0.75, 0.75), 1);
  assert.ok(Math.abs(sample(0.5, 0.5) - 0.5) < 1e-9);
  assert.ok(Math.abs(sample(0.75, 0.25) - 0.8) < 1e-9);
});
test('overlay preserves radial feather, brush flow, subtraction and inversion', () => {
  const radial = {
    kind: 'radial_gradient',
    center: [0.5, 0.5],
    radii: [0.25, 0.25],
    hardness: 0.5,
  } as const;
  const r = spatialMaskCoverage(
    { ...radial, center: [...radial.center], radii: [...radial.radii] },
    100,
    100
  )!;
  assert.equal(r(0.5, 0.5), 1);
  assert.equal(r(0.75, 0.5), 0);
  assert.equal(r(0.6875, 0.5), 0.5);
  const brush = {
    kind: 'brush' as const,
    strokes: [
      {
        points: [
          [0.25, 0.5],
          [0.75, 0.5],
        ],
        radius: 0.1,
        hardness: 1,
        flow: 0.5,
      },
    ],
  };
  const subtract = spatialMaskCoverage(
    {
      kind: 'combine',
      operation: 'subtract',
      left: { kind: 'invert', mask: { kind: 'brush', strokes: [] } },
      right: brush,
    },
    100,
    100
  )!;
  assert.equal(subtract(0.5, 0.5), 0.5);
  assert.equal(subtract(0.5, 0.9), 1);
  assert.equal(
    spatialMaskCoverage({ kind: 'luminance_range', low: 0.2, high: 0.8, feather: 0.1 }, 100, 100),
    null
  );
});
test('canvas crop edges cannot cross or move outside the source', () => {
  assert.deepEqual(dragCrop([0.1, 0.2, 0.5, 0.5], 'move', 2, -2), [0.5, 0, 0.5, 0.5]);
  const resized = dragCrop([0, 0, 1, 1], 'nw', 0.1, 0.2);
  assert.deepEqual(resized, [0.1, 0.2, 0.9, 0.8]);
  const crossed = dragCrop([0, 0, 1, 1], 'se', -2, -2);
  assert.deepEqual(crossed, [0, 0, 0.01, 0.01]);
});
