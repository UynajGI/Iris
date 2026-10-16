import type { MaskDefinition } from './editing-masks.js';

type Coverage = (x: number, y: number) => number;
const clamp = (value: number) => Math.max(0, Math.min(1, value));
function falloff(distance: number, inner: number, outer: number) {
  if (distance <= inner) return 1;
  if (distance >= outer || outer <= inner) return 0;
  const t = (distance - inner) / (outer - inner);
  return 1 - t * t * (3 - 2 * t);
}
function brushCoverage(
  mask: Extract<MaskDefinition, { kind: 'brush' }>,
  sx: number,
  sy: number
): Coverage {
  const strokes = mask.strokes.map((stroke) => ({
    ...stroke,
    points: stroke.points.map((p) => [p[0]! * sx, p[1]! * sy]),
  }));
  return (x, y) => {
    x *= sx;
    y *= sy;
    let weight = 0;
    for (const stroke of strokes) {
      let distance = Infinity;
      for (let i = 0; i < stroke.points.length; i++) {
        const a = stroke.points[Math.max(0, i - 1)]!,
          b = stroke.points[i]!;
        const dx = b[0]! - a[0]!,
          dy = b[1]! - a[1]!,
          length = dx * dx + dy * dy;
        const t = length ? clamp(((x - a[0]!) * dx + (y - a[1]!) * dy) / length) : 0;
        distance = Math.min(distance, Math.hypot(x - a[0]! - t * dx, y - a[1]! - t * dy));
      }
      const value = falloff(distance, stroke.radius * stroke.hardness, stroke.radius) * stroke.flow;
      weight += value - weight * value;
    }
    return weight;
  };
}
/** Spatial coverage mirrors iris-render/mask.rs; data-dependent ranges require backend pixels. */
export function spatialMaskCoverage(
  mask: MaskDefinition,
  width: number,
  height: number
): Coverage | null {
  const short = Math.max(1, Math.min(width, height)),
    sx = width / short,
    sy = height / short;
  if (mask.kind === 'brush') return brushCoverage(mask, sx, sy);
  if (mask.kind === 'linear_gradient') {
    const dx = (mask.end[0]! - mask.start[0]!) * sx,
      dy = (mask.end[1]! - mask.start[1]!) * sy;
    const length = dx * dx + dy * dy;
    return (x, y) =>
      length
        ? clamp(((x - mask.start[0]!) * sx * dx + (y - mask.start[1]!) * sy * dy) / length)
        : 0;
  }
  if (mask.kind === 'radial_gradient')
    return (x, y) =>
      falloff(
        Math.hypot((x - mask.center[0]!) / mask.radii[0]!, (y - mask.center[1]!) / mask.radii[1]!),
        mask.hardness,
        1
      );
  if (mask.kind === 'invert' || mask.kind === 'opacity') {
    const child = spatialMaskCoverage(mask.mask, width, height);
    return (
      child && ((x, y) => (mask.kind === 'invert' ? 1 - child(x, y) : child(x, y) * mask.opacity))
    );
  }
  if (mask.kind === 'combine') {
    const left = spatialMaskCoverage(mask.left, width, height),
      right = spatialMaskCoverage(mask.right, width, height);
    if (!left || !right) return null;
    return (x, y) =>
      mask.operation === 'add'
        ? Math.min(1, left(x, y) + right(x, y))
        : mask.operation === 'subtract'
          ? Math.max(0, left(x, y) - right(x, y))
          : Math.min(left(x, y), right(x, y));
  }
  return null;
}
