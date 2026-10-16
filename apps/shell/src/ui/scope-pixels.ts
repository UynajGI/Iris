export type Clipping = { shadows: boolean; highlights: boolean };
export const noClipping: Clipping = { shadows: false, highlights: false };
export const isShadowClipped = (r: number, g: number, b: number) => Math.max(r, g, b) <= 2;
export const isHighlightClipped = (r: number, g: number, b: number) => Math.max(r, g, b) >= 253;

/** The readout and warning overlay share exactly the same bounded preview samples. */
const samples = new Map<string, Promise<ImageData>>();
export function readScopePixels(url: string): Promise<ImageData> {
  const cached = samples.get(url);
  if (cached) return cached;
  const pending = decode(url);
  samples.set(url, pending);
  while (samples.size > 2) samples.delete(samples.keys().next().value!);
  void pending.catch(() => {
    if (samples.get(url) === pending) samples.delete(url);
  });
  return pending;
}
async function decode(url: string): Promise<ImageData> {
  const image = new Image();
  image.src = url;
  await image.decode();
  const scale = Math.min(1, 512 / Math.max(image.naturalWidth, image.naturalHeight));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (!context) throw new Error('canvas unavailable');
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  return context.getImageData(0, 0, canvas.width, canvas.height);
}
export function clippingPixels(pixels: Uint8ClampedArray, clipping: Clipping) {
  const overlay = new Uint8ClampedArray(pixels.length);
  for (let i = 0; i < pixels.length; i += 4) {
    if (!pixels[i + 3]) continue;
    const r = pixels[i]!,
      g = pixels[i + 1]!,
      b = pixels[i + 2]!;
    if (clipping.highlights && isHighlightClipped(r, g, b)) overlay.set([255, 40, 40, 230], i);
    else if (clipping.shadows && isShadowClipped(r, g, b)) overlay.set([35, 100, 255, 230], i);
  }
  return overlay;
}
