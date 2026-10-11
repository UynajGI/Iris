import { test } from 'node:test';
import assert from 'node:assert/strict';
import { histogramPeak, sampleScopes } from '../src/ui/scopes.js';

test('scope channels preserve totals, clipping and horizontal waveform position', () => {
  const data = sampleScopes(new Uint8ClampedArray([0,0,0,255, 255,255,255,255, 255,0,0,255, 0,255,0,255]), 2, 2);
  for (const bins of Object.values(data.histogram)) assert.equal(bins.reduce((a, b) => a + b, 0), 4);
  assert.equal(data.clipLow, 0.25);
  assert.equal(data.clipHigh, 0.75, 'any clipped RGB channel counts as a highlight');
  assert.equal(data.histogram.l[54], 1);
  assert.equal(data.histogram.l[182], 1);
  assert.equal(data.waveform.l[54], 1, 'red pixel stays in the left image column');
  assert.equal(data.waveform.l[256 + 182], 1, 'green pixel stays in the right column');
  assert.equal(data.waveform.l[256 + 255], 1);
  assert.equal(histogramPeak(data.histogram, []), 1);
});

test('white-only histogram scaling includes luminance bins', () => {
  const bins = { l: new Uint32Array(256).fill(12), r: new Uint32Array(256).fill(3), g: new Uint32Array(256), b: new Uint32Array(256) };
  assert.equal(histogramPeak(bins, ['l']), 12);
  assert.equal(histogramPeak(bins, ['r']), 3);
});
