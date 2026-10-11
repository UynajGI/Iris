export const channels = ['l', 'r', 'g', 'b'] as const;
export type Channel = (typeof channels)[number];
export type ScopeMode = 'histogram' | 'waveform';
export type ChannelBins = Record<Channel, Uint32Array>;
export type ScopeData = {
  histogram: ChannelBins;
  waveform: ChannelBins;
  width: number;
  clipLow: number;
  clipHigh: number;
};
const allocate = (size: number): ChannelBins => ({
  l: new Uint32Array(size),
  r: new Uint32Array(size),
  g: new Uint32Array(size),
  b: new Uint32Array(size),
});

/** Both scopes use oriented, downsampled preview pixels, not RAW data. */
export function sampleScopes(pixels: Uint8ClampedArray, width: number, height: number): ScopeData {
  if (width < 1 || height < 1 || pixels.length !== width * height * 4)
    throw new Error('Invalid scope image');
  const histogram = allocate(256),
    waveform = allocate(width * 256);
  let clipLow = 0,
    clipHigh = 0;
  for (let p = 0; p < width * height; p++) {
    const red = pixels[p * 4]!,
      green = pixels[p * 4 + 1]!,
      blue = pixels[p * 4 + 2]!;
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
    if (red <= 2 && green <= 2 && blue <= 2) clipLow++;
    if (red >= 253 || green >= 253 || blue >= 253) clipHigh++;
  }
  return {
    histogram,
    waveform,
    width,
    clipLow: clipLow / (width * height),
    clipHigh: clipHigh / (width * height),
  };
}

export function histogramPeak(data: ChannelBins, selected: readonly Channel[]): number {
  const values = selected.flatMap((channel) => [...data[channel]]).sort((a, b) => a - b);
  return Math.max(1, values[Math.floor(values.length * 0.992)] ?? 1);
}
