import { test } from 'node:test';
import assert from 'node:assert/strict';
import { histogramPeak, sampleScopes } from '../src/ui/scopes.js';

test('scope channels preserve totals, clipping and horizontal waveform position', () => {
  const data = sampleScopes(
    new Uint8ClampedArray([0, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 255, 0, 255, 0, 255]),
    2,
    2
  );
  for (const bins of Object.values(data.histogram))
    assert.equal(
      bins.reduce((a, b) => a + b, 0),
      4
    );
  assert.equal(data.clipLow, 0.25);
  assert.equal(data.clipHigh, 0.25, 'only near-white pixels count as highlight clipping');
  assert.equal(data.channelClipping.r.high, 0.5);
  assert.equal(data.channelClipping.g.high, 0.5);
  assert.equal(data.channelClipping.b.high, 0.25);
  assert.equal(data.histogram.l[54], 1);
  assert.equal(data.histogram.l[182], 1);
  assert.equal(data.waveform.l[54], 1, 'red pixel stays in the left image column');
  assert.equal(data.waveform.l[256 + 182], 1, 'green pixel stays in the right column');
  assert.equal(data.waveform.l[256 + 255], 1);
  assert.equal(histogramPeak(data.histogram, []), 1);
});

test('Y-only histogram scaling includes luminance bins', () => {
  const bins = {
    l: new Uint32Array(256).fill(12),
    r: new Uint32Array(256).fill(3),
    g: new Uint32Array(256),
    b: new Uint32Array(256),
  };
  assert.equal(histogramPeak(bins, ['l']), 12);
  assert.equal(histogramPeak(bins, ['r']), 3);
});

test('scope controls display YRGB with all channels selected by default', async () => {
  const { createElement } = await import('react');
  const { renderToStaticMarkup } = await import('react-dom/server');
  const { Histogram } = await import('../src/ui/Histogram.js');
  const { IrisClient } = await import('../src/client.js');
  const { Transport } = await import('../src/transport.js');
  const client = new IrisClient(
    new Transport({ base_url: 'http://127.0.0.1:1', token: 'test', version: 'test' })
  );
  const markup = renderToStaticMarkup(createElement(Histogram, { client, photo: undefined }));
  const buttons = [
    ...markup.matchAll(/<button\b[^>]*class="scope-channel[^>]*>([YRGB])<\/button>/g),
  ];
  assert.deepEqual(
    buttons.map((match) => match[1]),
    ['Y', 'R', 'G', 'B']
  );
  for (const [button] of buttons) assert.match(button, /aria-pressed="true"/);
  assert.match(buttons[0]![0], /aria-label="Y 亮度通道"/);
});
