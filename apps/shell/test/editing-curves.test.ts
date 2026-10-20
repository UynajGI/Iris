import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { curveValue, curvePath, type CurveData } from '../src/editing-curves.js';

const fixtures = JSON.parse(
  readFileSync(
    new URL('../../../crates/iris-render/tests/fixtures/curve-interpolation.json', import.meta.url),
    'utf8'
  )
) as {
  name: string;
  curve: CurveData;
  samples: [number, number][];
}[];

test('curve geometry matches the renderer shared analytic fixtures', () => {
  for (const fixture of fixtures)
    for (const [input, output] of fixture.samples)
      assert.ok(
        Math.abs(curveValue(fixture.curve, input) - output) < 1e-10,
        `${fixture.name}: ${input}`
      );
});

test('SVG handles match analytic midpoints and preserve linear legacy paths', () => {
  const path = curvePath(fixtures[0]!.curve, 1);
  const segments = path
    .split(' C')
    .slice(1)
    .map((s) => s.split(' ').map(Number));
  assert.equal(segments.length, 2);
  for (let i = 0; i < segments.length; i++) {
    const values = segments[i]!;
    const start = fixtures[0]!.curve.points[i]!;
    const midpointY = (1 - start.y + 3 * values[1]! + 3 * values[3]! + values[5]!) / 8;
    assert.ok(Math.abs(1 - midpointY - fixtures[0]!.samples[i === 0 ? 2 : 4]![1]) < 1e-10);
  }
  assert.equal(curvePath(fixtures[1]!.curve, 1), 'M0 1 L0.5 0.625 L1 0');
});

test('uneven knot spacing and plateaus do not overshoot or reverse', () => {
  const curve: CurveData = {
    interpolation: 'monotone_cubic',
    points: [
      { x: 0, y: 0.05 },
      { x: 0.0002, y: 0.2 },
      { x: 0.2, y: 0.2 },
      { x: 0.8, y: 0.9 },
      { x: 1, y: 0.95 },
    ],
  };
  for (let i = 0; i < curve.points.length - 1; i++) {
    const a = curve.points[i]!,
      b = curve.points[i + 1]!;
    let previous = a.y;
    for (let j = 0; j <= 256; j++) {
      const value = curveValue(curve, a.x + ((b.x - a.x) * j) / 256);
      assert.ok(value >= a.y - 1e-6 && value <= b.y + 1e-6);
      assert.ok(value >= previous - 1e-6);
      previous = value;
    }
  }
});

test('natural spline SVG has continuous curvature at uneven control points', () => {
  const curve: CurveData = {
    interpolation: 'natural_cubic',
    points: [
      { x: 0, y: 0 },
      { x: 0.2, y: 0.35 },
      { x: 0.6, y: 0.7 },
      { x: 1, y: 1 },
    ],
  };
  const pieces = curvePath(curve, 1)
    .split(' C')
    .slice(1)
    .map((s) => s.split(' ').map(Number));
  for (let i = 0; i < pieces.length - 1; i++) {
    const a = pieces[i]!,
      b = pieces[i + 1]!;
    const widthA = a[4]! - Math.fround(curve.points[i]!.x);
    const widthB = b[4]! - a[4]!;
    const slopeA = (3 * (a[5]! - a[3]!)) / widthA;
    const slopeB = (3 * (b[1]! - a[5]!)) / widthB;
    const curvatureA = (6 * (a[5]! - 2 * a[3]! + a[1]!)) / widthA ** 2;
    const curvatureB = (6 * (a[5]! - 2 * b[1]! + b[3]!)) / widthB ** 2;
    assert.ok(Math.abs(slopeA - slopeB) < 1e-9, 'no tangent discontinuity');
    assert.ok(Math.abs(curvatureA - curvatureB) < 1e-9, 'no curvature discontinuity');
  }
});
