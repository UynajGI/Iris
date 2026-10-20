import { isShadowClipped, isHighlightClipped } from './scope-pixels.js';
import { vectorIndex, vectorSize } from './scope-chroma.js';
export const channels = ['l', 'r', 'g', 'b'] as const;
export type Channel = (typeof channels)[number];
export const scopeNames = {
  histogram: '直方图',
  waveform: '波形图',
  parade: '分量图',
  vectorscope: '矢量图',
};
export type ScopeMode = keyof typeof scopeNames;
export type ChannelBins = Record<Channel, Uint32Array>;
export type ScopeData = {
  histogram: ChannelBins;
  waveform: ChannelBins;
  vectorscope: Uint32Array;
  width: number;
  clipLow: number;
  clipHigh: number;
  channelClipping: Record<'r' | 'g' | 'b', { low: number; high: number }>;
};
const allocate = (size: number): ChannelBins => ({
  l: new Uint32Array(size),
  r: new Uint32Array(size),
  g: new Uint32Array(size),
  b: new Uint32Array(size),
});

/** Scopes use oriented, downsampled preview pixels, not RAW data. */
export function sampleScopes(pixels: Uint8ClampedArray, width: number, height: number): ScopeData {
  if (width < 1 || height < 1 || pixels.length !== width * height * 4)
    throw new Error('Invalid scope image');
  const histogram = allocate(256),
    waveform = allocate(width * 256);
  const vectorscope = new Uint32Array(vectorSize * vectorSize);
  let clipLow = 0,
    clipHigh = 0;
  let visible = 0;
  const channelClipping = {
    r: { low: 0, high: 0 },
    g: { low: 0, high: 0 },
    b: { low: 0, high: 0 },
  };
  for (let p = 0; p < width * height; p++) {
    if (!pixels[p * 4 + 3]) continue;
    visible++;
    const red = pixels[p * 4]!,
      green = pixels[p * 4 + 1]!,
      blue = pixels[p * 4 + 2]!;
    vectorscope[vectorIndex(red / 255, green / 255, blue / 255)]!++;
    const values = {
      r: red,
      g: green,
      b: blue,
      l: Math.round(0.2126 * red + 0.7152 * green + 0.0722 * blue),
    };
    for (const channel of channels) {
      histogram[channel][values[channel]]!++;
      waveform[channel][(p % width) * 256 + values[channel]]!++;
    }
    if (isShadowClipped(red, green, blue)) clipLow++;
    if (isHighlightClipped(red, green, blue)) clipHigh++;
    for (const channel of ['r', 'g', 'b'] as const) {
      if (values[channel] <= 2) channelClipping[channel].low++;
      if (values[channel] >= 253) channelClipping[channel].high++;
    }
  }
  for (const channel of ['r', 'g', 'b'] as const) {
    channelClipping[channel].low /= Math.max(1, visible);
    channelClipping[channel].high /= Math.max(1, visible);
  }
  return {
    histogram,
    waveform,
    vectorscope,
    width,
    clipLow: clipLow / Math.max(1, visible),
    clipHigh: clipHigh / Math.max(1, visible),
    channelClipping,
  };
}

export function histogramPeak(data: ChannelBins, selected: readonly Channel[]): number {
  const values = selected.flatMap((channel) => [...data[channel]]).sort((a, b) => a - b);
  return Math.max(1, values[Math.floor(values.length * 0.992)] ?? 1);
}
