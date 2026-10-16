import { wheelPoint, type WheelColor } from './editing-color-wheel.js';

type Point = { x: number; y: number };
type Modifiers = { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean; altKey: boolean };
type Lock = 'hue' | 'saturation' | 'none';
const clamp = (value: number) => Math.max(0, Math.min(2, value));
const wrap = (value: number) => ((value % 360) + 360) % 360;
const angleDelta = (a: number, b: number) => ((a - b + 540) % 360) - 180;
const lockFor = (keys: Modifiers, ring: boolean): Lock =>
  ring || keys.ctrlKey || keys.metaKey ? 'saturation' : keys.shiftKey ? 'hue' : 'none';

/** A gesture owns its anchors, so modifier changes and React renders cannot cause jumps. */
export class WheelGesture {
  private previous: Point;
  private lock: Lock;
  private fine: boolean;
  private soft: boolean;
  constructor(
    private value: WheelColor,
    point: Point,
    keys: Modifiers,
    private ring: boolean,
    handle: boolean
  ) {
    this.previous = point;
    this.lock = lockFor(keys, ring);
    this.fine = keys.altKey;
    this.soft = handle && !ring && value.saturation > 0.01;
  }
  move(point: Point, keys: Modifiers): WheelColor {
    const lock = lockFor(keys, this.ring);
    if (lock !== this.lock || keys.altKey !== this.fine) {
      this.previous = point;
      this.lock = lock;
      this.fine = keys.altKey;
      this.soft = false;
      return this.value;
    }
    const current = wheelPoint(point.x, point.y, this.value.hue);
    const previous = wheelPoint(this.previous.x, this.previous.y, this.value.hue);
    const angle = (this.value.hue * Math.PI) / 180;
    const lateral = Math.abs((point.x - 0.5) * Math.sin(angle) + (point.y - 0.5) * Math.cos(angle));
    if (lateral > 0.035) this.soft = false;
    const radial = (p: Point) =>
      (((p.x - 0.5) * Math.cos(angle) - (p.y - 0.5) * Math.sin(angle)) / 0.46) * 2;
    const hueLocked = lock === 'hue' || (lock === 'none' && this.soft);
    const hue = hueLocked
      ? this.value.hue
      : this.fine
        ? wrap(this.value.hue + angleDelta(current.hue, previous.hue) * 0.1)
        : current.hue;
    const radius = hueLocked ? radial(point) : current.saturation;
    const oldRadius = hueLocked ? radial(this.previous) : previous.saturation;
    const saturation =
      lock === 'saturation'
        ? this.value.saturation
        : this.fine
          ? clamp(this.value.saturation + (radius - oldRadius) * 0.1)
          : clamp(radius);
    this.previous = point;
    this.value = { ...this.value, hue, saturation };
    return this.value;
  }
}
