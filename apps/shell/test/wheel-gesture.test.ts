import test from 'node:test';
import assert from 'node:assert/strict';
import { WheelGesture } from '../src/editing-wheel-gesture.js';
import { gainsToWheel, wheelToGains } from '../src/editing-color-wheel.js';

const keys = { shiftKey: false, ctrlKey: false, metaKey: false, altKey: false };
const base = { hue: 0, saturation: 1, luminance: 0.2 };
const start = { x: 0.73, y: 0.5 };
const near = (a: number, b: number) => assert.ok(Math.abs(a - b) < 1e-10, `${a} != ${b}`);
test('full-radius hue rotation survives the stored RGB representation without clipping drift', () => {
  for (const hue of [0, 31, 90, 127, 210, 359])
    for (const luminance of [-1.8, 0, 1.8]) {
      const actual = gainsToWheel(wheelToGains({ hue, saturation: 2, luminance }));
      near(actual.hue, hue);
      near(actual.saturation, 2);
      near(actual.luminance, luminance);
    }
});
test('Shift projects onto the hue spoke, ignoring sideways movement and clamping at center', () => {
  const shift = { ...keys, shiftKey: true };
  const drag = new WheelGesture(base, start, shift, false, true);
  const value = drag.move({ x: 0.615, y: 0.1 }, shift);
  near(value.hue, 0);
  near(value.saturation, 0.5);
  near(value.luminance, 0.2);
  near(drag.move({ x: 0.1, y: 0.1 }, shift).saturation, 0);
});
test('Ctrl, Command and the rim hold saturation while changing hue', () => {
  for (const [modifier, ring] of [
    [{ ...keys, ctrlKey: true }, false],
    [{ ...keys, metaKey: true }, false],
    [keys, true],
  ] as const) {
    const drag = new WheelGesture(base, start, modifier, ring, false);
    const value = drag.move({ x: 0.5, y: 0.1 }, modifier);
    near(value.hue, 90);
    near(value.saturation, 1);
  }
});
test('Alt slows movement and combines with Ctrl without wrapping the long way around', () => {
  const modifiers = { ...keys, altKey: true, ctrlKey: true };
  const point = (h: number) => ({
    x: 0.5 + 0.23 * Math.cos((h * Math.PI) / 180),
    y: 0.5 - 0.23 * Math.sin((h * Math.PI) / 180),
  });
  const drag = new WheelGesture({ ...base, hue: 359 }, point(359), modifiers, false, true);
  const value = drag.move(point(9), modifiers);
  near(value.hue, 0);
  near(value.saturation, 1);
});
test('changing modifiers rebases at the current value without jumping', () => {
  const drag = new WheelGesture(base, start, keys, false, false);
  const value = drag.move({ x: 0.5, y: 0.27 }, keys);
  const shift = { ...keys, shiftKey: true };
  assert.deepEqual(drag.move({ x: 0.51, y: 0.28 }, shift), value);
  near(drag.move({ x: 0.9, y: 0.385 }, shift).hue, 90);
});
test('dragging a handle soft-locks its spoke then releases to free movement', () => {
  const drag = new WheelGesture(base, start, keys, false, true);
  near(drag.move({ x: 0.8, y: 0.51 }, keys).hue, 0);
  assert.notEqual(drag.move({ x: 0.8, y: 0.6 }, keys).hue, 0);
});
