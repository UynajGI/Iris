import { type Channel, type ScopeData } from './scopes.js';
import { vectorSize } from './scope-chroma.js';

const traceColors = {
  l: [224, 230, 227],
  r: [230, 112, 105],
  g: [111, 211, 148],
  b: [111, 157, 236],
};

/** Bilinear pixel coverage removes integer-bin moire without changing sample mass.
 * Only the display raster is filtered; source measurements/clipping remain untouched.
 */
export function depositDensity(
  bins: Float32Array,
  width: number,
  height: number,
  x: number,
  y: number,
  count: number
) {
  if (!count) return;
  const left = Math.floor(x),
    top = Math.floor(y);
  const right = Math.min(width - 1, left + 1),
    bottom = Math.min(height - 1, top + 1);
  const dx = x - left,
    dy = y - top;
  bins[top * width + left]! += count * (1 - dx) * (1 - dy);
  bins[top * width + right]! += count * dx * (1 - dy);
  bins[bottom * width + left]! += count * (1 - dx) * dy;
  bins[bottom * width + right]! += count * dx * dy;
}

/** Aggregate coverage before applying display gain. */
export function waveformDensity(data: ScopeData, channel: Channel, width: number, height: number) {
  const bins = new Float32Array(width * height);
  for (let x = 0; x < data.width; x++) {
    const column = (x * (width - 1)) / Math.max(1, data.width - 1);
    for (let level = 0; level < 256; level++) {
      const row = ((255 - level) * (height - 1)) / 255;
      depositDensity(bins, width, height, column, row, data.waveform[channel][x * 256 + level]!);
    }
  }
  return bins;
}

export function vectorDensity(data: ScopeData, width: number, height: number) {
  const bins = new Float32Array(width * height);
  for (let y = 0; y < vectorSize; y++)
    for (let x = 0; x < vectorSize; x++) {
      const column = (x * (width - 1)) / (vectorSize - 1);
      const row = (y * (height - 1)) / (vectorSize - 1);
      depositDensity(bins, width, height, column, row, data.vectorscope[y * vectorSize + x]!);
    }
  return bins;
}

export function densityPeak(bins: readonly (Uint32Array | Float32Array)[]) {
  let peak = 1;
  for (const channel of bins) for (const count of channel) peak = Math.max(peak, count);
  return peak;
}

/** Additive phosphor-like traces; one common gain keeps RGB parade comparable. */
export function paintDensity(
  pixels: Uint8ClampedArray,
  bins: Uint32Array | Float32Array,
  channel: Channel,
  peak: number,
  weight = 1
) {
  for (let p = 0; p < bins.length; p++) {
    const count = bins[p]!;
    if (!count) continue;
    const gain = (weight * Math.log1p(count)) / Math.log1p(peak);
    for (let c = 0; c < 3; c++) pixels[p * 4 + c]! += traceColors[channel][c]! * gain;
    pixels[p * 4 + 3] = 255;
  }
}
