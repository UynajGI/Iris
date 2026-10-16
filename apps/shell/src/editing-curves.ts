// SPDX-License-Identifier: GPL-3.0-or-later
import type { components } from './generated/api.js';
export type CurvePoint = components['schemas']['CurvePoint'];
type CurveContract = components['schemas']['Curve'];
// Linear mode may be omitted by the renderer or an older saved document.
export type CurveData = Pick<CurveContract, 'points'> &
  Partial<Pick<CurveContract, 'interpolation'>>;
type Prepared = { points: CurvePoint[]; tangents: number[]; cubic: boolean };

function endpoint(h0: number, h1: number, d0: number, d1: number) {
  return Math.max(0, Math.min(3 * d0, ((2 * h0 + h1) * d0 - h0 * d1) / (h0 + h1)));
}

// Match the renderer's f32 control coordinates and f64 PCHIP tangent arithmetic.
// Fritsch-Butland weighted harmonic mean; see adjustments/curve.rs for the source.
function prepare(curve: CurveData): Prepared {
  const points = (
    curve.points.length
      ? curve.points
      : [
          { x: 0, y: 0 },
          { x: 1, y: 1 },
        ]
  ).map(({ x, y }) => ({ x: Math.fround(x), y: Math.fround(y) }));
  const cubic = curve.interpolation === 'monotone_cubic';
  if (!cubic) return { points, tangents: [], cubic };
  const widths = points.slice(1).map((point, i) => point.x - points[i]!.x);
  const slopes = points.slice(1).map((point, i) => (point.y - points[i]!.y) / widths[i]!);
  const n = slopes.length;
  if (n === 1) return { points, tangents: [slopes[0]!, slopes[0]!], cubic };
  const tangents = Array<number>(n + 1).fill(0);
  tangents[0] = endpoint(widths[0]!, widths[1]!, slopes[0]!, slopes[1]!);
  tangents[n] = endpoint(widths[n - 1]!, widths[n - 2]!, slopes[n - 1]!, slopes[n - 2]!);
  for (let i = 1; i < n; i++) {
    if (slopes[i - 1]! > 0 && slopes[i]! > 0) {
      const w1 = 2 * widths[i]! + widths[i - 1]!;
      const w2 = widths[i]! + 2 * widths[i - 1]!;
      tangents[i] = (w1 + w2) / (w1 / slopes[i - 1]! + w2 / slopes[i]!);
    }
  }
  return { points, tangents, cubic };
}

export function curveValue(curve: CurveData, x: number): number {
  if (!curve.points.length) return x;
  const { points, tangents, cubic } = prepare(curve);
  const found = points.findIndex((p, i) => i > 0 && x <= p.x);
  const i = found < 0 ? points.length - 2 : found - 1;
  const a = points[i]!,
    b = points[i + 1]!;
  if (!cubic) return a.y + ((x - a.x) * Math.fround(b.y - a.y)) / Math.fround(b.x - a.x);
  const first = points[0]!,
    last = points.at(-1)!;
  if (x < first.x) return first.y + (x - first.x) * tangents[0]!;
  if (x > last.x) return last.y + (x - last.x) * tangents.at(-1)!;
  const h = b.x - a.x,
    t = (x - a.x) / h;
  const delta = b.y - a.y,
    left = h * tangents[i]!,
    right = h * tangents[i + 1]!;
  return (((left + right - 2 * delta) * t + 3 * delta - 2 * left - right) * t + left) * t + a.y;
}

/** SVG cubic Bézier handles represent exactly the renderer's Hermite segments. */
export function curvePath(curve: CurveData, size = 200): string {
  const { points, tangents, cubic } = prepare(curve);
  const coordinate = (x: number, y: number) => `${x * size} ${(1 - y) * size}`;
  let path = `M${coordinate(points[0]!.x, points[0]!.y)}`;
  for (let i = 0; i < points.length - 1; i++) {
    const a = points[i]!,
      b = points[i + 1]!;
    if (!cubic) path += ` L${coordinate(b.x, b.y)}`;
    else {
      const third = (b.x - a.x) / 3;
      path += ` C${coordinate(a.x + third, a.y + third * tangents[i]!)} ${coordinate(b.x - third, b.y - third * tangents[i + 1]!)} ${coordinate(b.x, b.y)}`;
    }
  }
  return path;
}
