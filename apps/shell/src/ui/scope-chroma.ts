/** BT.709 colour-difference matrix on normalized display R'G'B'.
 * The source is sRGB preview code values, not a broadcast/legal-range signal.
 * Cb points right; Cr points up. Neutral pixels always land at the origin.
 */
export function scopeChroma(r: number, g: number, b: number) {
  const y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return { cb: (b - y) / 1.8556, cr: (r - y) / 1.5748 };
}

export const vectorSize = 257;
export const vectorExtent = 0.7;
export function vectorPosition(cb: number, cr: number, size = vectorSize) {
  const scale = (size - 1) / (2 * vectorExtent);
  return { x: (size - 1) / 2 + cb * scale, y: (size - 1) / 2 - cr * scale };
}
export function vectorIndex(r: number, g: number, b: number) {
  const { cb, cr } = scopeChroma(r, g, b);
  const { x, y } = vectorPosition(cb, cr);
  return Math.round(y) * vectorSize + Math.round(x);
}

export const vectorTargets = [
  { label: 'R', rgb: [1, 0, 0], color: '#d9908d' },
  { label: 'Mg', rgb: [1, 0, 1], color: '#c69dbe' },
  { label: 'B', rgb: [0, 0, 1], color: '#8baed5' },
  { label: 'Cy', rgb: [0, 1, 1], color: '#8dbec0' },
  { label: 'G', rgb: [0, 1, 0], color: '#89be9d' },
  { label: 'Yl', rgb: [1, 1, 0], color: '#c8bd87' },
] as const;
