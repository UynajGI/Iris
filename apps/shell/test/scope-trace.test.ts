import { test } from 'node:test';
import assert from 'node:assert/strict';
import { sampleScopes } from '../src/ui/scopes.js';
import {
  scopeChroma,
  vectorIndex,
  vectorPosition,
  vectorSize,
  vectorTargets,
} from '../src/ui/scope-chroma.js';
import {
  densityPeak,
  depositDensity,
  paintDensity,
  vectorDensity,
  waveformDensity,
} from '../src/ui/scope-raster.js';

test('fractional coverage distributes density continuously and preserves edge samples', () => {
  const bins = new Float32Array(9);
  depositDensity(bins, 3, 3, 0.25, 0.5, 8);
  assert.deepEqual([...bins], [3, 1, 0, 3, 1, 0, 0, 0, 0]);
  depositDensity(bins, 3, 3, 2, 2, 1);
  assert.equal(bins[8], 1);
  assert.equal(
    bins.reduce((a, b) => a + b, 0),
    9
  );
  const pixels = new Uint8ClampedArray(12);
  paintDensity(pixels, new Float32Array([0.0001, 0.25, 1]), 'l', 1);
  assert.ok(
    pixels[0]! < pixels[4]! && pixels[4]! < pixels[8]!,
    'no brightness floor exposing faint coverage as boxes'
  );
});

test('neutral values stay centered; BT.709 primaries have the expected vectorscope orientation', () => {
  for (const value of [0, 0.18, 0.5, 1]) {
    const { cb, cr } = scopeChroma(value, value, value);
    assert.ok(Math.abs(cb) < 1e-15 && Math.abs(cr) < 1e-15);
    assert.equal(vectorIndex(value, value, value), 128 * vectorSize + 128);
  }
  const red = scopeChroma(1, 0, 0);
  const blue = scopeChroma(0, 0, 1);
  assert.ok(Math.abs(red.cb + 0.114572106057) < 1e-10);
  assert.equal(red.cr, 0.5);
  assert.equal(blue.cb, 0.5);
  assert.ok(blue.cr < 0);
  for (const { rgb } of vectorTargets) {
    const full = scopeChroma(rgb[0], rgb[1], rgb[2]);
    const bars = scopeChroma(rgb[0] * 0.75, rgb[1] * 0.75, rgb[2] * 0.75);
    assert.ok(Math.abs(bars.cb - full.cb * 0.75) < 1e-15);
    assert.ok(Math.abs(bars.cr - full.cr * 0.75) < 1e-15);
    const { x, y } = vectorPosition(full.cb, full.cr);
    assert.ok(x > 0 && x < 256 && y > 0 && y < 256);
  }
});

test('scope rasters conserve visible samples and keep endpoints and horizontal position', () => {
  const data = sampleScopes(
    new Uint8ClampedArray([0, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 255, 0, 255, 0, 0]),
    2,
    2
  );
  const raster = waveformDensity(data, 'r', 2, 101);
  assert.equal(raster[200], 1, 'black at the bottom left');
  assert.equal(raster[1], 1, 'white at the top right');
  assert.equal(raster[0], 1, 'red stays at the top left');
  const vectors = vectorDensity(data, 181, 181);
  assert.equal(
    vectors.reduce((a, b) => a + b, 0),
    3
  );
  assert.equal(vectors[90 * 181 + 90], 2, 'black and white coincide at neutral');
  assert.equal(
    data.vectorscope.reduce((a, b) => a + b, 0),
    3
  );
  for (const width of [1, 5, 180]) {
    assert.equal(
      waveformDensity(data, 'r', width, 80).reduce((a, b) => a + b, 0),
      3
    );
  }
});

test('density display preserves blank bins and uses a shared gain across parade channels', () => {
  const a = new Uint32Array([0, 1, 16]);
  const b = new Uint32Array([0, 1, 1]);
  const peak = densityPeak([a, b]);
  const red = new Uint8ClampedArray(12),
    green = new Uint8ClampedArray(12);
  paintDensity(red, a, 'r', peak);
  paintDensity(green, b, 'g', peak);
  assert.equal(peak, 16);
  assert.deepEqual([...red.slice(0, 4)], [0, 0, 0, 0]);
  assert.ok(red[8]! > red[4]!);
  assert.ok(Math.abs(red[4]! / 230 - green[5]! / 211) < 0.01);
  const blank = sampleScopes(new Uint8ClampedArray(4), 1, 1);
  assert.equal(densityPeak([blank.vectorscope]), 1);
  assert.equal(blank.clipHigh, 0);
});
