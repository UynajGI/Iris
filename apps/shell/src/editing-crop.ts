export type Crop = [number, number, number, number];
const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));

// Ratio here is in normalized source coordinates, not displayed pixels.
export function fitCropRatio(base: Crop, ratio: number): Crop {
  if (!Number.isFinite(ratio) || ratio <= 0) return base;
  const cx = base[0] + base[2] / 2,
    cy = base[1] + base[3] / 2;
  const width = Math.min(
    Math.sqrt(base[2] * base[3] * ratio),
    2 * Math.min(cx, 1 - cx),
    2 * Math.min(cy, 1 - cy) * ratio
  );
  const height = width / ratio;
  return [cx - width / 2, cy - height / 2, width, height];
}

function cropAxis(start: number, size: number, active: boolean, negative: boolean) {
  const offset = active ? (negative ? 1 : 0) : 0.5;
  const anchor = start + size * offset;
  const limit = active ? (negative ? anchor : 1 - anchor) : 2 * Math.min(anchor, 1 - anchor);
  return { offset, anchor, limit, sign: negative ? -1 : 1 };
}

function constrainedCrop(base: Crop, handle: string, dx: number, dy: number): Crop {
  const [x, y, w, h] = base;
  const horizontal = handle.includes('w') || handle.includes('e');
  const vertical = handle.includes('n') || handle.includes('s');
  const ax = cropAxis(x, w, horizontal, handle.includes('w'));
  const ay = cropAxis(y, h, vertical, handle.includes('n'));
  // A side handle expands symmetrically on the other axis; corners anchor the opposite corner.
  const scaleX = (ax.sign * dx) / w;
  const scaleY = (ay.sign * dy) / h;
  const change = !vertical
    ? scaleX
    : !horizontal
      ? scaleY
      : Math.abs(scaleX) >= Math.abs(scaleY)
        ? scaleX
        : scaleY;
  const scale = clamp(
    1 + change,
    Math.min(1, Math.max(0.01 / w, 0.01 / h)),
    Math.min(ax.limit / w, ay.limit / h)
  );
  const width = w * scale,
    height = h * scale;
  return [ax.anchor - ax.offset * width, ay.anchor - ay.offset * height, width, height];
}

export function dragCrop(base: Crop, handle: string, dx: number, dy: number, locked = false): Crop {
  if (handle === 'move')
    return [
      clamp(base[0] + dx, 0, 1 - base[2]),
      clamp(base[1] + dy, 0, 1 - base[3]),
      base[2],
      base[3],
    ];
  if (locked) return constrainedCrop(base, handle, dx, dy);
  let left = base[0],
    top = base[1],
    right = left + base[2],
    bottom = top + base[3];
  if (handle.includes('w')) left = clamp(left + dx, 0, right - 0.01);
  if (handle.includes('e')) right = clamp(right + dx, left + 0.01, 1);
  if (handle.includes('n')) top = clamp(top + dy, 0, bottom - 0.01);
  if (handle.includes('s')) bottom = clamp(bottom + dy, top + 0.01, 1);
  return [left, top, right - left, bottom - top];
}
