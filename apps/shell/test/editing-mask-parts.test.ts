import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createMaskDefinition, type MaskDefinition } from '../src/editing-masks.js';
import {
  appendMaskPart,
  maskPartAt,
  maskParts,
  paintMaskPart,
  removeMaskPart,
  replaceMaskPart,
} from '../src/editing-mask-parts.js';
import { spatialMaskCoverage } from '../src/editing-mask-overlay.js';

test('adding, subtracting and intersecting retain the original expression and selectable parts', () => {
  const base = createMaskDefinition('linear_gradient');
  const subtraction = appendMaskPart(base, 'subtract', 'brush');
  const combined = appendMaskPart(subtraction, 'intersect', 'radial_gradient');
  assert.deepEqual(
    maskParts(combined).map((part) => [part.path, part.definition.kind, part.relation]),
    [
      ['LL', 'linear_gradient', '基础'],
      ['LR', 'brush', '减去'],
      ['R', 'radial_gradient', '相交'],
    ]
  );
  assert.equal(combined.kind === 'combine' && combined.left, subtraction);
  assert.equal(subtraction.kind === 'combine' && subtraction.left, base);
  assert.equal(maskPartAt(combined, 99).definition.kind, 'radial_gradient');
});

test('editing a selected component preserves outer inversion, opacity and neighbouring masks', () => {
  const original: MaskDefinition = {
    kind: 'invert',
    mask: {
      kind: 'opacity',
      opacity: 0.7,
      mask: appendMaskPart(createMaskDefinition('linear_gradient'), 'subtract', 'radial_gradient'),
    },
  };
  const before = structuredClone(original);
  const chosen = maskPartAt(original, 1);
  assert.equal(chosen.path, 'MMR');
  const replacement: MaskDefinition = {
    kind: 'radial_gradient',
    center: [0.1, 0.2],
    radii: [0.2, 0.3],
    hardness: 0.8,
  };
  const next = replaceMaskPart(original, chosen.path, replacement);
  assert.deepEqual(original, before);
  assert.equal(maskPartAt(next, 1).definition, replacement);
  assert.equal(maskPartAt(next, 0).definition, maskPartAt(original, 0).definition);
  assert.equal(next.kind, 'invert');
  assert.ok(next.kind === 'invert' && next.mask.kind === 'opacity' && next.mask.opacity === 0.7);
  assert.throws(() => replaceMaskPart(original, 'R', replacement), /已变化/);
});

test('removing one component prunes only its empty parent and cannot remove the final component', () => {
  const base = createMaskDefinition('linear_gradient');
  const combined = appendMaskPart(
    appendMaskPart(base, 'subtract', 'brush'),
    'intersect',
    'radial_gradient'
  );
  const next = removeMaskPart(combined, 'LR');
  assert.equal(next.kind, 'combine');
  assert.equal(next.kind === 'combine' && next.operation, 'intersect');
  assert.equal(next.kind === 'combine' && next.left, base);
  assert.equal(maskParts(next).length, 2);
  assert.throws(() => removeMaskPart(base, ''), /最后一个/);
});

test('paint and erase modify only the selected brush inside a combined mask', () => {
  const gradient = createMaskDefinition('linear_gradient');
  const combined = appendMaskPart(gradient, 'add', 'brush');
  const stroke = { points: [[0.5, 0.5]], radius: 0.2, hardness: 1, flow: 1 };
  const painted = paintMaskPart(combined, maskPartAt(combined, 1), stroke, false);
  const erased = paintMaskPart(painted, maskPartAt(painted, 1), stroke, true);
  assert.equal(erased.kind === 'combine' && erased.left, gradient);
  const sample = spatialMaskCoverage(erased, 100, 100)!;
  assert.equal(sample(0.5, 0.5), 0.5); // Erasing added brush must leave the base gradient intact.
  assert.equal(maskParts(erased).length, 3);
});

test('combined spatial overlay reports final add, subtract and intersection coverage', () => {
  const base: MaskDefinition = { kind: 'linear_gradient', start: [0, 0], end: [0, 1] };
  const right: MaskDefinition = {
    kind: 'brush',
    strokes: [{ points: [[0.5, 0.5]], radius: 0.2, hardness: 1, flow: 0.25 }],
  };
  for (const [operation, expected] of [
    ['add', 0.75],
    ['subtract', 0.25],
    ['intersect', 0.25],
  ] as const) {
    const sample: (x: number, y: number) => number = spatialMaskCoverage(
      { kind: 'combine', operation, left: base, right },
      100,
      100
    )!;
    assert.equal(sample(0.5, 0.5), expected);
  }
});

test('combination limits are enforced before an over-deep expression is accepted', () => {
  let mask = createMaskDefinition('brush');
  for (let i = 0; i < 16; i++) mask = appendMaskPart(mask, 'add', 'brush');
  const before = structuredClone(mask);
  assert.throws(() => appendMaskPart(mask, 'add', 'brush'), /上限/);
  assert.deepEqual(mask, before);
});
