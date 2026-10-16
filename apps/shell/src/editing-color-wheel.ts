export type WheelColor = { hue: number; saturation: number; luminance: number };
export function gainsToWheel(values: number[]): WheelColor {
  const [r, g, b] = values as [number, number, number];
  const high = Math.max(r, g, b),
    low = Math.min(r, g, b),
    chroma = high - low;
  let hue = 0;
  if (chroma > 1e-10) {
    if (high === r) hue = ((g - b) / chroma) % 6;
    else if (high === g) hue = (b - r) / chroma + 2;
    else hue = (r - g) / chroma + 4;
  }
  hue = (hue * 60 + 360) % 360;
  const luminance = (r + g + b) / 3;
  const limit = chromaLimit(hueVector(hue), luminance);
  return { hue, saturation: limit > 1e-10 ? (chroma / limit) * 2 : 0, luminance };
}
function hueVector(hue: number): number[] {
  const position = (((hue % 360) + 360) % 360) / 60;
  const x = 1 - Math.abs((position % 2) - 1);
  const rgb = [
    [1, x, 0],
    [x, 1, 0],
    [0, 1, x],
    [0, x, 1],
    [x, 0, 1],
    [1, 0, x],
  ][Math.floor(position)]!;
  const average = rgb.reduce((sum, value) => sum + value, 0) / 3;
  return rgb.map((value) => value - average);
}
function chromaLimit(vector: number[], luminance: number): number {
  return Math.min(
    ...vector
      .filter((value) => Math.abs(value) > 1e-10)
      .map((value) => (value > 0 ? (2 - luminance) / value : (-2 - luminance) / value))
  );
}
export function wheelToGains({ hue, saturation, luminance }: WheelColor): number[] {
  const vector = hueVector(hue);
  const chroma = (chromaLimit(vector, luminance) * Math.max(0, Math.min(2, saturation))) / 2;
  // Scale the whole tint into range, never clip channels independently (which rotates hue).
  return vector.map((value) => Math.max(-2, Math.min(2, luminance + chroma * value)));
}
/** Coordinates are normalized within the inner disk; the center retains the selected hue. */
export function wheelPoint(x: number, y: number, hue: number) {
  const dx = (x - 0.5) / 0.46,
    dy = (y - 0.5) / 0.46;
  const radius = Math.hypot(dx, dy);
  return {
    hue: radius < 0.01 ? hue : ((Math.atan2(-dy, dx) * 180) / Math.PI + 360) % 360,
    saturation: Math.min(1, radius) * 2,
  };
}
